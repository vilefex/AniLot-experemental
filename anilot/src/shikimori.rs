//! Двусторонняя синхронизация списков Anixart ↔ Shikimori.

use crate::aniskip::{self, first_number, normalize};
use crate::api::{Api, ListKind};
use crate::models::*;
use crate::storage;
use reqwest::Method;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BASE: &str = "https://shikimori.io";
/// Авторизация без собственного URL-схемы: Shikimori показывает код на странице — его вставляют в приложение.
pub const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const MIN_INTERVAL: Duration = Duration::from_millis(700);

static LAST_REQUEST: tokio::sync::Mutex<Option<Instant>> = tokio::sync::Mutex::const_new(None);

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub total: usize,
    pub synced: usize,
    pub progress_synced: usize,
    pub problems: Vec<String>,
    pub fatal: Option<String>,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SyncState {
    pub running: bool,
    pub message: String,
    pub fraction: f32,
    pub report: Option<Report>,
}

#[derive(Clone, Default)]
pub struct SyncHandle {
    pub state: Arc<Mutex<SyncState>>,
    pub cancel: Arc<AtomicBool>,
}

impl SyncHandle {
    pub fn snapshot(&self) -> SyncState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
    fn set(&self, msg: impl Into<String>, fraction: f32) {
        if let Ok(mut s) = self.state.lock() {
            s.message = msg.into();
            s.fraction = fraction;
        }
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

// ───────────── сопоставление ─────────────

pub fn anixart_status(shiki: &str) -> i64 {
    match shiki {
        "planned" => 2,
        "watching" | "rewatching" => 1,
        "completed" => 3,
        "on_hold" => 4,
        "dropped" => 5,
        _ => 0,
    }
}

pub fn shiki_status(anixart: i64) -> Option<&'static str> {
    match anixart {
        1 => Some("watching"),
        2 => Some("planned"),
        3 => Some("completed"),
        4 => Some("on_hold"),
        5 => Some("dropped"),
        _ => None,
    }
}

/// Оценка соответствия релиза Anixart записи Shikimori. Принимаем результат от 90.
pub fn score_release(release: &Release, wanted: &[String], year: i64, episodes: i64) -> i64 {
    let titles: Vec<String> = release.all_titles().iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    let (mut exact, mut contains) = (false, false);
    for c in &titles {
        for w in wanted {
            if c == w {
                exact = true;
            } else if c.chars().count() >= 6 && w.chars().count() >= 6 && (c.contains(w.as_str()) || w.contains(c.as_str())) {
                contains = true;
            }
        }
    }
    let mut score = if exact { 100 } else if contains { 40 } else { 0 };
    if score == 0 {
        return 0;
    }
    let ry = release.year_num();
    if year > 0 && ry > 0 {
        score += if year == ry { 30 } else { -60 };
    }
    if episodes > 0 && release.episodes_total > 0 {
        score += if release.episodes_total == episodes { 20 } else { -10 };
    }
    score
}

// ───────────── HTTP ─────────────

pub fn is_authorized() -> bool {
    storage::read(|s| !s.shiki.access_token.is_empty())
}

async fn throttle() {
    let mut last = LAST_REQUEST.lock().await;
    if let Some(t) = *last {
        let el = t.elapsed();
        if el < MIN_INTERVAL {
            tokio::time::sleep(MIN_INTERVAL - el).await;
        }
    }
    *last = Some(Instant::now());
}

struct Resp {
    status: u16,
    json: Value,
}

async fn request(
    method: Method,
    path: &str,
    query: &[(&str, String)],
    form: Option<&[(&str, String)]>,
    body: Option<Value>,
    authorized: bool,
) -> Resp {
    if authorized {
        refresh_if_needed().await;
    }
    let mut status = 0u16;
    for attempt in 0..3u64 {
        throttle().await;
        let mut rb = aniskip::http().request(method.clone(), format!("{BASE}{path}")).header("Accept", "application/json");
        if !query.is_empty() {
            rb = rb.query(query);
        }
        if authorized {
            let token = storage::read(|s| s.shiki.access_token.clone());
            if !token.is_empty() {
                rb = rb.bearer_auth(token);
            }
        }
        if let Some(f) = form {
            rb = rb.form(f);
        }
        if let Some(b) = &body {
            rb = rb.json(b);
        }
        match rb.send().await {
            Ok(resp) => {
                status = resp.status().as_u16();
                if status == 429 || status >= 500 {
                    tokio::time::sleep(Duration::from_millis(2500 * (attempt + 1))).await;
                    continue;
                }
                let text = resp.text().await.unwrap_or_default();
                let json = serde_json::from_str(&text).unwrap_or(Value::Null);
                return Resp { status, json };
            }
            Err(_) => {
                status = 0;
                tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1))).await;
            }
        }
    }
    Resp { status, json: Value::Null }
}

fn store_token(v: &Value) -> bool {
    let access = s(v, "access_token");
    if access.is_empty() {
        return false;
    }
    let created = i(v, "created_at");
    let created = if created > 0 { created } else { storage::now() };
    let expires = created + i(v, "expires_in").max(60);
    let refresh = s(v, "refresh_token");
    storage::write(|st| {
        st.shiki.access_token = access;
        if !refresh.is_empty() {
            st.shiki.refresh_token = refresh;
        }
        st.shiki.expires_at = expires;
    });
    true
}

async fn refresh_if_needed() {
    let (refresh, expires, id, secret) = storage::read(|s| {
        (
            s.shiki.refresh_token.clone(),
            s.shiki.expires_at,
            s.settings.shiki_client_id.clone(),
            s.settings.shiki_client_secret.clone(),
        )
    });
    if refresh.is_empty() || id.is_empty() || storage::now() < expires - 120 {
        return;
    }
    let form = [
        ("grant_type", "refresh_token".to_string()),
        ("client_id", id),
        ("client_secret", secret),
        ("refresh_token", refresh),
    ];
    throttle().await;
    if let Ok(resp) = aniskip::http().post(format!("{BASE}/oauth/token")).form(&form).send().await {
        if resp.status().is_success() {
            if let Ok(v) = resp.json::<Value>().await {
                store_token(&v);
            }
        }
    }
}

// ───────────── OAuth ─────────────

pub fn authorize_url() -> Result<String, String> {
    let (id, secret) = storage::read(|s| (s.settings.shiki_client_id.clone(), s.settings.shiki_client_secret.clone()));
    if id.trim().is_empty() || secret.trim().is_empty() {
        return Err("Сначала укажите Client ID и Client Secret приложения Shikimori.".into());
    }
    Ok(format!(
        "{BASE}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=user_rates",
        aniskip::urlencode(id.trim()),
        aniskip::urlencode(REDIRECT_URI)
    ))
}

/// Обмен кода на токен и получение данных аккаунта. `Err` содержит текст для пользователя.
pub async fn authorize_with_code(code: &str) -> Result<String, String> {
    let (id, secret) = storage::read(|s| (s.settings.shiki_client_id.clone(), s.settings.shiki_client_secret.clone()));
    if id.is_empty() || secret.is_empty() {
        return Err("Сначала укажите Client ID и Client Secret приложения Shikimori.".into());
    }
    let form = [
        ("grant_type", "authorization_code".to_string()),
        ("client_id", id.trim().to_string()),
        ("client_secret", secret.trim().to_string()),
        ("code", code.trim().to_string()),
        ("redirect_uri", REDIRECT_URI.to_string()),
    ];
    let r = request(Method::POST, "/oauth/token", &[], Some(&form), None, false).await;
    if r.status != 200 || !store_token(&r.json) {
        return Err(format!("Shikimori не выдал токен (HTTP {}). {}", r.status, s(&r.json, "error_description")));
    }
    let who = request(Method::GET, "/api/users/whoami", &[], None, None, true).await;
    if who.status != 200 || i(&who.json, "id") <= 0 {
        logout();
        return Err(format!("Не удалось получить данные аккаунта (HTTP {}).", who.status));
    }
    let nick = s(&who.json, "nickname");
    let uid = i(&who.json, "id");
    storage::write(|st| {
        st.shiki.user_id = uid;
        st.shiki.nickname = nick.clone();
    });
    Ok(nick)
}

pub fn logout() {
    storage::write(|s| s.shiki = Default::default());
}

// ───────────── импорт ─────────────

pub async fn import(handle: SyncHandle, nickname: String, api: Api) {
    handle.cancel.store(false, Ordering::Relaxed);
    if let Ok(mut s) = handle.state.lock() {
        *s = SyncState { running: true, message: "Подготовка…".into(), fraction: 0.0, report: None };
    }
    let mut report = Report::default();
    run_import(&handle, nickname.trim(), &api, &mut report).await;
    report.cancelled = handle.cancelled();
    storage::write(|s| s.last_sync = storage::now());
    finish(&handle, report);
}

fn finish(handle: &SyncHandle, report: Report) {
    if let Ok(mut s) = handle.state.lock() {
        s.running = false;
        s.fraction = 1.0;
        s.message = if report.cancelled { "Остановлено".into() } else { "Готово".into() };
        s.report = Some(report);
    }
}

async fn run_import(h: &SyncHandle, nick: &str, api: &Api, report: &mut Report) {
    if !api.is_authorized() {
        report.fatal = Some("Для синхронизации войдите в аккаунт Anixart.".into());
        return;
    }
    if nick.is_empty() {
        report.fatal = Some("Укажите ник Shikimori.".into());
        return;
    }
    storage::write(|s| s.last_import_nick = nick.to_string());
    let own = storage::read(|s| s.shiki.nickname.eq_ignore_ascii_case(nick) && !s.shiki.access_token.is_empty());
    h.set("Загрузка профиля Shikimori…", 0.0);
    let user = request(
        Method::GET,
        &format!("/api/users/{}", aniskip::urlencode(nick).replace('+', "%20")),
        &[("is_nickname", "1".into())],
        None,
        None,
        own,
    )
    .await;
    if user.status == 404 || i(&user.json, "id") <= 0 {
        report.fatal = Some(if user.status == 404 {
            "Пользователь Shikimori не найден.".into()
        } else {
            format!("Shikimori недоступен (HTTP {}).", user.status)
        });
        return;
    }
    let uid = i(&user.json, "id");
    let mut rates: Vec<Value> = vec![];
    let mut seen = std::collections::HashSet::new();
    for page in 1..=60 {
        if h.cancelled() {
            return;
        }
        h.set(format!("Чтение списка Shikimori… (стр. {page})"), 0.0);
        let chunk = request(
            Method::GET,
            &format!("/api/users/{uid}/anime_rates"),
            &[("limit", "100".into()), ("page", page.to_string()), ("censored", "false".into())],
            None,
            None,
            own,
        )
        .await;
        let Value::Array(items) = chunk.json else {
            if page == 1 {
                report.fatal = Some(format!(
                    "Не удалось получить список (HTTP {}). Возможно, список скрыт настройками приватности.",
                    chunk.status
                ));
                return;
            }
            break;
        };
        let n = items.len();
        for rate in items {
            let aid = rate.get("anime").map(|a| i(a, "id")).unwrap_or(0);
            if aid > 0 && seen.insert(aid) {
                rates.push(rate);
            }
        }
        if n < 100 {
            break;
        }
    }
    report.total = rates.len();
    let with_progress = storage::read(|s| s.settings.shiki_sync_progress);
    let total = rates.len().max(1);
    for (idx, rate) in rates.iter().enumerate() {
        if h.cancelled() {
            return;
        }
        let anime = rate.get("anime").cloned().unwrap_or(Value::Null);
        let title = if !s(&anime, "russian").is_empty() { s(&anime, "russian") } else { s(&anime, "name") };
        h.set(format!("{}/{} · {}", idx + 1, total, title), idx as f32 / total as f32);
        let status = anixart_status(&s(rate, "status"));
        if status == 0 {
            report.problems.push(title);
            continue;
        }
        let Some(rid) = release_for_anime(&anime, api).await else {
            report.problems.push(title);
            continue;
        };
        if api.set_list_status(rid, 0, status).await.is_err() {
            report.problems.push(title);
            continue;
        }
        report.synced += 1;
        let watched = i(rate, "episodes");
        let total_eps = i(&anime, "episodes");
        if with_progress && (watched > 0 || status == 3) {
            let marked = apply_progress(api, rid, watched, total_eps, status == 3, h).await;
            if marked > 0 {
                report.progress_synced += 1;
            }
        }
    }
}

/// Релиз Anixart для записи Shikimori: из кэша или поиском по названию.
async fn release_for_anime(anime: &Value, api: &Api) -> Option<i64> {
    let mal = i(anime, "id");
    let key = mal.to_string();
    if let Some(c) = storage::read(|s| s.shiki_map.get(&key).copied()).filter(|c| *c > 0) {
        return Some(c);
    }
    if let Some(k) = aniskip::known_release_for_mal(mal) {
        return Some(k);
    }
    let name = s(anime, "name");
    let russian = s(anime, "russian");
    let wanted: Vec<String> = [&name, &russian].iter().map(|t| normalize(t)).filter(|t| !t.is_empty()).collect();
    if wanted.is_empty() {
        return None;
    }
    let year = s(anime, "aired_on").get(..4).and_then(|y| y.parse().ok()).unwrap_or(0);
    let episodes = i(anime, "episodes");
    let mut queries: Vec<String> = vec![];
    if !name.is_empty() {
        queries.push(name.clone());
    }
    if !russian.is_empty() && russian != name {
        queries.push(russian);
    }
    let (mut best_id, mut best_score) = (0i64, 0i64);
    for q in queries {
        if let Ok(page) = api.releases_page(&ListKind::Search(q), 0).await {
            for r in &page.items {
                let sc = score_release(r, &wanted, year, episodes);
                if sc > best_score {
                    best_score = sc;
                    best_id = r.id;
                }
            }
        }
        if best_score >= 120 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if best_score >= 90 && best_id > 0 {
        storage::write(|s| {
            s.shiki_map.insert(key, best_id);
        });
        return Some(best_id);
    }
    None
}

/// Отмечает серии просмотренными в Anixart. Возвращает число отмеченных серий.
async fn apply_progress(api: &Api, rid: i64, watched: i64, total: i64, completed: bool, h: &SyncHandle) -> usize {
    let Ok(types) = api.voice_types(rid).await else { return 0 };
    // Озвучка с наибольшим числом серий (закреплённая — в приоритете).
    let Some(ty) = types.iter().max_by_key(|t| (t.pinned, t.episodes_count)) else { return 0 };
    let Ok(sources) = api.sources(rid, ty.id).await else { return 0 };
    let Some(src) = sources.iter().max_by_key(|s| s.episodes_count) else { return 0 };
    let Ok(mut eps) = api.episodes(rid, ty.id, src.id).await else { return 0 };
    eps.sort_by_key(|e| e.position);
    let limit = if completed {
        eps.len()
    } else {
        let w = if total > 0 { watched.min(total) } else { watched };
        (w.max(0) as usize).min(eps.len())
    };
    let mut marked = 0;
    for e in eps.iter().take(limit) {
        if h.cancelled() {
            break;
        }
        if e.is_watched {
            marked += 1;
            continue;
        }
        if api.mark_watched(rid, src.id, e.position).await.is_ok() {
            marked += 1;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    marked
}

// ───────────── экспорт ─────────────

pub async fn export(handle: SyncHandle, api: Api) {
    handle.cancel.store(false, Ordering::Relaxed);
    if let Ok(mut s) = handle.state.lock() {
        *s = SyncState { running: true, message: "Подготовка…".into(), fraction: 0.0, report: None };
    }
    let mut report = Report::default();
    run_export(&handle, &api, &mut report).await;
    report.cancelled = handle.cancelled();
    storage::write(|s| s.last_sync = storage::now());
    finish(&handle, report);
}

async fn run_export(h: &SyncHandle, api: &Api, report: &mut Report) {
    if !is_authorized() {
        report.fatal = Some("Для экспорта войдите в аккаунт Shikimori.".into());
        return;
    }
    if !api.is_authorized() {
        report.fatal = Some("Для синхронизации войдите в аккаунт Anixart.".into());
        return;
    }
    let user_id = storage::read(|s| s.shiki.user_id);
    let with_progress = storage::read(|s| s.settings.shiki_sync_progress);
    let mut items: Vec<(Release, i64)> = vec![];
    for status in 1..=5 {
        if h.cancelled() {
            return;
        }
        h.set(format!("Чтение списков Anixart… ({status}/5)"), 0.0);
        let kind = ListKind::ProfileList { profile: 0, status };
        let mut page = 0;
        loop {
            match api.releases_page(&kind, page).await {
                Ok(p) => {
                    let last = p.is_last();
                    items.extend(p.items.into_iter().map(|r| (r, status)));
                    if last || page > 200 || h.cancelled() {
                        break;
                    }
                    page += 1;
                }
                Err(e) => {
                    report.fatal = Some(format!("Не удалось прочитать списки Anixart: {e}"));
                    return;
                }
            }
        }
    }
    report.total = items.len();
    let total = items.len().max(1);
    for (idx, (release, status)) in items.iter().enumerate() {
        if h.cancelled() {
            return;
        }
        let title = release.title();
        h.set(format!("{}/{} · {}", idx + 1, total, title), idx as f32 / total as f32);
        let Some(mal) = aniskip::lookup_mal(release).await else {
            report.problems.push(title);
            continue;
        };
        let mut watched = 0;
        if *status == 3 {
            watched = release.episodes_total;
        } else if with_progress && matches!(*status, 1 | 4 | 5) {
            watched = watched_episodes(api, release).await;
            if release.episodes_total > 0 {
                watched = watched.min(release.episodes_total);
            }
        }
        let mut rate = json!({
            "user_id": user_id, "target_id": mal, "target_type": "Anime",
            "status": shiki_status(*status).unwrap_or("planned"),
        });
        if watched > 0 {
            rate["episodes"] = json!(watched);
        }
        if push_user_rate(rate, user_id, mal).await {
            report.synced += 1;
            if watched > 0 {
                report.progress_synced += 1;
            }
        } else {
            report.problems.push(title);
        }
    }
}

async fn watched_episodes(api: &Api, release: &Release) -> i64 {
    if !release.last_episode_name.is_empty() {
        return first_number(&release.last_episode_name);
    }
    match api.release(release.id).await {
        Ok(full) => first_number(&full.last_episode_name),
        Err(_) => 0,
    }
}

/// Создаёт запись списка в Shikimori или обновляет существующую.
async fn push_user_rate(rate: Value, user_id: i64, target: i64) -> bool {
    let r = request(Method::POST, "/api/v2/user_rates", &[], None, Some(json!({"user_rate": rate.clone()})), true).await;
    if r.status == 200 || r.status == 201 {
        return true;
    }
    if r.status != 422 && r.status != 409 {
        return false;
    }
    let existing = request(
        Method::GET,
        "/api/v2/user_rates",
        &[("user_id", user_id.to_string()), ("target_id", target.to_string()), ("target_type", "Anime".into())],
        None,
        None,
        true,
    )
    .await;
    let rate_id = existing.json.as_array().and_then(|a| a.first()).map(|f| i(f, "id")).unwrap_or(0);
    if rate_id <= 0 {
        return false;
    }
    let mut patch = json!({"status": rate["status"]});
    if rate.get("episodes").is_some() {
        patch["episodes"] = rate["episodes"].clone();
    }
    let p = request(Method::PATCH, &format!("/api/v2/user_rates/{rate_id}"), &[], None, Some(json!({"user_rate": patch})), true).await;
    p.status == 200 || p.status == 201
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping_roundtrip() {
        for st in 1..=5 {
            assert_eq!(anixart_status(shiki_status(st).unwrap()), st);
        }
        assert_eq!(anixart_status("rewatching"), 1);
        assert_eq!(anixart_status("???"), 0);
    }

    #[test]
    fn scoring() {
        let r = Release { title_original: "Sousou no Frieren".into(), year: "2023".into(), episodes_total: 28, ..Default::default() };
        let w = vec![normalize("Sousou no Frieren")];
        assert_eq!(score_release(&r, &w, 2023, 28), 150);
        assert!(score_release(&r, &w, 2010, 12) < 90);
        assert_eq!(score_release(&r, &[normalize("Other")], 2023, 28), 0);
    }
}
