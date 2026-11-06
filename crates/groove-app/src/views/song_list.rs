//! groove-gd1_16: danh sách bài của một album, thư mục hay playlist (đặc tả 1.2, bản cũ `music-list.vue`).
//!
//! Đầu danh sách (bìa, loại, tên, dòng phụ, nút Play all) dính trên đầu và thu lại khi cuộn quá 120 px, mở lại khi về dưới
//! 15 px (hai ngưỡng để không rung); quầng accent 380 px, độ đục 0,4 phía sau. Bảng bài là `materiant::list::VirtualList`:
//! chỉ dựng dòng trong khung nên 10 000 bài tốn như 100.
//!
//! banner-anim (04/11/2026): đầu danh sách là `views::banner::Banner` phủ lên đầu bảng, thu / nở mượt 300 ms. Bảng bài nằm trong
//! `Island` `cached`, khung bắt đầu ngay dưới đầu danh sách ở trạng thái thu; trạng thái mở lùi bảng thêm phần chênh hai trạng
//! thái (`table_part`). Bảng chỉ dời một lần ở lúc đổi trạng thái — lúc thu bù vị trí cuộn để các dòng đứng yên, banner co lại
//! để lộ dòng bên dưới; lúc nở (đã gần đầu danh sách) các dòng xuống ngay chỗ mới, banner nở vào khoảng trống.
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

use std::cell::Cell;
use std::rc::Rc;
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
    AnyElement, App, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement,
    MouseMoveEvent, ParentElement, Render, ScrollEvent, SharedString, Styled, Subscription, Task, Window, canvas, div,
    linear_color_stop, linear_gradient, px,
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
use crate::views::banner::Banner;
use crate::views::category_card::{accent_of, cover_path, subtitle};
use crate::views::cover_image::{CoverImage, CoverKind};
use crate::views::island::{self, Island};
use crate::views::layout;
use crate::views::song_row::{ROW_HEIGHT, RowPlay, SongRow, header_row, show_artist};

/// Lề hai bên của cả màn.
const PAGE_PADDING: f32 = 24.;
/// Đầu danh sách mở (bìa 160) và thu (bìa 80): cao thấp nhất của hai trạng thái. Tên dài xuống dòng thì cao thêm; cao thật
/// đo lúc vẽ (`BannerHeights`).
const EXPANDED_COVER: f32 = 160.;
const COLLAPSED_COVER: f32 = 80.;
const HEADER_VERTICAL_PADDING: f32 = 12.;
const EXPANDED_HEIGHT: f32 = EXPANDED_COVER + 2. * HEADER_VERTICAL_PADDING + 16.;
const COLLAPSED_HEIGHT: f32 = COLLAPSED_COVER + 2. * HEADER_VERTICAL_PADDING;
/// Cỡ chữ tên, khe bìa – chữ của hai trạng thái (web `text-4xl` / `text-xl`, `gap-6` / `gap-4`).
const EXPANDED_TITLE: f32 = 36.;
const COLLAPSED_TITLE: f32 = 20.;
const EXPANDED_GAP: f32 = 24.;
const COLLAPSED_GAP: f32 = 16.;
/// Cao dòng loại (Album / Folder / Playlist) trên tên ở trạng thái mở; thu thì co về 0 và mờ dần.
const KIND_LINE: f32 = 16.;
/// Cao hàng nút Play all (đệm trên 12 + nút) ở trạng thái mở; thu thì co về 0 và mờ dần (web `max-h-12` → `max-h-0`).
const PLAY_ROW: f32 = 48.;
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

/// Đầu danh sách thu lại hay mở ra sau khi cuộn tới `scroll_top`; `shrink` là phần cao đầu danh sách mất đi khi thu.
///
/// Thu làm khung bảng cao thêm `shrink` phía trên và vị trí cuộn lùi đúng chừng đó (để các dòng đứng yên): chỉ thu khi sau khi
/// lùi vị trí cuộn vẫn còn trên ngưỡng mở lại, không thì mở rồi thu rồi mở mãi (tên dài làm đầu danh sách cao).
fn next_collapsed(collapsed: bool, scroll_top: f32, shrink: f32) -> bool {
    if collapsed {
        scroll_top >= EXPAND_BELOW
    } else {
        scroll_top > COLLAPSE_ABOVE && scroll_top - shrink > EXPAND_BELOW + 1.
    }
}

/// Cao thật (đo lúc vẽ) của đầu danh sách ở trạng thái mở và thu; chưa đo thì là cao thấp nhất.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BannerHeights {
    expanded: f32,
    collapsed: f32,
}

impl Default for BannerHeights {
    fn default() -> Self {
        BannerHeights { expanded: EXPANDED_HEIGHT, collapsed: COLLAPSED_HEIGHT }
    }
}

impl BannerHeights {
    fn shrink(self) -> f32 {
        (self.expanded - self.collapsed).max(0.)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
    /// Trạng thái đích của đầu danh sách (banner có thể còn đang chuyển tới đó).
    collapsed: bool,
    banner: Entity<Banner<SongList>>,
    banner_heights: Rc<Cell<BannerHeights>>,
    /// Bảng bài (`cached`): khung chuyển của banner không dựng lại nó.
    table: Entity<Island<SongList>>,
    #[cfg(test)]
    table_renders: usize,
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
            banner: Banner::new(SongList::banner_part, cx),
            banner_heights: Rc::default(),
            table: Island::new(SongList::table_part, cx),
            #[cfg(test)]
            table_renders: 0,
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
        self.list.remeasure();
        self.selected = None;
        self.collapsed = false;
        self.banner.update(cx, |banner, cx| banner.jump_to(false, cx));
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
                // Bài khác ⇒ chữ khác ⇒ cao dòng khác (`measured_rows`).
                self.list.remeasure();
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

    /// Đọc vị trí cuộn, thu hoặc mở đầu danh sách theo hai ngưỡng. Gọi sau mọi thứ có thể đổi vị trí cuộn; chỉ làm gì khi qua
    /// ngưỡng (cuộn từng px không `notify`).
    fn sync_header(&mut self, cx: &mut Context<Self>) {
        let shrink = self.banner_heights.get().shrink();
        let collapsed = next_collapsed(self.collapsed, self.list.scroll_top(), shrink);
        if collapsed == self.collapsed {
            return;
        }
        self.collapsed = collapsed;
        if collapsed {
            // Khung bảng cao thêm `shrink` phía trên (`table_part`): lùi vị trí cuộn chừng đó để các dòng đứng yên trên màn.
            self.list.scroll_by(-shrink);
        }
        self.banner.update(cx, |banner, cx| banner.animate_to(collapsed, cx));
        // Bảng dời một lần (đệm trên đổi); các khung chuyển sau đó chỉ vẽ lại banner.
        cx.notify();
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
    /// Đầu danh sách ở tiến độ `p` (0 mở, 1 thu; `views::banner`). Nền đặc + bản sao quầng accent: lúc chuyển banner phủ lên
    /// các dòng bên dưới, nền trong suốt sẽ lộ chữ chồng nhau (D-10: khối nổi nền đặc thay `backdrop-blur` của web).
    fn banner_part(&mut self, p: f32, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.status == Status::Gone {
            return div().into_any_element();
        }
        let c = cx.theme().colors().clone();
        let heights = self.banner_heights.clone();
        let host = cx.entity().downgrade();
        // Đo cao thật ở hai trạng thái đứng yên (tên dài xuống dòng): khác số đang dùng thì chủ vẽ lại bảng một lần.
        let measure = (p == 0. || p == 1.).then(|| {
            canvas(
                move |bounds, _, cx| {
                    let height = bounds.size.height.as_f32();
                    let mut now = heights.get();
                    let slot = if p == 0. { &mut now.expanded } else { &mut now.collapsed };
                    if (*slot - height).abs() > 0.5 {
                        *slot = height;
                        heights.set(now);
                        let host = host.clone();
                        cx.defer(move |cx| {
                            host.update(cx, |_, cx| cx.notify()).ok();
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        });
        div()
            .id("song-list-banner")
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(c.surface)
            // Rê / nhấp trên banner không lọt xuống dòng đang nằm dưới lúc chuyển; bánh xe vẫn cuộn bảng.
            .block_mouse_except_scroll()
            .child(self.glow(&c))
            .child(div().relative().child(self.header(p, &c, cx)).children(measure))
            // Kiểu lưới: đường kẻ tách đầu danh sách khỏi bảng bài (`views::layout`).
            .children(layout::hline(cx))
            .into_any_element()
    }

    /// Quầng accent sau đầu trang (đặc tả 1.2).
    fn glow(&self, c: &ThemeColors) -> Div {
        let accent = self.summary.as_ref().and_then(accent_of).unwrap_or(c.primary);
        div().absolute().top_0().left_0().right_0().h(px(GLOW_HEIGHT)).bg(linear_gradient(
            180.,
            linear_color_stop(accent.opacity(GLOW_OPACITY), 0.),
            linear_color_stop(accent.opacity(0.), 1.),
        ))
    }

    fn header(&self, p: f32, c: &ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let collapsed = p >= 0.5;
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
        let side = lerp(EXPANDED_COVER, COLLAPSED_COVER, p);

        // Dòng loại co về 0 và mờ dần (web `opacity-0 h-0`); thu hẳn thì bỏ.
        let label = (p < 1.).then(|| {
            div()
                .flex_none()
                .h(px(KIND_LINE * (1. - p)))
                .overflow_hidden()
                .opacity(1. - p)
                .text_size(px(12.))
                .text_color(c.primary_ink)
                .child(kind_label(self.reference))
        });
        // Dòng phụ đổi nội dung ở giữa chừng như web (`v-if`), cỡ chữ chuyển dần.
        let info_size = px(lerp(14., 12., p));
        let info = if collapsed {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(info_size)
                .text_color(c.muted)
                .child(div().text_color(c.primary_ink).child(kind_label(self.reference)))
                .child("•")
                .when(count > 0, |row| row.child(count_text.clone()))
        } else {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(info_size)
                .text_color(c.muted)
                .when_some(sub, |row, sub| row.child(div().min_w_0().child(sub)).child("•"))
                .child(count_text)
        };

        let text = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .justify_end()
            .gap_1()
            .children(label)
            .child(
                // Tên dài xuống dòng (không cắt "…"); khung đầu cao thêm theo chữ.
                div()
                    .font_weight(FontWeight::NORMAL)
                    .text_size(px(lerp(EXPANDED_TITLE, COLLAPSED_TITLE, p)))
                    .text_color(c.text)
                    .child(title),
            )
            .child(info)
            // Hàng nút co về 0 và mờ dần (web `max-h-0 opacity-0`); đứng yên ở trạng thái mở thì không chặn cao.
            .when(p < 1., |col| {
                col.child(
                    div()
                        .flex_none()
                        .when(p > 0., |row| row.max_h(px(PLAY_ROW * (1. - p))).overflow_hidden())
                        .opacity(1. - p)
                        .child(
                            div().pt_3().flex().child(
                                Button::new("song-list-play-all", t!("song_list.header.play_all"))
                                    .filled()
                                    .icon(GrooveIcon::Play)
                                    .disabled(count == 0)
                                    .on_click(
                                        cx.listener(|this, _: &ClickEvent, window, cx| this.play_all(window, cx)),
                                    ),
                            ),
                        ),
                )
            });

        div()
            .flex_none()
            .min_h(px(lerp(EXPANDED_HEIGHT, COLLAPSED_HEIGHT, p)))
            .w_full()
            .px(px(PAGE_PADDING))
            .py(px(HEADER_VERTICAL_PADDING))
            .flex()
            .items_end()
            .gap(px(lerp(EXPANDED_GAP, COLLAPSED_GAP, p)))
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
        .measured_rows()
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

impl SongList {
    /// Phần bảng (`Island` `cached`): bảng bài hoặc thông báo trạng thái. Trạng thái mở thì lùi xuống phần chênh của đầu danh
    /// sách (khung bảng bắt đầu ở cao trạng thái thu).
    fn table_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        #[cfg(test)]
        {
            self.table_renders += 1;
        }
        // Hàng đang phát đặt anchor của `bars_overlay` (anchor ghi theo view đang vẽ: chính `Island` này); lần vẽ này ghi lại
        // nếu hàng còn hiện.
        crate::views::bars_overlay::forget(self.table.entity_id(), cx);
        let c = cx.theme().colors().clone();
        let inset = if self.collapsed || self.status == Status::Gone { 0. } else { self.banner_heights.get().shrink() };
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
        div().size_full().flex().flex_col().pt(px(inset)).text_color(c.text).child(body).into_any_element()
    }
}

impl Render for SongList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let gone = self.status == Status::Gone;
        // Khung bảng bắt đầu ngay dưới đầu danh sách ở trạng thái thu (cả đường kẻ của kiểu lưới); không đổi theo khung chuyển.
        let top =
            if gone { 0. } else { self.banner_heights.get().collapsed + if layout::is_grid(cx) { 1. } else { 0. } };
        div()
            .id("song-list")
            .relative()
            .size_full()
            .overflow_hidden()
            .text_color(c.text)
            .child(self.glow(&c))
            .child(
                div()
                    .absolute()
                    .top(px(top))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(island::cached(&self.table, |s| s.size_full())),
            )
            .when(!gone, |root| root.child(div().absolute().top_0().left_0().right_0().child(self.banner.clone())))
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
    use groove_core::test_dir::TestDir;
    use groove_library::db::queries::{self, SongUpsert};
    use groove_library::store::LibraryConfig;

    use super::*;
    use crate::player::PlayerDeps;

    #[test]
    fn header_hysteresis_collapses_at_120_and_expands_below_15() {
        let shrink = BannerHeights::default().shrink();
        // Mở: dưới / đúng ngưỡng thì giữ, quá ngưỡng thì thu.
        assert!(!next_collapsed(false, 0., shrink));
        assert!(!next_collapsed(false, 120., shrink));
        assert!(next_collapsed(false, 121., shrink));
        // Thu: còn cuộn từ 15 trở lên thì giữ, về dưới 15 thì mở.
        assert!(next_collapsed(true, 121., shrink));
        assert!(next_collapsed(true, 15., shrink));
        assert!(!next_collapsed(true, 14.9, shrink));
        // Vùng giữa hai ngưỡng không đổi trạng thái (không rung).
        for top in [16., 60., 119.] {
            assert!(!next_collapsed(false, top, shrink));
            assert!(next_collapsed(true, top, shrink));
        }
    }

    #[test]
    fn a_tall_banner_never_collapses_into_a_flip_flop() {
        // Thu lùi vị trí cuộn đúng phần cao mất đi: đầu danh sách mất 96 px ⇒ 125 - 96 = 29 > 16 vẫn thu; tên dài làm mất
        // 115 px ⇒ còn 10 < 15, thu xong lại mở ⇒ không thu.
        assert!(next_collapsed(false, 125., 96.));
        assert!(!next_collapsed(false, 125., 115.));
        assert!(next_collapsed(false, 140., 115.));
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
        _dir: TestDir,
        library: Library,
        covers: CoverStore,
    }

    fn env() -> Env {
        let dir = TestDir::new();
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

    /// banner-anim — qua ngưỡng thì banner chuyển vài khung rồi dừng hẳn; khung chuyển không dựng lại bảng bài (`cached`); lúc
    /// thu các dòng đứng yên trên màn (banner co lại để lộ dòng, không kéo bảng theo).
    #[::gpui::test]
    fn banner_transition_runs_briefly_without_rebuilding_the_table(cx: &mut TestAppContext) {
        let env = env();
        let names = (0..300).map(|n| format!("song-{n:03}")).collect();
        let id = seed(&env, names);
        let (view, _, cx) = open(cx, &env, album(id));
        cx.update(|window, _| window.activate_window());
        view.update(cx, |v, _| v.list.scroll_to(100, materiant::list::Align::Start));
        frame(cx);
        frame(cx);
        let row_y = |cx: &mut VisualTestContext| row_center(cx, 103).y.as_f32();
        let before = row_y(cx);

        // Khung thật: tới giờ thì gọi lại view đã xin khung, vẽ không `refresh` (view `cached` không bẩn thì dùng lại).
        let tick = |cx: &mut VisualTestContext| -> usize {
            cx.executor().advance_clock(Duration::from_millis(16));
            let delivered = cx.update(|window, cx| {
                let delivered = window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
                delivered
            });
            cx.run_until_parked();
            delivered
        };
        let start = view.read_with(cx, |v, _| v.table_renders);
        view.update(cx, |v, cx| v.sync_header(cx));
        view.read_with(cx, |v, cx| {
            assert!(v.collapsed && v.banner.read(cx).is_animating(), "qua ngưỡng ⇒ bắt đầu thu")
        });
        // Lúc đổi trạng thái bảng dời một lần: dựng lại vài lần trong hai khung đầu (đệm trên đổi, danh sách đo lại theo khung
        // mới), rồi thôi.
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let mut frames = 1;
        assert!(tick(cx) > 0, "banner xin khung");
        let renders = view.read_with(cx, |v, _| v.table_renders);
        assert!(renders - start <= 3, "đổi trạng thái dựng bảng {} lần", renders - start);
        assert!((row_y(cx) - before).abs() < 0.5, "thu: dòng đứng yên ({before} → {})", row_y(cx));

        while tick(cx) > 0 {
            frames += 1;
            assert!(frames < 40, "chuyển phải dừng");
        }
        assert!((10..=25).contains(&frames), "300 ms ≈ 19 khung, chạy {frames}");
        view.read_with(cx, |v, cx| {
            assert!(!v.banner.read(cx).is_animating());
            assert_eq!(v.table_renders, renders, "khung chuyển không dựng lại bảng");
        });
        assert!((row_y(cx) - before).abs() < 0.5, "xong vẫn đứng yên");
        assert_eq!(tick(cx), 0, "đứng yên thì không xin khung nào nữa");
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
