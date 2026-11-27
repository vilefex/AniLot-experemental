//! Состояние приложения и обработка сообщений (Elm-архитектура Iced).

use crate::aniskip::{self, Marker};
use crate::api::{Api, CommentKind, Filter, ListKind, SignIn};
use crate::hls;
use crate::models::*;
use crate::platform;
use crate::player::{self, LaunchOpts, PlayResult, Stream};
use crate::shikimori::{self, SyncHandle};
use crate::storage::{self, DownloadRecord, Settings};
use crate::update::{self, UpdateInfo};
use iced::widget::image;
use iced::{Subscription, Task};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Field {
    Login,
    Password,
    SignUpLogin,
    SignUpEmail,
    SignUpPassword,
    Code,
    RestoreData,
    RestorePassword,
    Search,
    ProfileSearch,
    BookmarkSearch,
    CollectionSearch,
    CommentText,
    ShikiClientId,
    ShikiSecret,
    ShikiCode,
    ShikiNick,
    StatusText,
    Vk,
    Tg,
    Inst,
    Tt,
    Discord,
    CurPass,
    NewPass,
    NewLogin,
    NewEmail,
    MpvPath,
    CustomApi,
    UserAgent,
    YearFrom,
    YearTo,
    EpFrom,
    EpTo,
    AccentHex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Toggle {
    RememberSource,
    RememberPosition,
    AutoNext,
    AniskipEnabled,
    AniskipAuto,
    AltConnection,
    ShikiProgress,
    FilterExclude,
    CommentSpoiler,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Search,
    Bookmarks,
    Downloads,
    Profile,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthMode {
    SignIn,
    SignUp,
    SignUpCode,
    Restore,
    RestoreCode,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Menu,
    Appearance,
    Player,
    AniSkip,
    Notifications,
    Profile,
    Security,
    Data,
    Connection,
    Updates,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListTarget {
    Search,
    Bookmarks,
    Page(u64),
}

#[derive(Clone, Debug)]
pub struct Listing {
    pub kind: ListKind,
    pub items: Vec<Release>,
    pub page: i64,
    pub last: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub gen: u64,
}

impl Listing {
    pub fn new(kind: ListKind) -> Listing {
        Listing { kind, items: vec![], page: 0, last: false, loading: false, error: None, gen: 0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReleasePage {
    pub id: i64,
    pub release: Option<Release>,
    pub error: Option<String>,
    pub expanded: bool,
    pub status_menu: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ProfilePage {
    pub id: i64,
    pub profile: Option<Profile>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct FriendsPage {
    pub id: i64,
    pub items: Vec<ProfileCompact>,
    pub page: i64,
    pub last: bool,
    pub loading: bool,
}

#[derive(Clone, Debug)]
pub struct CommentsPage {
    pub kind: CommentKind,
    pub target: i64,
    pub comments: Vec<Comment>,
    pub page: i64,
    pub last: bool,
    pub loading: bool,
    pub sort: i64,
    pub replies: HashMap<i64, Vec<Comment>>,
    pub revealed: HashSet<i64>,
    pub reply_to: Option<(i64, i64, String)>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CollectionsPage {
    pub items: Vec<Collection>,
    pub page: i64,
    pub last: bool,
    pub loading: bool,
    pub query: String,
}

#[derive(Clone, Debug)]
pub struct CollectionPage {
    pub id: i64,
    pub collection: Option<Collection>,
    pub listing: Listing,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ChooserPage {
    pub release: Release,
    pub types: Vec<VoiceType>,
    pub type_sel: Option<VoiceType>,
    pub sources: Vec<Source>,
    pub source_sel: Option<Source>,
    pub episodes: Vec<Episode>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlayerPage {
    pub release: Release,
    pub type_id: i64,
    pub source_id: i64,
    pub episodes: Vec<Episode>,
    pub index: usize,
    pub stream: Option<Stream>,
    pub embed_url: String,
    pub loading: bool,
    pub error: Option<String>,
    pub variant: Option<usize>,
    pub speed: f32,
    pub subtitle: Option<usize>,
    pub subtitle_path: Option<PathBuf>,
    pub markers: Vec<Marker>,
    pub playing: bool,
    pub status: String,
    pub auto_play: bool,
    pub download_url: String,
}

#[derive(Clone, Debug)]
pub struct SchedulePage {
    pub days: Vec<(String, Vec<Release>)>,
    pub day: usize,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum PageKind {
    Release(ReleasePage),
    List { title: String, listing: Listing },
    Profile(ProfilePage),
    Friends(FriendsPage),
    Comments(CommentsPage),
    Collections(CollectionsPage),
    Collection(CollectionPage),
    Chooser(ChooserPage),
    Player(Box<PlayerPage>),
    Gallery { urls: Vec<String>, index: usize },
    Videos(Vec<VideoBanner>),
    Schedule(SchedulePage),
    Settings(Section),
    Shikimori,
    Notifications(Option<serde_json::Value>),
}

#[derive(Clone, Debug)]
pub struct StackPage {
    pub uid: u64,
    pub kind: PageKind,
}

#[derive(Default)]
pub struct Home {
    pub interesting: Vec<Interesting>,
    pub sections: Vec<(String, Vec<Release>)>,
    pub feed: Vec<FeedItem>,
    pub feed_page: i64,
    pub feed_last: bool,
    pub feed_mode: bool,
    pub loading: bool,
    pub error: Option<String>,
}

pub struct Search {
    pub filter: Filter,
    pub listing: Option<Listing>,
    pub show_filter: bool,
    pub profiles_mode: bool,
    pub profiles: Vec<ProfileCompact>,
    pub profiles_loading: bool,
}

#[derive(Clone, Debug)]
pub struct Bookmarks {
    pub status: i64,
    pub listing: Listing,
}

#[derive(Clone)]
pub struct ActiveDownload {
    pub title: String,
    pub progress: Arc<hls::Progress>,
}

#[derive(Clone, Debug)]
pub enum Message {
    Noop,
    Tick,
    Back,
    Insets(iced_android::Insets),
    Input(Field, String),
    Toggle(Toggle, bool),
    SetTab(Tab),
    Image(String, Result<Vec<u8>, String>),
    Toast(String),
    OpenUrl(String),
    CopyText(String),
    // авторизация
    AuthMode(AuthMode),
    SignIn,
    SignedIn(Result<SignIn, String>),
    SignUp,
    SignedUp(Result<String, String>),
    Verify,
    Restore,
    RestoreSent(Result<String, String>),
    RestoreVerify,
    RestoreDone(Result<(), String>),
    Guest,
    Logout,
    // главная
    HomeInteresting(Result<Vec<Interesting>, String>),
    HomeSection(usize, Result<Vec<Release>, String>),
    HomeFeedMode(bool),
    FeedLoaded(Result<Page_<FeedItem>, String>),
    FeedMore,
    ReloadHome,
    // списки релизов
    ListLoaded(ListTarget, u64, Result<Page_<Release>, String>),
    ListMore(ListTarget),
    // навигация
    OpenRelease(i64),
    OpenList(String, ListKind),
    OpenProfile(i64),
    OpenFriends(i64),
    OpenComments(CommentKind, i64),
    OpenCollections,
    OpenCollection(i64),
    OpenWatch(Box<Release>),
    OpenGallery(Vec<String>, usize),
    OpenVideos(Vec<VideoBanner>),
    OpenSchedule,
    OpenSettings(Section),
    OpenShikimori,
    OpenNotifications,
    GalleryMove(i64),
    // поиск
    SubmitSearch,
    ToggleFilter,
    ApplyFilter,
    ResetFilter,
    FilterSort(i64),
    FilterCategory(Option<i64>),
    FilterStatus(Option<i64>),
    FilterSeason(Option<i64>),
    FilterGenre(String),
    SearchProfilesMode(bool),
    ProfilesFound(Result<Vec<ProfileCompact>, String>),
    HistoryQuery(String),
    ClearSearchHistory,
    // закладки
    BookmarkStatus(i64),
    SubmitBookmarkSearch,
    // релиз
    ReleaseLoaded(u64, Result<Release, String>),
    ToggleDescription(u64),
    ToggleStatusMenu(u64),
    SetListStatus(u64, i64),
    ListStatusSet(u64, i64, Result<(), String>),
    ToggleFavorite(u64),
    FavoriteSet(u64, bool, Result<(), String>),
    Vote(u64, i64),
    VoteSet(u64, i64, Result<(), String>),
    // профиль
    ProfileLoaded(u64, Result<Profile, String>),
    FriendsLoaded(u64, Result<Page_<ProfileCompact>, String>),
    FriendsMore(u64),
    FriendAction(u64, bool),
    FriendActionDone(Result<(), String>),
    // комментарии
    CommentsLoaded(u64, Result<Page_<Comment>, String>),
    CommentsMore(u64),
    CommentsSort(u64, i64),
    LoadReplies(u64, i64),
    RepliesLoaded(u64, i64, Result<Page_<Comment>, String>),
    RevealSpoiler(u64, i64),
    ReplyTo(u64, i64, i64, String),
    CancelReply(u64),
    SendComment(u64),
    CommentSent(u64, Result<(), String>),
    VoteComment(u64, i64, i64),
    DeleteComment(u64, i64),
    CommentChanged(u64, Result<(), String>),
    // коллекции
    CollectionsLoaded(u64, Result<Page_<Collection>, String>),
    CollectionsMore(u64),
    SubmitCollectionSearch(u64),
    CollectionLoaded(u64, Result<Collection, String>),
    ToggleCollectionFavorite(u64),
    CollectionFavoriteSet(u64, bool, Result<(), String>),
    // расписание
    ScheduleLoaded(u64, Result<Vec<(String, Vec<Release>)>, String>),
    ScheduleDay(u64, usize),
    // выбор серии
    ChooserTypes(u64, Result<Vec<VoiceType>, String>),
    ChooseType(u64, VoiceType),
    ChooserSources(u64, Result<Vec<Source>, String>),
    ChooseSource(u64, Source),
    ChooserEpisodes(u64, Result<Vec<Episode>, String>),
    ChooserBack(u64),
    OpenEpisode(u64, usize),
    // плеер
    PlayerReady(u64, Result<(Episode, Result<Stream, String>), String>),
    PlayerVariant(u64, Option<usize>),
    PlayerSpeed(u64, f32),
    PlayerSubtitle(u64, Option<usize>),
    PlayerSubtitleLoaded(u64, Option<usize>, Result<PathBuf, String>),
    PlayerMarkers(u64, Result<Vec<Marker>, String>),
    PlayerPlay(u64),
    PlayerFinished(u64, PlayResult),
    PlayerNext(u64),
    PlayerPrev(u64),
    PlayerMarkWatched(u64),
    PlayerOpenEmbed(u64),
    Watched(Result<(), String>),
    // загрузки
    DownloadStart(u64),
    DownloadDone(String, Result<u64, String>),
    DownloadCancel(String),
    DownloadDelete(String),
    PlayLocal(String),
    LocalPlayed(PlayResult),
    SaveImage(String),
    ImageSaved(Result<PathBuf, String>),
    // настройки
    SetTheme(String),
    SetAccent(String),
    ApplyAccentHex,
    SetPlayer(String),
    SaveStatus,
    SaveSocial,
    ChangePassword,
    ChangeLogin,
    ChangeEmail,
    ToggleIncognito,
    ServerNotify(&'static str),
    NotificationsLoaded(Result<serde_json::Value, String>),
    ProfileOpDone(String, Result<(), String>),
    ApplyConnection,
    ClearCaches,
    ClearDownloads,
    CheckUpdate,
    UpdateChecked(Result<UpdateInfo, String>),
    // Shikimori
    ShikiOpenAuth,
    ShikiConfirmCode,
    ShikiAuthDone(Result<String, String>),
    ShikiLogout,
    ShikiImport,
    ShikiExport,
    ShikiStop,
}

pub type Page_<T> = crate::models::Page<T>;

pub struct App {
    pub api: Api,
    pub settings: Settings,
    pub insets: iced_android::Insets,
    pub inputs: HashMap<Field, String>,
    pub toggles: HashMap<Toggle, bool>,
    pub logged_in: bool,
    pub guest: bool,
    pub auth_mode: AuthMode,
    pub auth_hash: String,
    pub auth_busy: bool,
    pub auth_error: Option<String>,
    pub me: ProfilePage,
    pub tab: Tab,
    pub stack: Vec<StackPage>,
    pub images: HashMap<String, image::Handle>,
    pub loading_images: HashSet<String>,
    pub toast: Option<(String, Instant)>,
    pub home: Home,
    pub search: Search,
    pub bookmarks: Bookmarks,
    pub active_downloads: HashMap<String, ActiveDownload>,
    pub sync: SyncHandle,
    pub update: Option<UpdateInfo>,
    pub next_uid: u64,
    pub next_gen: u64,
    pub busy: bool,
}

pub const HOME_SECTIONS: [&str; 5] = ["Сейчас смотрят", "Последние обновления", "Популярное", "Рекомендации", "Обсуждаемое"];

fn popular_filter() -> Filter {
    Filter { sort: 3, ..Default::default() }
}

impl App {
    pub fn new() -> (App, Task<Message>) {
        let settings = storage::settings();
        let (token, pid) = storage::read(|s| (s.token.clone(), s.profile_id));
        let api = Api::from_settings();
        let mut inputs = HashMap::new();
        inputs.insert(Field::ShikiClientId, settings.shiki_client_id.clone());
        inputs.insert(Field::ShikiSecret, settings.shiki_client_secret.clone());
        inputs.insert(Field::ShikiNick, storage::read(|s| {
            if s.last_import_nick.is_empty() { s.shiki.nickname.clone() } else { s.last_import_nick.clone() }
        }));
        inputs.insert(Field::MpvPath, settings.mpv_path.clone());
        inputs.insert(Field::CustomApi, settings.custom_api.clone());
        inputs.insert(Field::UserAgent, settings.user_agent.clone());
        inputs.insert(Field::AccentHex, settings.accent.clone());
        let mut toggles = HashMap::new();
        toggles.insert(Toggle::CommentSpoiler, false);
        let mut app = App {
            api,
            settings,
            insets: Default::default(),
            inputs,
            toggles,
            logged_in: !token.is_empty(),
            guest: false,
            auth_mode: AuthMode::SignIn,
            auth_hash: String::new(),
            auth_busy: false,
            auth_error: None,
            me: ProfilePage { id: pid, ..Default::default() },
            tab: Tab::Home,
            stack: vec![],
            images: HashMap::new(),
            loading_images: HashSet::new(),
            toast: None,
            home: Home { sections: HOME_SECTIONS.iter().map(|t| (t.to_string(), vec![])).collect(), ..Default::default() },
            search: Search { filter: Filter::default(), listing: None, show_filter: false, profiles_mode: false, profiles: vec![], profiles_loading: false },
            bookmarks: Bookmarks { status: 1, listing: Listing::new(ListKind::ProfileList { profile: 0, status: 1 }) },
            active_downloads: HashMap::new(),
            sync: SyncHandle::default(),
            update: None,
            next_uid: 1,
            next_gen: 1,
            busy: false,
        };
        let mut tasks = vec![];
        if app.logged_in || app.guest {
            tasks.push(app.load_home());
            if app.logged_in && pid > 0 {
                tasks.push(app.load_profile(0, pid));
            }
        }
        // Автопроверка обновлений не чаще раза в 12 часов.
        if storage::now() - app.settings.last_update_check > 12 * 3600 {
            tasks.push(Task::perform(update::check(), |r| Message::UpdateChecked(r.map(|mut i| {
                i.notes.clear();
                i
            }))));
        }
        (app, Task::batch(tasks))
    }

    pub fn inp(&self, f: Field) -> &str {
        self.inputs.get(&f).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn toggle(&self, t: Toggle) -> bool {
        self.toggles.get(&t).copied().unwrap_or(false)
    }

    pub fn is_ready(&self) -> bool {
        self.logged_in || self.guest
    }

    pub fn needs_tick(&self) -> bool {
        self.toast.is_some() || !self.active_downloads.is_empty() || self.sync.snapshot().running
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            iced_android::insets().map(Message::Insets),
            iced::keyboard::listen().filter_map(|e| match e {
                iced::keyboard::Event::KeyPressed { key, .. } => match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                    | iced::keyboard::Key::Named(iced::keyboard::key::Named::BrowserBack) => Some(Message::Back),
                    _ => None,
                },
                _ => None,
            }),
        ];
        if self.needs_tick() {
            subs.push(iced::time::every(Duration::from_millis(400)).map(|_| Message::Tick));
        }
        Subscription::batch(subs)
    }

    // ───────────── служебное ─────────────

    fn uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    fn gen(&mut self) -> u64 {
        self.next_gen += 1;
        self.next_gen
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn page_mut(&mut self, uid: u64) -> Option<&mut PageKind> {
        self.stack.iter_mut().find(|p| p.uid == uid).map(|p| &mut p.kind)
    }

    fn push(&mut self, kind: PageKind) -> u64 {
        let uid = self.uid();
        self.stack.push(StackPage { uid, kind });
        uid
    }

    pub fn image(&self, url: &str) -> Option<&image::Handle> {
        self.images.get(url)
    }

    fn ensure_images<I: IntoIterator<Item = String>>(&mut self, urls: I) -> Task<Message> {
        let mut tasks = vec![];
        for url in urls {
            if url.is_empty() || !url.starts_with("http") || self.images.contains_key(&url) || self.loading_images.contains(&url) {
                continue;
            }
            self.loading_images.insert(url.clone());
            let api = self.api.clone();
            let u = url.clone();
            tasks.push(Task::perform(async move { api.bytes(&u).await }, move |r| Message::Image(url.clone(), r)));
        }
        Task::batch(tasks)
    }

    fn load_home(&mut self) -> Task<Message> {
        self.home.loading = true;
        let mut tasks = vec![];
        let api = self.api.clone();
        tasks.push(Task::perform(async move { api.interesting().await }, Message::HomeInteresting));
        let kinds: Vec<Option<ListKind>> = vec![
            Some(ListKind::Watching),
            Some(ListKind::Filter(Filter::default())),
            Some(ListKind::Filter(popular_filter())),
            Some(ListKind::Recommendations),
            None,
        ];
        for (idx, kind) in kinds.into_iter().enumerate() {
            let api = self.api.clone();
            tasks.push(Task::perform(
                async move {
                    match kind {
                        Some(k) => api.releases_page(&k, 0).await.map(|p| p.items),
                        None => api.discussing().await,
                    }
                },
                move |r| Message::HomeSection(idx, r),
            ));
        }
        Task::batch(tasks)
    }

    fn load_feed(&mut self, page: i64) -> Task<Message> {
        let api = self.api.clone();
        Task::perform(async move { api.feed(page).await }, Message::FeedLoaded)
    }

    fn load_profile(&mut self, uid: u64, id: i64) -> Task<Message> {
        let api = self.api.clone();
        Task::perform(async move { api.profile(id).await }, move |r| Message::ProfileLoaded(uid, r))
    }

    fn listing_mut(&mut self, t: ListTarget) -> Option<&mut Listing> {
        match t {
            ListTarget::Search => self.search.listing.as_mut(),
            ListTarget::Bookmarks => Some(&mut self.bookmarks.listing),
            ListTarget::Page(uid) => match self.page_mut(uid)? {
                PageKind::List { listing, .. } => Some(listing),
                PageKind::Collection(c) => Some(&mut c.listing),
                _ => None,
            },
        }
    }

    fn fetch_listing(&mut self, t: ListTarget) -> Task<Message> {
        let gen = self.gen();
        let Some(l) = self.listing_mut(t) else { return Task::none() };
        if l.loading {
            return Task::none();
        }
        l.loading = true;
        l.error = None;
        l.gen = gen;
        let (kind, page) = (l.kind.clone(), l.page);
        let api = self.api.clone();
        Task::perform(async move { api.releases_page(&kind, page).await }, move |r| Message::ListLoaded(t, gen, r))
    }

    fn start_listing(&mut self, t: ListTarget, kind: ListKind) -> Task<Message> {
        let new = Listing::new(kind);
        match t {
            ListTarget::Search => self.search.listing = Some(new),
            ListTarget::Bookmarks => self.bookmarks.listing = new,
            ListTarget::Page(_) => {
                if let Some(l) = self.listing_mut(t) {
                    *l = new;
                }
            }
        }
        self.fetch_listing(t)
    }

    pub fn current_filter(&self) -> Filter {
        let mut f = self.search.filter.clone();
        f.start_year = self.inp(Field::YearFrom).trim().parse().ok();
        f.end_year = self.inp(Field::YearTo).trim().parse().ok();
        f.episodes_from = self.inp(Field::EpFrom).trim().parse().ok();
        f.episodes_to = self.inp(Field::EpTo).trim().parse().ok();
        f
    }

    fn reset_api(&mut self) {
        self.api = Api::from_settings();
    }

    fn save_settings(&mut self) {
        let s = self.settings.clone();
        storage::write(|st| st.settings = s);
    }

    fn open_release_page(&mut self, id: i64) -> Task<Message> {
        let uid = self.push(PageKind::Release(ReleasePage { id, ..Default::default() }));
        let api = self.api.clone();
        Task::perform(async move { api.release(id).await }, move |r| Message::ReleaseLoaded(uid, r))
    }

    fn load_comments(&mut self, uid: u64) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Comments(c)) = self.page_mut(uid) else { return Task::none() };
        if c.loading {
            return Task::none();
        }
        c.loading = true;
        let (kind, target, page, sort) = (c.kind, c.target, c.page, c.sort);
        Task::perform(async move { api.comments(kind, target, page, sort).await }, move |r| Message::CommentsLoaded(uid, r))
    }

    fn load_collections(&mut self, uid: u64) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Collections(c)) = self.page_mut(uid) else { return Task::none() };
        if c.loading {
            return Task::none();
        }
        c.loading = true;
        let (page, q) = (c.page, c.query.clone());
        Task::perform(async move { api.collections(page, &q).await }, move |r| Message::CollectionsLoaded(uid, r))
    }

    fn load_friends(&mut self, uid: u64) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Friends(f)) = self.page_mut(uid) else { return Task::none() };
        if f.loading {
            return Task::none();
        }
        f.loading = true;
        let (id, page) = (f.id, f.page);
        Task::perform(async move { api.friends(id, page).await }, move |r| Message::FriendsLoaded(uid, r))
    }

    /// Загружает типы озвучек для страницы выбора серии.
    fn chooser_load_types(&mut self, uid: u64) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Chooser(c)) = self.page_mut(uid) else { return Task::none() };
        c.loading = true;
        c.error = None;
        let id = c.release.id;
        Task::perform(async move { api.voice_types(id).await }, move |r| Message::ChooserTypes(uid, r))
    }

    fn chooser_load_sources(&mut self, uid: u64, ty: &VoiceType) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Chooser(c)) = self.page_mut(uid) else { return Task::none() };
        c.type_sel = Some(ty.clone());
        c.source_sel = None;
        c.sources.clear();
        c.episodes.clear();
        c.loading = true;
        c.error = None;
        let (rid, tid) = (c.release.id, ty.id);
        Task::perform(async move { api.sources(rid, tid).await }, move |r| Message::ChooserSources(uid, r))
    }

    fn chooser_load_episodes(&mut self, uid: u64, src: &Source) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Chooser(c)) = self.page_mut(uid) else { return Task::none() };
        c.source_sel = Some(src.clone());
        c.episodes.clear();
        c.loading = true;
        c.error = None;
        let Some(ty) = c.type_sel.clone() else { return Task::none() };
        let (rid, sid) = (c.release.id, src.id);
        let (tid, remember) = (ty.id, self.settings.remember_source);
        if remember {
            storage::write(|s| {
                s.remembered_source.insert(rid.to_string(), (tid, sid));
            });
        }
        Task::perform(async move { api.episodes(rid, tid, sid).await }, move |r| Message::ChooserEpisodes(uid, r))
    }

    /// Запрашивает серию и определяет поток.
    fn player_load(&mut self, uid: u64) -> Task<Message> {
        let api = self.api.clone();
        let Some(PageKind::Player(p)) = self.page_mut(uid) else { return Task::none() };
        let Some(ep) = p.episodes.get(p.index).cloned() else { return Task::none() };
        p.loading = true;
        p.error = None;
        p.stream = None;
        p.variant = None;
        p.subtitle = None;
        p.subtitle_path = None;
        p.markers.clear();
        p.status.clear();
        let (rid, sid) = (p.release.id, p.source_id);
        let release = p.release.clone();
        let aniskip_on = self.settings.aniskip_enabled;
        let api2 = api.clone();
        let ready = Task::perform(
            async move {
                let ep = api.episode_target(rid, sid, ep.position, false).await?;
                let stream = player::resolve_stream(&ep.url, ep.iframe).await;
                let _ = api.history_add(rid, sid, ep.position).await;
                Ok((ep, stream))
            },
            move |r| Message::PlayerReady(uid, r),
        );
        let name = self.player_episode_name(uid);
        let markers = if aniskip_on {
            Task::perform(async move { Ok(aniskip::load_for_episode(&release, &name).await) }, move |r| Message::PlayerMarkers(uid, r))
        } else {
            Task::none()
        };
        let _ = api2;
        Task::batch(vec![ready, markers])
    }

    fn player_episode_name(&mut self, uid: u64) -> String {
        match self.page_mut(uid) {
            Some(PageKind::Player(p)) => p.episodes.get(p.index).map(|e| e.name.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn mark_local_watched(&mut self, release: i64, position: i64) {
        for page in self.stack.iter_mut() {
            match &mut page.kind {
                PageKind::Chooser(c) if c.release.id == release => {
                    for e in c.episodes.iter_mut().filter(|e| e.position == position) {
                        e.is_watched = true;
                    }
                }
                PageKind::Player(p) if p.release.id == release => {
                    for e in p.episodes.iter_mut().filter(|e| e.position == position) {
                        e.is_watched = true;
                    }
                }
                _ => {}
            }
        }
    }

    fn sync_active(&self) -> bool {
        self.sync.snapshot().running
    }

    fn launch_opts(&self, p: &PlayerPage) -> Option<LaunchOpts> {
        let stream = p.stream.as_ref()?;
        let url = match p.variant.and_then(|i| stream.variants.get(i)) {
            Some(v) => v.url.clone(),
            None => stream.url.clone(),
        };
        let ep = p.episodes.get(p.index)?;
        let start = if self.settings.remember_position {
            player::remembered_position(p.release.id, p.source_id, ep.position)
        } else {
            0.0
        };
        Some(LaunchOpts {
            player: self.settings.player.clone(),
            url,
            title: format!("{} — {}", p.release.title(), ep.name),
            start,
            speed: p.speed,
            subtitle: p.subtitle_path.clone(),
            markers: p.markers.clone(),
            auto_skip: self.settings.aniskip_auto && self.settings.aniskip_enabled,
            mpv_path: self.settings.mpv_path.clone(),
        })
    }

    // ───────────── обновление ─────────────

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
            Message::Tick => {
                if let Some((_, at)) = &self.toast {
                    if at.elapsed() > Duration::from_secs(4) {
                        self.toast = None;
                    }
                }
                Task::none()
            }
            Message::Insets(i) => {
                self.insets = i;
                Task::none()
            }
            Message::Back => {
                if self.stack.pop().is_none() {
                    if self.tab != Tab::Home {
                        self.tab = Tab::Home;
                    } else {
                        iced_android::move_to_background();
                    }
                }
                Task::none()
            }
            Message::Input(f, v) => {
                match f {
                    Field::ShikiClientId => {
                        self.settings.shiki_client_id = v.trim().to_string();
                        self.save_settings();
                    }
                    Field::ShikiSecret => {
                        self.settings.shiki_client_secret = v.trim().to_string();
                        self.save_settings();
                    }
                    Field::MpvPath => {
                        self.settings.mpv_path = v.trim().to_string();
                        self.save_settings();
                    }
                    _ => {}
                }
                self.inputs.insert(f, v);
                Task::none()
            }
            Message::Toggle(t, v) => {
                self.toggles.insert(t, v);
                match t {
                    Toggle::RememberSource => self.settings.remember_source = v,
                    Toggle::RememberPosition => self.settings.remember_position = v,
                    Toggle::AutoNext => self.settings.auto_next = v,
                    Toggle::AniskipEnabled => self.settings.aniskip_enabled = v,
                    Toggle::AniskipAuto => self.settings.aniskip_auto = v,
                    Toggle::AltConnection => {
                        self.settings.alt_connection = v;
                        self.save_settings();
                        self.reset_api();
                        return Task::none();
                    }
                    Toggle::ShikiProgress => self.settings.shiki_sync_progress = v,
                    Toggle::FilterExclude => {
                        self.search.filter.exclude_genres = v;
                        return Task::none();
                    }
                    Toggle::CommentSpoiler => return Task::none(),
                }
                self.save_settings();
                Task::none()
            }
            Message::SetTab(t) => {
                self.tab = t;
                self.stack.clear();
                match t {
                    Tab::Bookmarks if self.bookmarks.listing.items.is_empty() && !self.bookmarks.listing.loading && self.logged_in => {
                        let kind = self.bookmarks.listing.kind.clone();
                        self.start_listing(ListTarget::Bookmarks, kind)
                    }
                    Tab::Profile if self.logged_in && self.me.profile.is_none() && self.me.id > 0 => {
                        let id = self.me.id;
                        self.load_profile(0, id)
                    }
                    _ => Task::none(),
                }
            }
            Message::Image(url, r) => {
                self.loading_images.remove(&url);
                if let Ok(bytes) = r {
                    self.images.insert(url, image::Handle::from_bytes(bytes));
                }
                Task::none()
            }
            Message::Toast(t) => {
                self.toast(t);
                Task::none()
            }
            Message::OpenUrl(u) => {
                if let Err(e) = platform::open_url(&u) {
                    self.toast(format!("Не удалось открыть ссылку: {e}"));
                }
                Task::none()
            }
            Message::CopyText(t) => {
                self.toast("Скопировано");
                iced::clipboard::write(t)
            }
            m @ (Message::AuthMode(_)
            | Message::SignIn
            | Message::SignedIn(_)
            | Message::SignUp
            | Message::SignedUp(_)
            | Message::Verify
            | Message::Restore
            | Message::RestoreSent(_)
            | Message::RestoreVerify
            | Message::RestoreDone(_)
            | Message::Guest
            | Message::Logout) => self.update_auth(m),
            m @ (Message::ShikiOpenAuth
            | Message::ShikiConfirmCode
            | Message::ShikiAuthDone(_)
            | Message::ShikiLogout
            | Message::ShikiImport
            | Message::ShikiExport
            | Message::ShikiStop) => self.update_shiki(m),
            m @ (Message::ChooserTypes(..)
            | Message::ChooseType(..)
            | Message::ChooserSources(..)
            | Message::ChooseSource(..)
            | Message::ChooserEpisodes(..)
            | Message::ChooserBack(_)
            | Message::OpenEpisode(..)
            | Message::PlayerReady(..)
            | Message::PlayerVariant(..)
            | Message::PlayerSpeed(..)
            | Message::PlayerSubtitle(..)
            | Message::PlayerSubtitleLoaded(..)
            | Message::PlayerMarkers(..)
            | Message::PlayerPlay(_)
            | Message::PlayerFinished(..)
            | Message::PlayerNext(_)
            | Message::PlayerPrev(_)
            | Message::PlayerMarkWatched(_)
            | Message::PlayerOpenEmbed(_)
            | Message::Watched(_)
            | Message::DownloadStart(_)
            | Message::DownloadDone(..)
            | Message::DownloadCancel(_)
            | Message::DownloadDelete(_)
            | Message::PlayLocal(_)
            | Message::LocalPlayed(_)
            | Message::SaveImage(_)
            | Message::ImageSaved(_)) => self.update_watch(m),
            m @ (Message::SetTheme(_)
            | Message::SetAccent(_)
            | Message::ApplyAccentHex
            | Message::SetPlayer(_)
            | Message::SaveStatus
            | Message::SaveSocial
            | Message::ChangePassword
            | Message::ChangeLogin
            | Message::ChangeEmail
            | Message::ToggleIncognito
            | Message::ServerNotify(_)
            | Message::NotificationsLoaded(_)
            | Message::ProfileOpDone(..)
            | Message::ApplyConnection
            | Message::ClearCaches
            | Message::ClearDownloads
            | Message::CheckUpdate
            | Message::UpdateChecked(_)
            | Message::OpenSettings(_)
            | Message::OpenNotifications
            | Message::OpenShikimori) => self.update_settings(m),
            m => self.update_content(m),
        }
    }

    fn update_auth(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::AuthMode(mode) => {
                self.auth_mode = mode;
                self.auth_error = None;
                Task::none()
            }
            Message::SignIn => {
                let (login, pass) = (self.inp(Field::Login).trim().to_string(), self.inp(Field::Password).to_string());
                if login.is_empty() || pass.is_empty() {
                    self.auth_error = Some("Введите логин и пароль".into());
                    return Task::none();
                }
                self.auth_busy = true;
                self.auth_error = None;
                let api = self.api.clone();
                Task::perform(async move { api.sign_in(&login, &pass).await }, Message::SignedIn)
            }
            Message::SignedIn(r) => {
                self.auth_busy = false;
                match r {
                    Ok(s) => self.finish_login(s),
                    Err(e) => {
                        self.auth_error = Some(e);
                        Task::none()
                    }
                }
            }
            Message::SignUp => {
                let (l, e, p) = (
                    self.inp(Field::SignUpLogin).trim().to_string(),
                    self.inp(Field::SignUpEmail).trim().to_string(),
                    self.inp(Field::SignUpPassword).to_string(),
                );
                if l.len() < 3 || !e.contains('@') || p.len() < 6 {
                    self.auth_error = Some("Логин — от 3 символов, нужна почта, пароль — от 6 символов".into());
                    return Task::none();
                }
                self.auth_busy = true;
                self.auth_error = None;
                let api = self.api.clone();
                Task::perform(async move { api.sign_up(&l, &e, &p).await }, Message::SignedUp)
            }
            Message::SignedUp(r) => {
                self.auth_busy = false;
                match r {
                    Ok(hash) => {
                        self.auth_hash = hash;
                        self.auth_mode = AuthMode::SignUpCode;
                    }
                    Err(e) => self.auth_error = Some(e),
                }
                Task::none()
            }
            Message::Verify => {
                let (l, e, p, c) = (
                    self.inp(Field::SignUpLogin).trim().to_string(),
                    self.inp(Field::SignUpEmail).trim().to_string(),
                    self.inp(Field::SignUpPassword).to_string(),
                    self.inp(Field::Code).trim().to_string(),
                );
                self.auth_busy = true;
                let (api, hash) = (self.api.clone(), self.auth_hash.clone());
                Task::perform(async move { api.verify(&l, &e, &p, &hash, &c).await }, Message::SignedIn)
            }
            Message::Restore => {
                let data = self.inp(Field::RestoreData).trim().to_string();
                if data.is_empty() {
                    self.auth_error = Some("Введите логин или почту".into());
                    return Task::none();
                }
                self.auth_busy = true;
                self.auth_error = None;
                let api = self.api.clone();
                Task::perform(async move { api.restore(&data).await }, Message::RestoreSent)
            }
            Message::RestoreSent(r) => {
                self.auth_busy = false;
                match r {
                    Ok(hash) => {
                        self.auth_hash = hash;
                        self.auth_mode = AuthMode::RestoreCode;
                    }
                    Err(e) => self.auth_error = Some(e),
                }
                Task::none()
            }
            Message::RestoreVerify => {
                let (d, p, c) = (
                    self.inp(Field::RestoreData).trim().to_string(),
                    self.inp(Field::RestorePassword).to_string(),
                    self.inp(Field::Code).trim().to_string(),
                );
                if p.len() < 6 {
                    self.auth_error = Some("Пароль — от 6 символов".into());
                    return Task::none();
                }
                self.auth_busy = true;
                let (api, hash) = (self.api.clone(), self.auth_hash.clone());
                Task::perform(async move { api.restore_verify(&d, &p, &hash, &c).await }, Message::RestoreDone)
            }
            Message::RestoreDone(r) => {
                self.auth_busy = false;
                match r {
                    Ok(()) => {
                        self.auth_mode = AuthMode::SignIn;
                        self.auth_error = None;
                        self.toast("Пароль изменён — войдите с новым паролем");
                    }
                    Err(e) => self.auth_error = Some(e),
                }
                Task::none()
            }
            Message::Guest => {
                self.guest = true;
                self.load_home()
            }
            Message::Logout => {
                storage::write(|s| {
                    s.token.clear();
                    s.profile_id = 0;
                    s.login.clear();
                });
                self.logged_in = false;
                self.guest = false;
                self.me = ProfilePage::default();
                self.stack.clear();
                self.tab = Tab::Home;
                self.bookmarks.listing = Listing::new(ListKind::ProfileList { profile: 0, status: 1 });
                self.reset_api();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn finish_login(&mut self, s: SignIn) -> Task<Message> {
        let (token, pid, login) = (s.token.clone(), s.profile.id, s.profile.login.clone());
        storage::write(|st| {
            st.token = token;
            st.profile_id = pid;
            st.login = login;
        });
        self.logged_in = true;
        self.me = ProfilePage { id: pid, profile: Some(s.profile), error: None };
        self.reset_api();
        self.inputs.remove(&Field::Password);
        let img = self.me.profile.as_ref().map(|p| p.avatar.clone()).unwrap_or_default();
        Task::batch(vec![self.load_home(), self.load_profile(0, pid), self.ensure_images(vec![img])])
    }

    fn update_shiki(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::ShikiOpenAuth => {
                match shikimori::authorize_url() {
                    Ok(u) => {
                        if let Err(e) = platform::open_url(&u) {
                            self.toast(format!("Не удалось открыть браузер: {e}"));
                        }
                    }
                    Err(e) => self.toast(e),
                }
                Task::none()
            }
            Message::ShikiConfirmCode => {
                let code = self.inp(Field::ShikiCode).trim().to_string();
                if code.is_empty() {
                    self.toast("Вставьте код авторизации со страницы Shikimori");
                    return Task::none();
                }
                self.busy = true;
                Task::perform(async move { shikimori::authorize_with_code(&code).await }, Message::ShikiAuthDone)
            }
            Message::ShikiAuthDone(r) => {
                self.busy = false;
                match r {
                    Ok(nick) => {
                        self.toast(format!("Shikimori: вход выполнен ({nick})"));
                        self.inputs.insert(Field::ShikiNick, nick);
                        self.inputs.remove(&Field::ShikiCode);
                    }
                    Err(e) => self.toast(e),
                }
                Task::none()
            }
            Message::ShikiLogout => {
                shikimori::logout();
                self.toast("Аккаунт Shikimori отключён");
                Task::none()
            }
            Message::ShikiImport => {
                if self.sync_active() {
                    return Task::none();
                }
                let nick = self.inp(Field::ShikiNick).trim().to_string();
                let handle = self.sync.clone();
                if let Ok(mut s) = handle.state.lock() {
                    s.running = true;
                    s.report = None;
                    s.message = "Подготовка…".into();
                }
                let api = self.api.clone();
                Task::perform(async move { shikimori::import(handle, nick, api).await }, |_| Message::Noop)
            }
            Message::ShikiExport => {
                if self.sync_active() {
                    return Task::none();
                }
                let handle = self.sync.clone();
                if let Ok(mut s) = handle.state.lock() {
                    s.running = true;
                    s.report = None;
                    s.message = "Подготовка…".into();
                }
                let api = self.api.clone();
                Task::perform(async move { shikimori::export(handle, api).await }, |_| Message::Noop)
            }
            Message::ShikiStop => {
                self.sync.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn update_settings(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::OpenSettings(sec) => {
                if sec == Section::Profile {
                    if let Some(p) = &self.me.profile {
                        let vals = [
                            (Field::StatusText, p.status.clone()),
                            (Field::Vk, p.vk.clone()),
                            (Field::Tg, p.tg.clone()),
                            (Field::Inst, p.inst.clone()),
                            (Field::Tt, p.tt.clone()),
                            (Field::Discord, p.discord.clone()),
                        ];
                        for (f, v) in vals {
                            self.inputs.insert(f, v);
                        }
                    }
                }
                // «Меню» заменяет вложенный раздел, а не наслаивается.
                if let Some(StackPage { kind: PageKind::Settings(_), .. }) = self.stack.last() {
                    if sec == Section::Menu {
                        self.stack.pop();
                        return Task::none();
                    }
                }
                self.push(PageKind::Settings(sec));
                Task::none()
            }
            Message::OpenShikimori => {
                self.push(PageKind::Shikimori);
                Task::none()
            }
            Message::OpenNotifications => {
                self.push(PageKind::Notifications(None));
                let api = self.api.clone();
                Task::perform(async move { api.notification_prefs().await }, Message::NotificationsLoaded)
            }
            Message::NotificationsLoaded(r) => {
                match r {
                    Ok(v) => {
                        if let Some(StackPage { kind: PageKind::Notifications(slot), .. }) = self.stack.last_mut() {
                            *slot = Some(v);
                        }
                    }
                    Err(e) => self.toast(e),
                }
                Task::none()
            }
            Message::ServerNotify(which) => {
                let api = self.api.clone();
                Task::perform(
                    async move {
                        api.toggle_notification(which).await?;
                        api.notification_prefs().await
                    },
                    Message::NotificationsLoaded,
                )
            }
            Message::SetTheme(t) => {
                self.settings.theme = t;
                self.save_settings();
                Task::none()
            }
            Message::SetAccent(a) => {
                self.inputs.insert(Field::AccentHex, a.clone());
                self.settings.accent = a;
                self.save_settings();
                Task::none()
            }
            Message::ApplyAccentHex => {
                let v = self.inp(Field::AccentHex).trim().to_string();
                let v = if v.starts_with('#') { v } else { format!("#{v}") };
                if crate::views::parse_hex(&v).is_some() {
                    self.settings.accent = v;
                    self.save_settings();
                } else {
                    self.toast("Цвет в формате #RRGGBB");
                }
                Task::none()
            }
            Message::SetPlayer(p) => {
                self.settings.player = p;
                self.save_settings();
                Task::none()
            }
            Message::SaveStatus => {
                let api = self.api.clone();
                let st = self.inp(Field::StatusText).to_string();
                Task::perform(async move { api.edit_status(&st).await }, |r| Message::ProfileOpDone("Статус сохранён".into(), r))
            }
            Message::SaveSocial => {
                let api = self.api.clone();
                let g = |f| self.inp(f).trim().to_string();
                let (vk, tg, inst, tt, ds) = (g(Field::Vk), g(Field::Tg), g(Field::Inst), g(Field::Tt), g(Field::Discord));
                Task::perform(async move { api.edit_social(&vk, &tg, &inst, &tt, &ds).await }, |r| {
                    Message::ProfileOpDone("Ссылки сохранены".into(), r)
                })
            }
            Message::ChangePassword => {
                let (cur, new) = (self.inp(Field::CurPass).to_string(), self.inp(Field::NewPass).to_string());
                if new.len() < 6 {
                    self.toast("Новый пароль — от 6 символов");
                    return Task::none();
                }
                let api = self.api.clone();
                Task::perform(async move { api.change_password(&cur, &new).await }, |r| Message::ProfileOpDone("Пароль изменён".into(), r))
            }
            Message::ChangeLogin => {
                let l = self.inp(Field::NewLogin).trim().to_string();
                let api = self.api.clone();
                Task::perform(async move { api.change_login(&l).await }, |r| Message::ProfileOpDone("Логин изменён".into(), r))
            }
            Message::ChangeEmail => {
                let (pass, new) = (self.inp(Field::CurPass).to_string(), self.inp(Field::NewEmail).trim().to_string());
                let api = self.api.clone();
                Task::perform(
                    async move {
                        let prefs = api.preferences().await?;
                        let hint = s(&prefs, "email_hint");
                        api.change_email(&hint, &pass, &new).await
                    },
                    |r| Message::ProfileOpDone("Запрос на смену почты отправлен".into(), r),
                )
            }
            Message::ToggleIncognito => {
                let api = self.api.clone();
                Task::perform(async move { api.set_incognito().await }, |r| Message::ProfileOpDone("Режим инкогнито переключён".into(), r))
            }
            Message::ProfileOpDone(text, r) => {
                match r {
                    Ok(()) => {
                        self.toast(text);
                        if self.me.id > 0 {
                            let id = self.me.id;
                            return self.load_profile(0, id);
                        }
                    }
                    Err(e) => self.toast(e),
                }
                Task::none()
            }
            Message::ApplyConnection => {
                self.settings.custom_api = self.inp(Field::CustomApi).trim().to_string();
                self.settings.user_agent = self.inp(Field::UserAgent).trim().to_string();
                self.save_settings();
                self.reset_api();
                self.toast("Параметры подключения применены");
                Task::none()
            }
            Message::ClearCaches => {
                storage::clear_caches();
                self.images.clear();
                self.toast("Кэш очищен");
                Task::none()
            }
            Message::ClearDownloads => {
                let _ = std::fs::remove_dir_all(platform::downloads_dir());
                storage::write(|s| s.downloads.clear());
                self.toast("Загрузки удалены");
                Task::none()
            }
            Message::CheckUpdate => Task::perform(update::check(), Message::UpdateChecked),
            Message::UpdateChecked(r) => {
                match r {
                    Ok(info) => {
                        if info.is_newer {
                            self.toast(format!("Доступна новая версия: {}", info.name));
                        }
                        self.update = Some(info);
                    }
                    Err(e) => {
                        if self.update.is_some() || matches!(self.stack.last(), Some(StackPage { kind: PageKind::Settings(Section::Updates), .. })) {
                            self.toast(e);
                        }
                    }
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn update_watch(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::ChooserTypes(uid, r) => {
                let mut next = Task::none();
                let remembered = {
                    let rid = match self.page_mut(uid) {
                        Some(PageKind::Chooser(c)) => c.release.id,
                        _ => return Task::none(),
                    };
                    if self.settings.remember_source {
                        storage::read(|s| s.remembered_source.get(&rid.to_string()).copied())
                    } else {
                        None
                    }
                };
                let mut resume: Option<VoiceType> = None;
                if let Some(PageKind::Chooser(c)) = self.page_mut(uid) {
                    c.loading = false;
                    match r {
                        Ok(types) => {
                            if let Some((tid, _)) = remembered {
                                resume = types.iter().find(|t| t.id == tid).cloned();
                            }
                            if resume.is_none() && types.len() == 1 {
                                resume = types.first().cloned();
                            }
                            if types.is_empty() {
                                c.error = Some("Для этого релиза нет доступных озвучек".into());
                            }
                            c.types = types;
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                if let Some(t) = resume {
                    next = self.chooser_load_sources(uid, &t);
                }
                next
            }
            Message::ChooseType(uid, t) => self.chooser_load_sources(uid, &t),
            Message::ChooserSources(uid, r) => {
                let rid = match self.page_mut(uid) {
                    Some(PageKind::Chooser(c)) => c.release.id,
                    _ => return Task::none(),
                };
                let remembered = if self.settings.remember_source {
                    storage::read(|s| s.remembered_source.get(&rid.to_string()).copied())
                } else {
                    None
                };
                let mut pick: Option<Source> = None;
                if let Some(PageKind::Chooser(c)) = self.page_mut(uid) {
                    c.loading = false;
                    match r {
                        Ok(sources) => {
                            if let Some((tid, sid)) = remembered {
                                if c.type_sel.as_ref().map(|t| t.id) == Some(tid) {
                                    pick = sources.iter().find(|s| s.id == sid).cloned();
                                }
                            }
                            if pick.is_none() && sources.len() == 1 {
                                pick = sources.first().cloned();
                            }
                            if sources.is_empty() {
                                c.error = Some("В этой озвучке нет источников".into());
                            }
                            c.sources = sources;
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                match pick {
                    Some(s) => self.chooser_load_episodes(uid, &s),
                    None => Task::none(),
                }
            }
            Message::ChooseSource(uid, s) => self.chooser_load_episodes(uid, &s),
            Message::ChooserEpisodes(uid, r) => {
                if let Some(PageKind::Chooser(c)) = self.page_mut(uid) {
                    c.loading = false;
                    match r {
                        Ok(mut eps) => {
                            eps.sort_by_key(|e| e.position);
                            let (rid, sid) = (c.release.id, c.source_sel.as_ref().map(|s| s.id).unwrap_or(0));
                            for e in eps.iter_mut() {
                                if e.release_id == 0 {
                                    e.release_id = rid;
                                }
                                if e.source_id == 0 {
                                    e.source_id = sid;
                                }
                            }
                            c.episodes = eps;
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                Task::none()
            }
            Message::ChooserBack(uid) => {
                if let Some(PageKind::Chooser(c)) = self.page_mut(uid) {
                    if c.source_sel.is_some() {
                        c.source_sel = None;
                        c.episodes.clear();
                    } else if c.type_sel.is_some() {
                        c.type_sel = None;
                        c.sources.clear();
                    } else {
                        self.stack.pop();
                    }
                }
                Task::none()
            }
            Message::OpenEpisode(uid, index) => {
                let (release, type_id, source_id, episodes) = match self.page_mut(uid) {
                    Some(PageKind::Chooser(c)) => (
                        c.release.clone(),
                        c.type_sel.as_ref().map(|t| t.id).unwrap_or(0),
                        c.source_sel.as_ref().map(|s| s.id).unwrap_or(0),
                        c.episodes.clone(),
                    ),
                    _ => return Task::none(),
                };
                let speed = self.settings.speed;
                let page = PlayerPage {
                    release,
                    type_id,
                    source_id,
                    episodes,
                    index,
                    stream: None,
                    embed_url: String::new(),
                    loading: true,
                    error: None,
                    variant: None,
                    speed,
                    subtitle: None,
                    subtitle_path: None,
                    markers: vec![],
                    playing: false,
                    status: String::new(),
                    auto_play: false,
                    download_url: String::new(),
                };
                let puid = self.push(PageKind::Player(Box::new(page)));
                self.player_load(puid)
            }
            Message::PlayerReady(uid, r) => {
                let mut tasks = vec![];
                let mut auto = false;
                let last_sub = self.settings.last_subtitle.clone();
                let mut sub_task: Option<(usize, crate::subtitles::Track, String)> = None;
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    p.loading = false;
                    match r {
                        Ok((ep, stream)) => {
                            if let Some(slot) = p.episodes.get_mut(p.index) {
                                slot.url = ep.url.clone();
                                slot.iframe = ep.iframe;
                            }
                            p.embed_url = ep.url.clone();
                            match stream {
                                Ok(st) => {
                                    if !last_sub.is_empty() {
                                        if let Some(i) = st.subtitles.iter().position(|t| t.label == last_sub) {
                                            let tag = format!("{}_{}_{}", p.release.id, ep.position, i);
                                            sub_task = Some((i, st.subtitles[i].clone(), tag));
                                        }
                                    }
                                    p.download_url = st.variants.first().map(|v| v.url.clone()).unwrap_or_else(|| st.url.clone());
                                    p.stream = Some(st);
                                    auto = std::mem::take(&mut p.auto_play);
                                }
                                Err(e) => p.error = Some(e),
                            }
                        }
                        Err(e) => p.error = Some(e),
                    }
                }
                if let Some((i, track, tag)) = sub_task {
                    tasks.push(Task::perform(async move { player::fetch_subtitle(&track, &tag).await }, move |r| {
                        Message::PlayerSubtitleLoaded(uid, Some(i), r)
                    }));
                }
                if auto {
                    tasks.push(Task::done(Message::PlayerPlay(uid)));
                }
                Task::batch(tasks)
            }
            Message::PlayerVariant(uid, v) => {
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    p.variant = v;
                }
                Task::none()
            }
            Message::PlayerSpeed(uid, sp) => {
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    p.speed = sp;
                }
                self.settings.speed = sp;
                self.save_settings();
                Task::none()
            }
            Message::PlayerSubtitle(uid, idx) => {
                let mut task = Task::none();
                match idx {
                    None => {
                        if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                            p.subtitle = None;
                            p.subtitle_path = None;
                        }
                        self.settings.last_subtitle.clear();
                        self.save_settings();
                    }
                    Some(i) => {
                        if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                            if let Some(track) = p.stream.as_ref().and_then(|s| s.subtitles.get(i)).cloned() {
                                p.status = format!("Загрузка субтитров: {}…", track.label);
                                let tag = format!("{}_{}_{}", p.release.id, p.episodes.get(p.index).map(|e| e.position).unwrap_or(0), i);
                                self.settings.last_subtitle = track.label.clone();
                                task = Task::perform(async move { player::fetch_subtitle(&track, &tag).await }, move |r| {
                                    Message::PlayerSubtitleLoaded(uid, Some(i), r)
                                });
                            }
                        }
                        self.save_settings();
                    }
                }
                task
            }
            Message::PlayerSubtitleLoaded(uid, idx, r) => {
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    p.status.clear();
                    match r {
                        Ok(path) => {
                            p.subtitle = idx;
                            p.subtitle_path = Some(path);
                        }
                        Err(e) => p.error = Some(format!("Не удалось загрузить субтитры: {e}")),
                    }
                }
                Task::none()
            }
            Message::PlayerMarkers(uid, r) => {
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    if let Ok(m) = r {
                        p.markers = m;
                    }
                }
                Task::none()
            }
            Message::PlayerPlay(uid) => {
                let opts = match self.page_mut(uid) {
                    Some(PageKind::Player(p)) => {
                        let p2 = (**p).clone();
                        p.playing = true;
                        p.status = "Воспроизведение во внешнем плеере…".into();
                        self.launch_opts(&p2)
                    }
                    _ => None,
                };
                match opts {
                    Some(o) => Task::perform(player::play(o), move |r| Message::PlayerFinished(uid, r)),
                    None => Task::none(),
                }
            }
            Message::PlayerFinished(uid, res) => {
                let mut tasks = vec![];
                let mut next = false;
                let mut watched_info: Option<(i64, i64, i64)> = None;
                if let Some(e) = &res.error {
                    self.toast(e.clone());
                }
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    p.playing = false;
                    p.status.clear();
                    if let Some(ep) = p.episodes.get(p.index).cloned() {
                        if res.tracked {
                            if let (Some(pos), Some(dur)) = (res.position, res.duration) {
                                let finished = player::is_finished(pos, dur);
                                storage::write(|s| {
                                    let key = storage::position_key(p.release.id, p.source_id, ep.position);
                                    if finished {
                                        s.positions.remove(&key);
                                    } else {
                                        s.positions.insert(key, pos);
                                    }
                                });
                                if finished {
                                    watched_info = Some((p.release.id, p.source_id, ep.position));
                                    next = p.index + 1 < p.episodes.len();
                                }
                            }
                        } else {
                            p.status = "Отметьте серию просмотренной вручную после просмотра".into();
                        }
                    }
                }
                if let Some((rid, sid, pos)) = watched_info {
                    self.mark_local_watched(rid, pos);
                    let api = self.api.clone();
                    tasks.push(Task::perform(async move { api.mark_watched(rid, sid, pos).await }, Message::Watched));
                }
                if next && self.settings.auto_next {
                    if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                        p.index += 1;
                        p.auto_play = true;
                    }
                    tasks.push(self.player_load(uid));
                }
                Task::batch(tasks)
            }
            Message::PlayerNext(uid) | Message::PlayerPrev(uid) => {
                let forward = matches!(m, Message::PlayerNext(_));
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    if forward && p.index + 1 < p.episodes.len() {
                        p.index += 1;
                    } else if !forward && p.index > 0 {
                        p.index -= 1;
                    } else {
                        return Task::none();
                    }
                }
                self.player_load(uid)
            }
            Message::PlayerMarkWatched(uid) => {
                let info = match self.page_mut(uid) {
                    Some(PageKind::Player(p)) => p.episodes.get(p.index).map(|e| (p.release.id, p.source_id, e.position)),
                    _ => None,
                };
                if let Some((rid, sid, pos)) = info {
                    self.mark_local_watched(rid, pos);
                    let api = self.api.clone();
                    if !self.logged_in {
                        self.toast("Войдите в аккаунт, чтобы сохранять просмотр");
                        return Task::none();
                    }
                    return Task::perform(async move { api.mark_watched(rid, sid, pos).await }, Message::Watched);
                }
                Task::none()
            }
            Message::PlayerOpenEmbed(uid) => {
                if let Some(PageKind::Player(p)) = self.page_mut(uid) {
                    let url = p.embed_url.clone();
                    if let Err(e) = platform::open_url(&url) {
                        self.toast(e);
                    }
                }
                Task::none()
            }
            Message::Watched(r) => {
                match r {
                    Ok(()) => self.toast("Серия отмечена просмотренной"),
                    Err(e) => self.toast(e),
                }
                Task::none()
            }
            Message::DownloadStart(uid) => {
                let info = match self.page_mut(uid) {
                    Some(PageKind::Player(p)) => p.episodes.get(p.index).map(|e| (p.release.clone(), p.source_id, e.clone())),
                    _ => None,
                };
                let Some((release, sid, ep)) = info else { return Task::none() };
                let id = format!("{}_{}_{}", release.id, sid, ep.position);
                if self.active_downloads.contains_key(&id) || storage::read(|s| s.downloads.iter().any(|d| d.id == id && d.done)) {
                    self.toast("Эта серия уже загружена или загружается");
                    return Task::none();
                }
                let progress = Arc::new(hls::Progress::default());
                self.active_downloads.insert(
                    id.clone(),
                    ActiveDownload { title: format!("{} — {}", release.title(), ep.name), progress: progress.clone() },
                );
                let api = self.api.clone();
                let rec_id = id.clone();
                let rec = DownloadRecord {
                    id: id.clone(),
                    release_id: release.id,
                    release_title: release.title(),
                    image: release.image.clone(),
                    episode_name: ep.name.clone(),
                    position: ep.position,
                    file: String::new(),
                    size: 0,
                    done: false,
                };
                self.toast("Загрузка началась");
                Task::perform(
                    async move {
                        let target = api.episode_target(release.id, sid, ep.position, true).await?;
                        let stream = player::resolve_stream(&target.url, target.iframe).await?;
                        let url = stream.variants.first().map(|v| v.url.clone()).unwrap_or(stream.url);
                        let ext = if url.split('?').next().unwrap_or("").to_lowercase().ends_with(".mp4") { "mp4" } else { "ts" };
                        let dest = platform::downloads_dir().join(format!("{rec_id}.{ext}"));
                        let size = hls::download(&url, &dest, progress).await?;
                        let mut rec = rec;
                        rec.file = dest.to_string_lossy().to_string();
                        rec.size = size;
                        rec.done = true;
                        storage::write(|s| {
                            s.downloads.retain(|d| d.id != rec.id);
                            s.downloads.push(rec);
                        });
                        Ok(size)
                    },
                    move |r| Message::DownloadDone(id.clone(), r),
                )
            }
            Message::DownloadDone(id, r) => {
                self.active_downloads.remove(&id);
                match r {
                    Ok(size) => self.toast(format!("Загружено ({})", platform::human_size(size))),
                    Err(e) => self.toast(format!("Ошибка загрузки: {e}")),
                }
                Task::none()
            }
            Message::DownloadCancel(id) => {
                if let Some(d) = self.active_downloads.get(&id) {
                    d.progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Task::none()
            }
            Message::DownloadDelete(id) => {
                let file = storage::read(|s| s.downloads.iter().find(|d| d.id == id).map(|d| d.file.clone()));
                if let Some(f) = file {
                    let _ = std::fs::remove_file(f);
                }
                storage::write(|s| s.downloads.retain(|d| d.id != id));
                Task::none()
            }
            Message::PlayLocal(id) => {
                let Some(rec) = storage::read(|s| s.downloads.iter().find(|d| d.id == id).cloned()) else { return Task::none() };
                let settings = self.settings.clone();
                Task::perform(
                    async move {
                        let release = Release { id: rec.release_id, title_ru: rec.release_title.clone(), ..Default::default() };
                        let markers = if settings.aniskip_enabled { aniskip::load_for_episode(&release, &rec.episode_name).await } else { vec![] };
                        player::play(LaunchOpts {
                            player: settings.player.clone(),
                            url: rec.file.clone(),
                            title: format!("{} — {}", rec.release_title, rec.episode_name),
                            start: 0.0,
                            speed: settings.speed,
                            subtitle: None,
                            markers,
                            auto_skip: settings.aniskip_auto && settings.aniskip_enabled,
                            mpv_path: settings.mpv_path.clone(),
                        })
                        .await
                    },
                    Message::LocalPlayed,
                )
            }
            Message::LocalPlayed(res) => {
                if let Some(e) = res.error {
                    self.toast(e);
                }
                Task::none()
            }
            Message::SaveImage(url) => {
                let api = self.api.clone();
                Task::perform(
                    async move {
                        let bytes = api.bytes(&url).await?;
                        let name = url.rsplit('/').next().unwrap_or("image.jpg").split('?').next().unwrap_or("image.jpg").to_string();
                        #[cfg(not(target_os = "android"))]
                        let dir = dirs::picture_dir().unwrap_or_else(platform::data_dir).join("AniLot");
                        #[cfg(target_os = "android")]
                        let dir = platform::data_dir().join("images");
                        tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
                        let path = dir.join(name);
                        tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
                        Ok(path)
                    },
                    Message::ImageSaved,
                )
            }
            Message::ImageSaved(r) => {
                match r {
                    Ok(p) => self.toast(format!("Сохранено: {}", p.display())),
                    Err(e) => self.toast(format!("Не удалось сохранить: {e}")),
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}

/// Тонкие обёртки, доступные из других модулей crate.
impl App {
    pub(crate) fn images_task(&mut self, urls: Vec<String>) -> Task<Message> {
        self.ensure_images(urls)
    }
    pub(crate) fn listing_ref(&mut self, t: ListTarget) -> Option<&mut Listing> {
        self.listing_mut(t)
    }
    pub(crate) fn fetch_listing_pub(&mut self, t: ListTarget) -> Task<Message> {
        self.fetch_listing(t)
    }
    pub(crate) fn start_listing_pub(&mut self, t: ListTarget, kind: ListKind) -> Task<Message> {
        self.start_listing(t, kind)
    }
    pub(crate) fn open_release_pub(&mut self, id: i64) -> Task<Message> {
        self.open_release_page(id)
    }
    pub(crate) fn push_pub(&mut self, kind: PageKind) -> u64 {
        self.push(kind)
    }
    pub(crate) fn page_mut_pub(&mut self, uid: u64) -> Option<&mut PageKind> {
        self.page_mut(uid)
    }
    pub(crate) fn toast_msg(&mut self, text: impl Into<String>) {
        self.toast(text)
    }
    pub(crate) fn profile_task(&mut self, uid: u64, id: i64) -> Task<Message> {
        self.load_profile(uid, id)
    }
    pub(crate) fn friends_task(&mut self, uid: u64) -> Task<Message> {
        self.load_friends(uid)
    }
    pub(crate) fn comments_task(&mut self, uid: u64) -> Task<Message> {
        self.load_comments(uid)
    }
    pub(crate) fn collections_task(&mut self, uid: u64) -> Task<Message> {
        self.load_collections(uid)
    }
    pub(crate) fn chooser_types_pub(&mut self, uid: u64) -> Task<Message> {
        self.chooser_load_types(uid)
    }
    pub(crate) fn request_feed(&mut self, page: i64) -> Task<Message> {
        self.load_feed(page)
    }
    pub(crate) fn reload_home_tasks(&mut self) -> Task<Message> {
        self.load_home()
    }
    pub(crate) fn inputs_remove(&mut self, f: Field) {
        self.inputs.remove(&f);
    }
    pub(crate) fn inputs_insert(&mut self, f: Field, v: String) {
        self.inputs.insert(f, v);
    }
}
