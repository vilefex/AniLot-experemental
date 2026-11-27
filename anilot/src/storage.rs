//! Постоянное хранилище: настройки, токены, кэши AniSkip/MAL/Shikimori, позиции просмотра.
//! Всё лежит в одном JSON-файле в каталоге данных приложения.

use crate::platform;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    pub accent: String,
    /// "default" | "vlc" | "outplayer" | "nplayer"
    pub player: String,
    pub remember_source: bool,
    pub remember_position: bool,
    pub auto_next: bool,
    pub aniskip_enabled: bool,
    pub aniskip_auto: bool,
    pub alt_connection: bool,
    pub custom_api: String,
    pub user_agent: String,
    pub speed: f32,
    pub mpv_path: String,
    pub shiki_client_id: String,
    pub shiki_client_secret: String,
    pub shiki_sync_progress: bool,
    pub last_subtitle: String,
    pub last_update_check: i64,
    pub notify_episode: bool,
    pub notify_first_episode: bool,
    pub notify_comment: bool,
    pub notify_related: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(),
            accent: "#FF8A00".into(),
            player: "default".into(),
            remember_source: true,
            remember_position: true,
            auto_next: true,
            aniskip_enabled: true,
            aniskip_auto: true,
            alt_connection: false,
            custom_api: String::new(),
            user_agent: crate::api::DEFAULT_UA.into(),
            speed: 1.0,
            mpv_path: String::new(),
            shiki_client_id: String::new(),
            shiki_client_secret: String::new(),
            shiki_sync_progress: true,
            last_subtitle: String::new(),
            last_update_check: 0,
            notify_episode: true,
            notify_first_episode: true,
            notify_comment: true,
            notify_related: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ShikiAuth {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub user_id: i64,
    pub nickname: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SkipEntry {
    pub time: i64,
    /// (название, начало, конец) — opening / ending / recap
    pub markers: Vec<(String, f64, f64)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DownloadRecord {
    pub id: String,
    pub release_id: i64,
    pub release_title: String,
    pub image: String,
    pub episode_name: String,
    pub position: i64,
    pub file: String,
    pub size: u64,
    pub done: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Store {
    pub settings: Settings,
    pub token: String,
    pub profile_id: i64,
    pub login: String,
    pub shiki: ShikiAuth,
    pub search_history: Vec<String>,
    /// "release:source:position" → секунды
    pub positions: HashMap<String, f64>,
    /// release_id → (type_id, source_id)
    pub remembered_source: HashMap<String, (i64, i64)>,
    /// release_id → MAL id; 0 — не найден
    pub mal_ids: HashMap<String, i64>,
    pub mal_checked: HashMap<String, i64>,
    /// "mal:episode" → метки
    pub skip_markers: HashMap<String, SkipEntry>,
    /// MAL id → id релиза Anixart
    pub shiki_map: HashMap<String, i64>,
    pub last_sync: i64,
    pub last_import_nick: String,
    pub downloads: Vec<DownloadRecord>,
}

static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| Mutex::new(load()));

fn path() -> std::path::PathBuf {
    platform::data_dir().join("state.json")
}

fn load() -> Store {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn read<R>(f: impl FnOnce(&Store) -> R) -> R {
    let g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(&g)
}

/// Изменяет хранилище и сразу сохраняет его на диск.
pub fn write<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    let mut g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let r = f(&mut g);
    let _ = std::fs::create_dir_all(platform::data_dir());
    if let Ok(text) = serde_json::to_string(&*g) {
        let tmp = path().with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path());
        }
    }
    r
}

pub fn settings() -> Settings {
    read(|s| s.settings.clone())
}

pub fn position_key(release: i64, source: i64, position: i64) -> String {
    format!("{release}:{source}:{position}")
}

pub fn add_search_history(q: &str) {
    let q = q.trim().to_string();
    if q.is_empty() {
        return;
    }
    write(|s| {
        s.search_history.retain(|x| x != &q);
        s.search_history.insert(0, q);
        s.search_history.truncate(30);
    });
}

/// Очистка кэшей (метки AniSkip, соответствия MAL). Токены и настройки не затрагиваются.
pub fn clear_caches() {
    write(|s| {
        s.skip_markers.clear();
        s.mal_ids.clear();
        s.mal_checked.clear();
        s.shiki_map.clear();
        s.positions.clear();
        s.remembered_source.clear();
    });
}
