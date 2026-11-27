//! Модели данных Anixart. Разбор ведётся вручную из `serde_json::Value`: сервер часто
//! возвращает `null` вместо строк и числа вместо строк (и наоборот), поэтому жёсткие
//! `#[derive(Deserialize)]` ломались бы на любом отклонении.

use serde_json::Value;

pub fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn i(v: &Value, k: &str) -> i64 {
    match v.get(k) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(Value::String(x)) => x.trim().parse().unwrap_or(0),
        Some(Value::Bool(true)) => 1,
        _ => 0,
    }
}

pub fn f(v: &Value, k: &str) -> f64 {
    match v.get(k) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(x)) => x.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

pub fn b(v: &Value, k: &str) -> bool {
    match v.get(k) {
        Some(Value::Bool(x)) => *x,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    }
}

pub fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(|x| x.as_array()).map(|x| x.as_slice()).unwrap_or(&[])
}

pub fn opt_i(v: &Value, k: &str) -> Option<i64> {
    match v.get(k) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(x)) => x.trim().parse().ok(),
        _ => None,
    }
}

/// Статусы списков Anixart.
pub const LIST_STATUSES: [(i64, &str); 5] = [
    (1, "Смотрю"),
    (2, "В планах"),
    (3, "Просмотрено"),
    (4, "Отложено"),
    (5, "Брошено"),
];

pub fn status_name(id: i64) -> &'static str {
    LIST_STATUSES.iter().find(|(i, _)| *i == id).map(|(_, n)| *n).unwrap_or("Не в списке")
}

#[derive(Clone, Debug, Default)]
pub struct VideoBanner {
    pub title: String,
    pub image: String,
    pub url: String,
    pub category: String,
    pub hosting: String,
}

#[derive(Clone, Debug, Default)]
pub struct Release {
    pub id: i64,
    pub title_ru: String,
    pub title_original: String,
    pub title_alt: String,
    pub description: String,
    pub image: String,
    pub year: String,
    pub season: i64,
    pub genres: String,
    pub country: String,
    pub studio: String,
    pub author: String,
    pub director: String,
    pub translators: String,
    pub source: String,
    pub status: String,
    pub category: String,
    pub episodes_total: i64,
    pub episodes_released: i64,
    pub grade: f64,
    pub vote_count: i64,
    pub favorites_count: i64,
    pub comment_count: i64,
    pub is_favorite: bool,
    pub list_status: i64,
    pub duration: i64,
    pub age_rating: i64,
    pub note: String,
    pub restriction_note: String,
    pub screenshots: Vec<String>,
    pub videos: Vec<VideoBanner>,
    pub related_id: i64,
    pub related_name: String,
    pub related_count: i64,
    pub related_releases: Vec<Release>,
    pub recommended: Vec<Release>,
    pub last_episode_name: String,
    pub last_episode_type: String,
    pub my_vote: i64,
    pub is_play_disabled: bool,
    pub is_view_blocked: bool,
    pub counts: [i64; 5],
    pub votes: [i64; 5],
    pub last_update_date: i64,
}

impl Release {
    pub fn from_json(v: &Value) -> Release {
        let mut r = Release {
            id: i(v, "id"),
            title_ru: s(v, "title_ru"),
            title_original: s(v, "title_original"),
            title_alt: s(v, "title_alt"),
            description: s(v, "description"),
            image: s(v, "image"),
            year: s(v, "year"),
            season: i(v, "season"),
            genres: s(v, "genres"),
            country: s(v, "country"),
            studio: s(v, "studio"),
            author: s(v, "author"),
            director: s(v, "director"),
            translators: s(v, "translators"),
            source: s(v, "source"),
            episodes_total: i(v, "episodes_total"),
            episodes_released: i(v, "episodes_released"),
            grade: f(v, "grade"),
            vote_count: i(v, "vote_count"),
            favorites_count: i(v, "favorites_count"),
            comment_count: i(v, "comment_count"),
            is_favorite: b(v, "is_favorite"),
            list_status: i(v, "profile_list_status"),
            duration: i(v, "duration"),
            age_rating: i(v, "age_rating"),
            note: s(v, "note"),
            restriction_note: s(v, "restriction_note"),
            my_vote: opt_i(v, "my_vote").or_else(|| opt_i(v, "your_vote")).unwrap_or(0),
            is_play_disabled: b(v, "is_play_disabled"),
            is_view_blocked: b(v, "is_view_blocked"),
            related_count: i(v, "related_count"),
            last_update_date: i(v, "last_update_date"),
            ..Default::default()
        };
        r.status = v.get("status").map(|x| s(x, "name")).unwrap_or_default();
        r.category = v.get("category").map(|x| s(x, "name")).unwrap_or_default();
        r.screenshots = arr(v, "screenshot_images")
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect();
        r.videos = arr(v, "video_banners")
            .iter()
            .map(|x| VideoBanner {
                title: s(x, "title"),
                image: s(x, "image"),
                url: s(x, "url"),
                category: x.get("category").map(|c| s(c, "name")).unwrap_or_default(),
                hosting: x.get("hosting").map(|c| s(c, "name")).unwrap_or_default(),
            })
            .collect();
        if let Some(rel) = v.get("related").filter(|x| x.is_object()) {
            r.related_id = i(rel, "id");
            r.related_name = s(rel, "name_ru");
            if r.related_count == 0 {
                r.related_count = i(rel, "release_count");
            }
        }
        r.related_releases = arr(v, "related_releases").iter().map(Release::from_json).collect();
        r.recommended = arr(v, "recommended_releases").iter().map(Release::from_json).collect();
        if let Some(e) = v.get("last_view_episode").filter(|x| x.is_object()) {
            r.last_episode_name = s(e, "name");
        }
        if r.last_episode_name.is_empty() {
            r.last_episode_name = s(v, "last_view_episode_name");
        }
        r.last_episode_type = s(v, "last_view_episode_type_name");
        r.counts = [
            i(v, "watching_count"),
            i(v, "plan_count"),
            i(v, "completed_count"),
            i(v, "hold_on_count"),
            i(v, "dropped_count"),
        ];
        r.votes = [
            i(v, "vote_1_count"),
            i(v, "vote_2_count"),
            i(v, "vote_3_count"),
            i(v, "vote_4_count"),
            i(v, "vote_5_count"),
        ];
        r
    }

    pub fn title(&self) -> String {
        if !self.title_ru.is_empty() {
            self.title_ru.clone()
        } else if !self.title_original.is_empty() {
            self.title_original.clone()
        } else {
            format!("#{}", self.id)
        }
    }

    pub fn year_num(&self) -> i64 {
        self.year.trim().parse().unwrap_or(0)
    }

    pub fn episodes_label(&self) -> String {
        let total = if self.episodes_total > 0 { self.episodes_total.to_string() } else { "?".into() };
        if self.episodes_released > 0 {
            format!("{} из {} эп.", self.episodes_released, total)
        } else {
            format!("{} эп.", total)
        }
    }

    /// Все названия, под которыми релиз может встретиться в Shikimori.
    pub fn all_titles(&self) -> Vec<String> {
        let mut out = vec![];
        for t in [&self.title_original, &self.title_ru] {
            if !t.is_empty() {
                out.push(t.clone());
            }
        }
        if !self.title_alt.is_empty() {
            out.push(self.title_alt.clone());
            for part in self.title_alt.split(|c| c == ';' || c == '/' || c == '|') {
                let p = part.trim();
                if !p.is_empty() {
                    out.push(p.to_string());
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct VoiceType {
    pub id: i64,
    pub name: String,
    pub workers: String,
    pub episodes_count: i64,
    pub is_sub: bool,
    pub pinned: bool,
    pub view_count: i64,
}

impl VoiceType {
    pub fn from_json(v: &Value) -> VoiceType {
        VoiceType {
            id: i(v, "id"),
            name: s(v, "name"),
            workers: s(v, "workers"),
            episodes_count: i(v, "episodes_count"),
            is_sub: b(v, "is_sub"),
            pinned: b(v, "pinned"),
            view_count: i(v, "view_count"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Source {
    pub id: i64,
    pub name: String,
    pub episodes_count: i64,
    pub is_download_disabled: bool,
}

impl Source {
    pub fn from_json(v: &Value) -> Source {
        Source {
            id: i(v, "id"),
            name: s(v, "name"),
            episodes_count: i(v, "episodes_count"),
            is_download_disabled: b(v, "is_download_disabled"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Episode {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub release_id: i64,
    pub source_id: i64,
    pub url: String,
    pub iframe: bool,
    pub is_watched: bool,
    pub playback_position: i64,
    pub is_filler: bool,
}

impl Episode {
    pub fn from_json(v: &Value) -> Episode {
        Episode {
            id: i(v, "id"),
            name: s(v, "name"),
            position: i(v, "position"),
            release_id: i(v, "release_id"),
            source_id: i(v, "source_id"),
            url: s(v, "url"),
            iframe: b(v, "iframe"),
            is_watched: b(v, "is_watched"),
            playback_position: i(v, "playback_position"),
            is_filler: b(v, "is_filler"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProfileCompact {
    pub id: i64,
    pub login: String,
    pub avatar: String,
    pub is_verified: bool,
    pub is_sponsor: bool,
}

impl ProfileCompact {
    pub fn from_json(v: &Value) -> ProfileCompact {
        ProfileCompact {
            id: i(v, "id"),
            login: s(v, "login"),
            avatar: s(v, "avatar"),
            is_verified: b(v, "is_verified"),
            is_sponsor: b(v, "is_sponsor"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub id: i64,
    pub login: String,
    pub avatar: String,
    pub status: String,
    pub register_date: i64,
    pub last_activity_time: i64,
    pub is_online: bool,
    pub is_verified: bool,
    pub is_sponsor: bool,
    pub is_my: bool,
    pub is_banned: bool,
    pub friend_status: Option<i64>,
    pub friend_count: i64,
    pub comment_count: i64,
    pub collection_count: i64,
    pub video_count: i64,
    pub favorite_count: i64,
    pub counts: [i64; 5],
    pub watched_episodes: i64,
    pub watched_time: i64,
    pub vk: String,
    pub tg: String,
    pub inst: String,
    pub tt: String,
    pub discord: String,
    pub is_stats_hidden: bool,
    pub is_social_hidden: bool,
    pub history: Vec<Release>,
    pub friends_preview: Vec<ProfileCompact>,
    pub is_blocked: bool,
}

impl Profile {
    pub fn from_json(v: &Value, is_my: bool) -> Profile {
        Profile {
            id: i(v, "id"),
            login: s(v, "login"),
            avatar: s(v, "avatar"),
            status: s(v, "status"),
            register_date: i(v, "register_date"),
            last_activity_time: i(v, "last_activity_time"),
            is_online: b(v, "is_online"),
            is_verified: b(v, "is_verified"),
            is_sponsor: b(v, "is_sponsor"),
            is_my,
            is_banned: b(v, "is_banned"),
            friend_status: opt_i(v, "friend_status"),
            friend_count: i(v, "friend_count"),
            comment_count: i(v, "comment_count"),
            collection_count: i(v, "collection_count"),
            video_count: i(v, "video_count"),
            favorite_count: i(v, "favorite_count"),
            counts: [
                i(v, "watching_count"),
                i(v, "plan_count"),
                i(v, "completed_count"),
                i(v, "hold_on_count"),
                i(v, "dropped_count"),
            ],
            watched_episodes: i(v, "watched_episode_count"),
            watched_time: i(v, "watched_time"),
            vk: s(v, "vk_page"),
            tg: s(v, "tg_page"),
            inst: s(v, "inst_page"),
            tt: s(v, "tt_page"),
            discord: s(v, "discord_page"),
            is_stats_hidden: b(v, "is_stats_hidden"),
            is_social_hidden: b(v, "is_social_hidden"),
            history: arr(v, "history").iter().map(Release::from_json).collect(),
            friends_preview: arr(v, "friends_preview").iter().map(ProfileCompact::from_json).collect(),
            is_blocked: b(v, "is_blocked"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Comment {
    pub id: i64,
    pub message: String,
    pub timestamp: i64,
    pub vote: i64,
    pub vote_count: i64,
    pub reply_count: i64,
    pub is_spoiler: bool,
    pub is_deleted: bool,
    pub is_edited: bool,
    pub parent_id: i64,
    pub author: ProfileCompact,
}

impl Comment {
    pub fn from_json(v: &Value) -> Comment {
        Comment {
            id: i(v, "id"),
            message: s(v, "message"),
            timestamp: i(v, "timestamp"),
            vote: i(v, "vote"),
            vote_count: i(v, "vote_count").max(i(v, "likes_count")),
            reply_count: i(v, "reply_count"),
            is_spoiler: b(v, "is_spoiler"),
            is_deleted: b(v, "is_deleted"),
            is_edited: b(v, "is_edited"),
            parent_id: opt_i(v, "parent_comment_id").unwrap_or(0),
            author: v.get("profile").map(ProfileCompact::from_json).unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Collection {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub image: String,
    pub creator: ProfileCompact,
    pub favorites_count: i64,
    pub comment_count: i64,
    pub release_count: i64,
    pub is_favorite: bool,
    pub preview: Vec<Release>,
}

impl Collection {
    pub fn from_json(v: &Value) -> Collection {
        let preview: Vec<Release> = arr(v, "releases_preview")
            .iter()
            .chain(arr(v, "releases").iter())
            .map(Release::from_json)
            .collect();
        Collection {
            id: i(v, "id"),
            title: s(v, "title"),
            description: s(v, "description"),
            image: s(v, "image"),
            creator: v.get("creator").map(ProfileCompact::from_json).unwrap_or_default(),
            favorites_count: i(v, "favorites_count"),
            comment_count: i(v, "comment_count"),
            release_count: opt_i(v, "release_count").unwrap_or(preview.len() as i64),
            is_favorite: b(v, "is_favorite"),
            preview,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Interesting {
    pub id: i64,
    pub kind: i64,
    pub title: String,
    pub description: String,
    pub image: String,
    pub action: String,
}

impl Interesting {
    pub fn from_json(v: &Value) -> Interesting {
        Interesting {
            id: i(v, "id"),
            kind: i(v, "type"),
            title: s(v, "title"),
            description: s(v, "description"),
            image: s(v, "image"),
            action: s(v, "action"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FeedItem {
    pub id: i64,
    pub channel: String,
    pub author: String,
    pub text: String,
    pub image: String,
    pub date: i64,
    pub comments: i64,
    pub votes: i64,
}

fn collect_text(v: &Value, out: &mut Vec<String>, image: &mut String) {
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                match x {
                    Value::String(t) => {
                        if matches!(k.as_str(), "text" | "title" | "description" | "caption") && !t.trim().is_empty() {
                            out.push(t.trim().to_string());
                        } else if image.is_empty() && matches!(k.as_str(), "image" | "url" | "cover") && t.starts_with("http") {
                            *image = t.clone();
                        }
                    }
                    _ => collect_text(x, out, image),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_text(x, out, image)),
        _ => {}
    }
}

impl FeedItem {
    pub fn from_json(v: &Value) -> FeedItem {
        let mut texts = vec![];
        let mut image = String::new();
        if let Some(p) = v.get("payload") {
            collect_text(p, &mut texts, &mut image);
        }
        FeedItem {
            id: i(v, "id"),
            channel: v.get("channel").map(|c| s(c, "title")).unwrap_or_default(),
            author: v.get("author").map(|c| s(c, "login")).unwrap_or_default(),
            text: texts.join("\n"),
            image,
            date: i(v, "creation_date"),
            comments: i(v, "comment_count"),
            votes: i(v, "vote_count"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: i64,
    pub total_pages: i64,
    pub total_count: i64,
}

impl<T> Page<T> {
    pub fn from_json(v: &Value, f: impl Fn(&Value) -> T) -> Page<T> {
        Page {
            items: arr(v, "content").iter().map(f).collect(),
            page: i(v, "current_page"),
            total_pages: i(v, "total_page_count"),
            total_count: i(v, "total_count"),
        }
    }
    pub fn is_last(&self) -> bool {
        self.items.is_empty() || self.page + 1 >= self.total_pages
    }
}

pub fn format_date(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    // Гражданская дата из unix-времени (алгоритм Говарда Хиннанта), без внешних зависимостей.
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:02}.{:02}.{} {:02}:{:02}", d, m, y, secs / 3600, (secs % 3600) / 60)
}

pub fn format_clock(seconds: f64) -> String {
    let t = seconds.max(0.0) as i64;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
    } else {
        format!("{:02}:{:02}", t / 60, t % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tolerates_nulls() {
        let v: Value = serde_json::json!({"id": 5, "title_ru": null, "year": 2020, "episodes_total": null, "status": {"id":1,"name":"Онгоинг"}});
        let r = Release::from_json(&v);
        assert_eq!(r.id, 5);
        assert_eq!(r.year, "2020");
        assert_eq!(r.status, "Онгоинг");
        assert_eq!(r.title(), "#5");
    }

    #[test]
    fn date_format() {
        assert_eq!(format_date(0), "");
        assert_eq!(format_date(1_700_000_000), "14.11.2023 22:13");
    }
}
