//! Клиент Anixart API (замена `libanixart.xcframework` из оригинального AniLot).

use crate::models::*;
use crate::storage;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_UA: &str = "AnixartApp/9.0-23121216 (Android 14; SDK 34; arm64-v8a; AniLot; ru)";
pub const BASE_PRIMARY: &str = "https://api-s.anixsekai.com";
pub const BASE_ALT: &str = "https://api.anixsekai.com";

#[derive(Clone)]
pub struct Api {
    client: reqwest::Client,
    base: String,
    pub token: String,
}

/// Фильтр каталога (`POST /filter/{page}`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Filter {
    pub sort: i64,
    pub category_id: Option<i64>,
    pub status_id: Option<i64>,
    pub season: Option<i64>,
    pub start_year: Option<i64>,
    pub end_year: Option<i64>,
    pub genres: Vec<String>,
    pub exclude_genres: bool,
    pub age_ratings: Vec<i64>,
    pub episodes_from: Option<i64>,
    pub episodes_to: Option<i64>,
    pub exclusions: Vec<i64>,
}

impl Filter {
    pub fn to_json(&self) -> Value {
        json!({
            "sort": self.sort,
            "category_id": self.category_id,
            "status_id": self.status_id,
            "season": self.season,
            "start_year": self.start_year,
            "end_year": self.end_year,
            "genres": self.genres,
            "genres_mode": if self.exclude_genres { 2 } else { 0 },
            "is_genres_exclude_mode_enabled": self.exclude_genres,
            "age_ratings": self.age_ratings,
            "episodes_from": self.episodes_from,
            "episodes_to": self.episodes_to,
            "profile_list_exclusions": self.exclusions,
            "types": [],
        })
    }
}

pub const SORTS: [(i64, &str); 6] = [
    (0, "По дате обновления"),
    (1, "По дате добавления"),
    (2, "По году"),
    (3, "По популярности"),
    (4, "По рейтингу"),
    (5, "По просмотрам"),
];
pub const CATEGORIES: [(i64, &str); 4] = [(1, "TV"), (2, "Фильм"), (3, "OVA/ONA"), (4, "Короткометражка")];
pub const RELEASE_STATUSES: [(i64, &str); 3] = [(1, "Завершён"), (2, "Выходит"), (3, "Анонс")];
pub const SEASONS: [(i64, &str); 4] = [(1, "Зима"), (2, "Весна"), (3, "Лето"), (4, "Осень")];

pub const GENRES: [&str; 45] = [
    "безумие", "боевик", "боевые искусства", "вампиры", "военное", "гарем", "демоны", "детектив", "детское", "дзёсей",
    "драма", "игры", "исторический", "история", "комедия", "космос", "магия", "машины", "меха", "мистика", "музыка",
    "музыкальный", "научная фантастика", "пародия", "повседневность", "полиция", "приключения", "психологическое",
    "романтика", "самураи", "сверхъестественное", "сейнен", "спорт", "супер сила", "сёдзё", "сёдзё-ай", "сёнен",
    "сёнен-ай", "триллер", "ужасы", "фантастика", "фэнтези", "школа", "экшен", "этти",
];

/// Откуда брать страницы релизов.
#[derive(Clone, Debug, PartialEq)]
pub enum ListKind {
    Filter(Filter),
    Search(String),
    /// profile == 0 — собственный список
    ProfileList { profile: i64, status: i64 },
    Favorites,
    History,
    /// Поиск внутри закладок: status 0 — избранное, -1 — история, 1..5 — списки
    Bookmarks { status: i64, query: String },
    Related(i64),
    CollectionReleases(i64),
    Votes(i64),
    Watching,
    Recommendations,
}

#[derive(Clone, Debug)]
pub struct SignIn {
    pub token: String,
    pub profile: Profile,
}

impl Api {
    pub fn from_settings() -> Api {
        let st = storage::read(|s| (s.settings.clone(), s.token.clone()));
        let (settings, token) = st;
        let base = if !settings.custom_api.trim().is_empty() {
            settings.custom_api.trim().trim_end_matches('/').to_string()
        } else if settings.alt_connection {
            BASE_ALT.to_string()
        } else {
            BASE_PRIMARY.to_string()
        };
        let ua = if settings.user_agent.trim().is_empty() { DEFAULT_UA.to_string() } else { settings.user_agent.clone() };
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("API-Version", reqwest::header::HeaderValue::from_static("v2"));
        let client = reqwest::Client::builder()
            .user_agent(ua)
            .default_headers(headers)
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap_or_default();
        Api { client, base, token }
    }

    pub fn is_authorized(&self) -> bool {
        !self.token.is_empty()
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    async fn send(&self, rb: reqwest::RequestBuilder, query: &[(&str, String)]) -> Result<Value, String> {
        let mut rb = rb;
        if !self.token.is_empty() {
            rb = rb.query(&[("token", self.token.as_str())]);
        }
        if !query.is_empty() {
            rb = rb.query(query);
        }
        let resp = rb.send().await.map_err(|e| format!("Сеть недоступна: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("Ошибка чтения ответа: {e}"))?;
        if !status.is_success() {
            return Err(format!("Сервер вернул HTTP {}", status.as_u16()));
        }
        if text.trim().is_empty() {
            return Ok(json!({"code": 0}));
        }
        serde_json::from_str(&text).map_err(|e| format!("Некорректный ответ сервера: {e}"))
    }

    pub async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        self.send(self.client.get(self.url(path)), query).await
    }

    pub async fn post_json(&self, path: &str, body: Value) -> Result<Value, String> {
        self.send(self.client.post(self.url(path)).json(&body), &[]).await
    }

    pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<Value, String> {
        self.send(self.client.post(self.url(path)).form(form), &[]).await
    }

    pub async fn post_empty(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        self.send(self.client.post(self.url(path)), query).await
    }

    /// Проверка поля `code`: 0 — успех.
    pub fn check(v: &Value) -> Result<(), String> {
        let code = i(v, "code");
        if code == 0 {
            Ok(())
        } else {
            Err(code_text(code))
        }
    }

    // ───────────── авторизация ─────────────

    pub async fn sign_in(&self, login: &str, password: &str) -> Result<SignIn, String> {
        let v = self
            .post_form("auth/signIn", &[("login", login.to_string()), ("password", password.to_string())])
            .await?;
        match i(&v, "code") {
            0 => {}
            2 => return Err("Неверный логин".into()),
            3 => return Err("Неверный пароль".into()),
            c => return Err(code_text(c)),
        }
        let token = v.get("profileToken").map(|t| s(t, "token")).unwrap_or_default();
        if token.is_empty() {
            return Err("Сервер не вернул токен авторизации".into());
        }
        let profile = v.get("profile").map(|p| Profile::from_json(p, true)).unwrap_or_default();
        Ok(SignIn { token, profile })
    }

    pub async fn check_login(&self, login: &str) -> Result<(), String> {
        let v = self.post_form("auth/checkLogin", &[("login", login.to_string())]).await?;
        match i(&v, "code") {
            0 => Ok(()),
            2 => Err("Логин занят".into()),
            3 => Err("Недопустимый логин".into()),
            c => Err(code_text(c)),
        }
    }

    /// Возвращает `hash` шага подтверждения.
    pub async fn sign_up(&self, login: &str, email: &str, password: &str) -> Result<String, String> {
        let rb = self
            .client
            .post(self.url("auth/signUp"))
            .header("X-Long-Code", "1")
            .form(&[("login", login), ("email", email), ("password", password)]);
        let v = self.send(rb, &[]).await?;
        match i(&v, "code") {
            0 => Ok(s(&v, "hash")),
            2 => Err("Логин занят или недопустим".into()),
            3 => Err("Недопустимый или занятый адрес почты".into()),
            c => Err(code_text(c)),
        }
    }

    pub async fn verify(&self, login: &str, email: &str, password: &str, hash: &str, code: &str) -> Result<SignIn, String> {
        let v = self
            .post_form(
                "auth/verify",
                &[
                    ("login", login.to_string()),
                    ("email", email.to_string()),
                    ("password", password.to_string()),
                    ("hash", hash.to_string()),
                    ("code", code.to_string()),
                ],
            )
            .await?;
        Api::check(&v).map_err(|_| "Неверный код подтверждения".to_string())?;
        let token = v.get("profileToken").map(|t| s(t, "token")).unwrap_or_default();
        if token.is_empty() {
            return Err("Сервер не вернул токен авторизации".into());
        }
        Ok(SignIn { token, profile: v.get("profile").map(|p| Profile::from_json(p, true)).unwrap_or_default() })
    }

    pub async fn restore(&self, data: &str) -> Result<String, String> {
        let rb = self.client.post(self.url("auth/restore")).header("X-Long-Code", "1").form(&[("data", data)]);
        let v = self.send(rb, &[]).await?;
        match i(&v, "code") {
            0 => Ok(s(&v, "hash")),
            2 => Err("Пользователь не найден".into()),
            c => Err(code_text(c)),
        }
    }

    pub async fn restore_verify(&self, data: &str, password: &str, hash: &str, code: &str) -> Result<(), String> {
        let v = self
            .post_form(
                "auth/restore/verify",
                &[
                    ("data", data.to_string()),
                    ("password", password.to_string()),
                    ("hash", hash.to_string()),
                    ("code", code.to_string()),
                ],
            )
            .await?;
        Api::check(&v).map_err(|_| "Неверный код подтверждения".to_string())
    }

    // ───────────── релизы ─────────────

    pub async fn release(&self, id: i64) -> Result<Release, String> {
        let v = self.get(&format!("release/{id}"), &[]).await?;
        Api::check(&v)?;
        v.get("release").map(Release::from_json).ok_or_else(|| "Релиз не найден".to_string())
    }

    pub async fn releases_page(&self, kind: &ListKind, page: i64) -> Result<Page<Release>, String> {
        let v = match kind {
            ListKind::Filter(f) => self.post_json(&format!("filter/{page}"), f.to_json()).await?,
            ListKind::Search(q) => {
                self.post_json(&format!("search/releases/{page}"), json!({"query": q, "search_by": 0})).await?
            }
            ListKind::ProfileList { profile, status } => {
                if *profile == 0 {
                    self.get(&format!("profile/list/all/{status}/{page}"), &[("sort", "1".into())]).await?
                } else {
                    self.get(&format!("profile/list/all/{profile}/{status}/{page}"), &[("sort", "1".into())]).await?
                }
            }
            ListKind::Favorites => self.get(&format!("favorite/all/{page}"), &[]).await?,
            ListKind::History => self.get(&format!("history/{page}"), &[]).await?,
            ListKind::Bookmarks { status, query } => {
                let body = json!({"query": query, "search_by": 0});
                match *status {
                    0 => self.post_json(&format!("search/favorites/{page}"), body).await?,
                    -1 => self.post_json(&format!("search/history/{page}"), body).await?,
                    st => self.post_json(&format!("search/profile/list/{st}/{page}"), body).await?,
                }
            }
            ListKind::Related(id) => self.get(&format!("related/{id}/{page}"), &[]).await?,
            ListKind::CollectionReleases(id) => self.get(&format!("collection/{id}/releases/{page}"), &[]).await?,
            ListKind::Votes(id) => self.get(&format!("profile/vote/release/voted/{id}/{page}"), &[]).await?,
            ListKind::Watching => self.post_empty(&format!("discover/watching/{page}"), &[]).await?,
            ListKind::Recommendations => {
                self.post_empty(&format!("discover/recommendations/{page}"), &[("previous_page", (page - 1).max(0).to_string())])
                    .await?
            }
        };
        Api::check(&v)?;
        Ok(Page::from_json(&v, Release::from_json))
    }

    pub async fn interesting(&self) -> Result<Vec<Interesting>, String> {
        let v = self.post_empty("discover/interesting", &[]).await?;
        Api::check(&v)?;
        Ok(arr(&v, "content").iter().map(Interesting::from_json).collect())
    }

    pub async fn discussing(&self) -> Result<Vec<Release>, String> {
        let v = self.post_empty("discover/discussing", &[]).await?;
        Api::check(&v)?;
        Ok(arr(&v, "content").iter().map(Release::from_json).collect())
    }

    pub async fn schedule(&self) -> Result<Vec<(String, Vec<Release>)>, String> {
        let v = self.get("schedule", &[]).await?;
        Api::check(&v)?;
        let days = [
            ("monday", "Понедельник"),
            ("tuesday", "Вторник"),
            ("wednesday", "Среда"),
            ("thursday", "Четверг"),
            ("friday", "Пятница"),
            ("saturday", "Суббота"),
            ("sunday", "Воскресенье"),
        ];
        Ok(days
            .iter()
            .map(|(k, n)| (n.to_string(), arr(&v, k).iter().map(Release::from_json).collect::<Vec<_>>()))
            .collect())
    }

    pub async fn feed(&self, page: i64) -> Result<Page<FeedItem>, String> {
        let v = self.get(&format!("feed/all/{page}"), &[]).await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, FeedItem::from_json))
    }

    // ───────────── серии ─────────────

    pub async fn voice_types(&self, release: i64) -> Result<Vec<VoiceType>, String> {
        let v = self.get(&format!("episode/{release}"), &[]).await?;
        Api::check(&v)?;
        Ok(arr(&v, "types").iter().map(VoiceType::from_json).collect())
    }

    pub async fn sources(&self, release: i64, type_id: i64) -> Result<Vec<Source>, String> {
        let v = self.get(&format!("episode/{release}/{type_id}"), &[]).await?;
        Api::check(&v)?;
        Ok(arr(&v, "sources").iter().map(Source::from_json).collect())
    }

    pub async fn episodes(&self, release: i64, type_id: i64, source: i64) -> Result<Vec<Episode>, String> {
        let v = self.get(&format!("episode/{release}/{type_id}/{source}"), &[]).await?;
        Api::check(&v)?;
        Ok(arr(&v, "episodes").iter().map(Episode::from_json).collect())
    }

    pub async fn episode_target(&self, release: i64, source: i64, position: i64, download: bool) -> Result<Episode, String> {
        let q: Vec<(&str, String)> = if download { vec![("download", "true".into())] } else { vec![] };
        let v = self.get(&format!("episode/target/{release}/{source}/{position}"), &q).await?;
        match i(&v, "code") {
            0 => {}
            2 => return Err("Загрузка этого релиза запрещена".into()),
            3 => return Err("Загрузка из этого источника запрещена".into()),
            c => return Err(code_text(c)),
        }
        v.get("episode").map(Episode::from_json).ok_or_else(|| "Серия не найдена".to_string())
    }

    pub async fn watched_positions(&self, release: i64) -> Result<Vec<(i64, Vec<i64>)>, String> {
        let v = self.get("episode/watched", &[("release_ids", release.to_string())]).await?;
        Ok(arr(&v, "sources")
            .iter()
            .map(|x| (i(x, "source_id"), arr(x, "positions").iter().filter_map(|p| p.as_i64()).collect()))
            .collect())
    }

    pub async fn mark_watched(&self, release: i64, source: i64, position: i64) -> Result<(), String> {
        if !self.is_authorized() {
            return Ok(());
        }
        let v = self.post_empty(&format!("episode/watch/{release}/{source}/{position}"), &[]).await?;
        Api::check(&v)
    }

    pub async fn mark_unwatched(&self, release: i64, source: i64, position: i64) -> Result<(), String> {
        let v = self.post_empty(&format!("episode/unwatch/{release}/{source}/{position}"), &[]).await?;
        Api::check(&v)
    }

    pub async fn history_add(&self, release: i64, source: i64, position: i64) -> Result<(), String> {
        if !self.is_authorized() {
            return Ok(());
        }
        let v = self.get(&format!("history/add/{release}/{source}/{position}"), &[]).await?;
        Api::check(&v)
    }

    // ───────────── списки, оценки ─────────────

    pub async fn set_list_status(&self, release: i64, old: i64, new: i64) -> Result<(), String> {
        if old > 0 && old != new {
            self.get(&format!("profile/list/delete/{old}/{release}"), &[]).await?;
        }
        if new > 0 {
            let v = self.get(&format!("profile/list/add/{new}/{release}"), &[]).await?;
            Api::check(&v)?;
        } else if old == 0 {
            return Ok(());
        }
        Ok(())
    }

    pub async fn set_favorite(&self, release: i64, on: bool) -> Result<(), String> {
        let p = if on { "add" } else { "delete" };
        let v = self.get(&format!("favorite/{p}/{release}"), &[]).await?;
        Api::check(&v)
    }

    pub async fn vote(&self, release: i64, vote: i64) -> Result<(), String> {
        let v = if vote > 0 {
            self.get(&format!("release/vote/add/{release}/{vote}"), &[]).await?
        } else {
            self.get(&format!("release/vote/delete/{release}"), &[]).await?
        };
        Api::check(&v)
    }

    // ───────────── комментарии ─────────────

    fn comments_base(kind: CommentKind) -> &'static str {
        match kind {
            CommentKind::Release => "release/comment",
            CommentKind::Collection => "collection/comment",
        }
    }

    pub async fn comments(&self, kind: CommentKind, target: i64, page: i64, sort: i64) -> Result<Page<Comment>, String> {
        let v = self
            .get(&format!("{}/all/{target}/{page}", Self::comments_base(kind)), &[("sort", sort.to_string())])
            .await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, Comment::from_json))
    }

    pub async fn replies(&self, kind: CommentKind, comment: i64, page: i64) -> Result<Page<Comment>, String> {
        let v = self
            .post_empty(&format!("{}/replies/{comment}/{page}", Self::comments_base(kind)), &[("sort", "1".into())])
            .await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, Comment::from_json))
    }

    pub async fn add_comment(
        &self,
        kind: CommentKind,
        target: i64,
        message: &str,
        parent: Option<i64>,
        reply_to: Option<i64>,
        spoiler: bool,
    ) -> Result<(), String> {
        let v = self
            .post_json(
                &format!("{}/add/{target}", Self::comments_base(kind)),
                json!({"message": message, "parent_comment_id": parent, "reply_to_profile_id": reply_to, "spoiler": spoiler}),
            )
            .await?;
        match i(&v, "code") {
            0 => Ok(()),
            2 => Err("Комментарий слишком короткий или длинный".into()),
            c => Err(code_text(c)),
        }
    }

    pub async fn delete_comment(&self, kind: CommentKind, comment: i64) -> Result<(), String> {
        let v = self.get(&format!("{}/delete/{comment}", Self::comments_base(kind)), &[]).await?;
        Api::check(&v)
    }

    pub async fn vote_comment(&self, kind: CommentKind, comment: i64, vote: i64) -> Result<(), String> {
        let v = self.get(&format!("{}/vote/{comment}/{vote}", Self::comments_base(kind)), &[]).await?;
        Api::check(&v)
    }

    // ───────────── коллекции ─────────────

    pub async fn collections(&self, page: i64, query: &str) -> Result<Page<Collection>, String> {
        let v = if query.trim().is_empty() {
            self.get(&format!("collection/all/{page}"), &[("sort", "1".into())]).await?
        } else {
            self.post_json(&format!("search/collections/{page}"), json!({"query": query, "search_by": 0})).await?
        };
        Api::check(&v)?;
        Ok(Page::from_json(&v, Collection::from_json))
    }

    pub async fn collection(&self, id: i64) -> Result<Collection, String> {
        let v = self.get(&format!("collection/{id}"), &[]).await?;
        Api::check(&v)?;
        v.get("collection").map(Collection::from_json).ok_or_else(|| "Коллекция не найдена".to_string())
    }

    pub async fn set_collection_favorite(&self, id: i64, on: bool) -> Result<(), String> {
        let p = if on { "add" } else { "delete" };
        let v = self.get(&format!("collectionFavorite/{p}/{id}"), &[]).await?;
        Api::check(&v)
    }

    pub async fn profile_collections(&self, profile: i64, page: i64) -> Result<Page<Collection>, String> {
        let v = self.get(&format!("collection/all/profile/{profile}/{page}"), &[]).await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, Collection::from_json))
    }

    // ───────────── профили ─────────────

    pub async fn profile(&self, id: i64) -> Result<Profile, String> {
        let v = self.get(&format!("profile/{id}"), &[]).await?;
        Api::check(&v)?;
        let my = b(&v, "is_my_profile");
        v.get("profile").map(|p| Profile::from_json(p, my)).ok_or_else(|| "Профиль не найден".to_string())
    }

    pub async fn friends(&self, id: i64, page: i64) -> Result<Page<ProfileCompact>, String> {
        let v = self.get(&format!("profile/friend/all/{id}/{page}"), &[]).await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, ProfileCompact::from_json))
    }

    pub async fn search_profiles(&self, q: &str, page: i64) -> Result<Page<ProfileCompact>, String> {
        let v = self.post_json(&format!("search/profiles/{page}"), json!({"query": q, "search_by": 0})).await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, ProfileCompact::from_json))
    }

    pub async fn profile_comments(&self, profile: i64, page: i64) -> Result<Page<Comment>, String> {
        let v = self.get(&format!("release/comment/all/profile/{profile}/{page}"), &[("sort", "1".into())]).await?;
        Api::check(&v)?;
        Ok(Page::from_json(&v, Comment::from_json))
    }

    pub async fn friend_request(&self, id: i64, send: bool) -> Result<(), String> {
        let p = if send { "send" } else { "remove" };
        let v = self.get(&format!("profile/friend/request/{p}/{id}"), &[]).await?;
        Api::check(&v)
    }

    pub async fn edit_status(&self, status: &str) -> Result<(), String> {
        let v = self.post_json("profile/preference/status/edit", json!({"status": status})).await?;
        Api::check(&v)
    }

    pub async fn edit_social(&self, vk: &str, tg: &str, inst: &str, tt: &str, discord: &str) -> Result<(), String> {
        let v = self
            .post_json(
                "profile/preference/social/edit",
                json!({"vk_page": vk, "tg_page": tg, "inst_page": inst, "tt_page": tt, "discord_page": discord}),
            )
            .await?;
        Api::check(&v)
    }

    pub async fn change_password(&self, current: &str, new: &str) -> Result<(), String> {
        let v = self
            .post_form("profile/preference/password/change", &[("current", current.to_string()), ("new", new.to_string())])
            .await?;
        match i(&v, "code") {
            0 => Ok(()),
            2 => Err("Неверный текущий пароль".into()),
            3 => Err("Недопустимый новый пароль".into()),
            c => Err(code_text(c)),
        }
    }

    pub async fn change_login(&self, login: &str) -> Result<(), String> {
        let v = self.post_empty("profile/preference/login/change", &[("login", login.to_string())]).await?;
        match i(&v, "code") {
            0 => Ok(()),
            2 => Err("Логин занят или недопустим".into()),
            c => Err(code_text(c)),
        }
    }

    pub async fn change_email(&self, current_email: &str, password: &str, new_email: &str) -> Result<(), String> {
        let v = self
            .post_form(
                "profile/preference/email/change",
                &[
                    ("current_email", current_email.to_string()),
                    ("current_password", password.to_string()),
                    ("new_email", new_email.to_string()),
                ],
            )
            .await?;
        Api::check(&v)
    }

    pub async fn set_incognito(&self) -> Result<(), String> {
        let v = self.get("profile/preference/privacy/incognito/edit", &[]).await?;
        Api::check(&v)
    }

    pub async fn preferences(&self) -> Result<Value, String> {
        let v = self.get("profile/preference/my", &[]).await?;
        Api::check(&v)?;
        Ok(v)
    }

    pub async fn notification_prefs(&self) -> Result<Value, String> {
        let v = self.get("profile/preference/notification/my", &[]).await?;
        Api::check(&v)?;
        Ok(v)
    }

    /// Серверные переключатели уведомлений (каждый вызов инвертирует настройку).
    pub async fn toggle_notification(&self, which: &str) -> Result<(), String> {
        let path = match which {
            "episode" => "profile/preference/notification/episode/edit",
            "first" => "profile/preference/notification/episode/first/edit",
            "comment" => "profile/preference/notification/comment/edit",
            "related" => "profile/preference/notification/related/release/edit",
            "article" => "profile/preference/notification/article/edit",
            _ => return Err("Неизвестная настройка".into()),
        };
        let v = self.get(path, &[]).await?;
        Api::check(&v)
    }

    pub async fn notification_count(&self) -> Result<i64, String> {
        let v = self.get("notification/count", &[]).await?;
        Ok(i(&v, "count"))
    }

    // ───────────── картинки и произвольные запросы ─────────────

    /// Загрузка картинки каскадом: https-апгрейд → прямой запрос → зеркала CDN (альтернативное
    /// подключение) → прокси-fallback (wsrv.nl / weserv.nl). Первый успешный ответ побеждает.
    pub async fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let upgraded = upgrade_https(url);
        let alt_first = self.base == BASE_ALT;
        let mut last = String::from("нет адресов для загрузки");
        for candidate in image_candidates(&upgraded, alt_first) {
            match self.fetch_bytes(&candidate).await {
                Ok(b) if !b.is_empty() => return Ok(b),
                Ok(_) => last = "пустой ответ".to_string(),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }

    pub async fn text(&self, url: &str, referer: Option<&str>) -> Result<String, String> {
        let mut rb = self.client.get(url);
        if let Some(r) = referer {
            rb = rb.header("Referer", r);
        }
        let resp = rb.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        resp.text().await.map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommentKind {
    Release,
    Collection,
}

pub fn code_text(code: i64) -> String {
    match code {
        0 => "ОК".into(),
        1 => "Неизвестная ошибка сервера".into(),
        4 => "Требуется авторизация".into(),
        5 => "Доступ запрещён".into(),
        6 => "Профиль заблокирован".into(),
        c => format!("Ошибка API (код {c})"),
    }
}

// ───────────── загрузка картинок: https-апгрейд, зеркала, прокси ─────────────

/// Домены CDN, которые взаимозаменяемы (альтернативное подключение).
const IMAGE_MIRRORS: [&str; 2] = ["anixmirai.com", "anixsekai.com"];

/// `http://` и `//host` → `https://`.
pub fn upgrade_https(src: &str) -> String {
    if let Some(rest) = src.strip_prefix("http://") {
        format!("https://{rest}")
    } else if let Some(rest) = src.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        src.to_string()
    }
}

/// Список адресов для загрузки картинки в порядке попыток.
pub fn image_candidates(src: &str, alt_first: bool) -> Vec<String> {
    let mut mirrors: Vec<String> = Vec::new();
    if let Ok(parsed) = url::Url::parse(src) {
        if let Some(host) = parsed.host_str() {
            for d in IMAGE_MIRRORS {
                if host == d || host.ends_with(&format!(".{d}")) {
                    for other in IMAGE_MIRRORS {
                        if other != d {
                            let new_host = format!("{}{}", &host[..host.len() - d.len()], other);
                            let mut u = parsed.clone();
                            if u.set_host(Some(&new_host)).is_ok() {
                                mirrors.push(u.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    let bare = src.trim_start_matches("https://").trim_start_matches("http://");
    let proxies = vec![
        format!("https://wsrv.nl/?url={}&output=jpg&q=85", crate::player::enc(bare)),
        format!("https://images.weserv.nl/?url={}&output=jpg&q=85", crate::player::enc(bare)),
    ];
    let mut out: Vec<String> = Vec::new();
    if alt_first {
        out.extend(mirrors.iter().cloned());
    }
    out.push(src.to_string());
    if !alt_first {
        out.extend(mirrors);
    }
    out.extend(proxies);
    out
}

#[cfg(test)]
mod image_tests {
    use super::*;

    #[test]
    fn upgrades_http() {
        assert_eq!(upgrade_https("http://s.anixmirai.com/a.jpg"), "https://s.anixmirai.com/a.jpg");
        assert_eq!(upgrade_https("//s.anixmirai.com/a.jpg"), "https://s.anixmirai.com/a.jpg");
        assert_eq!(upgrade_https("https://x.y/a.jpg"), "https://x.y/a.jpg");
    }

    #[test]
    fn candidates_order() {
        let direct = image_candidates("https://s.anixmirai.com/posters/a.jpg", false);
        assert_eq!(direct[0], "https://s.anixmirai.com/posters/a.jpg");
        assert!(direct[1].contains("anixsekai.com"));
        assert!(direct.last().unwrap().contains("weserv.nl"));
        let alt = image_candidates("https://s.anixmirai.com/posters/a.jpg", true);
        assert!(alt[0].contains("anixsekai.com"));
        assert_eq!(alt[1], "https://s.anixmirai.com/posters/a.jpg");
    }
}
