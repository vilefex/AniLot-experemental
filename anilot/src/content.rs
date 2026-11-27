//! Обработка сообщений контента: лента, поиск, закладки, релиз, профиль, комментарии, коллекции, расписание.

use crate::api::{CommentKind, ListKind};
use crate::app::*;
use crate::models::*;
use crate::storage;
use iced::Task;
use std::collections::{HashMap, HashSet};

impl App {
    pub(crate) fn update_content(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::ReloadHome => {
                let mut tasks = vec![self.reload_home_tasks()];
                if self.home.feed_mode {
                    self.home.feed.clear();
                    self.home.feed_page = 0;
                    self.home.feed_last = false;
                    tasks.push(self.request_feed(0));
                }
                Task::batch(tasks)
            }
            Message::HomeInteresting(r) => {
                self.home.loading = false;
                match r {
                    Ok(list) => {
                        let imgs: Vec<String> = list.iter().map(|i| i.image.clone()).collect();
                        self.home.interesting = list;
                        self.images_task(imgs)
                    }
                    Err(e) => {
                        self.home.error = Some(e);
                        Task::none()
                    }
                }
            }
            Message::HomeSection(idx, r) => match r {
                Ok(list) => {
                    let imgs: Vec<String> = list.iter().map(|r| r.image.clone()).collect();
                    if let Some(slot) = self.home.sections.get_mut(idx) {
                        slot.1 = list;
                    }
                    self.images_task(imgs)
                }
                Err(e) => {
                    self.home.error = Some(e);
                    Task::none()
                }
            },
            Message::HomeFeedMode(on) => {
                self.home.feed_mode = on;
                if on && self.home.feed.is_empty() {
                    return self.request_feed(0);
                }
                Task::none()
            }
            Message::FeedMore => {
                let p = self.home.feed_page + 1;
                self.request_feed(p)
            }
            Message::FeedLoaded(r) => match r {
                Ok(p) => {
                    self.home.feed_page = p.page;
                    self.home.feed_last = p.is_last();
                    let imgs: Vec<String> = p.items.iter().map(|i| i.image.clone()).collect();
                    self.home.feed.extend(p.items);
                    self.images_task(imgs)
                }
                Err(e) => {
                    self.toast_msg(e);
                    Task::none()
                }
            },
            Message::ListLoaded(t, gen, r) => {
                let mut imgs = vec![];
                if let Some(l) = self.listing_ref(t) {
                    if l.gen != gen {
                        return Task::none();
                    }
                    l.loading = false;
                    match r {
                        Ok(p) => {
                            l.page = p.page;
                            l.last = p.is_last();
                            imgs = p.items.iter().map(|r| r.image.clone()).collect();
                            l.items.extend(p.items);
                        }
                        Err(e) => {
                            l.error = Some(e);
                            if l.page > 0 {
                                l.page -= 1;
                            }
                        }
                    }
                }
                self.images_task(imgs)
            }
            Message::ListMore(t) => {
                if let Some(l) = self.listing_ref(t) {
                    if l.loading || l.last {
                        return Task::none();
                    }
                    l.page += 1;
                }
                self.fetch_listing_pub(t)
            }
            Message::OpenRelease(id) => self.open_release_pub(id),
            Message::OpenList(title, kind) => {
                let uid = self.push_pub(PageKind::List { title, listing: Listing::new(kind) });
                self.fetch_listing_pub(ListTarget::Page(uid))
            }
            Message::OpenProfile(id) => {
                let uid = self.push_pub(PageKind::Profile(ProfilePage { id, ..Default::default() }));
                self.profile_task(uid, id)
            }
            Message::OpenFriends(id) => {
                let uid = self.push_pub(PageKind::Friends(FriendsPage { id, ..Default::default() }));
                self.friends_task(uid)
            }
            Message::OpenComments(kind, target) => {
                let uid = self.push_pub(PageKind::Comments(CommentsPage {
                    kind,
                    target,
                    comments: vec![],
                    page: 0,
                    last: false,
                    loading: false,
                    sort: 1,
                    replies: HashMap::new(),
                    revealed: HashSet::new(),
                    reply_to: None,
                    error: None,
                }));
                self.inputs_remove(Field::CommentText);
                self.comments_task(uid)
            }
            Message::OpenCollections => {
                let uid = self.push_pub(PageKind::Collections(CollectionsPage::default()));
                self.collections_task(uid)
            }
            Message::OpenCollection(id) => {
                let uid = self.push_pub(PageKind::Collection(CollectionPage {
                    id,
                    collection: None,
                    listing: Listing::new(ListKind::CollectionReleases(id)),
                    error: None,
                }));
                let api = self.api.clone();
                Task::batch(vec![
                    Task::perform(async move { api.collection(id).await }, move |r| Message::CollectionLoaded(uid, r)),
                    self.fetch_listing_pub(ListTarget::Page(uid)),
                ])
            }
            Message::OpenWatch(release) => {
                let uid = self.push_pub(PageKind::Chooser(ChooserPage {
                    release: *release,
                    types: vec![],
                    type_sel: None,
                    sources: vec![],
                    source_sel: None,
                    episodes: vec![],
                    loading: true,
                    error: None,
                }));
                self.chooser_types_pub(uid)
            }
            Message::OpenGallery(urls, index) => {
                let t = self.images_task(urls.clone());
                self.push_pub(PageKind::Gallery { urls, index });
                t
            }
            Message::GalleryMove(d) => {
                let mut want = vec![];
                if let Some(StackPage { kind: PageKind::Gallery { urls, index }, .. }) = self.stack.last_mut() {
                    let n = urls.len() as i64;
                    if n > 0 {
                        *index = ((*index as i64 + d).rem_euclid(n)) as usize;
                        want.push(urls[*index].clone());
                    }
                }
                self.images_task(want)
            }
            Message::OpenVideos(v) => {
                let imgs: Vec<String> = v.iter().map(|x| x.image.clone()).collect();
                self.push_pub(PageKind::Videos(v));
                self.images_task(imgs)
            }
            Message::OpenSchedule => {
                let uid = self.push_pub(PageKind::Schedule(SchedulePage { days: vec![], day: weekday_now(), loading: true, error: None }));
                let api = self.api.clone();
                Task::perform(async move { api.schedule().await }, move |r| Message::ScheduleLoaded(uid, r))
            }
            Message::ScheduleLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Schedule(s)) = self.page_mut_pub(uid) {
                    s.loading = false;
                    match r {
                        Ok(days) => {
                            imgs = days.iter().flat_map(|(_, v)| v.iter().map(|r| r.image.clone())).collect();
                            s.days = days;
                        }
                        Err(e) => s.error = Some(e),
                    }
                }
                self.images_task(imgs)
            }
            Message::ScheduleDay(uid, d) => {
                if let Some(PageKind::Schedule(s)) = self.page_mut_pub(uid) {
                    s.day = d;
                }
                Task::none()
            }
            // ── поиск ──
            Message::SubmitSearch => {
                let q = self.inp(Field::Search).trim().to_string();
                if self.search.profiles_mode {
                    if q.is_empty() {
                        return Task::none();
                    }
                    self.search.profiles_loading = true;
                    let api = self.api.clone();
                    return Task::perform(async move { api.search_profiles(&q, 0).await.map(|p| p.items) }, Message::ProfilesFound);
                }
                if q.is_empty() {
                    let f = self.current_filter();
                    return self.start_listing_pub(ListTarget::Search, ListKind::Filter(f));
                }
                storage::add_search_history(&q);
                self.start_listing_pub(ListTarget::Search, ListKind::Search(q))
            }
            Message::ProfilesFound(r) => {
                self.search.profiles_loading = false;
                match r {
                    Ok(list) => {
                        let imgs: Vec<String> = list.iter().map(|p| p.avatar.clone()).collect();
                        self.search.profiles = list;
                        self.images_task(imgs)
                    }
                    Err(e) => {
                        self.toast_msg(e);
                        Task::none()
                    }
                }
            }
            Message::SearchProfilesMode(on) => {
                self.search.profiles_mode = on;
                Task::none()
            }
            Message::ToggleFilter => {
                self.search.show_filter = !self.search.show_filter;
                Task::none()
            }
            Message::ApplyFilter => {
                self.inputs_remove(Field::Search);
                self.search.show_filter = false;
                let f = self.current_filter();
                self.start_listing_pub(ListTarget::Search, ListKind::Filter(f))
            }
            Message::ResetFilter => {
                self.search.filter = Default::default();
                for f in [Field::YearFrom, Field::YearTo, Field::EpFrom, Field::EpTo] {
                    self.inputs_remove(f);
                }
                Task::none()
            }
            Message::FilterSort(v) => {
                self.search.filter.sort = v;
                Task::none()
            }
            Message::FilterCategory(v) => {
                self.search.filter.category_id = v;
                Task::none()
            }
            Message::FilterStatus(v) => {
                self.search.filter.status_id = v;
                Task::none()
            }
            Message::FilterSeason(v) => {
                self.search.filter.season = v;
                Task::none()
            }
            Message::FilterGenre(g) => {
                let gs = &mut self.search.filter.genres;
                if let Some(pos) = gs.iter().position(|x| *x == g) {
                    gs.remove(pos);
                } else {
                    gs.push(g);
                }
                Task::none()
            }
            Message::HistoryQuery(q) => {
                self.inputs_insert(Field::Search, q);
                self.update_content(Message::SubmitSearch)
            }
            Message::ClearSearchHistory => {
                storage::write(|s| s.search_history.clear());
                Task::none()
            }
            // ── закладки ──
            Message::BookmarkStatus(st) => {
                self.bookmarks.status = st;
                self.inputs_remove(Field::BookmarkSearch);
                let kind = match st {
                    0 => ListKind::Favorites,
                    -1 => ListKind::History,
                    s => ListKind::ProfileList { profile: 0, status: s },
                };
                self.start_listing_pub(ListTarget::Bookmarks, kind)
            }
            Message::SubmitBookmarkSearch => {
                let q = self.inp(Field::BookmarkSearch).trim().to_string();
                let st = self.bookmarks.status;
                if q.is_empty() {
                    return self.update_content(Message::BookmarkStatus(st));
                }
                self.start_listing_pub(ListTarget::Bookmarks, ListKind::Bookmarks { status: st, query: q })
            }
            // ── релиз ──
            Message::ReleaseLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                    match r {
                        Ok(rel) => {
                            imgs.push(rel.image.clone());
                            imgs.extend(rel.screenshots.iter().take(8).cloned());
                            imgs.extend(rel.related_releases.iter().map(|r| r.image.clone()));
                            imgs.extend(rel.recommended.iter().map(|r| r.image.clone()));
                            imgs.extend(rel.videos.iter().map(|v| v.image.clone()));
                            p.release = Some(rel);
                        }
                        Err(e) => p.error = Some(e),
                    }
                }
                self.images_task(imgs)
            }
            Message::ToggleDescription(uid) => {
                if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                    p.expanded = !p.expanded;
                }
                Task::none()
            }
            Message::ToggleStatusMenu(uid) => {
                if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                    p.status_menu = !p.status_menu;
                }
                Task::none()
            }
            Message::SetListStatus(uid, status) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт, чтобы вести списки");
                    return Task::none();
                }
                let (id, old) = match self.page_mut_pub(uid) {
                    Some(PageKind::Release(p)) => {
                        p.status_menu = false;
                        match &p.release {
                            Some(r) => (r.id, r.list_status),
                            None => return Task::none(),
                        }
                    }
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.set_list_status(id, old, status).await }, move |r| Message::ListStatusSet(uid, status, r))
            }
            Message::ListStatusSet(uid, status, r) => {
                match r {
                    Ok(()) => {
                        if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                            if let Some(rel) = &mut p.release {
                                rel.list_status = status;
                            }
                        }
                        self.bookmarks.listing.items.clear();
                        self.toast_msg("Список обновлён");
                    }
                    Err(e) => self.toast_msg(e),
                }
                Task::none()
            }
            Message::ToggleFavorite(uid) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт, чтобы пользоваться избранным");
                    return Task::none();
                }
                let (id, on) = match self.page_mut_pub(uid) {
                    Some(PageKind::Release(p)) => match &p.release {
                        Some(r) => (r.id, !r.is_favorite),
                        None => return Task::none(),
                    },
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.set_favorite(id, on).await }, move |r| Message::FavoriteSet(uid, on, r))
            }
            Message::FavoriteSet(uid, on, r) => {
                match r {
                    Ok(()) => {
                        if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                            if let Some(rel) = &mut p.release {
                                rel.is_favorite = on;
                            }
                        }
                    }
                    Err(e) => self.toast_msg(e),
                }
                Task::none()
            }
            Message::Vote(uid, v) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт, чтобы оценивать");
                    return Task::none();
                }
                let (id, vote) = match self.page_mut_pub(uid) {
                    Some(PageKind::Release(p)) => match &p.release {
                        Some(r) => (r.id, if r.my_vote == v { 0 } else { v }),
                        None => return Task::none(),
                    },
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.vote(id, vote).await }, move |r| Message::VoteSet(uid, vote, r))
            }
            Message::VoteSet(uid, vote, r) => {
                match r {
                    Ok(()) => {
                        if let Some(PageKind::Release(p)) = self.page_mut_pub(uid) {
                            if let Some(rel) = &mut p.release {
                                rel.my_vote = vote;
                            }
                        }
                    }
                    Err(e) => self.toast_msg(e),
                }
                Task::none()
            }
            // ── профиль ──
            Message::ProfileLoaded(uid, r) => {
                let mut imgs = vec![];
                match r {
                    Ok(p) => {
                        imgs.push(p.avatar.clone());
                        imgs.extend(p.history.iter().map(|r| r.image.clone()));
                        imgs.extend(p.friends_preview.iter().map(|f| f.avatar.clone()));
                        if uid == 0 {
                            self.me.id = p.id;
                            self.me.profile = Some(p);
                        } else if let Some(PageKind::Profile(page)) = self.page_mut_pub(uid) {
                            page.profile = Some(p);
                        }
                    }
                    Err(e) => {
                        if uid == 0 {
                            self.me.error = Some(e);
                        } else if let Some(PageKind::Profile(page)) = self.page_mut_pub(uid) {
                            page.error = Some(e);
                        }
                    }
                }
                self.images_task(imgs)
            }
            Message::FriendsLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Friends(f)) = self.page_mut_pub(uid) {
                    f.loading = false;
                    match r {
                        Ok(p) => {
                            f.page = p.page;
                            f.last = p.is_last();
                            imgs = p.items.iter().map(|x| x.avatar.clone()).collect();
                            f.items.extend(p.items);
                        }
                        Err(_) => f.last = true,
                    }
                }
                self.images_task(imgs)
            }
            Message::FriendsMore(uid) => {
                if let Some(PageKind::Friends(f)) = self.page_mut_pub(uid) {
                    if f.loading || f.last {
                        return Task::none();
                    }
                    f.page += 1;
                }
                self.friends_task(uid)
            }
            Message::FriendAction(uid, send) => {
                let id = match self.page_mut_pub(uid) {
                    Some(PageKind::Profile(p)) => p.id,
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                let reload = self.profile_task(uid, id);
                Task::batch(vec![Task::perform(async move { api.friend_request(id, send).await }, Message::FriendActionDone), reload])
            }
            Message::FriendActionDone(r) => {
                if let Err(e) = r {
                    self.toast_msg(e);
                }
                Task::none()
            }
            // ── комментарии ──
            Message::CommentsLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    c.loading = false;
                    match r {
                        Ok(p) => {
                            c.page = p.page;
                            c.last = p.is_last();
                            imgs = p.items.iter().map(|x| x.author.avatar.clone()).collect();
                            c.comments.extend(p.items);
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                self.images_task(imgs)
            }
            Message::CommentsMore(uid) => {
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    if c.loading || c.last {
                        return Task::none();
                    }
                    c.page += 1;
                }
                self.comments_task(uid)
            }
            Message::CommentsSort(uid, sort) => {
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    c.sort = sort;
                    c.comments.clear();
                    c.replies.clear();
                    c.page = 0;
                    c.last = false;
                    c.loading = false;
                }
                self.comments_task(uid)
            }
            Message::LoadReplies(uid, cid) => {
                let (kind, have) = match self.page_mut_pub(uid) {
                    Some(PageKind::Comments(c)) => (c.kind, c.replies.contains_key(&cid)),
                    _ => return Task::none(),
                };
                if have {
                    if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                        c.replies.remove(&cid);
                    }
                    return Task::none();
                }
                let api = self.api.clone();
                Task::perform(async move { api.replies(kind, cid, 0).await }, move |r| Message::RepliesLoaded(uid, cid, r))
            }
            Message::RepliesLoaded(uid, cid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    match r {
                        Ok(p) => {
                            imgs = p.items.iter().map(|x| x.author.avatar.clone()).collect();
                            c.replies.insert(cid, p.items);
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                self.images_task(imgs)
            }
            Message::RevealSpoiler(uid, cid) => {
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    c.revealed.insert(cid);
                }
                Task::none()
            }
            Message::ReplyTo(uid, cid, pid, login) => {
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    c.reply_to = Some((cid, pid, login));
                }
                Task::none()
            }
            Message::CancelReply(uid) => {
                if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                    c.reply_to = None;
                }
                Task::none()
            }
            Message::SendComment(uid) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт, чтобы комментировать");
                    return Task::none();
                }
                let text = self.inp(Field::CommentText).trim().to_string();
                if text.is_empty() {
                    return Task::none();
                }
                let spoiler = self.toggle(Toggle::CommentSpoiler);
                let (kind, target, parent, reply_to) = match self.page_mut_pub(uid) {
                    Some(PageKind::Comments(c)) => {
                        let (parent, to) = match &c.reply_to {
                            Some((cid, pid, _)) => {
                                // Ответ на ответ крепится к корневому комментарию.
                                let root = c
                                    .replies
                                    .iter()
                                    .find(|(_, v)| v.iter().any(|x| x.id == *cid))
                                    .map(|(k, _)| *k)
                                    .unwrap_or(*cid);
                                (Some(root), Some(*pid))
                            }
                            None => (None, None),
                        };
                        (c.kind, c.target, parent, to)
                    }
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(
                    async move { api.add_comment(kind, target, &text, parent, reply_to, spoiler).await },
                    move |r| Message::CommentSent(uid, r),
                )
            }
            Message::CommentSent(uid, r) => match r {
                Ok(()) => {
                    self.inputs_remove(Field::CommentText);
                    if let Some(PageKind::Comments(c)) = self.page_mut_pub(uid) {
                        c.reply_to = None;
                        c.comments.clear();
                        c.replies.clear();
                        c.page = 0;
                        c.last = false;
                        c.loading = false;
                    }
                    self.comments_task(uid)
                }
                Err(e) => {
                    self.toast_msg(e);
                    Task::none()
                }
            },
            Message::VoteComment(uid, cid, vote) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт, чтобы голосовать");
                    return Task::none();
                }
                let kind = match self.page_mut_pub(uid) {
                    Some(PageKind::Comments(c)) => {
                        let apply = |x: &mut Comment| {
                            if x.id == cid {
                                if x.vote == vote {
                                    x.vote = 0;
                                    x.vote_count -= if vote == 1 { 1 } else { -1 };
                                } else {
                                    x.vote_count += if vote == 1 { 1 } else { -1 } - match x.vote { 1 => 1, 2 => -1, _ => 0 };
                                    x.vote = vote;
                                }
                            }
                        };
                        c.comments.iter_mut().for_each(apply);
                        c.replies.values_mut().flatten().for_each(apply);
                        c.kind
                    }
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.vote_comment(kind, cid, vote).await }, move |r| Message::CommentChanged(uid, r))
            }
            Message::DeleteComment(uid, cid) => {
                let kind = match self.page_mut_pub(uid) {
                    Some(PageKind::Comments(c)) => c.kind,
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.delete_comment(kind, cid).await }, move |r| Message::CommentChanged(uid, r))
            }
            Message::CommentChanged(uid, r) => match r {
                Ok(()) => Task::none(),
                Err(e) => {
                    self.toast_msg(e);
                    let _ = uid;
                    Task::none()
                }
            },
            // ── коллекции ──
            Message::CollectionsLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Collections(c)) = self.page_mut_pub(uid) {
                    c.loading = false;
                    match r {
                        Ok(p) => {
                            c.page = p.page;
                            c.last = p.is_last();
                            imgs = p.items.iter().map(|x| x.image.clone()).collect();
                            c.items.extend(p.items);
                        }
                        Err(_) => c.last = true,
                    }
                }
                self.images_task(imgs)
            }
            Message::CollectionsMore(uid) => {
                if let Some(PageKind::Collections(c)) = self.page_mut_pub(uid) {
                    if c.loading || c.last {
                        return Task::none();
                    }
                    c.page += 1;
                }
                self.collections_task(uid)
            }
            Message::SubmitCollectionSearch(uid) => {
                let q = self.inp(Field::CollectionSearch).trim().to_string();
                if let Some(PageKind::Collections(c)) = self.page_mut_pub(uid) {
                    c.items.clear();
                    c.page = 0;
                    c.last = false;
                    c.loading = false;
                    c.query = q;
                }
                self.collections_task(uid)
            }
            Message::CollectionLoaded(uid, r) => {
                let mut imgs = vec![];
                if let Some(PageKind::Collection(c)) = self.page_mut_pub(uid) {
                    match r {
                        Ok(col) => {
                            imgs.push(col.image.clone());
                            imgs.push(col.creator.avatar.clone());
                            c.collection = Some(col);
                        }
                        Err(e) => c.error = Some(e),
                    }
                }
                self.images_task(imgs)
            }
            Message::ToggleCollectionFavorite(uid) => {
                if !self.logged_in {
                    self.toast_msg("Войдите в аккаунт");
                    return Task::none();
                }
                let (id, on) = match self.page_mut_pub(uid) {
                    Some(PageKind::Collection(c)) => match &c.collection {
                        Some(col) => (col.id, !col.is_favorite),
                        None => return Task::none(),
                    },
                    _ => return Task::none(),
                };
                let api = self.api.clone();
                Task::perform(async move { api.set_collection_favorite(id, on).await }, move |r| Message::CollectionFavoriteSet(uid, on, r))
            }
            Message::CollectionFavoriteSet(uid, on, r) => {
                match r {
                    Ok(()) => {
                        if let Some(PageKind::Collection(c)) = self.page_mut_pub(uid) {
                            if let Some(col) = &mut c.collection {
                                col.is_favorite = on;
                            }
                        }
                    }
                    Err(e) => self.toast_msg(e),
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}

fn weekday_now() -> usize {
    // 1970-01-01 — четверг; 0 = понедельник.
    let days = storage::now().div_euclid(86_400);
    ((days + 3).rem_euclid(7)) as usize
}

// Список комментариев к релизу доступен также из других модулей.
pub fn comment_kind_release() -> CommentKind {
    CommentKind::Release
}
