//! Экраны: выбор серии, плеер, комментарии, коллекции, галерея, расписание, настройки и Shikimori.

use crate::api::CommentKind;
use crate::app::*;
use crate::aniskip::marker_label;
use crate::models::*;
use crate::platform;
use crate::storage;
use crate::update;
use crate::views::*;
use iced::widget::{button, column, container, progress_bar, row, scrollable, text, Space};
use iced::{Alignment, Color, ContentFit, Length, Theme};

const DANGER: Color = Color { r: 0.90, g: 0.28, b: 0.30, a: 1.0 };

impl App {
    // ───────────── выбор серии ─────────────

    pub(crate) fn chooser_view<'a>(&'a self, uid: u64, c: &'a ChooserPage) -> El<'a> {
        let mut col = column![].spacing(8);
        let step = if c.source_sel.is_some() {
            "Серии"
        } else if c.type_sel.is_some() {
            "Источник"
        } else {
            "Озвучка"
        };
        col = col.push(muted(format!("{} · {}", c.release.title(), step)));
        if let Some(e) = &c.error {
            col = col.push(text(format!("Ошибка: {e}")).size(14).color(DANGER));
        }
        if c.loading {
            col = col.push(muted("Загрузка…"));
        }
        if let Some(src) = &c.source_sel {
            col = col.push(muted(format!("{} · {}", c.type_sel.as_ref().map(|t| t.name.clone()).unwrap_or_default(), src.name)));
            let local: Vec<i64> = c.episodes.iter().filter(|e| storage::read(|s| s.positions.contains_key(&storage::position_key(c.release.id, src.id, e.position)))).map(|e| e.position).collect();
            for (i, e) in c.episodes.iter().enumerate() {
                let mut label = row![text(e.name.clone()).size(16).width(Length::Fill)].align_y(Alignment::Center).spacing(8);
                if e.is_filler {
                    label = label.push(muted("филлер").size(12));
                }
                if local.contains(&e.position) {
                    label = label.push(muted("есть позиция").size(12));
                }
                if e.is_watched {
                    label = label.push(muted("✓").size(16));
                }
                col = col.push(
                    button(container(label).padding([12, 14]).width(Length::Fill).style(card_style))
                        .padding(0)
                        .width(Length::Fill)
                        .style(button::text)
                        .on_press(Message::OpenEpisode(uid, i)),
                );
            }
        } else if c.type_sel.is_some() {
            for s in &c.sources {
                col = col.push(
                    button(container(row![text(s.name.clone()).size(16).width(Length::Fill), muted(format!("{} эп.", s.episodes_count))].align_y(Alignment::Center)).padding([12, 14]).width(Length::Fill).style(card_style))
                        .padding(0)
                        .width(Length::Fill)
                        .style(button::text)
                        .on_press(Message::ChooseSource(uid, s.clone())),
                );
            }
        } else {
            let mut types: Vec<&VoiceType> = c.types.iter().collect();
            types.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.view_count.cmp(&a.view_count)));
            for t in types {
                let mut name_row = row![text(t.name.clone()).size(16)].spacing(8);
                if t.is_sub {
                    name_row = name_row.push(muted("субтитры"));
                }
                let mut info = column![name_row].spacing(2).width(Length::Fill);
                if !t.workers.is_empty() {
                    info = info.push(muted(t.workers.clone()).size(12));
                }
                col = col.push(
                    button(container(row![info, muted(format!("{} эп.", t.episodes_count))].align_y(Alignment::Center)).padding([12, 14]).width(Length::Fill).style(card_style))
                        .padding(0)
                        .width(Length::Fill)
                        .style(button::text)
                        .on_press(Message::ChooseType(uid, t.clone())),
                );
            }
        }
        let bar = row![
            button(text("‹").size(26)).padding([0, 12]).on_press(Message::ChooserBack(uid)).style(button::text),
            text("Выбор серии").size(20).width(Length::Fill),
            link("Закрыть", Message::Back),
        ]
        .align_y(Alignment::Center);
        column![bar, scrollable(container(col).padding([4, 14]).width(Length::Fill)).height(Length::Fill)].spacing(6).into()
    }

    // ───────────── плеер ─────────────

    pub(crate) fn player_view<'a>(&'a self, uid: u64, p: &'a PlayerPage) -> El<'a> {
        let ep = p.episodes.get(p.index);
        let mut col = column![].spacing(12);
        col = col.push(text(p.release.title()).size(20));
        col = col.push(text(ep.map(|e| e.name.clone()).unwrap_or_default()).size(17).color(parse_hex(&self.settings.accent).unwrap_or(Color::WHITE)));
        if p.loading {
            col = col.push(muted("Определяем поток…"));
        }
        if let Some(e) = &p.error {
            col = col.push(text(e.clone()).size(14).color(DANGER));
            if !p.embed_url.is_empty() {
                col = col.push(row![ghost("Открыть в браузере", Message::PlayerOpenEmbed(uid)), ghost("Копировать ссылку", Message::CopyText(p.embed_url.clone()))].spacing(6));
            }
        }
        if !p.status.is_empty() {
            col = col.push(muted(p.status.clone()));
        }
        if let Some(st) = &p.stream {
            let resume = ep.map(|e| crate::player::remembered_position(p.release.id, p.source_id, e.position)).unwrap_or(0.0);
            let mut play_label = "▶ Смотреть".to_string();
            if self.settings.remember_position && resume > 5.0 {
                play_label = format!("▶ Продолжить с {}", format_clock(resume));
            }
            let play: El = if p.playing {
                button(text("Воспроизводится…").size(16)).padding([12, 22]).style(button::secondary).into()
            } else {
                button(text(play_label).size(16)).padding([12, 22]).on_press(Message::PlayerPlay(uid)).style(button::primary).into()
            };
            col = col.push(play);
            // Качество
            if !st.variants.is_empty() {
                let mut q: Vec<El> = vec![chip("Авто", p.variant.is_none(), Message::PlayerVariant(uid, None))];
                q.extend(st.variants.iter().enumerate().map(|(i, v)| chip(v.label(), p.variant == Some(i), Message::PlayerVariant(uid, Some(i)))));
                col = col.push(column![muted("Качество"), wrap(q)].spacing(4));
            }
            // Скорость
            let speeds = [0.5f32, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
            col = col.push(column![muted("Скорость"), wrap(speeds.iter().map(|s| chip(format!("{s}x"), (p.speed - s).abs() < 0.01, Message::PlayerSpeed(uid, *s))).collect())].spacing(4));
            // Субтитры
            if !st.subtitles.is_empty() {
                let mut s: Vec<El> = vec![chip("Выкл.", p.subtitle.is_none(), Message::PlayerSubtitle(uid, None))];
                s.extend(st.subtitles.iter().enumerate().map(|(i, t)| chip(t.label.clone(), p.subtitle == Some(i), Message::PlayerSubtitle(uid, Some(i)))));
                col = col.push(column![muted("Субтитры"), wrap(s)].spacing(4));
            }
            // AniSkip
            if self.settings.aniskip_enabled {
                if p.markers.is_empty() {
                    col = col.push(muted("AniSkip: меток для этой серии нет (или они ещё ищутся)"));
                } else {
                    let list: Vec<String> = p.markers.iter().map(|m| format!("{}: {} – {}", marker_label(&m.name), format_clock(m.start), format_clock(m.end))).collect();
                    col = col.push(column![muted(if self.settings.aniskip_auto { "AniSkip (автопропуск включён)" } else { "AniSkip" }), text(list.join("\n")).size(13)].spacing(2));
                }
            }
            // Внешние плееры
            let url = match p.variant.and_then(|i| st.variants.get(i)) {
                Some(v) => v.url.clone(),
                None => st.url.clone(),
            };
            let mut ext: Vec<El> = vec![];
            for (id, name) in [("default", if platform::is_mobile() { "Системный" } else { "mpv / VLC" }), ("vlc", "VLC"), ("outplayer", "Outplayer"), ("nplayer", "nPlayer")] {
                ext.push(chip(name, self.settings.player == id, Message::SetPlayer(id.into())));
            }
            col = col.push(column![muted("Плеер"), wrap(ext)].spacing(4));
            col = col.push(row![ghost("Поделиться ссылкой", Message::CopyText(url.clone())), ghost("Открыть в браузере", Message::OpenUrl(url))].spacing(6));
            col = col.push(ghost("⬇ Скачать серию", Message::DownloadStart(uid)));
        }
        // Навигация
        let mut nav = row![].spacing(6);
        if p.index > 0 {
            nav = nav.push(ghost("‹ Предыдущая", Message::PlayerPrev(uid)));
        }
        nav = nav.push(ghost("✓ Отметить просмотренной", Message::PlayerMarkWatched(uid)));
        if p.index + 1 < p.episodes.len() {
            nav = nav.push(ghost("Следующая ›", Message::PlayerNext(uid)));
        }
        col = col.push(nav);
        col = col.push(muted(if platform::is_mobile() { "Видео открывается в выбранном внешнем приложении." } else { "Видео открывается в mpv (авто-пропуск OP/ED и сохранение позиции) или VLC." }).size(12));
        self.scroll_page_pub(self.top_bar("Плеер", vec![]), col.into())
    }

    pub(crate) fn scroll_page_pub<'a>(&'a self, bar: El<'a>, body: El<'a>) -> El<'a> {
        column![bar, scrollable(container(body).padding([4, 14]).width(Length::Fill)).height(Length::Fill)].spacing(6).into()
    }

    // ───────────── комментарии ─────────────

    fn comment_card<'a>(&'a self, uid: u64, c: &'a CommentsPage, cm: &'a Comment, reply: bool) -> El<'a> {
        let mut head = row![self.avatar(&cm.author.avatar, 32.0), column![text(cm.author.login.clone()).size(14), muted(format!("{}{}", format_date(cm.timestamp), if cm.is_edited { " · изменён" } else { "" })).size(11)].width(Length::Fill)]
            .spacing(8)
            .align_y(Alignment::Center);
        if cm.author.id > 0 && cm.author.id == self.me.id {
            head = head.push(link("Удалить", Message::DeleteComment(uid, cm.id)));
        }
        let body: El = if cm.is_deleted {
            muted("Комментарий удалён").into()
        } else if cm.is_spoiler && !c.revealed.contains(&cm.id) {
            button(text("Спойлер — нажмите, чтобы показать").size(13)).on_press(Message::RevealSpoiler(uid, cm.id)).style(button::secondary).into()
        } else {
            text(cm.message.clone()).size(14).into()
        };
        let like_style = |v: i64| if cm.vote == v { button::primary } else { button::text };
        let mut actions = row![
            button(text("▲").size(13)).padding([4, 8]).on_press(Message::VoteComment(uid, cm.id, 1)).style(like_style(1)),
            text(format!("{}", cm.vote_count)).size(13),
            button(text("▼").size(13)).padding([4, 8]).on_press(Message::VoteComment(uid, cm.id, 2)).style(like_style(2)),
            link("Ответить", Message::ReplyTo(uid, cm.id, cm.author.id, cm.author.login.clone())),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        if !reply && cm.reply_count > 0 {
            actions = actions.push(link(if c.replies.contains_key(&cm.id) { "Скрыть ответы".to_string() } else { format!("Ответы ({})", cm.reply_count) }, Message::LoadReplies(uid, cm.id)));
        }
        container(column![head, body, actions].spacing(6)).padding(10).width(Length::Fill).style(card_style).into()
    }

    pub(crate) fn comments_view<'a>(&'a self, uid: u64, c: &'a CommentsPage) -> El<'a> {
        let mut col = column![].spacing(8);
        col = col.push(row![chip("Новые", c.sort == 1, Message::CommentsSort(uid, 1)), chip("Популярные", c.sort == 2, Message::CommentsSort(uid, 2)), chip("Старые", c.sort == 3, Message::CommentsSort(uid, 3))].spacing(6));
        for cm in &c.comments {
            col = col.push(self.comment_card(uid, c, cm, false));
            if let Some(rs) = c.replies.get(&cm.id) {
                for r in rs {
                    col = col.push(container(self.comment_card(uid, c, r, true)).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 22.0 }));
                }
            }
        }
        if c.loading {
            col = col.push(muted("Загрузка…"));
        } else if let Some(e) = &c.error {
            col = col.push(text(e.clone()).size(14).color(DANGER));
        } else if c.comments.is_empty() {
            col = col.push(muted("Комментариев пока нет"));
        } else if !c.last {
            col = col.push(ghost("Загрузить ещё", Message::CommentsMore(uid)));
        }
        let mut composer = column![].spacing(6);
        if let Some((_, _, login)) = &c.reply_to {
            composer = composer.push(row![muted(format!("Ответ для {login}")).width(Length::Fill), link("Отмена", Message::CancelReply(uid))]);
        }
        composer = composer.push(
            row![
                field(self, Field::CommentText, "Написать комментарий…").on_submit(Message::SendComment(uid)).width(Length::Fill),
                btn("Отправить", Message::SendComment(uid)),
            ]
            .spacing(6),
        );
        composer = composer.push(toggle_row("Спойлер", self.toggle(Toggle::CommentSpoiler), Toggle::CommentSpoiler));
        let bar = self.top_bar("Комментарии", vec![]);
        column![bar, scrollable(container(col).padding([4, 14]).width(Length::Fill)).height(Length::Fill), container(composer).padding(10).style(card_style)].spacing(6).into()
    }

    // ───────────── коллекции ─────────────

    fn collection_card<'a>(&'a self, c: &Collection) -> El<'a> {
        button(
            container(
                row![
                    self.pic(&c.image, 90.0, 90.0),
                    column![text(c.title.clone()).size(16), muted(format!("{} · релизов: {} · ♥ {}", c.creator.login, c.release_count, c.favorites_count)).size(12), muted(c.description.chars().take(90).collect::<String>()).size(12)]
                        .spacing(3)
                        .width(Length::Fill)
                ]
                .spacing(12),
            )
            .padding(8)
            .width(Length::Fill)
            .style(card_style),
        )
        .padding(0)
        .width(Length::Fill)
        .style(button::text)
        .on_press(Message::OpenCollection(c.id))
        .into()
    }

    pub(crate) fn collections_view<'a>(&'a self, uid: u64, c: &'a CollectionsPage) -> El<'a> {
        let mut col = column![row![field(self, Field::CollectionSearch, "Поиск коллекций").on_submit(Message::SubmitCollectionSearch(uid)).width(Length::Fill), btn("Найти", Message::SubmitCollectionSearch(uid))].spacing(6)].spacing(8);
        for it in &c.items {
            col = col.push(self.collection_card(it));
        }
        if c.loading {
            col = col.push(muted("Загрузка…"));
        } else if c.items.is_empty() {
            col = col.push(muted("Ничего не найдено"));
        } else if !c.last {
            col = col.push(ghost("Загрузить ещё", Message::CollectionsMore(uid)));
        }
        self.scroll_page_pub(self.top_bar("Коллекции", vec![]), col.into())
    }

    pub(crate) fn collection_view<'a>(&'a self, uid: u64, c: &'a CollectionPage) -> El<'a> {
        let mut col = column![].spacing(10);
        match &c.collection {
            Some(cl) => {
                col = col.push(row![self.pic(&cl.image, 110.0, 110.0), column![text(cl.title.clone()).size(20), button(row![self.avatar(&cl.creator.avatar, 24.0), text(cl.creator.login.clone()).size(14)].spacing(6).align_y(Alignment::Center)).padding(0).style(button::text).on_press(Message::OpenProfile(cl.creator.id)), muted(format!("Релизов: {} · В избранном: {}", cl.release_count, cl.favorites_count))].spacing(4)].spacing(12));
                if !cl.description.is_empty() {
                    col = col.push(text(cl.description.clone()).size(14));
                }
                col = col.push(row![ghost(if cl.is_favorite { "★ В избранном" } else { "☆ В избранное" }, Message::ToggleCollectionFavorite(uid)), ghost(format!("Комментарии ({})", cl.comment_count), Message::OpenComments(CommentKind::Collection, cl.id))].spacing(6));
            }
            None => {
                col = col.push(match &c.error {
                    Some(e) => text(format!("Ошибка: {e}")).size(14).color(DANGER),
                    None => muted("Загрузка…").into(),
                });
            }
        }
        col = col.push(self.listing_view(&c.listing, ListTarget::Page(uid)));
        self.scroll_page_pub(self.top_bar("Коллекция", vec![]), col.into())
    }

    // ───────────── галерея, видео, расписание ─────────────

    pub(crate) fn gallery_view<'a>(&'a self, urls: &'a [String], index: usize) -> El<'a> {
        let url = urls.get(index).cloned().unwrap_or_default();
        let img: El = match self.image(&url) {
            Some(h) => iced::widget::image(h.clone()).content_fit(ContentFit::Contain).width(Length::Fill).height(Length::Fill).into(),
            None => container(muted("Загрузка…")).center_x(Length::Fill).center_y(Length::Fill).width(Length::Fill).height(Length::Fill).into(),
        };
        let nav = row![
            ghost("‹", Message::GalleryMove(-1)),
            text(format!("{} / {}", index + 1, urls.len())).size(14),
            ghost("›", Message::GalleryMove(1)),
            ghost("Сохранить", Message::SaveImage(url.clone())),
            ghost("Копировать ссылку", Message::CopyText(url.clone())),
            ghost("Открыть", Message::OpenUrl(url)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        column![self.top_bar("Галерея", vec![]), img, container(nav).padding(10).center_x(Length::Fill)].spacing(6).into()
    }

    pub(crate) fn videos_view<'a>(&'a self, videos: &'a [VideoBanner]) -> El<'a> {
        let mut col = column![].spacing(8);
        let mut cats: Vec<String> = vec![];
        for v in videos {
            if !cats.contains(&v.category) {
                cats.push(v.category.clone());
            }
        }
        for cat in cats {
            col = col.push(heading(if cat.is_empty() { "Видео".to_string() } else { cat.clone() }).size(18));
            for v in videos.iter().filter(|v| v.category == cat) {
                col = col.push(
                    container(
                        row![
                            self.pic(&v.image, 120.0, 68.0),
                            column![text(v.title.clone()).size(14), muted(v.hosting.clone()).size(12), row![ghost("Открыть", Message::OpenUrl(v.url.clone())), ghost("Копировать ссылку", Message::CopyText(v.url.clone())), ghost("Превью", Message::OpenGallery(vec![v.image.clone()], 0))].spacing(6)].spacing(4).width(Length::Fill)
                        ]
                        .spacing(10),
                    )
                    .padding(8)
                    .width(Length::Fill)
                    .style(card_style),
                );
            }
        }
        self.scroll_page_pub(self.top_bar("Видео", vec![]), col.into())
    }

    pub(crate) fn schedule_view<'a>(&'a self, uid: u64, s: &'a SchedulePage) -> El<'a> {
        let mut col = column![].spacing(8);
        if s.loading {
            col = col.push(muted("Загрузка…"));
        }
        if let Some(e) = &s.error {
            col = col.push(text(e.clone()).size(14).color(DANGER));
        }
        if !s.days.is_empty() {
            col = col.push(wrap(s.days.iter().enumerate().map(|(i, (n, v))| chip(format!("{} ({})", n, v.len()), s.day == i, Message::ScheduleDay(uid, i))).collect()));
            if let Some((_, list)) = s.days.get(s.day) {
                for r in list {
                    col = col.push(self.release_row(r));
                }
                if list.is_empty() {
                    col = col.push(muted("В этот день выходов нет"));
                }
            }
        }
        self.scroll_page_pub(self.top_bar("Расписание", vec![]), col.into())
    }

    // ───────────── настройки ─────────────

    fn menu_item<'a>(&'a self, label: &'a str, m: Message) -> El<'a> {
        button(container(row![text(label).size(16).width(Length::Fill), text("›").size(18)].align_y(Alignment::Center)).padding([13, 14]).width(Length::Fill).style(card_style))
            .padding(0)
            .width(Length::Fill)
            .style(button::text)
            .on_press(m)
            .into()
    }

    pub(crate) fn settings_view<'a>(&'a self, sec: Section) -> El<'a> {
        let mut col = column![].spacing(10);
        let title = match sec {
            Section::Menu => "Настройки",
            Section::Appearance => "Внешний вид",
            Section::Player => "Плеер",
            Section::AniSkip => "AniSkip",
            Section::Notifications => "Уведомления",
            Section::Profile => "Профиль",
            Section::Security => "Безопасность",
            Section::Data => "Данные",
            Section::Connection => "Подключение",
            Section::Updates => "Обновления",
            Section::About => "О приложении",
        };
        match sec {
            Section::Notifications => {}
            Section::Menu => {
                col = col
                    .push(self.menu_item("Внешний вид", Message::OpenSettings(Section::Appearance)))
                    .push(self.menu_item("Плеер", Message::OpenSettings(Section::Player)))
                    .push(self.menu_item("AniSkip", Message::OpenSettings(Section::AniSkip)))
                    .push(self.menu_item("Уведомления", Message::OpenNotifications))
                    .push(self.menu_item("Профиль", Message::OpenSettings(Section::Profile)))
                    .push(self.menu_item("Безопасность", Message::OpenSettings(Section::Security)))
                    .push(self.menu_item("Данные", Message::OpenSettings(Section::Data)))
                    .push(self.menu_item("Подключение", Message::OpenSettings(Section::Connection)))
                    .push(self.menu_item("Shikimori", Message::OpenShikimori))
                    .push(self.menu_item("Обновления", Message::OpenSettings(Section::Updates)))
                    .push(self.menu_item("О приложении", Message::OpenSettings(Section::About)))
                    .push(self.menu_item("Сообщить о проблеме", Message::OpenUrl("https://github.com/vilefex/AniLot/issues".into())))
                    .push(ghost("Выйти из аккаунта", Message::Logout));
            }
            Section::Appearance => {
                col = col.push(muted("Тема"));
                col = col.push(row![chip("Системная", self.settings.theme == "system", Message::SetTheme("system".into())), chip("Светлая", self.settings.theme == "light", Message::SetTheme("light".into())), chip("Тёмная", self.settings.theme == "dark", Message::SetTheme("dark".into()))].spacing(6));
                col = col.push(muted("Акцентный цвет"));
                let presets = [("Оранжевый", "#FF8A00"), ("Красный", "#E5484D"), ("Розовый", "#E5489C"), ("Фиолетовый", "#8E4EC6"), ("Синий", "#3E63DD"), ("Бирюзовый", "#12A594"), ("Зелёный", "#30A46C"), ("Жёлтый", "#F5C400")];
                col = col.push(wrap(presets.iter().map(|(n, h)| chip(*n, self.settings.accent.eq_ignore_ascii_case(h), Message::SetAccent(h.to_string()))).collect()));
                col = col.push(row![field(self, Field::AccentHex, "#RRGGBB").on_submit(Message::ApplyAccentHex).width(Length::Fill), btn("Применить", Message::ApplyAccentHex)].spacing(6));
            }
            Section::Player => {
                col = col.push(muted("Плеер"));
                let mut ps: Vec<El> = vec![];
                for (id, name) in [("default", if platform::is_mobile() { "Системный" } else { "mpv / VLC" }), ("vlc", "VLC"), ("outplayer", "Outplayer"), ("nplayer", "nPlayer")] {
                    ps.push(chip(name, self.settings.player == id, Message::SetPlayer(id.into())));
                }
                col = col.push(wrap(ps));
                col = col.push(toggle_row("Запоминать источник", self.settings.remember_source, Toggle::RememberSource));
                col = col.push(toggle_row("Запоминать позицию просмотра", self.settings.remember_position, Toggle::RememberPosition));
                col = col.push(toggle_row("Автоматически следующая серия", self.settings.auto_next, Toggle::AutoNext));
                col = col.push(muted("Скорость по умолчанию"));
                let speeds = [0.5f32, 0.75, 1.0, 1.25, 1.5, 2.0];
                col = col.push(wrap(speeds.iter().map(|s| chip(format!("{s}x"), (self.settings.speed - s).abs() < 0.01, Message::PlayerSpeed(0, *s))).collect()));
                if !platform::is_mobile() {
                    col = col.push(muted("Путь к mpv (необязательно)"));
                    col = col.push(field(self, Field::MpvPath, "mpv"));
                }
            }
            Section::AniSkip => {
                col = col.push(toggle_row("Искать метки AniSkip", self.settings.aniskip_enabled, Toggle::AniskipEnabled));
                col = col.push(toggle_row("Автоматически пропускать OP/ED", self.settings.aniskip_auto, Toggle::AniskipAuto));
                col = col.push(muted("Метки находятся по MAL ID, который подбирается через Shikimori. Найденные интервалы кэшируются локально и работают без сети (для уже получённых серий). Автопропуск работает в mpv; в других плеерах метки показываются на экране серии."));
                let n = storage::read(|s| (s.skip_markers.len(), s.mal_ids.len()));
                col = col.push(muted(format!("В кэше: меток — {}, соответствий MAL — {}", n.0, n.1)));
                col = col.push(ghost("Очистить кэш", Message::ClearCaches));
            }
            Section::Profile => {
                col = col.push(muted("Статус"));
                col = col.push(row![field(self, Field::StatusText, "О себе").on_submit(Message::SaveStatus).width(Length::Fill), btn("Сохранить", Message::SaveStatus)].spacing(6));
                col = col.push(muted("Социальные сети"));
                for (f, p) in [(Field::Vk, "VK"), (Field::Tg, "Telegram"), (Field::Inst, "Instagram"), (Field::Tt, "TikTok"), (Field::Discord, "Discord")] {
                    col = col.push(field(self, f, p));
                }
                col = col.push(btn("Сохранить ссылки", Message::SaveSocial));
            }
            Section::Security => {
                col = col.push(muted("Смена пароля"));
                col = col.push(secret(self, Field::CurPass, "Текущий пароль"));
                col = col.push(secret(self, Field::NewPass, "Новый пароль"));
                col = col.push(btn("Сменить пароль", Message::ChangePassword));
                col = col.push(muted("Смена логина"));
                col = col.push(field(self, Field::NewLogin, "Новый логин"));
                col = col.push(btn("Сменить логин", Message::ChangeLogin));
                col = col.push(muted("Смена почты (используется «Текущий пароль» выше)"));
                col = col.push(field(self, Field::NewEmail, "Новая почта"));
                col = col.push(btn("Сменить почту", Message::ChangeEmail));
                col = col.push(muted("Приватность"));
                col = col.push(ghost("Переключить режим инкогнито", Message::ToggleIncognito));
            }
            Section::Data => {
                col = col.push(ghost("Очистить кэш (AniSkip, MAL, позиции, картинки)", Message::ClearCaches));
                col = col.push(ghost("Очистить историю поиска", Message::ClearSearchHistory));
                col = col.push(ghost("Удалить все загрузки", Message::ClearDownloads));
                col = col.push(muted(format!("Каталог данных: {}", platform::data_dir().display())).size(12));
            }
            Section::Connection => {
                col = col.push(toggle_row("Альтернативное подключение", self.settings.alt_connection, Toggle::AltConnection));
                col = col.push(muted("Собственный адрес API (необязательно)"));
                col = col.push(field(self, Field::CustomApi, "https://…"));
                col = col.push(muted("User-Agent"));
                col = col.push(field(self, Field::UserAgent, "AnixartApp/…"));
                col = col.push(btn("Применить", Message::ApplyConnection));
            }
            Section::Updates => {
                col = col.push(text(format!("Установлена версия: v{} ({})", update::app_build(), update::APP_VERSION)).size(16));
                let last = self.settings.last_update_check;
                col = col.push(muted(if last > 0 { format!("Последняя проверка: {}", format_date(last)) } else { "Проверка ещё не выполнялась".to_string() }));
                col = col.push(btn("Проверить обновления", Message::CheckUpdate));
                if let Some(u) = &self.update {
                    col = col.push(text(if u.is_newer { format!("Доступна новая версия: {}", u.name) } else { format!("Установлена актуальная версия ({})", u.name) }).size(15));
                    if !u.notes.is_empty() {
                        col = col.push(container(text(u.notes.clone()).size(13)).padding(10).width(Length::Fill).style(card_style));
                    }
                    let mut links: Vec<El> = vec![];
                    if !u.page_url.is_empty() {
                        links.push(ghost("Страница релиза", Message::OpenUrl(u.page_url.clone())));
                    }
                    if !u.asset_url.is_empty() {
                        links.push(ghost(format!("Скачать {}", u.asset_name), Message::OpenUrl(u.asset_url.clone())));
                    }
                    col = col.push(wrap(links));
                }
                col = col.push(muted("Приложение только сообщает о новой версии и не устанавливает её автоматически."));
            }
            Section::About => {
                col = col.push(text("AniLot").size(26));
                col = col.push(text(format!("Версия v{} · Rust + Iced", update::app_build())).size(14));
                col = col.push(muted("Неофициальный клиент Anixart с расширенным плеером, AniSkip и синхронизацией Shikimori. Не связан с разработчиками Anixart и Shikimori."));
                col = col.push(wrap(vec![ghost("Репозиторий", Message::OpenUrl("https://github.com/vilefex/AniLot".into())), ghost("Релизы", Message::OpenUrl("https://github.com/vilefex/AniLot/releases".into())), ghost("Сообщить об ошибке", Message::OpenUrl("https://github.com/vilefex/AniLot/issues".into()))]));
            }
        }
        self.scroll_page_pub(self.top_bar(title, vec![]), col.into())
    }

    pub(crate) fn notifications_view<'a>(&'a self, v: Option<&'a serde_json::Value>) -> El<'a> {
        let mut col = column![].spacing(10);
        match v {
            None => col = col.push(muted("Загрузка…")),
            Some(v) => {
                let items = [
                    ("Новые серии", "is_episode_notifications_enabled", "episode"),
                    ("Первая серия", "is_first_episode_notification_enabled", "first"),
                    ("Комментарии", "is_comment_notifications_enabled", "comment"),
                    ("Связанные релизы", "is_related_release_notifications_enabled", "related"),
                    ("Статьи", "is_article_notifications_enabled", "article"),
                ];
                for (label, key, which) in items {
                    let on = b(v, key);
                    col = col.push(iced::widget::toggler(on).label(label).on_toggle(move |_| Message::ServerNotify(which)).size(22));
                }
                col = col.push(muted("Настройки хранятся на сервере Anixart и применяются к push-уведомлениям официального сервиса."));
            }
        }
        self.scroll_page_pub(self.top_bar("Уведомления", vec![]), col.into())
    }

    // ───────────── Shikimori ─────────────

    pub(crate) fn shikimori_view(&self) -> El<'_> {
        let mut col = column![].spacing(10);
        let (authorized, nick, last_sync) = storage::read(|s| (!s.shiki.access_token.is_empty(), s.shiki.nickname.clone(), s.last_sync));
        col = col.push(muted("Двусторонняя синхронизация списков и прогресса просмотра. Для обеих операций нужен вход в аккаунт Anixart."));
        col = col.push(heading("Аккаунт Shikimori").size(18));
        if authorized {
            col = col.push(text(format!("Подключён: {nick}")).size(15));
            col = col.push(ghost("Отключить аккаунт", Message::ShikiLogout));
        } else {
            col = col.push(muted("Для экспорта нужен OAuth2: создайте приложение на shikimori.io/oauth/applications с redirect URI «urn:ietf:wg:oauth:2.0:oob» и укажите его ключи."));
            col = col.push(field(self, Field::ShikiClientId, "Client ID"));
            col = col.push(secret(self, Field::ShikiSecret, "Client Secret"));
            col = col.push(ghost("1. Открыть страницу авторизации", Message::ShikiOpenAuth));
            col = col.push(row![field(self, Field::ShikiCode, "2. Код авторизации").on_submit(Message::ShikiConfirmCode).width(Length::Fill), btn(if self.busy { "…" } else { "Войти" }, Message::ShikiConfirmCode)].spacing(6));
        }
        col = col.push(toggle_row("Переносить прогресс просмотра", self.settings.shiki_sync_progress, Toggle::ShikiProgress));
        col = col.push(heading("Импорт: Shikimori → Anixart").size(18));
        col = col.push(muted("Публичный список читается по нику без авторизации."));
        col = col.push(field(self, Field::ShikiNick, "Ник на Shikimori").on_submit(Message::ShikiImport));
        let st = self.sync.snapshot();
        if st.running {
            col = col.push(row![button(text("Идёт синхронизация…").size(15)).padding([9, 16]).style(button::secondary), ghost("Остановить", Message::ShikiStop)].spacing(6));
        } else {
            col = col.push(row![btn("Импортировать", Message::ShikiImport), if authorized { btn("Экспортировать в Shikimori", Message::ShikiExport) } else { Space::new().into() }].spacing(8));
        }
        if last_sync > 0 {
            col = col.push(muted(format!("Последняя синхронизация: {}", format_date(last_sync))));
        }
        if st.running || st.report.is_some() {
            col = col.push(progress_bar(0.0..=1.0, st.fraction));
            col = col.push(muted(st.message.clone()));
        }
        if let Some(r) = &st.report {
            let mut rep = column![text("Отчёт").size(16)].spacing(4);
            if let Some(e) = &r.fatal {
                rep = rep.push(text(e.clone()).size(14).color(DANGER));
            }
            rep = rep.push(text(format!("Всего записей: {} · Перенесено: {} · С прогрессом: {} · Проблемных: {}", r.total, r.synced, r.progress_synced, r.problems.len())).size(13));
            if r.cancelled {
                rep = rep.push(muted("Операция остановлена пользователем"));
            }
            if !r.problems.is_empty() {
                rep = rep.push(muted("Не удалось сопоставить или перенести:"));
                rep = rep.push(text(r.problems.iter().take(60).cloned().collect::<Vec<_>>().join("\n")).size(12));
                if r.problems.len() > 60 {
                    rep = rep.push(muted(format!("…и ещё {}", r.problems.len() - 60)));
                }
            }
            col = col.push(container(rep).padding(12).width(Length::Fill).style(|t: &Theme| card_style(t)));
        }
        self.scroll_page_pub(self.top_bar("Shikimori", vec![]), col.into())
    }
}
