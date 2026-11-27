//! AniSkip: сопоставление релиза Anixart с MAL ID (через Shikimori) и получение меток OP/ED.

use crate::models::Release;
use crate::storage::{self, SkipEntry};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

const MAL_NOT_FOUND_TTL: i64 = 7 * 24 * 3600;
const MARKERS_TTL: i64 = 14 * 24 * 3600;
const MARKERS_MISS_TTL: i64 = 24 * 3600;
const MARKERS_PRUNE_AGE: i64 = 60 * 24 * 3600;

pub fn http() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .user_agent("AniLot")
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default()
    });
    &CLIENT
}

/// GET с повторами; 4xx (кроме 429) не повторяем.
pub async fn get_retry(url: &str, tries: u32) -> (u16, Option<String>) {
    let mut code = 0u16;
    for n in 0..tries {
        match http().get(url).header("Accept", "application/json").send().await {
            Ok(resp) => {
                code = resp.status().as_u16();
                if code == 200 {
                    return (code, resp.text().await.ok());
                }
                if (400..500).contains(&code) && code != 429 {
                    break;
                }
            }
            Err(_) => code = 0,
        }
        tokio::time::sleep(Duration::from_millis(800 * (n as u64 + 1))).await;
    }
    (code, None)
}

pub fn normalize(title: &str) -> String {
    title.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Номер серии из названия. Не берём первое произвольное число: оно может быть номером сезона.
pub fn episode_number(title: &str) -> i64 {
    static EXPLICIT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(?:episode|ep\.?|series|серия|сер\.?|эпизод)\s*#?\s*(\d{1,4})").unwrap());
    static LEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(\d{1,4})(?:\s*(?:[-.:)]|серия|эпизод|episode|ep\.?))").unwrap()
    });
    static ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,4})\s*$").unwrap());
    if title.is_empty() {
        return 0;
    }
    let name = title.split(" · ").next().unwrap_or(title);
    for re in [&*EXPLICIT, &*LEADING, &*ONLY] {
        if let Some(c) = re.captures(name) {
            return c[1].parse().unwrap_or(0);
        }
    }
    0
}

/// Первое число в названии (для переноса прогресса в Shikimori: «5 серия» → 5).
pub fn first_number(title: &str) -> i64 {
    static ANY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)").unwrap());
    ANY.captures(title).and_then(|c| c[1].parse().ok()).unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: i64,
    pub names: Vec<String>,
    pub year: i64,
    pub episodes: i64,
}

fn has_exact_title(c: &Candidate, query: &str) -> bool {
    let q = normalize(query);
    !q.is_empty() && c.names.iter().any(|n| normalize(n) == q)
}

/// Сначала точное название, затем совместимые год и число серий. Не выбираем первый похожий результат.
pub fn choose_candidate(cands: &[Candidate], queries: &[String], year: i64, episodes_total: i64) -> Option<i64> {
    let mut exact: Vec<&Candidate> = vec![];
    for c in cands {
        if c.id <= 0 || !queries.iter().any(|q| has_exact_title(c, q)) {
            continue;
        }
        if year > 0 && c.year > 0 && (c.year - year).abs() > 1 {
            continue;
        }
        if episodes_total > 0 && c.episodes > 0 && c.episodes != episodes_total {
            continue;
        }
        exact.push(c);
    }
    if !exact.is_empty() {
        let score = |c: &Candidate| {
            let ys = if year <= 0 || c.year <= 0 { 2 } else { (c.year - year).abs() };
            let es = if episodes_total <= 0 || c.episodes <= 0 { 1 } else { 0 };
            (ys, es)
        };
        let best = exact.iter().map(|c| score(c)).min()?;
        let mut ids: Vec<i64> = exact.iter().filter(|c| score(c) == best).map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        // Два равноценных разных MAL ID — неоднозначность, а не повод брать первый.
        return if ids.len() == 1 { Some(ids[0]) } else { None };
    }
    if year > 0 && episodes_total > 0 {
        let mut ids: Vec<i64> = cands
            .iter()
            .filter(|c| c.id > 0 && c.year == year && c.episodes == episodes_total)
            .map(|c| c.id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() == 1 {
            return Some(ids[0]);
        }
    }
    None
}

fn year_of(aired: &str) -> i64 {
    aired.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0)
}

/// Публичный API Shikimori: поиск по названию. `Err` — сетевой сбой (не кэшируется).
async fn resolve_via_shikimori(queries: &[String], year: i64, episodes: i64) -> Result<Option<i64>, String> {
    let mut all: Vec<Candidate> = vec![];
    let mut succeeded = false;
    for q in queries.iter().take(2) {
        let url = format!("https://shikimori.one/api/animes?search={}&limit=8", urlencode(q));
        let (code, body) = get_retry(&url, 3).await;
        let Some(body) = body.filter(|_| code == 200) else { continue };
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&body) else { continue };
        succeeded = true;
        for it in items {
            let id = crate::models::i(&it, "id");
            if id <= 0 {
                continue;
            }
            let names = ["name", "russian"]
                .iter()
                .map(|k| crate::models::s(&it, k))
                .filter(|n| !n.is_empty())
                .collect();
            all.push(Candidate {
                id,
                names,
                year: year_of(&crate::models::s(&it, "aired_on")),
                episodes: crate::models::i(&it, "episodes"),
            });
        }
        if let Some(found) = choose_candidate(&all, queries, year, episodes) {
            return Ok(Some(found));
        }
    }
    if !succeeded {
        return Err("Shikimori недоступен".into());
    }
    Ok(choose_candidate(&all, queries, year, episodes))
}

pub fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// MAL ID кэшируется по релизу. Отрицательный результат повторно ищется через неделю.
pub async fn lookup_mal(release: &Release) -> Option<i64> {
    let key = release.id.to_string();
    let (cached, checked) = storage::read(|s| (s.mal_ids.get(&key).copied(), s.mal_checked.get(&key).copied()));
    match cached {
        Some(id) if id > 0 => return Some(id),
        Some(_) => {
            if let Some(at) = checked {
                let age = storage::now() - at;
                if (0..MAL_NOT_FOUND_TTL).contains(&age) {
                    return None;
                }
            }
        }
        None => {}
    }
    let mut queries: Vec<String> = vec![];
    if !release.title_original.is_empty() {
        queries.push(release.title_original.clone());
    }
    if !release.title_ru.is_empty() {
        queries.push(release.title_ru.clone());
    }
    for t in release.all_titles() {
        if !queries.contains(&t) && t.len() > 2 {
            queries.push(t);
        }
    }
    if queries.is_empty() {
        return None;
    }
    // Поиск идёт по первым двум названиям, сопоставление — по всем.
    match resolve_via_shikimori_all(&queries, release.year_num(), release.episodes_total).await {
        Ok(Some(id)) => {
            storage::write(|s| {
                s.mal_ids.insert(key, id);
            });
            Some(id)
        }
        Ok(None) => {
            storage::write(|s| {
                s.mal_ids.insert(key.clone(), 0);
                s.mal_checked.insert(key, storage::now());
            });
            None
        }
        Err(_) => None,
    }
}

async fn resolve_via_shikimori_all(queries: &[String], year: i64, episodes: i64) -> Result<Option<i64>, String> {
    resolve_via_shikimori(queries, year, episodes).await
}

/// Релиз Anixart, уже сопоставленный с этим MAL ID.
pub fn known_release_for_mal(mal: i64) -> Option<i64> {
    storage::read(|s| s.mal_ids.iter().find(|(_, v)| **v == mal).and_then(|(k, _)| k.parse().ok()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    pub start: f64,
    pub end: f64,
}

pub fn marker_label(name: &str) -> &'static str {
    match name {
        "opening" => "Опенинг",
        "ending" => "Эндинг",
        _ => "Рекап",
    }
}

fn cache_key(mal: i64, episode: i64) -> String {
    format!("{mal}:{episode}")
}

/// Метки из AniSkip: op/ed, mixed-op/mixed-ed (чистые важнее) и recap.
pub async fn fetch_markers(mal: i64, episode: i64) -> Vec<Marker> {
    let key = cache_key(mal, episode);
    let cached = storage::read(|s| s.skip_markers.get(&key).cloned());
    let to_markers = |e: &SkipEntry| {
        e.markers.iter().map(|(n, a, b)| Marker { name: n.clone(), start: *a, end: *b }).collect::<Vec<_>>()
    };
    if let Some(c) = &cached {
        let age = storage::now() - c.time;
        let ttl = if c.markers.is_empty() { MARKERS_MISS_TTL } else { MARKERS_TTL };
        if (0..ttl).contains(&age) {
            return to_markers(c);
        }
    }
    let url = format!(
        "https://api.aniskip.com/v2/skip-times/{mal}/{episode}?types%5B%5D=op&types%5B%5D=ed&types%5B%5D=mixed-op&types%5B%5D=mixed-ed&types%5B%5D=recap&episodeLength=0"
    );
    let (code, body) = get_retry(&url, 3).await;
    let store = |m: &[(String, f64, f64)]| {
        let entry = SkipEntry { time: storage::now(), markers: m.to_vec() };
        let key = key.clone();
        storage::write(|s| {
            s.skip_markers.insert(key, entry);
            // чистим записи старше 60 дней
            let now = storage::now();
            s.skip_markers.retain(|_, v| now - v.time < MARKERS_PRUNE_AGE);
        });
    };
    if code == 404 {
        store(&[]);
        return vec![];
    }
    let Some(body) = body.filter(|_| code == 200) else {
        // нет сети — берём устаревший кэш
        return cached.map(|c| to_markers(&c)).unwrap_or_default();
    };
    let Ok(json) = serde_json::from_str::<Value>(&body) else {
        return cached.map(|c| to_markers(&c)).unwrap_or_default();
    };
    let markers = parse_markers(&json);
    store(&markers.iter().map(|m| (m.name.clone(), m.start, m.end)).collect::<Vec<_>>());
    markers
}

pub fn parse_markers(json: &Value) -> Vec<Marker> {
    if !crate::models::b(json, "found") {
        return vec![];
    }
    let mut primary: Vec<Marker> = vec![];
    let mut mixed: Vec<Marker> = vec![];
    for item in crate::models::arr(json, "results") {
        let Some(interval) = item.get("interval") else { continue };
        let ty = crate::models::s(item, "skipType");
        let name = match ty.as_str() {
            "op" | "mixed-op" => "opening",
            "ed" | "mixed-ed" => "ending",
            "recap" => "recap",
            _ => continue,
        };
        let start = crate::models::f(interval, "startTime");
        let end = crate::models::f(interval, "endTime");
        if end <= start {
            continue;
        }
        let target = if ty.starts_with("mixed-") { &mut mixed } else { &mut primary };
        if !target.iter().any(|m| m.name == name) {
            target.push(Marker { name: name.into(), start, end });
        }
    }
    let mut out = vec![];
    for name in ["opening", "ending", "recap"] {
        if let Some(m) = primary.iter().chain(mixed.iter()).find(|m| m.name == name) {
            out.push(m.clone());
        }
    }
    out
}

/// Полная цепочка: релиз → MAL ID → метки серии.
pub async fn load_for_episode(release: &Release, episode_name: &str) -> Vec<Marker> {
    let episode = episode_number(episode_name);
    if episode <= 0 {
        return vec![];
    }
    match lookup_mal(release).await {
        Some(mal) => fetch_markers(mal, episode).await,
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_numbers() {
        assert_eq!(episode_number("5 серия"), 5);
        assert_eq!(episode_number("Серия 12"), 12);
        assert_eq!(episode_number("07"), 7);
        assert_eq!(episode_number("01 - Начало"), 1);
        assert_eq!(episode_number("Сезон 2 Финал"), 0);
        assert_eq!(first_number("Серия 5"), 5);
    }

    #[test]
    fn candidate_choice() {
        let c = |id, n: &str, year, eps| Candidate { id, names: vec![n.into()], year, episodes: eps };
        let cands = vec![c(1, "Naruto", 2002, 220), c(2, "Naruto: Shippuuden", 2007, 500)];
        assert_eq!(choose_candidate(&cands, &["Naruto".into()], 2002, 220), Some(1));
        let ambiguous = vec![c(1, "Foo", 2020, 12), c(2, "Foo", 2020, 12)];
        assert_eq!(choose_candidate(&ambiguous, &["Foo".into()], 2020, 12), Some(1).filter(|_| false));
    }

    #[test]
    fn markers_prefer_primary() {
        let j = serde_json::json!({"found": true, "results": [
            {"skipType": "mixed-op", "interval": {"startTime": 1.0, "endTime": 50.0}},
            {"skipType": "op", "interval": {"startTime": 10.0, "endTime": 90.0}},
            {"skipType": "ed", "interval": {"startTime": 1300.0, "endTime": 1390.0}}]});
        let m = parse_markers(&j);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], Marker { name: "opening".into(), start: 10.0, end: 90.0 });
    }
}
