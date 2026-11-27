//! Воспроизведение: определение потока серии и запуск плееров (mpv/VLC/Outplayer/nPlayer/системный).
//!
//! У Iced нет встроенного видеовиджета, поэтому AniLot управляет сессией просмотра сам
//! (качество, скорость, субтитры, AniSkip, позиция), а картинку показывает внешний плеер.
//! Для mpv на десктопе авто-пропуск OP/ED и сохранение позиции реализованы Lua-скриптом.

use crate::aniskip::{http, Marker};
use crate::hls::{self, Variant};
use crate::platform;
use crate::storage;
use crate::subtitles::{self, Track};
use regex::Regex;
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Clone, Debug, Default)]
pub struct Stream {
    pub url: String,
    pub is_hls: bool,
    pub variants: Vec<Variant>,
    pub subtitles: Vec<Track>,
    /// Страница встроенного плеера, если прямой поток найти не удалось.
    pub embed_url: String,
}

fn unescape(s: &str) -> String {
    s.replace("\\/", "/").replace("\\u0026", "&").replace("&amp;", "&")
}

/// Определяет прямой поток по ссылке серии: HLS, mp4 или страница плеера (поиск потока и субтитров в HTML).
pub async fn resolve_stream(url: &str, iframe: bool) -> Result<Stream, String> {
    static M3U8: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"https?:(?:\\?/){2}[^"'\s<>\\]+?\.m3u8[^"'\s<>\\]*"#).unwrap());
    static MP4: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?:(?:\\?/){2}[^"'\s<>\\]+?\.mp4[^"'\s<>\\]*"#).unwrap());
    let path = url.split('?').next().unwrap_or(url).to_lowercase();
    let mut stream = Stream { url: url.to_string(), ..Default::default() };
    if path.ends_with(".m3u8") {
        stream.is_hls = true;
    } else if path.ends_with(".mp4") || path.ends_with(".mkv") || path.ends_with(".webm") {
        return Ok(stream);
    } else {
        // Страница встроенного плеера.
        let resp = http().get(url).header("Referer", url).send().await.map_err(|e| format!("Сеть недоступна: {e}"))?;
        let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
        if ct.contains("video/") || ct.contains("mpegurl") {
            stream.is_hls = ct.contains("mpegurl");
            return Ok(stream);
        }
        let html = resp.text().await.map_err(|e| e.to_string())?;
        stream.subtitles = subtitles::discover_tracks(&html, url);
        if let Some(m) = M3U8.find(&html) {
            stream.url = unescape(m.as_str());
            stream.is_hls = true;
        } else if let Some(m) = MP4.find(&html) {
            stream.url = unescape(m.as_str());
            return Ok(stream);
        } else {
            stream.embed_url = url.to_string();
            let _ = iframe;
            return Err("Источник использует встроенный веб-плеер — откройте серию в браузере.".into());
        }
    }
    if stream.is_hls {
        if let Ok(text) = http().get(&stream.url).send().await.and_then(|r| r.error_for_status()) {
            if let Ok(text) = text.text().await {
                if hls::is_master(&text) {
                    stream.variants = hls::parse_master(&text, &stream.url);
                }
            }
        }
    }
    Ok(stream)
}

/// Скачивает дорожку субтитров в каталог кэша и возвращает путь к файлу.
pub async fn fetch_subtitle(track: &Track, tag: &str) -> Result<PathBuf, String> {
    let resp = http().get(&track.url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let fmt = subtitles::detect_format(&text, &track.url);
    let dir = platform::cache_dir().join("subtitles");
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let safe: String = tag.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
    let path = dir.join(format!("{safe}.{}", fmt.extension()));
    tokio::fs::write(&path, text).await.map_err(|e| e.to_string())?;
    Ok(path)
}

#[derive(Clone, Debug, Default)]
pub struct LaunchOpts {
    /// "default" | "vlc" | "outplayer" | "nplayer"
    pub player: String,
    pub url: String,
    pub title: String,
    pub start: f64,
    pub speed: f32,
    pub subtitle: Option<PathBuf>,
    pub markers: Vec<Marker>,
    pub auto_skip: bool,
    pub mpv_path: String,
}

#[derive(Clone, Debug, Default)]
pub struct PlayResult {
    /// Позиция и длительность (если плеер удалось отследить).
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub tracked: bool,
    pub error: Option<String>,
}

fn lua_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
}

/// Lua-скрипт для mpv: пропуск OP/ED по меткам AniSkip и запись позиции в файл.
pub fn mpv_script(markers: &[Marker], auto_skip: bool, pos_file: &str) -> String {
    let list: Vec<String> = markers
        .iter()
        .map(|m| format!("{{name={}, s={:.3}, e={:.3}}}", lua_str(crate::aniskip::marker_label(&m.name)), m.start, m.end))
        .collect();
    format!(
        r#"local markers = {{{}}}
local auto = {}
local posfile = {}
local last_pos, last_dur = 0, 0
local skipped = {{}}
local function write_pos()
  local f = io.open(posfile, "w")
  if f then f:write(string.format("%.3f %.3f", last_pos, last_dur)); f:close() end
end
mp.observe_property("duration", "number", function(_, d) if d then last_dur = d end end)
mp.observe_property("time-pos", "number", function(_, t)
  if not t then return end
  last_pos = t
  if not auto then return end
  for i, m in ipairs(markers) do
    if t >= m.s and t < m.e - 1 and not skipped[i] then
      skipped[i] = true
      mp.osd_message("Пропуск: " .. m.name, 2)
      mp.set_property_number("time-pos", m.e)
      return
    end
  end
end)
mp.add_periodic_timer(5, write_pos)
mp.register_event("shutdown", write_pos)
mp.add_key_binding("S", "anilot-skip", function()
  for i, m in ipairs(markers) do
    if last_pos >= m.s and last_pos < m.e then mp.set_property_number("time-pos", m.e); return end
  end
end)
"#,
        list.join(", "),
        if auto_skip { "true" } else { "false" },
        lua_str(&pos_file.replace('\\', "/")),
    )
}

fn candidates(kind: &str, custom: &str) -> Vec<String> {
    let mut v = vec![];
    if !custom.trim().is_empty() && kind == "mpv" {
        v.push(custom.trim().to_string());
    }
    match kind {
        "mpv" => {
            v.push("mpv".into());
            v.push("/opt/homebrew/bin/mpv".into());
            v.push("/usr/local/bin/mpv".into());
            v.push("/Applications/mpv.app/Contents/MacOS/mpv".into());
            v.push("C:\\Program Files\\mpv\\mpv.exe".into());
            v.push("mpv.exe".into());
        }
        _ => {
            v.push("vlc".into());
            v.push("/Applications/VLC.app/Contents/MacOS/VLC".into());
            v.push("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe".into());
            v.push("vlc.exe".into());
        }
    }
    v
}

async fn spawn_first(kind: &str, custom: &str, args: &[String]) -> Result<tokio::process::Child, String> {
    for bin in candidates(kind, custom) {
        match tokio::process::Command::new(&bin).args(args).kill_on_drop(false).spawn() {
            Ok(child) => return Ok(child),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("{bin}: {e}")),
        }
    }
    Err(format!("Плеер {kind} не найден в системе"))
}

pub fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Ссылка для открытия в стороннем приложении на мобильных платформах.
pub fn external_scheme_url(player: &str, url: &str) -> Option<String> {
    match player {
        "vlc" => Some(if platform::is_ios() {
            format!("vlc-x-callback://x-callback-url/stream?url={}", enc(url))
        } else {
            format!("vlc://{url}")
        }),
        "outplayer" => Some(format!("outplayer://{url}")),
        "nplayer" => Some(format!("nplayer-{url}")),
        _ => None,
    }
}

pub async fn play(opts: LaunchOpts) -> PlayResult {
    let mut res = PlayResult::default();
    // Мобильные: схемы сторонних плееров или системный выбор приложения.
    if platform::is_mobile() {
        let target = external_scheme_url(&opts.player, &opts.url).unwrap_or_else(|| opts.url.clone());
        if let Err(e) = platform::open_url(&target) {
            res.error = Some(format!("Не удалось открыть плеер: {e}"));
        }
        return res;
    }
    if opts.player != "default" {
        if matches!(opts.player.as_str(), "outplayer" | "nplayer") {
            // Эти плееры есть только на Apple/Android — здесь открываем системным способом.
            if let Some(t) = external_scheme_url(&opts.player, &opts.url) {
                if platform::open_url(&t).is_err() {
                    res.error = Some("Плеер недоступен на этой платформе".into());
                }
            }
            return res;
        }
        return play_vlc(&opts).await;
    }
    play_mpv(opts).await
}

async fn play_vlc(opts: &LaunchOpts) -> PlayResult {
    let mut res = PlayResult::default();
    let mut args = vec![opts.url.clone(), format!("--meta-title={}", opts.title)];
    if opts.start > 1.0 {
        args.push(format!("--start-time={:.0}", opts.start));
    }
    if (opts.speed - 1.0).abs() > 0.01 {
        args.push(format!("--rate={}", opts.speed));
    }
    if let Some(s) = &opts.subtitle {
        args.push(format!("--sub-file={}", s.display()));
    }
    match spawn_first("vlc", "", &args).await {
        Ok(_) => {}
        Err(e) => {
            // Запасной вариант — системный плеер по умолчанию.
            if let Err(e2) = platform::open_url(&opts.url) {
                res.error = Some(format!("{e}; {e2}"));
            }
        }
    }
    res
}

async fn play_mpv(opts: LaunchOpts) -> PlayResult {
    let mut res = PlayResult::default();
    let dir = platform::cache_dir().join("player");
    if tokio::fs::create_dir_all(&dir).await.is_err() {
        res.error = Some("Не удалось создать каталог кэша".into());
        return res;
    }
    let pos_file = dir.join("position.txt");
    let _ = tokio::fs::remove_file(&pos_file).await;
    let script = dir.join("anilot.lua");
    let markers: Vec<Marker> = opts.markers.clone();
    if tokio::fs::write(&script, mpv_script(&markers, opts.auto_skip, &pos_file.to_string_lossy())).await.is_err() {
        res.error = Some("Не удалось записать скрипт mpv".into());
        return res;
    }
    let mut args = vec![
        format!("--force-media-title={}", opts.title),
        format!("--script={}", script.display()),
        "--save-position-on-quit=no".to_string(),
    ];
    if opts.start > 1.0 {
        args.push(format!("--start={:.1}", opts.start));
    }
    if (opts.speed - 1.0).abs() > 0.01 {
        args.push(format!("--speed={}", opts.speed));
    }
    if let Some(s) = &opts.subtitle {
        args.push(format!("--sub-file={}", s.display()));
    }
    // Chapters: метки AniSkip видны на полосе перемотки.
    if !markers.is_empty() {
        let chapters = dir.join("chapters.ffmetadata");
        let mut meta = String::from(";FFMETADATA1\n");
        for m in &markers {
            meta.push_str(&format!(
                "[CHAPTER]\nTIMEBASE=1/1000\nSTART={}\nEND={}\ntitle={}\n",
                (m.start * 1000.0) as i64,
                (m.end * 1000.0) as i64,
                crate::aniskip::marker_label(&m.name)
            ));
        }
        if tokio::fs::write(&chapters, meta).await.is_ok() {
            args.push(format!("--chapters-file={}", chapters.display()));
        }
    }
    args.push("--".into());
    args.push(opts.url.clone());
    match spawn_first("mpv", &opts.mpv_path, &args).await {
        Ok(mut child) => {
            let _ = child.wait().await;
            res.tracked = true;
            if let Ok(text) = tokio::fs::read_to_string(&pos_file).await {
                let mut it = text.split_whitespace().filter_map(|x| x.parse::<f64>().ok());
                res.position = it.next();
                res.duration = it.next();
            }
        }
        Err(_) => {
            // mpv нет — пробуем VLC, затем системный плеер.
            return play_vlc(&opts).await;
        }
    }
    res
}

/// Доля просмотра, после которой серия считается просмотренной.
pub const WATCHED_FRACTION: f64 = 0.85;

pub fn is_finished(position: f64, duration: f64) -> bool {
    duration > 60.0 && position / duration >= WATCHED_FRACTION
}

pub fn remembered_position(release: i64, source: i64, position: i64) -> f64 {
    storage::read(|s| s.positions.get(&storage::position_key(release, source, position)).copied().unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_urls() {
        assert_eq!(external_scheme_url("outplayer", "https://a/b.m3u8").unwrap(), "outplayer://https://a/b.m3u8");
        assert_eq!(external_scheme_url("nplayer", "https://a/b").unwrap(), "nplayer-https://a/b");
        assert!(external_scheme_url("default", "x").is_none());
    }

    #[test]
    fn lua_has_markers() {
        let m = vec![Marker { name: "opening".into(), start: 10.0, end: 90.0 }];
        let s = mpv_script(&m, true, "C:\\tmp\\p.txt");
        assert!(s.contains("s=10.000, e=90.000"));
        assert!(s.contains("C:/tmp/p.txt"));
        assert!(is_finished(1300.0, 1400.0));
        assert!(!is_finished(100.0, 1400.0));
    }
}
