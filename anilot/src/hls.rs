//! HLS: разбор плейлистов (выбор качества) и загрузка серий в файл.

use crate::aniskip::http;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub height: u32,
    pub bandwidth: u64,
    pub url: String,
}

impl Variant {
    pub fn label(&self) -> String {
        if self.height > 0 {
            format!("{}p", self.height)
        } else {
            format!("{} кбит/с", self.bandwidth / 1000)
        }
    }
}

fn resolve(base: &str, link: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|b| b.join(link.trim()).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| link.trim().to_string())
}

pub fn is_master(text: &str) -> bool {
    text.contains("#EXT-X-STREAM-INF")
}

/// Варианты качества из мастер-плейлиста, от лучшего к худшему.
pub fn parse_master(text: &str, base: &str) -> Vec<Variant> {
    let mut out = vec![];
    let mut pending: Option<(u32, u64)> = None;
    for line in text.lines().map(str::trim) {
        if let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let bandwidth = attr(attrs, "BANDWIDTH").and_then(|v| v.parse().ok()).unwrap_or(0);
            let height = attr(attrs, "RESOLUTION")
                .and_then(|r| r.split('x').nth(1).and_then(|h| h.parse().ok()))
                .unwrap_or(0);
            pending = Some((height, bandwidth));
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some((height, bandwidth)) = pending.take() {
                out.push(Variant { height, bandwidth, url: resolve(base, line) });
            }
        }
    }
    out.sort_by(|a, b| b.height.cmp(&a.height).then(b.bandwidth.cmp(&a.bandwidth)));
    out.dedup_by(|a, b| a.height == b.height && a.height != 0);
    out
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let start = attrs.find(&key)? + key.len();
    let rest = &attrs[start..];
    if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next().map(String::from)
    } else {
        rest.split(',').next().map(String::from)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Media {
    pub segments: Vec<String>,
    pub encrypted: bool,
    pub init_map: Option<String>,
}

pub fn parse_media(text: &str, base: &str) -> Media {
    let mut m = Media::default();
    for line in text.lines().map(str::trim) {
        if let Some(a) = line.strip_prefix("#EXT-X-KEY:") {
            if attr(a, "METHOD").map(|v| v != "NONE").unwrap_or(false) {
                m.encrypted = true;
            }
        } else if let Some(a) = line.strip_prefix("#EXT-X-MAP:") {
            m.init_map = attr(a, "URI").map(|u| resolve(base, &u));
        } else if !line.is_empty() && !line.starts_with('#') {
            m.segments.push(resolve(base, line));
        }
    }
    m
}

#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub bytes: AtomicU64,
    pub cancel: AtomicBool,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 {
            0.0
        } else {
            (self.done.load(Ordering::Relaxed) as f32 / t as f32).min(1.0)
        }
    }
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let resp = http().get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// Загружает серию в `dest` (mp4 — как есть; HLS — склейка сегментов в один .ts/.mp4-контейнер).
pub async fn download(url: &str, dest: &Path, progress: Arc<Progress>) -> Result<u64, String> {
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    let result = download_inner(url, &tmp, &progress).await;
    match result {
        Ok(size) => {
            tokio::fs::rename(&tmp, dest).await.map_err(|e| e.to_string())?;
            Ok(size)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

async fn download_inner(url: &str, tmp: &Path, progress: &Progress) -> Result<u64, String> {
    let is_hls = url.split('?').next().unwrap_or(url).to_lowercase().ends_with(".m3u8");
    let mut file = tokio::fs::File::create(tmp).await.map_err(|e| e.to_string())?;
    let mut total_bytes = 0u64;
    if !is_hls {
        let mut resp = http().get(url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        progress.total.store(resp.content_length().unwrap_or(0), Ordering::Relaxed);
        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            if progress.cancel.load(Ordering::Relaxed) {
                return Err("Загрузка отменена".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            total_bytes += chunk.len() as u64;
            progress.done.store(total_bytes, Ordering::Relaxed);
            progress.bytes.store(total_bytes, Ordering::Relaxed);
        }
        file.flush().await.map_err(|e| e.to_string())?;
        return Ok(total_bytes);
    }
    let mut playlist_url = url.to_string();
    let mut text = fetch_text(&playlist_url).await?;
    if is_master(&text) {
        let variants = parse_master(&text, &playlist_url);
        let best = variants.first().ok_or("В плейлисте нет потоков")?;
        playlist_url = best.url.clone();
        text = fetch_text(&playlist_url).await?;
    }
    let media = parse_media(&text, &playlist_url);
    if media.encrypted {
        return Err("Поток зашифрован — загрузка не поддерживается".into());
    }
    if media.segments.is_empty() {
        return Err("В плейлисте нет сегментов".into());
    }
    let mut urls = vec![];
    if let Some(init) = media.init_map {
        urls.push(init);
    }
    let init_count = urls.len();
    urls.extend(media.segments);
    progress.total.store((urls.len() - init_count) as u64, Ordering::Relaxed);
    for (n, seg) in urls.iter().enumerate() {
        let mut data = None;
        for attempt in 0..3u64 {
            if progress.cancel.load(Ordering::Relaxed) {
                return Err("Загрузка отменена".into());
            }
            match http().get(seg).send().await {
                Ok(r) if r.status().is_success() => {
                    if let Ok(b) = r.bytes().await {
                        data = Some(b);
                        break;
                    }
                }
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_millis(700 * (attempt + 1))).await;
        }
        let data = data.ok_or_else(|| format!("Не удалось загрузить сегмент {}", n + 1))?;
        file.write_all(&data).await.map_err(|e| e.to_string())?;
        total_bytes += data.len() as u64;
        progress.bytes.store(total_bytes, Ordering::Relaxed);
        if n >= init_count {
            progress.done.store((n + 1 - init_count) as u64, Ordering::Relaxed);
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    Ok(total_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_variants() {
        let t = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\n360/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2800000,RESOLUTION=1920x1080,CODECS=\"avc1.4d401f,mp4a.40.2\"\nhttps://cdn/1080.m3u8\n";
        let v = parse_master(t, "https://host/a/master.m3u8");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].height, 1080);
        assert_eq!(v[1].url, "https://host/a/360/index.m3u8");
        assert_eq!(v[0].label(), "1080p");
    }

    #[test]
    fn media_segments() {
        let t = "#EXTM3U\n#EXTINF:4,\nseg0.ts\n#EXTINF:4,\nseg1.ts\n#EXT-X-ENDLIST";
        let m = parse_media(t, "https://h/x/p.m3u8");
        assert_eq!(m.segments, vec!["https://h/x/seg0.ts", "https://h/x/seg1.ts"]);
        assert!(!m.encrypted);
        assert!(parse_media("#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\ns.ts", "https://h/").encrypted);
    }
}
