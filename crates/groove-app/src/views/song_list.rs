//! groove-gd1_16: danh sách bài của một album, thư mục hay playlist (đặc tả 1.2, bản cũ `music-list.vue`).
//!
//! Đầu danh sách (bìa, loại, tên, dòng phụ, nút Play all) dính trên đầu và thu lại khi cuộn quá 120 px, mở lại khi về dưới
//! 15 px (hai ngưỡng để không rung); quầng accent 380 px, độ đục 0,4 phía sau. Bảng bài là `materiant::list::VirtualList`:
//! chỉ dựng dòng trong khung nên 10 000 bài tốn như 100.
//!
//! Hành vi (đặc tả 1.2): bấm dòng bài đang phát thì đảo phát / dừng, bài khác thì phát với cả danh sách làm hàng đợi; Enter
//! / nhấp đúp tương tự (nhấp đúp không đảo lần hai: lần nhấp đầu đã làm); chuột phải mở menu bài (`song_menu`).
//! Thư viện đổi (`Synced`, `PlaylistsChanged`, `Moved`, `Opened`) thì nạp lại. Mọi lần đọc chạy ở nền (UI không chạm đĩa).
//!
//! # Cho task sau
//!
//! - `LibraryPage` dựng view bằng `SongList::with_deps`; đổi category thì `set_reference(reference, cx)` (đặt lại cuộn, đầu
//!   danh sách mở).
//! - Nghe [`SongListEvent`] bằng `cx.subscribe`: `CategoryGone` (mục đang xem biến mất: `app::go_back`).
//! - Nút Play all của đầu danh sách do view tự làm (player + toast khi rỗng).

use std::sync::Arc;

use futures::StreamExt;
use groove_core::model::{CategoryRef, Song, TrackRef};
use groove_core::t;
use groove_library::actor::{LibraryError, LibraryEvent, ProgressKind};
use groove_library::covers::CoverStore;
use groove_library::db::queries::CategorySummary;
use groove_library::store::Library;
use materiant::button::Button;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement, MouseMoveEvent,
    ParentElement, Render, ScrollEvent, SharedString, Styled, Subscription, Task, Window, div, linear_color_stop,
    linear_gradient, px,
};
use materiant::icon::Icon;
use materiant::list::{VirtualList, VirtualListHandle};
use materiant::menu::ContextMenuExt;
use materiant::progress::ProgressCircular;
use materiant::theme::{ActiveTheme, ThemeColors};
use materiant::toast::{Toast, ToastKind, ToastSpec};
use materiant::virtual_grid::LruImageCache;

use crate::icons::GrooveIcon;
use crate::player::{self, Player};
use crate::views::category_card::{accent_of, cover_path, subtitle};
use crate::views::cover_image::{CoverImage, CoverKind};
use crate::views::song_row::{ROW_HEIGHT, RowPlay, SongRow, header_row, show_artist};

/// Lề hai bên của cả màn.
const PAGE_PADDING: f32 = 24.;
/// Đầu danh sách mở (bìa 160) và thu (bìa 80): cao cố định để hai trạng thái so được với nhau.
const EXPANDED_COVER: f32 = 160.;
const COLLAPSED_COVER: f32 = 80.;
const HEADER_VERTICAL_PADDING: f32 = 12.;
const EXPANDED_HEIGHT: f32 = EXPANDED_COVER + 2. * HEADER_VERTICAL_PADDING + 16.;
const COLLAPSED_HEIGHT: f32 = COLLAPSED_COVER + 2. * HEADER_VERTICAL_PADDING;
/// Cuộn quá ngưỡng này thì thu đầu danh sách; về dưới ngưỡng kia thì mở lại (đặc tả 1.2).
const COLLAPSE_ABOVE: f32 = 120.;
const EXPAND_BELOW: f32 = 15.;
/// Quầng accent phía sau (đặc tả 1.2).
const GLOW_HEIGHT: f32 = 380.;
const GLOW_OPACITY: f32 = 0.4;
/// Bìa của đầu danh sách chỉ cần vài ô nhớ: bìa 256 px giải mã ≈ 256 KB mỗi cái.
const COVER_CACHE_CAPACITY: usize = 4;

/// Thứ view báo lên cho nơi gắn view (điều hướng).
#[derive(Clone, Debug)]
pub enum SongListEvent {
    /// Mục đang xem không còn trong thư viện (xoá, hoặc quét lại làm album biến mất).
    CategoryGone(CategoryRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Chưa có kết quả đầu tiên.
    Loading,
    Ready,
    /// Đọc thư viện lỗi (chi tiết chỉ vào log).
    Failed,
    /// Mục không còn.
    Gone,
}

/// Bấm một dòng thì làm gì.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClickAction {
    /// Bài đang chọn: đảo phát / dừng.
    Toggle,
    /// Bài khác: phát với cả danh sách làm hàng đợi.
    PlayList,
}

fn click_action(current: Option<&TrackRef>, clicked: &TrackRef) -> ClickAction {
    if current == Some(clicked) { ClickAction::Toggle } else { ClickAction::PlayList }
}

/// Đầu danh sách thu lại hay mở ra sau khi cuộn tới `scroll_top`; `max_top` là tầm cuộn tối đa của trạng thái hiện tại.
///
/// Thu đi làm khung danh sách cao thêm nên tầm cuộn giảm đúng chừng đó: chỉ thu khi sau khi thu vị trí cuộn vẫn còn trên
/// ngưỡng mở lại, không thì mở rồi thu rồi mở mãi (danh sách ngắn, cuộn được chút ít).
fn next_collapsed(collapsed: bool, scroll_top: f32, max_top: f32) -> bool {
    if collapsed {
        scroll_top >= EXPAND_BELOW
    } else {
        let shrink = EXPANDED_HEIGHT - COLLAPSED_HEIGHT;
        scroll_top > COLLAPSE_ABOVE && max_top - shrink > EXPAND_BELOW + 1.
    }
}

pub struct SongList {
    reference: CategoryRef,
    library: Option<Library>,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    player: Entity<Player>,
    status: Status,
    summary: Option<CategorySummary>,
    songs: Arc<Vec<Song>>,
    selected: Option<usize>,
    collapsed: bool,
    list: VirtualListHandle,
    focus: FocusHandle,
    /// Bài đang chọn và có đang phát không ở lần vẽ gần nhất: player báo 4 lần mỗi giây, chỉ vẽ lại khi cái này đổi.
    seen_playback: (Option<TrackRef>, bool),
    load_task: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
    _observer: Subscription,
}

impl EventEmitter<SongListEvent> for SongList {}

impl SongList {
    /// Thư viện và thư mục bìa do nơi gọi cấp (test trỏ thư mục tạm). Cần `player::install` đã chạy.
    pub fn with_deps(
        library: Option<Library>,
        covers: Option<CoverStore>,
        reference: CategoryRef,
        cx: &mut Context<Self>,
    ) -> SongList {
        let player = player::player(cx);
        let observer = cx.observe(&player, |this, player, cx| this.on_player(&player, cx));
        let seen_playback = playback_of(&player, cx);
        let mut list = SongList {
            reference,
            library,
            covers,
            cache: LruImageCache::new(COVER_CACHE_CAPACITY, cx),
            player,
            status: Status::Loading,
            summary: None,
            songs: Arc::default(),
            selected: None,
            collapsed: false,
            list: VirtualListHandle::new(),
            focus: cx.focus_handle(),
            seen_playback,
            load_task: None,
            _tasks: Vec::new(),
            _observer: observer,
        };
        list.listen(cx);
        list.reload(cx);
        list
    }

    #[cfg(test)]
    pub fn reference(&self) -> CategoryRef {
        self.reference
    }

    /// Bài menu đang nhắm: dòng đang chọn (chuột phải đã chọn sẵn), chưa chọn thì dòng đầu (Shift+F10).
    fn menu_song(&self) -> Option<&Song> {
        self.selected.and_then(|ix| self.songs.get(ix)).or_else(|| self.songs.first())
    }

    #[cfg(test)]
    pub fn menu_opens_on_first_song(&self) -> bool {
        self.selected.is_none() && self.menu_song() == self.songs.first()
    }

    /// Giữ focus cho danh sách (phím ↑ ↓ Enter).
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    /// Đổi sang category khác trong cùng view: đặt lại mọi thứ và nạp lại.
    pub fn set_reference(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        if reference == self.reference {
            return;
        }
        self.reference = reference;
        self.status = Status::Loading;
        self.summary = None;
        self.songs = Arc::default();
        self.selected = None;
        self.collapsed = false;
        self.list.scroll_to(0, materiant::list::Align::Start);
        self.reload(cx);
        cx.notify();
    }

    fn on_player(&mut self, player: &Entity<Player>, cx: &mut Context<Self>) {
        let now = playback_of(player, cx);
        if now != self.seen_playback {
            self.seen_playback = now;
            cx.notify();
        }
    }

    /// Nghe sự kiện thư viện: nạp lại khi đổi.
    fn listen(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &self.library else { return };
        let mut events = library.subscribe();
        self._tasks.push(cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                let reload = matches!(
                    event,
                    LibraryEvent::Opened(_)
                        | LibraryEvent::Synced(_)
                        | LibraryEvent::PlaylistsChanged
                        | LibraryEvent::Moved(_)
                ) || matches!(
                    event,
                    LibraryEvent::Progress(progress)
                        if progress.kind == ProgressKind::MetadataBulk && progress.done == progress.total
                );
                if reload && this.update(cx, |this, cx| this.reload(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    /// Nạp lại từ thư viện (hộp metadata vừa lưu).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload(cx);
    }

    /// Đọc lại ở nền. Lần đọc mới thay lần đang chạy.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else {
            self.status = Status::Failed;
            cx.notify();
            return;
        };
        let reference = self.reference;
        self.load_task = Some(cx.spawn(async move |this, cx| {
            // Lúc thư viện còn `Opening` việc đọc tự chờ.
            let summary = library.category(reference).await;
            let songs = library.songs(reference).await;
            this.update(cx, |this, cx| this.apply(reference, summary, songs, cx)).ok();
        }));
    }

    fn apply(
        &mut self,
        reference: CategoryRef,
        summary: Result<Option<CategorySummary>, LibraryError>,
        songs: Result<Vec<Song>, LibraryError>,
        cx: &mut Context<Self>,
    ) {
        if reference != self.reference {
            return;
        }
        match (summary, songs) {
            (Ok(Some(summary)), Ok(songs)) => {
                self.summary = Some(summary);
                self.songs = Arc::new(songs);
                self.status = Status::Ready;
                self.selected = self.selected.filter(|ix| *ix < self.songs.len());
            }
            (Ok(None), Ok(_)) => {
                self.summary = None;
                self.songs = Arc::default();
                self.selected = None;
                self.status = Status::Gone;
                cx.emit(SongListEvent::CategoryGone(reference));
            }
            (Err(err), _) | (_, Err(err)) => {
                // Chi tiết kỹ thuật chỉ vào log; người dùng thấy chữ i18n.
                log::warn!("song list: cannot read {reference:?}: {err}");
                self.status = Status::Failed;
            }
        }
        self.sync_header(cx);
        cx.notify();
    }

    // ---- Đầu danh sách thu / mở ----

    /// Đọc vị trí cuộn, thu hoặc mở đầu danh sách theo hai ngưỡng. Gọi sau mọi thứ có thể đổi vị trí cuộn.
    fn sync_header(&mut self, cx: &mut Context<Self>) {
        let (scroll_top, max_top) = {
            let handle = self.list.scroll_handle().0.borrow();
            (self.list.scroll_top(), handle.base_handle.max_offset().y.as_f32().abs())
        };
        let collapsed = next_collapsed(self.collapsed, scroll_top, max_top);
        if collapsed != self.collapsed {
            self.collapsed = collapsed;
            cx.notify();
        }
    }

    // ---- Phát ----

    fn tracks(&self) -> Vec<TrackRef> {
        self.songs.iter().map(|song| song.track.clone()).collect()
    }

    /// Bấm / Enter dòng `ix`.
    fn click_song(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(song) = self.songs.get(ix) else { return };
        let track = song.track.clone();
        let action = click_action(self.player.read(cx).current_track(), &track);
        let list = if action == ClickAction::PlayList { self.tracks() } else { Vec::new() };
        self.player.update(cx, |player, cx| match action {
            ClickAction::Toggle => player.toggle_play(cx),
            ClickAction::PlayList => player.play_list(track, list, cx),
        });
    }

    /// Play all: bài đầu, cả danh sách làm hàng đợi. Rỗng thì toast.
    fn play_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tracks = self.tracks();
        let Some(first) = tracks.first().cloned() else {
            Toast::push(window, cx, ToastSpec::new(ToastKind::Info, t!("song_list.empty_play.message")));
            return;
        };
        self.player.update(cx, |player, cx| player.play_list(first, tracks, cx));
    }

    /// Chuột phải một dòng: chọn dòng đó, menu bài (`song_menu`, lớp cha) đọc [`Self::menu_song`].
    fn context_menu(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.songs.len() {
            self.selected = Some(ix);
            cx.notify();
        }
    }
}

fn playback_of(player: &Entity<Player>, cx: &App) -> (Option<TrackRef>, bool) {
    let player = player.read(cx);
    (player.current_track().cloned(), player.is_playing())
}

// ---- Vẽ ----

fn kind_label(reference: CategoryRef) -> &'static str {
    match reference {
        CategoryRef::Album(_) => t!("song_list.kind.album"),
        CategoryRef::Folder(_) => t!("song_list.kind.folder"),
        CategoryRef::Playlist(_) => t!("song_list.kind.playlist"),
    }
}

/// Icon + tiêu đề + gợi ý ở giữa vùng nội dung (rỗng, lỗi, mục biến mất).
fn centered_message(icon: GrooveIcon, title: &'static str, hint: &'static str, c: &ThemeColors) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .px(px(PAGE_PADDING))
        .text_center()
        .child(Icon::new(icon).size(px(48.)).color(c.muted.opacity(0.6)))
        .child(div().text_size(px(14.)).text_color(c.muted).child(title))
        .child(div().max_w(px(420.)).text_size(px(12.)).text_color(c.muted.opacity(0.8)).child(hint))
        .into_any_element()
}

impl SongList {
    fn header(&self, c: &ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let collapsed = self.collapsed;
        let summary = self.summary.as_ref();
        let kind = CoverKind::of(self.reference);
        let accent = summary.and_then(accent_of);
        let cover = summary.and_then(|summary| cover_path(summary, self.covers.as_ref()));
        let title: SharedString = summary.map(|s| s.name.clone()).unwrap_or_default().into();
        let count = self.songs.len();
        let count_text: SharedString = t!("song_list.header.items", count = count).into();
        // Playlist: dòng phụ của thẻ chính là số bài, đừng nói hai lần.
        let sub: Option<SharedString> = summary
            .filter(|s| !matches!(s.reference, CategoryRef::Playlist(_)))
            .map(subtitle)
            .filter(|text| !text.is_empty());
        let side = if collapsed { COLLAPSED_COVER } else { EXPANDED_COVER };

        let label = div().text_size(px(12.)).text_color(c.primary_ink).child(kind_label(self.reference));
        let info = if collapsed {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .text_color(c.muted)
                .child(div().text_color(c.primary_ink).child(kind_label(self.reference)))
                .child("•")
                .when(count > 0, |row| row.child(count_text.clone()))
        } else {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(14.))
                .text_color(c.muted)
                .when_some(sub, |row, sub| row.child(div().truncate().child(sub)).child("•"))
                .child(count_text)
        };

        let text = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .justify_end()
            .gap_1()
            .when(!collapsed, |col| col.child(label))
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::NORMAL)
                    .text_size(px(if collapsed { 20. } else { 36. }))
                    .text_color(c.text)
                    .child(title),
            )
            .child(info)
            .when(!collapsed, |col| {
                col.child(
                    div().pt_3().flex().child(
                        Button::new("song-list-play-all", t!("song_list.header.play_all"))
                            .filled()
                            .icon(GrooveIcon::Play)
                            .disabled(count == 0)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.play_all(window, cx))),
                    ),
                )
            });

        div()
            .flex_none()
            .h(px(if collapsed { COLLAPSED_HEIGHT } else { EXPANDED_HEIGHT }))
            .w_full()
            .px(px(PAGE_PADDING))
            .py(px(HEADER_VERTICAL_PADDING))
            .flex()
            .items_end()
            .gap(px(if collapsed { 16. } else { 24. }))
            .child(div().flex_none().size(px(side)).child(CoverImage::new(cover, kind, accent, &self.cache)))
            .child(text)
            .into_any_element()
    }

    fn table(&self, window: &mut Window, c: &ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let artist = show_artist(window.viewport_size().width.as_f32());
        let (playing_track, is_playing) = self.seen_playback.clone();
        let songs = self.songs.clone();
        let this = cx.entity().downgrade();
        let rows = VirtualList::new("song-list-rows", songs.len(), px(ROW_HEIGHT), move |ix, _, _| {
            let song = &songs[ix];
            let play = match (&playing_track, is_playing) {
                (Some(current), true) if *current == song.track => RowPlay::Playing,
                (Some(current), false) if *current == song.track => RowPlay::Paused,
                _ => RowPlay::Other,
            };
            let this = this.clone();
            SongRow::new(song, ix + 1, play, artist).on_context(std::rc::Rc::new(move |_, _, cx| {
                this.update(cx, |this, cx| this.context_menu(ix, cx)).ok();
            }))
        })
        .track(&self.list)
        .padding_x(px(PAGE_PADDING))
        .track_focus(&self.focus)
        // Space là phím phát / dừng toàn cục: không để nó bật vòng focus quanh cả bảng.
        .focus_ring(false)
        .selected(self.selected)
        // Chọn bằng chuột là bấm dòng ⇒ phát; chọn bằng ↑ ↓ chỉ di con trỏ, không phát.
        .on_select(cx.listener(|this, ix: &usize, window, cx| {
            this.selected = Some(*ix);
            if !window.last_input_was_keyboard() {
                this.click_song(*ix, cx);
            }
            this.sync_header(cx);
            cx.notify();
        }))
        // Enter phát; nhấp đúp thì lần nhấp đầu đã phát rồi, không đảo lần hai.
        .on_activate(cx.listener(|this, ix: &usize, window, cx| {
            if window.last_input_was_keyboard() {
                this.click_song(*ix, cx);
            }
        }))
        .flex_1()
        .min_h_0();
        // Menu bài: chuột phải (dòng đã chọn sẵn ở `on_context`, con chạy trước) hoặc Shift+F10 khi focus ở danh sách.
        let menu_list = cx.entity().downgrade();
        let rows =
            div().id("song-list-rows-host").relative().flex_1().min_h_0().flex().flex_col().child(rows).context_menu(
                move |menu, _, cx| {
                    let Some(list) = menu_list.upgrade() else { return menu };
                    let list = list.read(cx);
                    match list.menu_song() {
                        Some(song) => crate::views::song_menu::build(menu, list.reference, song, cx),
                        None => menu,
                    }
                },
            );
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            // D-16: khung cuộn chạm hai mép màn, lề `PAGE_PADDING` nằm trong danh sách (`padding_x`); dòng tiêu đề cột tự lùi
            // cùng số đó để thẳng cột.
            // Cuộn bằng bánh xe hoặc kéo thanh cuộn: đọc lại vị trí sau khi danh sách đã xử lý (con chạy trước).
            .on_scroll(cx.listener(|this, _: &ScrollEvent, _, cx| this.sync_header(cx)))
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| this.sync_header(cx)))
            .child(div().px(px(PAGE_PADDING)).child(header_row(artist, c)))
            .child(rows)
            .into_any_element()
    }
}

impl Render for SongList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let accent = self.summary.as_ref().and_then(accent_of).unwrap_or(c.primary);
        let glow = div().absolute().top_0().left_0().right_0().h(px(GLOW_HEIGHT)).bg(linear_gradient(
            180.,
            linear_color_stop(accent.opacity(GLOW_OPACITY), 0.),
            linear_color_stop(accent.opacity(0.), 1.),
        ));
        let body = match self.status {
            Status::Loading => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(ProgressCircular::new("song-list-loading").scale(0.8))
                .child(div().text_size(px(12.)).text_color(c.muted).child(t!("song_list.loading.title")))
                .into_any_element(),
            Status::Failed => {
                centered_message(GrooveIcon::Album, t!("song_list.error.title"), t!("song_list.error.hint"), &c)
            }
            Status::Gone => {
                centered_message(GrooveIcon::Album, t!("song_list.gone.title"), t!("song_list.gone.hint"), &c)
            }
            Status::Ready if self.songs.is_empty() => {
                centered_message(GrooveIcon::Song, t!("song_list.empty.title"), t!("song_list.empty.hint"), &c)
            }
            Status::Ready => self.table(window, &c, cx),
        };
        let header = (self.status != Status::Gone).then(|| self.header(&c, cx));
        div()
            .id("song-list")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_color(c.text)
            .child(glow)
            .children(header)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use ::gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_library::db::queries::{self, SongUpsert};
    use groove_library::store::LibraryConfig;
    use tempfile::TempDir;

    use super::*;
    use crate::player::PlayerDeps;

    #[test]
    fn header_hysteresis_collapses_at_120_and_expands_below_15() {
        // Mở: dưới / đúng ngưỡng thì giữ, quá ngưỡng (và còn chỗ cuộn) thì thu.
        assert!(!next_collapsed(false, 0., 5000.));
        assert!(!next_collapsed(false, 120., 5000.));
        assert!(next_collapsed(false, 121., 5000.));
        // Thu: còn cuộn từ 15 trở lên thì giữ, về dưới 15 thì mở.
        assert!(next_collapsed(true, 121., 5000.));
        assert!(next_collapsed(true, 15., 5000.));
        assert!(!next_collapsed(true, 14.9, 5000.));
        // Vùng giữa hai ngưỡng không đổi trạng thái (không rung).
        for top in [16., 60., 119.] {
            assert!(!next_collapsed(false, top, 5000.));
            assert!(next_collapsed(true, top, 5000.));
        }
    }

    #[test]
    fn short_lists_never_collapse_into_a_flip_flop() {
        // Tầm cuộn chỉ 130 px: thu đi còn 130 - 96 = 34 > 16 nên vẫn thu; 100 px thì thu xong chỉ còn 4 ⇒ không thu.
        assert!(next_collapsed(false, 125., 130.));
        assert!(!next_collapsed(false, 125., 100.));
        assert!(!next_collapsed(false, 125., 0.));
    }

    #[test]
    fn click_toggles_the_current_track_and_plays_others() {
        let (a, b) = (TrackRef::Library(1), TrackRef::Library(2));
        assert_eq!(click_action(Some(&a), &a), ClickAction::Toggle);
        assert_eq!(click_action(Some(&a), &b), ClickAction::PlayList);
        assert_eq!(click_action(None, &a), ClickAction::PlayList);
    }

    /// Mọi khoá chữ view dùng phải có trong catalog (thiếu thì `t!` âm thầm trả chính khoá).
    #[test]
    fn every_key_exists_in_both_languages() {
        use groove_core::i18n::t;
        let keys = [
            "song_list.kind.album",
            "song_list.kind.folder",
            "song_list.kind.playlist",
            "song_list.header.items",
            "song_list.header.play_all",
            "song_list.column.index",
            "song_list.column.title",
            "song_list.column.duration",
            "song_list.column.artist",
            "song_list.loading.title",
            "song_list.empty.title",
            "song_list.empty.hint",
            "song_list.empty_play.message",
            "song_list.error.title",
            "song_list.error.hint",
            "song_list.gone.title",
            "song_list.gone.hint",
        ];
        for key in keys {
            assert_ne!(t(key), key, "thiếu `{key}`");
        }
    }

    // ---- Test view trong App ảo: thư viện thật ở thư mục tạm, `FakeOutput` (không ra loa, không chạm DB người dùng) ----

    struct Env {
        _dir: TempDir,
        library: Library,
        covers: CoverStore,
    }

    fn env() -> Env {
        let dir = TempDir::new().unwrap();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let covers = CoverStore::new(dir.path().join("covers"));
        let library = Library::open(config, Box::new(CoverStore::new(dir.path().join("covers")))).unwrap();
        Env { _dir: dir, library, covers }
    }

    /// Một album `Abbey` với các bài `names` (đường dẫn không có tệp: player không bao giờ phải mở chúng trong test này).
    /// Trả id album. `title`/`artist` rỗng để thử chữ thay thế.
    fn seed(env: &Env, names: Vec<String>) -> i64 {
        // Ít bài thì có tệp wav thật (player phát được, không nhảy bài vì lỗi); nhiều bài thì chỉ là dòng trong DB.
        let root = env._dir.path().join("music");
        std::fs::create_dir_all(&root).unwrap();
        if names.len() <= 10 {
            for name in &names {
                let sample =
                    groove_library::fixtures::write_sample(&root, name, groove_library::fixtures::AudioKind::Wav)
                        .unwrap();
                assert!(sample.exists());
            }
        }
        let library = &env.library;
        library
            .writer()
            .transaction(move |conn| {
                let folder = queries::get_or_create_folder(conn, "Rock", &root)?;
                let album = queries::get_or_create_album(conn, "Abbey", "Abbey", Some("The Beatles"))?;
                for name in &names {
                    let song = SongUpsert {
                        path: root.join(format!("{name}.wav")),
                        folder_id: folder,
                        album_id: Some(album),
                        title: name.starts_with("t-").then(|| name.clone()),
                        duration_sec: 125.,
                        ..SongUpsert::default()
                    };
                    queries::upsert_song(conn, &song)?;
                }
                Ok(album)
            })
            .wait()
            .unwrap()
    }

    type Events = Rc<RefCell<Vec<SongListEvent>>>;

    fn open<'a>(
        cx: &'a mut TestAppContext,
        env: &Env,
        reference: CategoryRef,
    ) -> (Entity<SongList>, Events, &'a mut VisualTestContext) {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.update(materiant::init);
        // Luồng đọc của thư viện đánh thức future từ thread khác.
        cx.executor().allow_parking();
        let deps = PlayerDeps {
            library: Some(env.library.clone()),
            output: Box::new(FakeOutput::new(48_000, 2)),
            options: EngineOptions::default(),
            session_file: None,
        };
        cx.update(|cx| {
            player::install_for_test(deps, cx);
        });
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let (view, cx) =
            cx.add_window_view(move |_, cx| SongList::with_deps(Some(library), Some(covers), reference, cx));
        let events: Events = Rc::default();
        let log = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&view, move |_, event: &SongListEvent, _| log.borrow_mut().push(event.clone())).detach()
        });
        wait_until(cx, "lần đọc đầu", |cx| view.read_with(cx, |v, _| v.status != Status::Loading));
        (view, events, cx)
    }

    fn frame(cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    fn wait_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if done(cx) {
                frame(cx);
                return;
            }
            assert!(Instant::now() < deadline, "quá hạn chờ: {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn selector(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    fn row_center(cx: &mut VisualTestContext, number: usize) -> ::gpui::Point<::gpui::Pixels> {
        cx.debug_bounds(selector(format!("song-row-{number}")))
            .unwrap_or_else(|| panic!("dòng {number} chưa vẽ"))
            .center()
    }

    fn album(id: i64) -> CategoryRef {
        CategoryRef::Album(id)
    }

    /// C1 — nạp đúng bài theo thứ tự tên tệp, đầu danh sách có tên và số bài; chữ thay thế đúng.
    #[::gpui::test]
    fn loads_songs_in_filename_order(cx: &mut TestAppContext) {
        let env = env();
        let id = seed(&env, vec!["b-two".into(), "a-one".into(), "t-Titled".into()]);
        let (view, _, cx) = open(cx, &env, album(id));
        view.read_with(cx, |v, _| {
            let names: Vec<_> = v.songs.iter().map(|s| s.filename.clone()).collect();
            assert_eq!(names, ["a-one.wav", "b-two.wav", "t-Titled.wav"]);
            assert_eq!(v.summary.as_ref().map(|s| s.name.as_str()), Some("Abbey"));
            assert_eq!(v.summary.as_ref().map(|s| s.song_count), Some(3));
            assert_eq!(v.status, Status::Ready);
        });
        assert!(cx.debug_bounds("song-row-1").is_some());
    }

    /// C2 — bấm bài khác phát với cả danh sách làm hàng đợi, bài bấm là bài hiện tại.
    #[::gpui::test]
    fn clicking_a_row_plays_it_with_the_whole_list(cx: &mut TestAppContext) {
        let env = env();
        let id = seed(&env, vec!["a".into(), "b".into(), "c".into()]);
        let (view, _, cx) = open(cx, &env, album(id));
        let tracks: Vec<TrackRef> = view.read_with(cx, |v, _| v.tracks());
        let p = cx.update(|_, cx| player::player(cx));

        let at = row_center(cx, 2);
        cx.simulate_mouse_move(at, None, Modifiers::default());
        cx.simulate_click(at, Modifiers::default());
        cx.run_until_parked();
        p.read_with(cx, |p, _| {
            assert_eq!(p.queue().current(), Some(&tracks[1]));
            assert_eq!(p.queue().original(), tracks.as_slice());
        });
        view.read_with(cx, |v, _| assert_eq!(v.selected, Some(1)));
    }

    /// C3 — ↓ chỉ di con trỏ (không phát); Enter phát bài đó.
    #[::gpui::test]
    fn arrow_keys_move_and_enter_plays(cx: &mut TestAppContext) {
        let env = env();
        let id = seed(&env, vec!["a".into(), "b".into(), "c".into()]);
        let (view, _, cx) = open(cx, &env, album(id));
        let tracks: Vec<TrackRef> = view.read_with(cx, |v, _| v.tracks());
        let p = cx.update(|_, cx| player::player(cx));
        let focus = view.read_with(cx, |v, _| v.focus.clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        frame(cx);

        cx.simulate_keystrokes("down down");
        frame(cx);
        view.read_with(cx, |v, _| assert_eq!(v.selected, Some(1)));
        p.read_with(cx, |p, _| assert_eq!(p.queue().current(), None, "↓ không được phát"));

        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        p.read_with(cx, |p, _| assert_eq!(p.queue().current(), Some(&tracks[1])));
    }

    /// C4 — Play all phát bài đầu với cả danh sách; danh sách rỗng thì không phát gì (và không hỏng).
    #[::gpui::test]
    fn play_all_queues_the_whole_list_and_empty_does_nothing(cx: &mut TestAppContext) {
        let env = env();
        let id = seed(&env, vec!["a".into(), "b".into()]);
        let (view, _, cx) = open(cx, &env, album(id));
        let tracks: Vec<TrackRef> = view.read_with(cx, |v, _| v.tracks());
        let p = cx.update(|_, cx| player::player(cx));
        cx.update(|window, cx| view.update(cx, |v, cx| v.play_all(window, cx)));
        cx.run_until_parked();
        p.read_with(cx, |p, _| {
            assert_eq!(p.queue().current(), Some(&tracks[0]));
            assert_eq!(p.queue().original(), tracks.as_slice());
        });

        let empty = env.library.create_playlist("Trống", "#ff0000").wait().unwrap();
        view.update(cx, |v, cx| v.set_reference(CategoryRef::Playlist(empty), cx));
        wait_until(cx, "playlist trống", |cx| {
            view.read_with(cx, |v, _| v.status == Status::Ready && v.reference() == CategoryRef::Playlist(empty))
        });
        view.read_with(cx, |v, _| assert!(v.songs.is_empty()));
        cx.update(|window, cx| view.update(cx, |v, cx| v.play_all(window, cx)));
        cx.run_until_parked();
        p.read_with(cx, |p, _| assert_eq!(p.queue().current(), Some(&tracks[0]), "toast, không đổi hàng đợi"));
    }

    /// C5 — chuột phải chọn đúng bài cho menu bài.
    #[::gpui::test]
    fn right_click_reports_the_song(cx: &mut TestAppContext) {
        let env = env();
        let id = seed(&env, vec!["a".into(), "b".into()]);
        let (view, _, cx) = open(cx, &env, album(id));
        let at = row_center(cx, 2);
        cx.simulate_mouse_move(at, None, Modifiers::default());
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
        view.read_with(cx, |v, _| {
            assert_eq!(v.selected, Some(1));
            assert_eq!(v.menu_song().map(|song| song.filename.as_str()), Some("b.wav"));
        });
    }

    /// C6 — nhiều bài mà chỉ dựng dòng trong khung.
    #[::gpui::test]
    fn renders_only_visible_rows(cx: &mut TestAppContext) {
        let env = env();
        let names = (0..3000).map(|n| format!("song-{n:05}")).collect();
        let id = seed(&env, names);
        let (view, _, cx) = open(cx, &env, album(id));
        view.read_with(cx, |v, _| assert_eq!(v.songs.len(), 3000));
        assert!(cx.debug_bounds("song-row-1").is_some());
        assert!(cx.debug_bounds("song-row-2999").is_none(), "dòng ngoài khung không được dựng");
        let rendered = (1..=3000).filter(|n| cx.debug_bounds(selector(format!("song-row-{n}"))).is_some()).count();
        assert!(rendered < 40, "dựng {rendered} dòng");
    }

    /// C7 — cuộn quá 120 px thì đầu danh sách thu; về đầu thì mở lại.
    #[::gpui::test]
    fn header_collapses_when_scrolled_and_expands_back(cx: &mut TestAppContext) {
        let env = env();
        let names = (0..300).map(|n| format!("song-{n:03}")).collect();
        let id = seed(&env, names);
        let (view, _, cx) = open(cx, &env, album(id));
        view.read_with(cx, |v, _| assert!(!v.collapsed));

        view.update(cx, |v, cx| {
            v.list.scroll_to(100, materiant::list::Align::Start);
            v.sync_header(cx);
        });
        frame(cx);
        view.read_with(cx, |v, _| assert!(v.collapsed, "cuộn xa ⇒ thu"));

        view.update(cx, |v, cx| {
            v.list.scroll_to(0, materiant::list::Align::Start);
            v.sync_header(cx);
        });
        frame(cx);
        view.read_with(cx, |v, _| assert!(!v.collapsed, "về đầu ⇒ mở"));
    }

    /// C8 — thư viện đổi thì nạp lại; mục bị xoá thì báo `CategoryGone`.
    #[::gpui::test]
    fn reloads_on_change_and_reports_a_vanished_category(cx: &mut TestAppContext) {
        let env = env();
        let playlist = env.library.create_playlist("Gym", "#ff0000").wait().unwrap();
        let id = seed(&env, vec!["a".into()]);
        let song = env.library.song_ids(album(id)).wait().unwrap()[0];
        let (view, events, cx) = open(cx, &env, CategoryRef::Playlist(playlist));
        view.read_with(cx, |v, _| assert!(v.songs.is_empty()));

        env.library.add_song_to_playlist(playlist, song).wait().unwrap();
        wait_until(cx, "bài mới trong playlist", |cx| view.read_with(cx, |v, _| v.songs.len() == 1));

        env.library.delete_playlist(playlist).wait().unwrap();
        wait_until(cx, "playlist biến mất", |cx| view.read_with(cx, |v, _| v.status == Status::Gone));
        assert!(
            matches!(events.borrow().last(), Some(SongListEvent::CategoryGone(r)) if *r == CategoryRef::Playlist(playlist))
        );
    }
}
