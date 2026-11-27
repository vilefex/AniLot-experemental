//! Проверка обновлений через последний GitHub Release.

use crate::aniskip::http;
use crate::models::*;
use crate::storage;
use serde_json::Value;

pub const REPO: &str = "vilefex/AniLot-experemental";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Номер сборки — мажорная часть версии (v10).
pub fn app_build() -> i64 {
    env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0)
}

#[derive(Clone, Debug, Default)]
pub struct UpdateInfo {
    pub tag: String,
    pub name: String,
    pub notes: String,
    pub page_url: String,
    pub asset_url: String,
    pub asset_name: String,
    pub remote_build: i64,
    pub is_newer: bool,
}

fn build_from_tag(tag: &str) -> i64 {
    let digits: String = tag.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().unwrap_or(0)
}

/// Подходит ли файл релиза текущей платформе.
pub fn asset_matches(name: &str) -> bool {
    let n = name.to_lowercase();
    if cfg!(target_os = "ios") {
        n.ends_with(".ipa")
    } else if cfg!(target_os = "android") {
        n.ends_with(".apk")
    } else if cfg!(target_os = "macos") {
        n.ends_with(".dmg")
    } else if cfg!(target_os = "windows") {
        n.ends_with(".exe")
    } else {
        n.ends_with(".tar.gz") || n.ends_with(".tar.zst")
    }
}

pub async fn check() -> Result<UpdateInfo, String> {
    let resp = http()
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Сеть недоступна: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub вернул HTTP {}", resp.status().as_u16()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = s(&v, "tag_name");
    let remote = build_from_tag(&tag);
    let mut info = UpdateInfo {
        name: if s(&v, "name").is_empty() { tag.clone() } else { s(&v, "name") },
        notes: s(&v, "body"),
        page_url: s(&v, "html_url"),
        remote_build: remote,
        is_newer: remote > app_build(),
        tag,
        ..Default::default()
    };
    for a in arr(&v, "assets") {
        let name = s(a, "name");
        if asset_matches(&name) {
            info.asset_url = s(a, "browser_download_url");
            info.asset_name = name;
            break;
        }
    }
    storage::write(|st| st.settings.last_update_check = storage::now());
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags() {
        assert_eq!(build_from_tag("anilot-10"), 10);
        assert_eq!(build_from_tag("anilot-9"), 9);
        assert_eq!(build_from_tag("v12"), 12);
        assert_eq!(build_from_tag("none"), 0);
    }
}
