//! Экраны: общие элементы, вход, вкладки, релиз и профиль.

use crate::api::{self, CommentKind, ListKind, GENRES};
use crate::app::*;
use crate::models::*;
use crate::storage;
use iced::widget::{button, column, container, image, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length, Padding, Theme};

pub type El<'a> = Element<'a, Message>;

pub fn parse_hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub const MUTED: Color = Color { r: 0.56, g: 0.56, b: 0.60, a: 1.0 };

pub fn is_dark(app: &App) -> bool {
    app.settings.theme != "light"
}

pub fn make_theme(app: &App) -> Theme {
    let accent = parse_hex(&app.settings.accent).unwrap_or(Color::from_rgb8(0xFF, 0x8A, 0x00));
    let palette = if is_dark(app) {
        iced::theme::Palette {
            background: Color::from_rgb8(0x0E, 0x0E, 0x10),
            text: Color::from_rgb8(0xF2, 0xF2, 0xF2),
            primary: accent,
            success: Color::from_rgb8(0x3C, 0xC2, 0x6B),
            warning: Color::from_rgb8(0xF5, 0xB0, 0x31),
            danger: Color::from_rgb8(0xE5, 0x48, 0x4D),
        }
    } else {
        iced::theme::Palette {
            background: Color::from_rgb8(0xF6, 0xF6, 0xF7),
            text: Color::from_rgb8(0x14, 0x14, 0x16),
            primary: accent,
            success: Color::from_rgb8(0x1F, 0x9D, 0x55),
            warning: Color::from_rgb8(0xD9, 0x8E, 0x04),
            danger: Color::from_rgb8(0xD1, 0x2D, 0x35),
        }
    };
    Theme::custom("AniLot".to_string(), palette)
}

pub fn card_style(theme: &Theme) -> container::Style {
    let p = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(p.background.weak.color)),
        border: Border { radius: 14.0.into(), ..Default::default() },
        ..Default::default()
    }
}

fn placeholder_style(theme: &Theme) -> container::Style {
    let p = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(p.background.strong.color)),
        border: Border { radius: 10.0.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn muted<'a>(s: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(s).size(13).color(MUTED)
}

pub fn heading<'a>(s: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(s).size(22)
}

pub fn btn<'a>(label: &'a str, msg: Message) -> El<'a> {
    button(text(label).size(15)).padding([9, 16]).on_press(msg).style(button::primary).into()
}

pub fn ghost<'a>(label: impl text::IntoFragment<'a>, msg: Message) -> El<'a> {
    button(text(label).size(14)).padding([8, 14]).on_press(msg).style(button::secondary).into()
}

pub fn link<'a>(label: impl text::IntoFragment<'a>, msg: Message) -> El<'a> {
    button(text(label).size(14)).padding([4, 8]).on_press(msg).style(button::text).into()
}

pub fn chip<'a>(label: impl text::IntoFragment<'a>, selected: bool, msg: Message) -> El<'a> {
    button(text(label).size(13))
        .padding([6, 12])
        .on_press(msg)
        .style(if selected { button::primary } else { button::secondary })
        .into()
}

pub fn wrap<'a>(items: Vec<El<'a>>) -> El<'a> {
    row(items).spacing(6).wrap().into()
}

pub fn field<'a>(app: &'a App, f: Field, placeholder: &'a str) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, app.inp(f)).on_input(move |v| Message::Input(f, v)).padding(10).size(15)
}

pub fn secret<'a>(app: &'a App, f: Field, placeholder: &'a str) -> iced::widget::TextInput<'a, Message> {
    field(app, f, placeholder).secure(true)
}

pub fn toggle_row<'a>(label: &'a str, value: bool, t: Toggle) -> El<'a> {
    iced::widget::toggler(value).label(label).on_toggle(move |v| Message::Toggle(t, v)).size(22).into()
}

fn scroll_h<'a>(content: El<'a>) -> El<'a> {
    scrollable(content)
        .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(4).scroller_width(4)))
        .width(Length::Fill)
        .into()
}

impl App {
    // ───────────── картинки и карточки ─────────────

    pub fn pic<'a>(&'a self, url: &str, w: f32, h: f32) -> El<'a> {
        match self.image(url) {
            Some(handle) => image(handle.clone())
                .width(w)
                .height(h)
                .content_fit(ContentFit::Cover)
                .border_radius(10.0)
                .into(),
            None => container(Space::new()).width(w).height(h).style(placeholder_style).into(),
        }
    }

    pub fn avatar<'a>(&'a self, url: &str, size: f32) -> El<'a> {
        match self.image(url) {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .content_fit(ContentFit::Cover)
                .border_radius(size / 2.0)
                .into(),
            None => container(Space::new()).width(size).height(size).style(move |t: &Theme| {
                let mut s = placeholder_style(t);
                s.border.radius = (size / 2.0).into();
                s
            })
            .into(),
        }
    }

    pub fn poster_card<'a>(&'a self, r: &Release) -> El<'a> {
        let meta = [r.year.clone(), r.category.clone()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
        button(
            column![
                self.pic(&r.image, 120.0, 170.0),
                text(r.title()).size(13).width(120),
                muted(meta).size(12),
            ]
            .spacing(4),
        )
        .padding(0)
        .style(button::text)
        .on_press(Message::OpenRelease(r.id))
        .into()
    }

    pub fn release_row<'a>(&'a self, r: &Release) -> El<'a> {
        let mut info = column![text(r.title()).size(16)].spacing(3).width(Length::Fill);
        let sub = [r.year.clone(), r.category.clone(), r.status.clone()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
        if !sub.is_empty() {
            info = info.push(muted(sub));
        }
        if !r.genres.is_empty() {
            info = info.push(muted(r.genres.clone()).size(12));
        }
        let mut tail = vec![r.episodes_label()];
        if r.grade > 0.0 {
            tail.push(format!("★ {:.1}", r.grade));
        }
        if r.list_status > 0 {
            tail.push(status_name(r.list_status).to_string());
        }
        info = info.push(text(tail.join("  ")).size(12).color(MUTED));
        button(container(row![self.pic(&r.image, 70.0, 100.0), info].spacing(12)).padding(8).width(Length::Fill).style(card_style))
            .padding(0)
            .width(Length::Fill)
            .style(button::text)
            .on_press(Message::OpenRelease(r.id))
            .into()
    }

    pub fn listing_view<'a>(&'a self, l: &'a Listing, t: ListTarget) -> El<'a> {
        let mut col = column![].spacing(8);
        for r in &l.items {
            col = col.push(self.release_row(r));
        }
        if l.loading {
            col = col.push(muted("Загрузка…"));
        } else if let Some(e) = &l.error {
            col = col.push(text(format!("Ошибка: {e}")).size(14).color(Color::from_rgb8(0xE5, 0x48, 0x4D)));
            col = col.push(ghost("Повторить", Message::ListMore(t)));
        } else if l.items.is_empty() {
            col = col.push(muted("Ничего не найдено"));
        } else if !l.last {
            col = col.push(ghost("Загрузить ещё", Message::ListMore(t)));
        }
        col.into()
    }

    pub fn top_bar<'a>(&'a self, title: impl text::IntoFragment<'a>, extra: Vec<El<'a>>) -> El<'a> {
        let mut r = row![button(text("‹").size(26)).padding([0, 12]).on_press(Message::Back).style(button::text), text(title).size(20).width(Length::Fill)]
            .spacing(6)
            .align_y(Alignment::Center);
        for e in extra {
            r = r.push(e);
        }
        r.into()
    }

    fn scroll_page<'a>(&'a self, bar: El<'a>, body: El<'a>) -> El<'a> {
        column![bar, scrollable(container(body).padding([4, 14]).width(Length::Fill)).height(Length::Fill)].spacing(6).into()
    }

    // ───────────── корневой вид ─────────────

    pub fn view(&self) -> El<'_> {
        let body: El = if !self.is_ready() {
            self.auth_view()
        } else if let Some(page) = self.stack.last() {
            self.page_view(page)
        } else {
            let content = match self.tab {
                Tab::Home => self.home_view(),
                Tab::Search => self.search_view(),
                Tab::Bookmarks => self.bookmarks_view(),
                Tab::Downloads => self.downloads_view(),
                Tab::Profile => self.profile_tab_view(),
            };
            column![container(content).height(Length::Fill), self.tab_bar()].into()
        };
        let mut layers = column![body];
        if let Some((t, _)) = &self.toast {
            layers = layers.push(
                container(text(t.clone()).size(14))
                    .padding([10, 16])
                    .width(Length::Fill)
                    .style(|theme: &Theme| container::Style {
                        background: Some(Background::Color(theme.extended_palette().primary.base.color)),
                        text_color: Some(theme.extended_palette().primary.base.text),
                        ..Default::default()
                    }),
            );
        }
        let i = self.insets;
        container(layers)
            .padding(Padding { top: i.top, right: i.right, bottom: i.bottom, left: i.left })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn tab_bar(&self) -> El<'_> {
        let tabs = [
            (Tab::Home, "Главная"),
            (Tab::Search, "Поиск"),
            (Tab::Bookmarks, "Закладки"),
            (Tab::Downloads, "Загрузки"),
            (Tab::Profile, "Профиль"),
        ];
        let mut r = row![].spacing(2).width(Length::Fill);
        for (t, label) in tabs {
            r = r.push(
                button(text(label).size(13).width(Length::Fill).align_x(Alignment::Center))
                    .width(Length::Fill)
                    .padding([12, 4])
                    .on_press(Message::SetTab(t))
                    .style(if self.tab == t { button::primary } else { button::text }),
            );
        }
        container(r).padding(4).style(card_style).into()
    }

    fn page_view<'a>(&'a self, page: &'a StackPage) -> El<'a> {
        let uid = page.uid;
        match &page.kind {
            PageKind::Release(p) => self.release_view(uid, p),
            PageKind::List { title, listing } => {
                self.scroll_page(self.top_bar(title.clone(), vec![]), self.listing_view(listing, ListTarget::Page(uid)))
            }
            PageKind::Profile(p) => self.profile_view(uid, p, false),
            PageKind::Friends(f) => self.friends_view(uid, f),
            PageKind::Comments(c) => self.comments_view(uid, c),
            PageKind::Collections(c) => self.collections_view(uid, c),
            PageKind::Collection(c) => self.collection_view(uid, c),
            PageKind::Chooser(c) => self.chooser_view(uid, c),
            PageKind::Player(p) => self.player_view(uid, p),
            PageKind::Gallery { urls, index } => self.gallery_view(urls, *index),
            PageKind::Videos(v) => self.videos_view(v),
            PageKind::Schedule(s) => self.schedule_view(uid, s),
            PageKind::Settings(sec) => self.settings_view(*sec),
            PageKind::Shikimori => self.shikimori_view(),
            PageKind::Notifications(v) => self.notifications_view(v.as_ref()),
        }
    }

    // ───────────── вход ─────────────

    fn auth_view(&self) -> El<'_> {
        let mut col = column![text("AniLot").size(40).color(parse_hex(&self.settings.accent).unwrap_or(Color::WHITE)), muted("Неофициальный клиент Anixart")]
            .spacing(10)
            .align_x(Alignment::Center);
        let busy = self.auth_busy;
        let action = |label: &'static str, m: Message| -> El<'_> {
            if busy {
                button(text("Подождите…").size(15)).padding([10, 18]).style(button::secondary).into()
            } else {
                btn(label, m)
            }
        };
        match self.auth_mode {
            AuthMode::SignIn => {
                col = col
                    .push(field(self, Field::Login, "Логин или почта").on_submit(Message::SignIn))
                    .push(secret(self, Field::Password, "Пароль").on_submit(Message::SignIn))
                    .push(action("Войти", Message::SignIn))
                    .push(
                        row![
                            link("Регистрация", Message::AuthMode(AuthMode::SignUp)),
                            link("Забыли пароль?", Message::AuthMode(AuthMode::Restore))
                        ]
                        .spacing(8),
                    );
            }
            AuthMode::SignUp => {
                col = col
                    .push(field(self, Field::SignUpLogin, "Логин"))
                    .push(field(self, Field::SignUpEmail, "Почта"))
                    .push(secret(self, Field::SignUpPassword, "Пароль (от 6 символов)"))
                    .push(action("Зарегистрироваться", Message::SignUp))
                    .push(link("Уже есть аккаунт", Message::AuthMode(AuthMode::SignIn)));
            }
            AuthMode::SignUpCode => {
                col = col
                    .push(muted("Мы отправили код подтверждения на вашу почту"))
                    .push(field(self, Field::Code, "Код").on_submit(Message::Verify))
                    .push(action("Подтвердить", Message::Verify))
                    .push(link("Назад", Message::AuthMode(AuthMode::SignUp)));
            }
            AuthMode::Restore => {
                col = col
                    .push(muted("Восстановление доступа"))
                    .push(field(self, Field::RestoreData, "Логин или почта").on_submit(Message::Restore))
                    .push(action("Отправить код", Message::Restore))
                    .push(link("Назад", Message::AuthMode(AuthMode::SignIn)));
            }
            AuthMode::RestoreCode => {
                col = col
                    .push(muted("Введите код из письма и новый пароль"))
                    .push(field(self, Field::Code, "Код"))
                    .push(secret(self, Field::RestorePassword, "Новый пароль"))
                    .push(action("Сменить пароль", Message::RestoreVerify))
                    .push(link("Назад", Message::AuthMode(AuthMode::Restore)));
            }
        }
        if let Some(e) = &self.auth_error {
            col = col.push(text(e.clone()).size(14).color(Color::from_rgb8(0xE5, 0x48, 0x4D)));
        }
        col = col.push(link("Продолжить без входа", Message::Guest));
        container(container(col.width(Length::Fill).max_width(420)).padding(24))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    // ───────────── главная ─────────────

    fn home_view(&self) -> El<'_> {
        let head = row![
            text("AniLot").size(26).color(parse_hex(&self.settings.accent).unwrap_or(Color::WHITE)).width(Length::Fill),
            ghost("Расписание", Message::OpenSchedule),
            ghost("Коллекции", Message::OpenCollections),
            ghost("⟳", Message::ReloadHome),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        let modes = row![
            chip("Релизы", !self.home.feed_mode, Message::HomeFeedMode(false)),
            chip("Лента", self.home.feed_mode, Message::HomeFeedMode(true))
        ]
        .spacing(6);
        let mut col = column![head, modes].spacing(12);
        if self.home.feed_mode {
            for item in &self.home.feed {
                let mut c = column![].spacing(4);
                let who = [item.channel.clone(), item.author.clone()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
                c = c.push(muted(format!("{} {}", who, format_date(item.date))));
                if !item.text.is_empty() {
                    let t: String = item.text.chars().take(400).collect();
                    c = c.push(text(t).size(14));
                }
                if !item.image.is_empty() {
                    c = c.push(self.pic(&item.image, 280.0, 160.0));
                }
                c = c.push(muted(format!("Комментариев: {} · Рейтинг: {}", item.comments, item.votes)).size(12));
                col = col.push(container(c).padding(12).width(Length::Fill).style(card_style));
            }
            if self.home.feed.is_empty() {
                col = col.push(muted("Лента пуста или загружается…"));
            } else if !self.home.feed_last {
                col = col.push(ghost("Загрузить ещё", Message::FeedMore));
            }
        } else {
            if !self.home.interesting.is_empty() {
                let mut r = row![].spacing(10);
                for it in &self.home.interesting {
                    let card = column![self.pic(&it.image, 260.0, 130.0), text(it.title.clone()).size(14).width(260), muted(it.description.chars().take(60).collect::<String>()).size(12).width(260)]
                        .spacing(3);
                    r = r.push(card);
                }
                col = col.push(scroll_h(r.into()));
            }
            for (idx, (title, items)) in self.home.sections.iter().enumerate() {
                if items.is_empty() {
                    continue;
                }
                let kind = match idx {
                    0 => Some(ListKind::Watching),
                    1 => Some(ListKind::Filter(api::Filter::default())),
                    2 => Some(ListKind::Filter(api::Filter { sort: 3, ..Default::default() })),
                    3 => Some(ListKind::Recommendations),
                    _ => None,
                };
                let mut header = row![heading(title.clone()).size(18).width(Length::Fill)].align_y(Alignment::Center);
                if let Some(k) = kind {
                    header = header.push(link("Все ›", Message::OpenList(title.clone(), k)));
                }
                let mut r = row![].spacing(10);
                for rel in items {
                    r = r.push(self.poster_card(rel));
                }
                col = col.push(column![header, scroll_h(r.into())].spacing(6));
            }
            if self.home.sections.iter().all(|s| s.1.is_empty()) {
                if let Some(e) = &self.home.error {
                    col = col.push(text(format!("Не удалось загрузить: {e}")).size(14).color(Color::from_rgb8(0xE5, 0x48, 0x4D)));
                    col = col.push(ghost("Повторить", Message::ReloadHome));
                } else {
                    col = col.push(muted("Загрузка…"));
                }
            }
        }
        scrollable(container(col).padding([10, 14]).width(Length::Fill)).height(Length::Fill).into()
    }

    // ───────────── поиск ─────────────

    fn search_view(&self) -> El<'_> {
        let mut col = column![].spacing(10);
        col = col.push(
            row![
                chip("Релизы", !self.search.profiles_mode, Message::SearchProfilesMode(false)),
                chip("Профили", self.search.profiles_mode, Message::SearchProfilesMode(true)),
            ]
            .spacing(6),
        );
        let mut bar = row![field(self, Field::Search, "Поиск аниме").on_submit(Message::SubmitSearch).width(Length::Fill), btn("Найти", Message::SubmitSearch)].spacing(6);
        if !self.search.profiles_mode {
            bar = bar.push(ghost("Фильтр", Message::ToggleFilter));
        }
        col = col.push(bar);
        if self.search.profiles_mode {
            for p in &self.search.profiles {
                col = col.push(self.profile_row(p));
            }
            if self.search.profiles_loading {
                col = col.push(muted("Поиск…"));
            }
            return scrollable(container(col).padding([10, 14]).width(Length::Fill)).height(Length::Fill).into();
        }
        if self.search.show_filter {
            col = col.push(self.filter_panel());
        }
        match &self.search.listing {
            Some(l) => col = col.push(self.listing_view(l, ListTarget::Search)),
            None => {
                let history = storage::read(|s| s.search_history.clone());
                if !history.is_empty() {
                    let chips: Vec<El> = history.iter().map(|h| chip(h.clone(), false, Message::HistoryQuery(h.clone()))).collect();
                    col = col.push(row![muted("История поиска").width(Length::Fill), link("Очистить", Message::ClearSearchHistory)]);
                    col = col.push(wrap(chips));
                } else {
                    col = col.push(muted("Введите название или откройте фильтр"));
                }
            }
        }
        scrollable(container(col).padding([10, 14]).width(Length::Fill)).height(Length::Fill).into()
    }

    pub fn profile_row<'a>(&'a self, p: &ProfileCompact) -> El<'a> {
        button(container(row![self.avatar(&p.avatar, 44.0), text(p.login.clone()).size(16)].spacing(12).align_y(Alignment::Center)).padding(8).width(Length::Fill).style(card_style))
            .padding(0)
            .width(Length::Fill)
            .style(button::text)
            .on_press(Message::OpenProfile(p.id))
            .into()
    }

    fn filter_panel(&self) -> El<'_> {
        let f = &self.search.filter;
        let mut col = column![].spacing(8);
        col = col.push(text("Сортировка").size(14));
        col = col.push(wrap(api::SORTS.iter().map(|(id, n)| chip(*n, f.sort == *id, Message::FilterSort(*id))).collect()));
        col = col.push(text("Тип").size(14));
        let mut cats: Vec<El> = vec![chip("Любой", f.category_id.is_none(), Message::FilterCategory(None))];
        cats.extend(api::CATEGORIES.iter().map(|(id, n)| chip(*n, f.category_id == Some(*id), Message::FilterCategory(Some(*id)))));
        col = col.push(wrap(cats));
        col = col.push(text("Статус").size(14));
        let mut sts: Vec<El> = vec![chip("Любой", f.status_id.is_none(), Message::FilterStatus(None))];
        sts.extend(api::RELEASE_STATUSES.iter().map(|(id, n)| chip(*n, f.status_id == Some(*id), Message::FilterStatus(Some(*id)))));
        col = col.push(wrap(sts));
        col = col.push(text("Сезон").size(14));
        let mut ss: Vec<El> = vec![chip("Любой", f.season.is_none(), Message::FilterSeason(None))];
        ss.extend(api::SEASONS.iter().map(|(id, n)| chip(*n, f.season == Some(*id), Message::FilterSeason(Some(*id)))));
        col = col.push(wrap(ss));
        col = col.push(
            row![
                field(self, Field::YearFrom, "Год от").width(Length::Fill),
                field(self, Field::YearTo, "Год до").width(Length::Fill),
                field(self, Field::EpFrom, "Серий от").width(Length::Fill),
                field(self, Field::EpTo, "Серий до").width(Length::Fill),
            ]
            .spacing(6),
        );
        col = col.push(text("Жанры").size(14));
        col = col.push(toggle_row("Исключить выбранные жанры", f.exclude_genres, Toggle::FilterExclude));
        col = col.push(wrap(GENRES.iter().map(|g| chip(*g, f.genres.iter().any(|x| x == g), Message::FilterGenre(g.to_string()))).collect()));
        col = col.push(row![btn("Применить", Message::ApplyFilter), ghost("Сбросить", Message::ResetFilter)].spacing(8));
        container(col).padding(12).width(Length::Fill).style(card_style).into()
    }

    // ───────────── закладки ─────────────

    fn bookmarks_view(&self) -> El<'_> {
        if !self.logged_in {
            return self.login_prompt("Закладки доступны после входа в аккаунт");
        }
        let mut chips: Vec<El> = vec![
            chip("Избранное", self.bookmarks.status == 0, Message::BookmarkStatus(0)),
            chip("История", self.bookmarks.status == -1, Message::BookmarkStatus(-1)),
        ];
        chips.extend(LIST_STATUSES.iter().map(|(id, n)| chip(*n, self.bookmarks.status == *id, Message::BookmarkStatus(*id))));
        let col = column![
            heading("Закладки"),
            wrap(chips),
            row![field(self, Field::BookmarkSearch, "Поиск по закладкам").on_submit(Message::SubmitBookmarkSearch).width(Length::Fill), btn("Найти", Message::SubmitBookmarkSearch)].spacing(6),
            self.listing_view(&self.bookmarks.listing, ListTarget::Bookmarks),
        ]
        .spacing(10);
        scrollable(container(col).padding([10, 14]).width(Length::Fill)).height(Length::Fill).into()
    }

    fn login_prompt(&self, msg: &'static str) -> El<'_> {
        container(column![muted(msg), btn("Войти", Message::Logout)].spacing(12).align_x(Alignment::Center))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    // ───────────── загрузки ─────────────

    fn downloads_view(&self) -> El<'_> {
        let mut col = column![heading("Загрузки")].spacing(10);
        for (id, d) in &self.active_downloads {
            let frac = d.progress.fraction();
            col = col.push(
                container(
                    column![
                        text(d.title.clone()).size(15),
                        iced::widget::progress_bar(0.0..=1.0, frac),
                        row![muted(format!("{:.0}% · {}", frac * 100.0, crate::platform::human_size(d.progress.bytes.load(std::sync::atomic::Ordering::Relaxed)))).width(Length::Fill), ghost("Отмена", Message::DownloadCancel(id.clone()))]
                            .align_y(Alignment::Center),
                    ]
                    .spacing(6),
                )
                .padding(12)
                .width(Length::Fill)
                .style(card_style),
            );
        }
        let records = storage::read(|s| s.downloads.clone());
        if records.is_empty() && self.active_downloads.is_empty() {
            col = col.push(muted("Скачанные серии появятся здесь. Загрузка запускается на экране серии."));
        }
        // Группировка по названию аниме.
        let mut groups: Vec<(i64, String, Vec<storage::DownloadRecord>)> = vec![];
        for r in records.into_iter().filter(|r| r.done) {
            match groups.iter_mut().find(|g| g.0 == r.release_id) {
                Some(g) => g.2.push(r),
                None => groups.push((r.release_id, r.release_title.clone(), vec![r])),
            }
        }
        for (rid, title, mut recs) in groups {
            recs.sort_by_key(|r| r.position);
            let mut g = column![row![text(title).size(17).width(Length::Fill), link("Страница", Message::OpenRelease(rid))].align_y(Alignment::Center)].spacing(6);
            for r in recs {
                g = g.push(
                    row![
                        column![text(r.episode_name.clone()).size(15), muted(crate::platform::human_size(r.size)).size(12)].width(Length::Fill),
                        ghost("Смотреть", Message::PlayLocal(r.id.clone())),
                        ghost("Удалить", Message::DownloadDelete(r.id.clone())),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                );
            }
            col = col.push(container(g).padding(12).width(Length::Fill).style(card_style));
        }
        scrollable(container(col).padding([10, 14]).width(Length::Fill)).height(Length::Fill).into()
    }

    // ───────────── профиль ─────────────

    fn profile_tab_view(&self) -> El<'_> {
        if !self.logged_in {
            return self.login_prompt("Войдите, чтобы увидеть свой профиль");
        }
        self.profile_view(0, &self.me, true)
    }

    fn profile_view<'a>(&'a self, uid: u64, p: &'a ProfilePage, tab: bool) -> El<'a> {
        let bar: El = if tab {
            row![heading("Профиль").width(Length::Fill), ghost("Настройки", Message::OpenSettings(Section::Menu))].align_y(Alignment::Center).into()
        } else {
            self.top_bar("Профиль", vec![])
        };
        let body: El = match (&p.profile, &p.error) {
            (Some(pr), _) => self.profile_body(uid, pr),
            (None, Some(e)) => column![text(format!("Ошибка: {e}")).size(14), ghost("Повторить", Message::OpenProfile(p.id))].spacing(8).into(),
            _ => muted("Загрузка…").into(),
        };
        if tab {
            column![container(bar).padding([10, 14]), scrollable(container(body).padding([4, 14]).width(Length::Fill)).height(Length::Fill)].into()
        } else {
            self.scroll_page(bar, body)
        }
    }

    fn profile_body<'a>(&'a self, uid: u64, pr: &'a Profile) -> El<'a> {
        let mut col = column![].spacing(12);
        let mut name = row![text(pr.login.clone()).size(24)].spacing(8).align_y(Alignment::Center);
        if pr.is_verified {
            name = name.push(muted("✓ подтверждён"));
        }
        if pr.is_sponsor {
            name = name.push(muted("♥ спонсор"));
        }
        col = col.push(row![self.avatar(&pr.avatar, 84.0), column![name, muted(if pr.is_online { "в сети".to_string() } else { format!("был(а) {}", format_date(pr.last_activity_time)) }), muted(format!("Регистрация: {}", format_date(pr.register_date)))].spacing(4)].spacing(14).align_y(Alignment::Center));
        if !pr.status.is_empty() {
            col = col.push(text(pr.status.clone()).size(15));
        }
        if !pr.is_my && self.logged_in {
            let (label, send) = match pr.friend_status {
                Some(2) => ("Удалить из друзей", false),
                Some(0) => ("Отменить заявку", false),
                Some(1) => ("Принять заявку", true),
                _ => ("Добавить в друзья", true),
            };
            col = col.push(ghost(label, Message::FriendAction(uid, send)));
        }
        if !pr.is_stats_hidden {
            let mut chips = vec![];
            for (i, (id, n)) in LIST_STATUSES.iter().enumerate() {
                let kind = ListKind::ProfileList { profile: if pr.is_my { 0 } else { pr.id }, status: *id };
                chips.push(ghost(format!("{n}: {}", pr.counts[i]), Message::OpenList(format!("{n} — {}", pr.login), kind)));
            }
            col = col.push(wrap(chips));
            col = col.push(muted(format!("Просмотрено серий: {} · Время: {} ч", pr.watched_episodes, pr.watched_time / 60)));
        }
        let mut links: Vec<El> = vec![
            ghost(format!("Друзья ({})", pr.friend_count), Message::OpenFriends(pr.id)),
            ghost(format!("Голоса"), Message::OpenList(format!("Оценки — {}", pr.login), ListKind::Votes(pr.id))),
        ];
        if pr.is_my {
            links.push(ghost("Избранное", Message::OpenList("Избранное".into(), ListKind::Favorites)));
            links.push(ghost("История", Message::OpenList("История".into(), ListKind::History)));
        }
        col = col.push(wrap(links));
        if !pr.is_social_hidden {
            let mut soc: Vec<El> = vec![];
            for (label, v, base) in [("VK", &pr.vk, "https://vk.com/"), ("Telegram", &pr.tg, "https://t.me/"), ("Instagram", &pr.inst, "https://instagram.com/"), ("TikTok", &pr.tt, "https://tiktok.com/@")] {
                if !v.is_empty() {
                    let url = if v.starts_with("http") { v.clone() } else { format!("{base}{}", v.trim_start_matches('@')) };
                    soc.push(ghost(label, Message::OpenUrl(url)));
                }
            }
            if !soc.is_empty() {
                col = col.push(wrap(soc));
            }
        }
        if !pr.history.is_empty() {
            col = col.push(heading("Недавно смотрел(а)").size(18));
            let mut r = row![].spacing(10);
            for rel in &pr.history {
                r = r.push(self.poster_card(rel));
            }
            col = col.push(scroll_h(r.into()));
        }
        if !pr.friends_preview.is_empty() {
            let mut r = row![].spacing(10);
            for f in &pr.friends_preview {
                r = r.push(button(column![self.avatar(&f.avatar, 52.0), text(f.login.clone()).size(12).width(60)].spacing(3).align_x(Alignment::Center)).padding(0).style(button::text).on_press(Message::OpenProfile(f.id)));
            }
            col = col.push(scroll_h(r.into()));
        }
        col.into()
    }

    fn friends_view<'a>(&'a self, uid: u64, f: &'a FriendsPage) -> El<'a> {
        let mut col = column![].spacing(8);
        for p in &f.items {
            col = col.push(self.profile_row(p));
        }
        if f.loading {
            col = col.push(muted("Загрузка…"));
        } else if f.items.is_empty() {
            col = col.push(muted("Список пуст"));
        } else if !f.last {
            col = col.push(ghost("Загрузить ещё", Message::FriendsMore(uid)));
        }
        self.scroll_page(self.top_bar("Друзья", vec![]), col.into())
    }

    // ───────────── релиз ─────────────

    fn release_view<'a>(&'a self, uid: u64, p: &'a ReleasePage) -> El<'a> {
        let Some(r) = &p.release else {
            let body: El = match &p.error {
                Some(e) => column![text(format!("Ошибка: {e}")).size(14), ghost("Повторить", Message::OpenRelease(p.id))].spacing(8).into(),
                None => muted("Загрузка…").into(),
            };
            return self.scroll_page(self.top_bar("Релиз", vec![]), body);
        };
        let mut col = column![].spacing(12);
        let mut info = column![text(r.title_ru.clone()).size(22)].spacing(4).width(Length::Fill);
        if !r.title_original.is_empty() {
            info = info.push(muted(r.title_original.clone()));
        }
        let line = [r.year.clone(), r.category.clone(), r.status.clone(), r.country.clone()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
        info = info.push(muted(line));
        info = info.push(muted(r.episodes_label()));
        if r.duration > 0 {
            info = info.push(muted(format!("~{} мин. · {}+", r.duration, r.age_rating)));
        }
        if r.grade > 0.0 {
            info = info.push(text(format!("★ {:.2}  ({} оценок)", r.grade, r.vote_count)).size(15));
        }
        let poster: El = match r.screenshots.first().map(|_| ()) {
            _ => button(self.pic(&r.image, 130.0, 190.0)).padding(0).style(button::text).on_press(Message::OpenGallery(vec![r.image.clone()], 0)).into(),
        };
        col = col.push(row![poster, info].spacing(14));
        if !r.restriction_note.is_empty() {
            col = col.push(text(r.restriction_note.clone()).size(13).color(Color::from_rgb8(0xF5, 0xB0, 0x31)));
        }
        // Действия
        let play_label = if r.last_episode_name.is_empty() { "Смотреть".to_string() } else { format!("Продолжить: {}", r.last_episode_name) };
        let watch: El = if r.is_play_disabled || r.is_view_blocked {
            muted("Просмотр недоступен").into()
        } else {
            button(text(play_label).size(16)).padding([11, 20]).on_press(Message::OpenWatch(Box::new(r.clone()))).style(button::primary).into()
        };
        col = col.push(watch);
        let list_label = if r.list_status > 0 { status_name(r.list_status).to_string() } else { "В список".to_string() };
        col = col.push(
            row![
                ghost(list_label, Message::ToggleStatusMenu(uid)),
                ghost(if r.is_favorite { "★ В избранном" } else { "☆ В избранное" }, Message::ToggleFavorite(uid)),
                ghost(format!("Комментарии ({})", r.comment_count), Message::OpenComments(CommentKind::Release, r.id)),
            ]
            .spacing(6),
        );
        if p.status_menu {
            let mut chips: Vec<El> = LIST_STATUSES.iter().map(|(id, n)| chip(*n, r.list_status == *id, Message::SetListStatus(uid, *id))).collect();
            if r.list_status > 0 {
                chips.push(chip("Убрать из списка", false, Message::SetListStatus(uid, 0)));
            }
            col = col.push(container(wrap(chips)).padding(10).width(Length::Fill).style(card_style));
        }
        // Оценка
        let stars: Vec<El> = (1..=5).map(|i| chip(format!("{}{}", i, if r.my_vote >= i { " ★" } else { " ☆" }), r.my_vote == i, Message::Vote(uid, i))).collect();
        col = col.push(column![muted("Ваша оценка"), row(stars).spacing(4)].spacing(4));
        // Описание
        if !r.description.is_empty() {
            let d: String = if p.expanded || r.description.chars().count() < 300 { r.description.clone() } else { format!("{}…", r.description.chars().take(300).collect::<String>()) };
            col = col.push(column![text(d).size(14), if r.description.chars().count() >= 300 { link(if p.expanded { "Свернуть" } else { "Читать полностью" }, Message::ToggleDescription(uid)) } else { Space::new().into() }].spacing(2));
        }
        // Свойства
        let mut props = column![].spacing(2);
        for (k, v) in [("Жанры", &r.genres), ("Студия", &r.studio), ("Режиссёр", &r.director), ("Автор", &r.author), ("Источник", &r.source), ("Озвучка", &r.translators)] {
            if !v.is_empty() {
                props = props.push(row![muted(format!("{k}: ")), text(v.clone()).size(13)]);
            }
        }
        col = col.push(props);
        col = col.push(muted(format!(
            "Смотрят: {} · В планах: {} · Просмотрено: {} · Отложено: {} · Брошено: {}",
            r.counts[0], r.counts[1], r.counts[2], r.counts[3], r.counts[4]
        )).size(12));
        // Видео
        if !r.videos.is_empty() {
            col = col.push(row![heading("Видео").size(18).width(Length::Fill), link("Показать все ›", Message::OpenVideos(r.videos.clone()))].align_y(Alignment::Center));
            let mut rr = row![].spacing(10);
            for v in r.videos.iter().take(6) {
                rr = rr.push(button(column![self.pic(&v.image, 200.0, 112.0), text(v.title.clone()).size(12).width(200)].spacing(3)).padding(0).style(button::text).on_press(Message::OpenUrl(v.url.clone())));
            }
            col = col.push(scroll_h(rr.into()));
        }
        // Скриншоты
        if !r.screenshots.is_empty() {
            col = col.push(heading("Кадры").size(18));
            let mut rr = row![].spacing(10);
            for (i, u) in r.screenshots.iter().enumerate() {
                rr = rr.push(button(self.pic(u, 200.0, 112.0)).padding(0).style(button::text).on_press(Message::OpenGallery(r.screenshots.clone(), i)));
            }
            col = col.push(scroll_h(rr.into()));
        }
        if r.related_count > 0 && r.related_id > 0 {
            col = col.push(ghost(format!("{} ({})", if r.related_name.is_empty() { "Связанные релизы" } else { &r.related_name }, r.related_count), Message::OpenList("Связанные релизы".into(), ListKind::Related(r.related_id))));
        }
        for (title, list) in [("Связанные", &r.related_releases), ("Рекомендации", &r.recommended)] {
            if !list.is_empty() {
                col = col.push(heading(title).size(18));
                let mut rr = row![].spacing(10);
                for rel in list.iter() {
                    rr = rr.push(self.poster_card(rel));
                }
                col = col.push(scroll_h(rr.into()));
            }
        }
        let share = format!("{} — Anixart #{}", r.title(), r.id);
        let bar = self.top_bar(r.title(), vec![link("Копировать", Message::CopyText(share))]);
        self.scroll_page(bar, col.into())
    }
}
