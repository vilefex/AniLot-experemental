//! Субтитры: обнаружение внешних дорожек и разбор WebVTT / SubRip / ASS-SSA.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Vtt,
    Srt,
    Ass,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Vtt => "vtt",
            Format::Srt => "srt",
            Format::Ass => "ass",
        }
    }
}

pub fn detect_format(text: &str, url: &str) -> Format {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("WEBVTT") {
        return Format::Vtt;
    }
    if head.starts_with("[Script Info]") || head.contains("\n[Events]") || head.starts_with("[V4") {
        return Format::Ass;
    }
    let lower = url.split('?').next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".ass") || lower.ends_with(".ssa") {
        return Format::Ass;
    }
    if lower.ends_with(".vtt") {
        return Format::Vtt;
    }
    Format::Srt
}

/// Разбор «часы:минуты:секунды.доли» или «минуты:секунды.доли».
pub fn parse_time(t: &str) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    let parts: Vec<&str> = t.split(':').collect();
    let (h, m, s) = match parts.len() {
        3 => (parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?, parts[2].parse::<f64>().ok()?),
        2 => (0.0, parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + s)
}

fn strip_tags(text: &str) -> String {
    static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>|\{\\[^}]*\}").unwrap());
    TAGS.replace_all(text, "")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// WebVTT и SubRip отличаются только заголовком и разделителем долей секунды.
pub fn parse_cues_blocks(text: &str) -> Vec<Cue> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = vec![];
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(idx) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let mut it = lines[idx].split("-->");
        let (Some(a), Some(b)) = (it.next(), it.next()) else { continue };
        let end_str = b.trim().split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (parse_time(a), parse_time(end_str)) else { continue };
        let body = lines[idx + 1..].join("\n");
        let body = strip_tags(&body).trim().to_string();
        if !body.is_empty() {
            cues.push(Cue { start, end, text: body });
        }
    }
    cues
}

pub fn parse_ass(text: &str) -> Vec<Cue> {
    let mut cues = vec![];
    let mut in_events = false;
    let (mut i_start, mut i_end, mut i_text, mut n_fields) = (1usize, 2usize, 9usize, 10usize);
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(fmt) = line.strip_prefix("Format:") {
            let names: Vec<String> = fmt.split(',').map(|s| s.trim().to_lowercase()).collect();
            n_fields = names.len();
            i_start = names.iter().position(|n| n == "start").unwrap_or(1);
            i_end = names.iter().position(|n| n == "end").unwrap_or(2);
            i_text = names.iter().position(|n| n == "text").unwrap_or(n_fields - 1);
        } else if let Some(d) = line.strip_prefix("Dialogue:") {
            let fields: Vec<&str> = d.trim_start().splitn(n_fields, ',').collect();
            if fields.len() <= i_text.max(i_start).max(i_end) {
                continue;
            }
            let (Some(start), Some(end)) = (parse_time(fields[i_start]), parse_time(fields[i_end])) else { continue };
            let body = strip_tags(fields[i_text]).replace("\\N", "\n").replace("\\n", "\n").replace("\\h", " ");
            let body = body.trim().to_string();
            if !body.is_empty() {
                cues.push(Cue { start, end, text: body });
            }
        }
    }
    cues
}

pub fn parse(text: &str, url: &str) -> Vec<Cue> {
    match detect_format(text, url) {
        Format::Ass => parse_ass(text),
        _ => parse_cues_blocks(text),
    }
}

pub fn cue_at(cues: &[Cue], t: f64) -> Option<&Cue> {
    cues.iter().find(|c| t >= c.start && t < c.end)
}

pub fn to_srt(cues: &[Cue]) -> String {
    fn ts(t: f64) -> String {
        let ms = (t * 1000.0).round() as i64;
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, ms % 1000)
    }
    cues.iter()
        .enumerate()
        .map(|(i, c)| format!("{}\n{} --> {}\n{}\n", i + 1, ts(c.start), ts(c.end), c.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve(base: &str, link: &str) -> String {
    let link = link.replace("\\/", "/");
    url::Url::parse(base)
        .ok()
        .and_then(|b| b.join(&link).ok())
        .map(|u| u.to_string())
        .unwrap_or(link)
}

/// Ищет внешние дорожки в HTML встроенного плеера: теги `<track>` и описания PlayerJS (`subtitle:"[Rus]url,[Eng]url"`).
pub fn discover_tracks(html: &str, base_url: &str) -> Vec<Track> {
    static TRACK_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<track\b[^>]*>"#).unwrap());
    static ATTR_SRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bsrc\s*=\s*["']([^"']+)["']"#).unwrap());
    static ATTR_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\blabel\s*=\s*["']([^"']*)["']"#).unwrap());
    static ATTR_KIND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bkind\s*=\s*["']([^"']*)["']"#).unwrap());
    static PJS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?is)["']?subtitles?["']?\s*:\s*["']([^"']+)["']"#).unwrap());
    static PJS_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?:\[([^\]]*)\])?\s*(https?:[^,\s\[\]]+)"#).unwrap());

    let mut out: Vec<Track> = vec![];
    let push = |out: &mut Vec<Track>, label: String, url: String| {
        if !out.iter().any(|t| t.url == url) {
            out.push(Track { label: if label.trim().is_empty() { format!("Дорожка {}", out.len() + 1) } else { label }, url });
        }
    };
    for tag in TRACK_TAG.find_iter(html) {
        let tag = tag.as_str();
        if let Some(kind) = ATTR_KIND.captures(tag) {
            let k = kind[1].to_lowercase();
            if k != "subtitles" && k != "captions" {
                continue;
            }
        }
        if let Some(src) = ATTR_SRC.captures(tag) {
            let label = ATTR_LABEL.captures(tag).map(|l| l[1].to_string()).unwrap_or_default();
            push(&mut out, label, resolve(base_url, &src[1]));
        }
    }
    for cap in PJS.captures_iter(html) {
        let value = cap[1].replace("\\/", "/");
        for item in PJS_ITEM.captures_iter(&value) {
            let label = item.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            push(&mut out, label, item[2].to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtt_and_srt() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nПривет <i>мир</i>\n\n00:01:00.000 --> 00:01:02.000\nВторая";
        let cues = parse(vtt, "a.vtt");
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Привет мир");
        assert_eq!(cue_at(&cues, 2.0).unwrap().text, "Привет мир");
        let srt = "1\n00:00:01,500 --> 00:00:02,000\nHi\n";
        assert_eq!(parse(srt, "a.srt")[0].start, 1.5);
        assert!(to_srt(&cues).contains("00:00:01,000 --> 00:00:03,500"));
    }

    #[test]
    fn ass() {
        let ass = "[Script Info]\nTitle: x\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:05.00,0:00:07.50,Default,,0,0,0,,{\\an8}Hello, world\\Nline2";
        let cues = parse_ass(ass);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start, 5.0);
        assert_eq!(cues[0].text, "Hello, world\nline2");
    }

    #[test]
    fn discovery() {
        let html = r#"<video><track kind="subtitles" src="/subs/ru.vtt" label="Русский"></video>
            <script>new Playerjs({file:"x", subtitle:"[English]https://a.b/en.srt,[Русский]https://a.b/ru.ass"})</script>"#;
        let t = discover_tracks(html, "https://player.example/embed/1");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].url, "https://player.example/subs/ru.vtt");
        assert_eq!(t[2].label, "Русский");
    }
}
