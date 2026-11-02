//! groove-gd1_15: màn lưới My Music và Playlists (đặc tả 1.2).
//!
//! Một view chung cho `Slot::Library`, đọc route:
//!
//! - `Route::MyMusic`: tiêu đề, hai thẻ hành động (nhập thư mục, đọc lại thẻ), tab Album / Thư mục, lưới;
//! - `Route::Playlists`: tiêu đề, thẻ "tạo danh sách phát mới", lưới playlist;
//! - `Route::Category(..)`: danh sách bài của một category: `SongList` của task 16, tạo một lần, đổi mục bằng
//!   `set_reference` (task 17 gắn).
//!
//! Dữ liệu từ `app::library(cx)` (`Library`, đọc nền, UI không chạm đĩa); nạp lại khi `Opened`, `Synced`,
//! `PlaylistsChanged`, `Moved`, khi sửa thẻ cả nhóm xong. Lưới là `VirtualGrid` với một `LruImageCache` cho bìa. Nghiệp vụ (nhập, quét lại, tạo / xoá
//! playlist, phát) không nằm ở đây: view phát [`LibraryPageEvent`], `library_ctl` nghe (task 17).
//!
//! # Cho task sau
//!
//! - `views::library_page::install(cx)` gọi một lần trong `app::run` (đã nối): mở `Library` chuẩn, đặt global
//!   `app::library(cx)`, tạo view, gắn vào `Slot::Library`, nối route và `Open` thành `app::navigate`.
//! - `library_ctl::install` (gọi trong `install_with`) nghe `LibraryPageEvent` và báo ngược bằng `set_importing`,
//!   `set_rescanning`.

use std::sync::Arc;

use futures::StreamExt;
use groove_core::model::CategoryRef;
use groove_core::t;
use groove_library::actor::{LibraryError, LibraryEvent, OpenFailure, OpenStatus, ProgressKind};
use groove_library::covers::CoverStore;
use groove_library::db::queries::CategorySummary;
use groove_library::store::Library;
use materiant::button::Button;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyView, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Task, Window, div, px,
};
use materiant::icon::Icon;
use materiant::progress::ProgressCircular;
use materiant::tabs::Tabs;
use materiant::theme::{ActiveTheme, Neutral, ThemeColors};
use materiant::virtual_grid::{LruImageCache, VirtualGrid, VirtualGridHandle};

use crate::app::{self, ShellEvent, Slot};
use crate::icons::GrooveIcon;
use crate::library_ctl;
use crate::route::Route;
use crate::views::category_card::{CARD_EXTRA_HEIGHT, CategoryCard};
use crate::views::metadata::dialog::MetadataDialog;
use crate::views::song_list::{SongList, SongListEvent};

/// Lề hai bên của cả màn (đặc tả: `p-6`); lưới dùng cùng số này làm `padding_x` nên thanh cuộn nổi trên dải bên phải.
const PAGE_PADDING: f32 = 24.;
const GRID_GAP: f32 = 16.;
/// Ô rộng tối thiểu. 130 để cửa sổ nhỏ nhất (560 px, cột trái mở) vẫn ra 2 cột; trần 6 cột xem [`min_cell_width`].
const MIN_CELL_WIDTH: f32 = 130.;
const MAX_COLUMNS: f32 = 6.;
/// Bề ngang cột trái lúc thu: dùng để ước bề ngang vùng nội dung theo hướng rộng nhất, nên không bao giờ quá `MAX_COLUMNS`.
const NAV_COLLAPSED_WIDTH: f32 = crate::views::side_nav::COLLAPSED_WIDTH;
/// Số bìa giữ trong bộ nhớ: gấp 2–3 lần số ô trong một khung (6 cột × ~6 hàng), xem
/// `materiant/plan/completed/gd5/materiant-gd5_04_virtual-grid.md` mục 9. Bìa 256 px giải mã ≈ 256 KB.
const COVER_CACHE_CAPACITY: usize = 192;

/// Thứ view báo lên cho task 17 (nghiệp vụ) và cho `install` (điều hướng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryPageEvent {
    /// Bấm thẻ, hoặc Enter / nhấp đúp: vào danh sách bài.
    Open(CategoryRef),
    /// Nút Play all trên thẻ: phát bài đầu với cả danh sách làm hàng đợi; danh sách rỗng thì toast (task 17).
    PlayAll(CategoryRef),
    /// Nút Delete trên thẻ: hộp xác nhận rồi xoá (task 17).
    Delete(CategoryRef),
    /// Thẻ "Not finding everything?": nhập thư mục.
    Import,
    /// Thẻ "Albums look wrong?": đọc lại thẻ. Không phát lúc `rescanning` (bản cũ cũng bỏ qua).
    Rescan,
    /// Thẻ "Create a New Playlist": hộp tên + màu.
    CreatePlaylist,
    /// Mục đang xem không còn: `library_ctl` lùi về màn trước.
    CategoryGone(CategoryRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Albums,
    Folders,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Albums, Tab::Folders];

    fn index(self) -> usize {
        Tab::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
}

type Items = Arc<Vec<CategorySummary>>;

pub struct LibraryPage {
    library: Option<Library>,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    route: Route,
    status: OpenStatus,
    /// Đã có kết quả đọc đầu tiên (hoặc thất bại): trước đó hiện vòng quay thay vì "chưa có album".
    loaded: bool,
    albums: Items,
    folders: Items,
    playlists: Items,
    tab: Tab,
    selected: Option<usize>,
    importing: bool,
    rescanning: bool,
    /// Đang chờ `retry_open` / `start_fresh`.
    busy: bool,
    grid: VirtualGridHandle,
    focus: FocusHandle,
    /// Danh sách bài, dựng khi vào category lần đầu.
    song_list: Option<Entity<SongList>>,
    /// Vừa vào category: lần vẽ tới giao focus cho danh sách.
    focus_list: bool,
    /// Hộp thoại của `library_ctl`, vẽ ở mọi route của slot này.
    overlays: Vec<AnyView>,
    /// Hộp metadata đang mở; `LibraryCtl` giữ vòng đời.
    metadata_dialog: Option<Entity<MetadataDialog>>,
    load_task: Option<Task<()>>,
    _events: Option<Task<()>>,
    _song_list_events: Option<Subscription>,
}

impl EventEmitter<LibraryPageEvent> for LibraryPage {}

/// Bề ngang tối thiểu của ô để lưới không vượt `MAX_COLUMNS` cột, cho vùng cuộn rộng `content_width` (đã trừ lề).
/// `columns_for = floor((w + gap) / (min + gap))` nên cần `min + gap > (w + gap) / (MAX_COLUMNS + 1)`.
pub fn min_cell_width(content_width: f32) -> f32 {
    let capped = (content_width + GRID_GAP) / (MAX_COLUMNS + 1.) - GRID_GAP + 0.5;
    MIN_CELL_WIDTH.max(capped)
}

/// Bề ngang vùng cuộn của lưới ước từ bề ngang cửa sổ (cột trái thu, trừ lề hai bên).
fn content_width_estimate(window_width: f32) -> f32 {
    (window_width - NAV_COLLAPSED_WIDTH - 2. * PAGE_PADDING).max(0.)
}

impl LibraryPage {
    pub fn new(
        library: Option<Library>,
        covers: Option<CoverStore>,
        route: Route,
        cx: &mut Context<Self>,
    ) -> LibraryPage {
        let cache = LruImageCache::new(COVER_CACHE_CAPACITY, cx);
        let status = library.as_ref().map(Library::status).unwrap_or(OpenStatus::Closed);
        let mut page = LibraryPage {
            library,
            covers,
            cache,
            route,
            status,
            loaded: false,
            albums: Items::default(),
            folders: Items::default(),
            playlists: Items::default(),
            tab: Tab::Albums,
            selected: None,
            importing: false,
            rescanning: false,
            busy: false,
            grid: VirtualGridHandle::new(),
            focus: cx.focus_handle(),
            song_list: None,
            focus_list: matches!(route, Route::Category(_)),
            overlays: Vec::new(),
            metadata_dialog: None,
            load_task: None,
            _events: None,
            _song_list_events: None,
        };
        page.listen(cx);
        page.reload(cx);
        page
    }

    /// Nghe sự kiện thư viện: nạp lại khi đổi, cập nhật trạng thái mở.
    fn listen(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &self.library else { return };
        let mut events = library.subscribe();
        self._events = Some(cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |this, cx| this.on_event(event, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn on_event(&mut self, event: LibraryEvent, cx: &mut Context<Self>) {
        match event {
            LibraryEvent::Opened(_) | LibraryEvent::OpenFailed(_) => {
                self.refresh_status();
                self.reload(cx);
            }
            LibraryEvent::Synced(_) | LibraryEvent::PlaylistsChanged | LibraryEvent::Moved(_) => self.reload(cx),
            // Sửa thẻ cả nhóm xong: bìa, tên nhóm có thể đã đổi.
            LibraryEvent::Progress(progress)
                if progress.kind == ProgressKind::MetadataBulk && progress.done == progress.total =>
            {
                self.reload(cx)
            }
            LibraryEvent::Progress(_) => {}
        }
    }

    fn refresh_status(&mut self) {
        if let Some(library) = &self.library {
            self.status = library.status();
        }
    }

    fn failure(&self) -> Option<&OpenFailure> {
        match &self.status {
            OpenStatus::Failed(failure) => Some(failure),
            _ => None,
        }
    }

    /// Đọc lại lưới ở nền (UI không chạm đĩa). Lần đọc mới thay lần đang chạy.
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.failure().is_some() {
            self.loaded = true;
            cx.notify();
            return;
        }
        let Some(library) = self.library.clone() else {
            self.loaded = true;
            return;
        };
        self.load_task = Some(cx.spawn(async move |this, cx| {
            // Lúc `Opening` việc đọc tự chờ.
            let categories = library.categories().await;
            let playlists = library.playlists().await;
            this.update(cx, |this, cx| this.apply(categories, playlists, cx)).ok();
        }));
    }

    fn apply(
        &mut self,
        categories: Result<groove_library::db::queries::Categories, LibraryError>,
        playlists: Result<Vec<CategorySummary>, LibraryError>,
        cx: &mut Context<Self>,
    ) {
        match (categories, playlists) {
            (Ok(categories), Ok(playlists)) => {
                self.albums = Arc::new(categories.albums);
                self.folders = Arc::new(categories.folders);
                self.playlists = Arc::new(playlists);
            }
            (Err(err), _) | (_, Err(err)) => {
                if let LibraryError::Unavailable(failure) = &err {
                    self.status = OpenStatus::Failed(failure.clone());
                }
                // Chi tiết kỹ thuật chỉ vào log; người dùng thấy chữ i18n.
                log::warn!("library page: cannot read categories: {err}");
            }
        }
        self.loaded = true;
        self.clamp_selection();
        cx.notify();
    }

    fn clamp_selection(&mut self) {
        let count = self.cell_count();
        if self.selected.is_some_and(|ix| ix >= count) {
            self.selected = None;
        }
    }

    /// Route đổi (nghe `ShellEvent::RouteChanged`). Sang mục khác thì về tab Album, bỏ chọn; lùi từ danh sách bài về
    /// lưới cũ thì giữ tab và ô chọn.
    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        let section_changed = route.nav_section() != self.route.nav_section();
        self.route = route;
        self.focus_list = matches!(route, Route::Category(_));
        if section_changed && !matches!(route, Route::Category(_)) {
            self.tab = Tab::Albums;
            self.selected = None;
        }
        cx.notify();
    }

    /// Đang nhập thư mục: thêm ô chờ "Importing..." đầu lưới My Music (task 17 gọi).
    pub fn set_importing(&mut self, importing: bool, cx: &mut Context<Self>) {
        if self.importing != importing {
            self.importing = importing;
            self.clamp_selection();
            cx.notify();
        }
    }

    /// Đang đọc lại thẻ: thẻ hành động đổi chữ và bỏ qua nhấp (task 17 gọi).
    pub fn set_rescanning(&mut self, rescanning: bool, cx: &mut Context<Self>) {
        if self.rescanning != rescanning {
            self.rescanning = rescanning;
            cx.notify();
        }
    }

    /// Hộp thoại của `library_ctl`.
    pub fn set_overlays(&mut self, overlays: Vec<AnyView>, cx: &mut Context<Self>) {
        self.overlays = overlays;
        cx.notify();
    }

    /// Thêm một hộp thoại dựng muộn (lyrics, timeline).
    pub fn add_overlay(&mut self, overlay: AnyView, cx: &mut Context<Self>) {
        self.overlays.push(overlay);
        cx.notify();
    }

    pub fn set_metadata_dialog(&mut self, dialog: Option<Entity<MetadataDialog>>, cx: &mut Context<Self>) {
        self.metadata_dialog = dialog;
        cx.notify();
    }

    #[cfg(test)]
    pub fn has_metadata_dialog(&self) -> bool {
        self.metadata_dialog.is_some()
    }

    /// Thẻ của bài vừa sửa: nạp lại lưới và danh sách đang mở.
    pub fn refresh_lists(&mut self, cx: &mut Context<Self>) {
        self.reload(cx);
        if let Some(list) = &self.song_list {
            list.update(cx, |list, cx| list.refresh(cx));
        }
    }

    /// `SongList` của `reference`: dùng lại view đã có, chỉ đổi mục.
    fn song_list(&mut self, reference: CategoryRef, cx: &mut Context<Self>) -> Entity<SongList> {
        if let Some(list) = &self.song_list {
            list.update(cx, |list, cx| list.set_reference(reference, cx));
            return list.clone();
        }
        let (library, covers) = (self.library.clone(), self.covers.clone());
        let list = cx.new(|cx| SongList::with_deps(library, covers, reference, cx));
        self._song_list_events = Some(cx.subscribe(&list, |_, _, event: &SongListEvent, cx| {
            // Menu bài do `SongList` tự vẽ qua `song_menu`; ở đây chỉ còn điều hướng.
            if let SongListEvent::CategoryGone(reference) = event {
                cx.emit(LibraryPageEvent::CategoryGone(*reference));
            }
        }));
        self.song_list = Some(list.clone());
        list
    }

    fn is_playlists(&self) -> bool {
        self.route.nav_section() == Route::Playlists
    }

    /// Số ô chờ ở đầu lưới (chỉ My Music, lúc đang nhập).
    fn pending_cells(&self) -> usize {
        usize::from(self.importing && !self.is_playlists())
    }

    fn items(&self) -> &Items {
        if self.is_playlists() {
            &self.playlists
        } else if self.tab == Tab::Folders {
            &self.folders
        } else {
            &self.albums
        }
    }

    fn cell_count(&self) -> usize {
        self.items().len() + self.pending_cells()
    }

    /// Enter / nhấp đúp ô `ix` của lưới.
    fn activate(&mut self, ix: usize, cx: &mut Context<Self>) {
        let pending = self.pending_cells();
        if ix < pending {
            return;
        }
        if let Some(item) = self.items().get(ix - pending) {
            cx.emit(LibraryPageEvent::Open(item.reference));
        }
    }

    fn retry_open(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        self.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let outcome = library.retry_open().await;
            this.update(cx, |this, cx| this.open_finished(outcome.map(drop), cx)).ok();
        })
        .detach();
    }

    fn start_fresh(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        self.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let outcome = library.start_fresh().await;
            this.update(cx, |this, cx| this.open_finished(outcome.map(drop), cx)).ok();
        })
        .detach();
    }

    fn open_finished(&mut self, outcome: Result<(), LibraryError>, cx: &mut Context<Self>) {
        self.busy = false;
        if let Err(err) = outcome {
            log::warn!("library page: reopening the library failed: {err}");
        }
        self.refresh_status();
        self.reload(cx);
        cx.notify();
    }
}

/// Thẻ hành động ở đầu màn (bản cũ: khối `p-4` có icon, tên đậm, dòng gợi ý màu nhấn).
fn action_card(
    id: &'static str,
    icon: GrooveIcon,
    title: &'static str,
    hint: impl Into<SharedString>,
    c: &ThemeColors,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let hover = c.layer(Neutral::N100);
    div()
        .id(id)
        .flex_none()
        .max_w(px(360.))
        .flex()
        .items_start()
        .gap_3()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(c.border)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
        .child(div().flex_none().pt(px(2.)).child(Icon::new(icon).size(px(20.)).color(c.muted)))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c.text).child(title))
                .child(div().text_size(px(12.)).text_color(c.primary_ink).child(hint.into())),
        )
}

/// Icon + tiêu đề + gợi ý ở giữa vùng nội dung (lưới rỗng, lỗi mở thư viện).
fn centered_message(icon: GrooveIcon, title: &'static str, hint: &'static str, c: &ThemeColors) -> gpui::Div {
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
        .child(Icon::new(icon).size(px(64.)).color(c.muted.opacity(0.6)))
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c.muted).child(title))
        .child(div().max_w(px(420.)).text_size(px(12.)).text_color(c.muted.opacity(0.8)).child(hint))
}

impl LibraryPage {
    fn title(&self) -> &'static str {
        if self.is_playlists() { t!("library.title.playlists") } else { t!("library.title.my_music") }
    }

    fn header(&self, c: &ThemeColors) -> gpui::Div {
        div()
            .flex_none()
            .pt(px(PAGE_PADDING))
            .px(px(PAGE_PADDING))
            .pb_3()
            .text_size(px(28.))
            .font_weight(FontWeight::LIGHT)
            .text_color(c.text)
            .child(self.title())
    }

    fn banners(&self, c: &ThemeColors, cx: &mut Context<Self>) -> gpui::Div {
        let row = div().flex_none().flex().flex_wrap().gap_3().px(px(PAGE_PADDING)).pb_3();
        let this = cx.entity().downgrade();
        let emit = move |event: LibraryPageEvent| {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| {
                this.update(cx, |_, cx| cx.emit(event)).ok();
            }
        };
        if self.is_playlists() {
            return row.child(action_card(
                "banner-create-playlist",
                GrooveIcon::Album,
                t!("library.action.create_playlist.title"),
                t!("library.action.create_playlist.hint"),
                c,
                emit(LibraryPageEvent::CreatePlaylist),
            ));
        }
        let rescan_hint =
            if self.rescanning { t!("library.action.rescan.running") } else { t!("library.action.rescan.hint") };
        let rescanning = self.rescanning;
        let rescan = emit(LibraryPageEvent::Rescan);
        row.child(action_card(
            "banner-import",
            GrooveIcon::Folder,
            t!("library.action.import.title"),
            t!("library.action.import.hint"),
            c,
            emit(LibraryPageEvent::Import),
        ))
        .child(action_card(
            "banner-rescan",
            GrooveIcon::Loop,
            t!("library.action.rescan.title"),
            rescan_hint,
            c,
            move |window, cx| {
                if !rescanning {
                    rescan(window, cx);
                }
            },
        ))
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_none().px(px(PAGE_PADDING)).child(
            Tabs::new("library-tabs")
                .tab(t!("library.tab.albums"))
                .tab(t!("library.tab.folders"))
                .selected(self.tab.index())
                .on_change(cx.listener(|this, ix: &usize, _, cx| {
                    this.tab = Tab::ALL.get(*ix).copied().unwrap_or(Tab::Albums);
                    this.selected = None;
                    cx.notify();
                }))
                .w_full(),
        )
    }

    fn failure_panel(&self, failure: &OpenFailure, c: &ThemeColors, cx: &mut Context<Self>) -> gpui::Div {
        match failure {
            OpenFailure::TooNew { .. } => centered_message(
                GrooveIcon::DefaultCover,
                t!("library.open_error.too_new.title"),
                t!("library.open_error.too_new.hint"),
                c,
            ),
            OpenFailure::LegacyFailed { .. } => {
                let busy = self.busy;
                centered_message(
                    GrooveIcon::DefaultCover,
                    t!("library.open_error.legacy_failed.title"),
                    t!("library.open_error.legacy_failed.hint"),
                    c,
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .pt_3()
                        .child(
                            Button::new("library-retry-open", t!("library.open_error.legacy_failed.retry"))
                                .filled()
                                .loading(busy)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if !this.busy {
                                        this.retry_open(cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("library-start-fresh", t!("library.open_error.legacy_failed.start_fresh"))
                                .outlined()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if !this.busy {
                                        this.start_fresh(cx);
                                    }
                                })),
                        ),
                )
            }
            OpenFailure::NotLibrary(_) | OpenFailure::Other(_) => centered_message(
                GrooveIcon::DefaultCover,
                t!("library.open_error.other.title"),
                t!("library.open_error.other.hint"),
                c,
            ),
        }
    }

    fn grid(&self, window: &Window, c: &ThemeColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let items = self.items().clone();
        let pending = self.pending_cells();
        let count = items.len() + pending;
        if count == 0 {
            let (icon, title, hint) = if self.is_playlists() {
                (GrooveIcon::Album, t!("library.empty.playlists.title"), t!("library.empty.playlists.hint"))
            } else if self.tab == Tab::Folders {
                (GrooveIcon::Folder, t!("library.empty.folders.title"), t!("library.empty.folders.hint"))
            } else {
                (GrooveIcon::Album, t!("library.empty.albums.title"), t!("library.empty.albums.hint"))
            };
            return centered_message(icon, title, hint, c).into_any_element();
        }

        let this = cx.entity().downgrade();
        let covers = self.covers.clone();
        let cache = self.cache.clone();
        let colors = c.clone();
        let emit = {
            let this = this.clone();
            move |event: fn(CategoryRef) -> LibraryPageEvent| {
                let this = this.clone();
                move |reference: CategoryRef, _: &mut Window, cx: &mut App| {
                    this.update(cx, |_, cx| cx.emit(event(reference))).ok();
                }
            }
        };
        let cell = move |ix: usize, _: &mut Window, _: &mut App| -> gpui::AnyElement {
            if ix < pending {
                return importing_tile(&colors).into_any_element();
            }
            match items.get(ix - pending) {
                Some(summary) => CategoryCard::new(
                    summary,
                    covers.as_ref(),
                    &cache,
                    emit(LibraryPageEvent::Open),
                    emit(LibraryPageEvent::PlayAll),
                    emit(LibraryPageEvent::Delete),
                )
                .into_any_element(),
                None => div().into_any_element(),
            }
        };

        let content_width = content_width_estimate(window.viewport_size().width.into());
        VirtualGrid::new("library-grid", count, px(min_cell_width(content_width)), px(CARD_EXTRA_HEIGHT), cell)
            .height_per_width(1.)
            .gap(px(GRID_GAP))
            .padding_x(px(PAGE_PADDING))
            .track(&self.grid)
            .track_focus(&self.focus)
            .selected(self.selected)
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                this.selected = Some(*ix);
                cx.notify();
            }))
            .on_activate(cx.listener(|this, ix: &usize, _, cx| this.activate(*ix, cx)))
            .flex_1()
            .min_h_0()
            .into_any_element()
    }
}

/// Ô chờ lúc đang nhập: vòng quay + "Importing...".
fn importing_tile(c: &ThemeColors) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .w_full()
                .aspect_square()
                .rounded_lg()
                .bg(c.neutral(Neutral::N200))
                .flex()
                .items_center()
                .justify_center()
                .child(ProgressCircular::new("library-importing").scale(0.6)),
        )
        .child(
            div()
                .child(
                    div()
                        .h(px(20.))
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text)
                        .child(t!("library.importing.title")),
                )
                .child(
                    div()
                        .h(px(16.))
                        .truncate()
                        .text_size(px(12.))
                        .text_color(c.muted)
                        .child(t!("library.importing.hint")),
                ),
        )
}

impl Render for LibraryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let root = div()
            .id("library-page")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_color(c.text)
            .children(self.overlays.iter().map(|view| view.clone().into_any_element()))
            .children(self.metadata_dialog.clone());

        if let Route::Category(reference) = self.route {
            let list = self.song_list(reference, cx);
            if std::mem::take(&mut self.focus_list) {
                list.update(cx, |list, cx| list.focus(window, cx));
            }
            return root.child(list);
        }
        if self.library.is_none() {
            return root.child(self.header(&c)).child(centered_message(
                GrooveIcon::DefaultCover,
                t!("library.open_error.unavailable.title"),
                t!("library.open_error.unavailable.hint"),
                &c,
            ));
        }
        if let Some(failure) = self.failure().cloned() {
            return root.child(self.header(&c)).child(self.failure_panel(&failure, &c, cx));
        }

        let body = if !self.loaded {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(ProgressCircular::new("library-loading").scale(0.8))
                .into_any_element()
        } else {
            self.grid(window, &c, cx)
        };
        let root = root.child(self.header(&c)).child(self.banners(&c, cx));
        let root = if self.is_playlists() { root } else { root.child(self.tabs(cx)) };
        root.child(div().h(px(12.)).flex_none()).child(body)
    }
}

/// Mở `Library` chuẩn, đặt global `app::library(cx)`, tạo view và gắn vào `Slot::Library`. Gọi một lần trong `app::run`.
pub fn install(cx: &mut App) -> Entity<LibraryPage> {
    let library = Library::open_standard().map_err(|err| log::error!("cannot start the library: {err}")).ok();
    let covers = CoverStore::default_location().map_err(|err| log::warn!("cannot locate covers: {err}")).ok();
    install_with(library, covers, cx)
}

/// Như [`install`] nhưng với `Library` do nơi gọi cấp (test trỏ thư mục tạm).
pub fn install_with(library: Option<Library>, covers: Option<CoverStore>, cx: &mut App) -> Entity<LibraryPage> {
    if let Some(library) = &library {
        app::set_library(library.clone(), cx);
    }
    let route = app::current_route(cx);
    let (page_library, page_covers) = (library.clone(), covers.clone());
    let page = cx.new(|cx| LibraryPage::new(page_library, page_covers, route, cx));
    let weak = page.downgrade();
    cx.subscribe(&app::shell(cx), move |_, event: &ShellEvent, cx| {
        if let ShellEvent::RouteChanged(route) = event {
            weak.update(cx, |page, cx| page.set_route(*route, cx)).ok();
        }
    })
    .detach();
    cx.subscribe(&page, |_, event: &LibraryPageEvent, cx| {
        if let LibraryPageEvent::Open(reference) = event {
            app::navigate(Route::Category(*reference), cx);
        }
    })
    .detach();
    app::install_slot(Slot::Library, page.clone(), cx);
    // Giữ `Indexer`, watcher; đóng thư viện khi thoát (watcher dừng trước).
    library_ctl::install(library, covers, &page, cx);
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_never_exceeds_six_columns_nor_drops_below_two() {
        for width in [200., 456., 800., 1200., 1600., 2560., 5000.] {
            let columns = materiant::virtual_grid::columns_for(width, min_cell_width(width), GRID_GAP);
            assert!(columns <= 6, "{width} px ra {columns} cột");
        }
        // Cửa sổ nhỏ nhất, cột trái mở: 560 - 230 - 48 = 282.
        let narrow = materiant::virtual_grid::columns_for(282., min_cell_width(content_width_estimate(560.)), GRID_GAP);
        assert_eq!(narrow, 2);
        // Màn rộng, cột trái mở: đúng 6 cột, ô không nhỏ hơn mức tối thiểu.
        assert_eq!(materiant::virtual_grid::columns_for(2000., min_cell_width(2000.), GRID_GAP), 6);
        assert!(min_cell_width(100.) >= MIN_CELL_WIDTH);
    }

    #[test]
    fn content_width_never_negative() {
        assert_eq!(content_width_estimate(10.), 0.);
        assert_eq!(content_width_estimate(800.), 800. - NAV_COLLAPSED_WIDTH - 2. * PAGE_PADDING);
    }

    /// Mọi khoá chữ view dùng phải có trong catalog (thiếu thì `t!` âm thầm trả chính khoá). Test i18n đã bắt hai ngôn
    /// ngữ lệch bộ khoá nên kiểm một ngôn ngữ là đủ.
    #[test]
    fn every_key_exists_in_both_languages() {
        use groove_core::i18n::t;
        let keys = [
            "library.title.my_music",
            "library.title.playlists",
            "library.tab.albums",
            "library.tab.folders",
            "library.action.import.title",
            "library.action.import.hint",
            "library.action.rescan.title",
            "library.action.rescan.hint",
            "library.action.rescan.running",
            "library.action.create_playlist.title",
            "library.action.create_playlist.hint",
            "library.card.unknown_artist",
            "library.card.items",
            "library.card.play_all",
            "library.card.delete",
            "library.importing.title",
            "library.importing.hint",
            "library.empty.albums.title",
            "library.empty.albums.hint",
            "library.empty.folders.title",
            "library.empty.folders.hint",
            "library.empty.playlists.title",
            "library.empty.playlists.hint",
            "library.open_error.too_new.title",
            "library.open_error.too_new.hint",
            "library.open_error.legacy_failed.title",
            "library.open_error.legacy_failed.hint",
            "library.open_error.legacy_failed.retry",
            "library.open_error.legacy_failed.start_fresh",
            "library.open_error.other.title",
            "library.open_error.other.hint",
            "library.open_error.unavailable.title",
            "library.open_error.unavailable.hint",
        ];
        {
            for key in keys {
                assert_ne!(t(key), key, "thiếu `{key}`");
            }
        }
    }

    // ---- Test view trong App ảo, thư viện thật trong thư mục tạm (không chạm DB của người dùng) ----

    use std::cell::RefCell;
    use std::fs;
    use std::path::Path;
    use std::rc::Rc;
    use std::time::Duration;

    use ::gpui::{Modifiers, TestAppContext, VisualTestContext};
    use groove_library::db::queries::{self, SongUpsert};
    use groove_library::store::LibraryConfig;
    use tempfile::TempDir;

    struct Env {
        _dir: TempDir,
        library: Library,
        covers: CoverStore,
    }

    /// Thư viện mới trong thư mục tạm; `legacy` là nội dung tệp DB cũ (để ép lỗi chuyển).
    fn env(legacy: Option<&[u8]>) -> Env {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("Groove X").join("library.db");
        let legacy_path = dir.path().join("old").join("groovex.db");
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        if let Some(bytes) = legacy {
            fs::write(&legacy_path, bytes).unwrap();
        }
        let covers = CoverStore::new(dir.path().join("covers"));
        let config = LibraryConfig { target, legacy: Some(legacy_path) };
        let library = Library::open(config, Box::new(CoverStore::new(dir.path().join("covers")))).unwrap();
        Env { _dir: dir, library, covers }
    }

    /// Hai album (một không nghệ sĩ), một thư mục, một playlist.
    fn seed(library: &Library) {
        library
            .writer()
            .transaction(|conn| {
                let folder = queries::get_or_create_folder(conn, "Rock", Path::new("/music/rock"))?;
                for (name, artist) in [("Abbey", Some("The Beatles")), ("Zed", None)] {
                    let album = queries::get_or_create_album(conn, name, name, artist)?;
                    let song = SongUpsert {
                        path: format!("/music/rock/{name}.mp3").into(),
                        folder_id: folder,
                        album_id: Some(album),
                        ..SongUpsert::default()
                    };
                    queries::upsert_song(conn, &song)?;
                }
                queries::create_playlist(conn, "Gym", "#ff0000")?;
                Ok(())
            })
            .wait()
            .unwrap();
    }

    type Events = Rc<RefCell<Vec<LibraryPageEvent>>>;

    fn open_page<'a>(
        cx: &'a mut TestAppContext,
        env: &Env,
        route: Route,
    ) -> (Entity<LibraryPage>, Events, &'a mut VisualTestContext) {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.update(materiant::init);
        // Luồng đọc của thư viện đánh thức future từ thread khác: cho phép, không thì lịch test báo "không tất định".
        cx.executor().allow_parking();
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let (page, cx) = cx.add_window_view(move |_, cx| LibraryPage::new(Some(library), Some(covers), route, cx));
        let events: Events = Rc::default();
        let log = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&page, move |_, event: &LibraryPageEvent, _| log.borrow_mut().push(*event)).detach()
        });
        wait_loaded(&page, cx);
        (page, events, cx)
    }

    fn frame(cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    /// Luồng đọc chạy ở thread khác: lặp cho tới khi view nhận kết quả (trần ~5 s).
    fn wait_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        for _ in 0..500 {
            cx.run_until_parked();
            if done(cx) {
                frame(cx);
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("quá hạn chờ: {what}");
    }

    fn wait_loaded(page: &Entity<LibraryPage>, cx: &mut VisualTestContext) {
        wait_until(cx, "lần đọc đầu", |cx| page.read_with(cx, |p, _| p.loaded));
    }

    fn names(items: &Items) -> Vec<String> {
        items.iter().map(|i| i.name.clone()).collect()
    }

    /// Selector của `debug_bounds` cần `&'static str`; test nhỏ nên rò chuỗi cũng được.
    fn selector(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    /// C1 — lưới nạp album, thư mục, playlist theo tên; tab và route chọn đúng danh sách.
    #[::gpui::test]
    fn loads_albums_folders_and_playlists(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        page.read_with(cx, |p, _| {
            assert_eq!(names(&p.albums), ["Abbey", "Zed"]);
            assert_eq!(names(&p.folders), ["Rock"]);
            assert_eq!(names(&p.playlists), ["Gym"]);
            assert_eq!(names(p.items()), ["Abbey", "Zed"], "My Music mặc định tab Album");
            assert_eq!(p.cell_count(), 2);
        });
        page.update(cx, |p, cx| {
            p.tab = Tab::Folders;
            cx.notify();
        });
        page.read_with(cx, |p, _| assert_eq!(names(p.items()), ["Rock"]));
        page.update(cx, |p, cx| p.set_route(Route::Playlists, cx));
        page.read_with(cx, |p, _| {
            assert_eq!(names(p.items()), ["Gym"]);
            assert_eq!(p.tab, Tab::Albums, "sang mục khác ⇒ về tab Album");
        });
        frame(cx);
    }

    /// C2 — số cột đo được ở khung đầu khớp công thức (trần 6 cột kiểm ở test thuần).
    #[::gpui::test]
    fn grid_measures_columns_within_limits(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        let columns = page.read_with(cx, |p, _| p.grid.columns());
        let viewport: f32 = cx.update(|window, _| window.viewport_size().width.into());
        // Cửa sổ test không có cột trái nên rộng hơn ước lượng của app; công thức vẫn phải khớp số đo.
        let expected = materiant::virtual_grid::columns_for(
            viewport - 2. * PAGE_PADDING,
            min_cell_width(content_width_estimate(viewport)),
            GRID_GAP,
        );
        assert_eq!(columns, expected);
        assert!(columns >= 2);
    }

    /// C3 — bàn phím: ↓ chọn ô đầu, Enter mở đúng category; ô chờ "Importing..." không mở gì.
    #[::gpui::test]
    fn keyboard_opens_the_selected_card(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, events, cx) = open_page(cx, &env, Route::MyMusic);
        let focus = page.read_with(cx, |p, _| p.focus.clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        frame(cx);
        cx.simulate_keystrokes("down enter");
        frame(cx);
        let abbey = page.read_with(cx, |p, _| p.albums[0].reference);
        assert_eq!(*events.borrow(), [LibraryPageEvent::Open(abbey)]);

        // Đang nhập: ô chờ chiếm ô 0, các ô thật dịch một.
        events.borrow_mut().clear();
        page.update(cx, |p, cx| p.set_importing(true, cx));
        page.update(cx, |p, cx| {
            assert_eq!(p.cell_count(), 3);
            p.activate(0, cx);
        });
        assert!(events.borrow().is_empty(), "ô chờ không mở gì");
        page.update(cx, |p, cx| p.activate(1, cx));
        assert_eq!(*events.borrow(), [LibraryPageEvent::Open(abbey)]);
    }

    /// C4 — nhấp thẻ mở danh sách; rê chuột hiện nút Play all / Delete, nhấp nút chỉ phát sự kiện của nút, không mở thẻ.
    #[::gpui::test]
    fn click_opens_but_overlay_buttons_do_not(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, events, cx) = open_page(cx, &env, Route::MyMusic);
        let abbey = page.read_with(cx, |p, _| p.albums[0].reference);
        let CategoryRef::Album(id) = abbey else { unreachable!() };
        let card = cx.debug_bounds(selector(format!("card-album-{id}"))).expect("thẻ đã vẽ");

        cx.simulate_mouse_move(card.center(), None, Modifiers::default());
        frame(cx);
        let play = cx.debug_bounds(selector(format!("card-play-{id}"))).expect("nút Play all");
        cx.simulate_mouse_move(play.center(), None, Modifiers::default());
        frame(cx);
        cx.simulate_click(play.center(), Modifiers::default());
        cx.run_until_parked();
        assert_eq!(*events.borrow(), [LibraryPageEvent::PlayAll(abbey)], "nút không lọt nhấp lên thẻ");

        events.borrow_mut().clear();
        let delete = cx.debug_bounds(selector(format!("card-delete-{id}"))).expect("nút Delete");
        cx.simulate_mouse_move(delete.center(), None, Modifiers::default());
        frame(cx);
        cx.simulate_click(delete.center(), Modifiers::default());
        assert_eq!(*events.borrow(), [LibraryPageEvent::Delete(abbey)]);

        events.borrow_mut().clear();
        // Góc trên trái của thẻ, ngoài hai nút.
        cx.simulate_click(card.origin + ::gpui::point(px(8.), px(8.)), Modifiers::default());
        assert_eq!(*events.borrow(), [LibraryPageEvent::Open(abbey)]);
    }

    /// C5 — thư viện đổi (tạo playlist) thì lưới nạp lại.
    #[::gpui::test]
    fn reloads_when_playlists_change(cx: &mut TestAppContext) {
        let env = env(None);
        let (page, _, cx) = open_page(cx, &env, Route::Playlists);
        page.read_with(cx, |p, _| assert!(p.playlists.is_empty() && p.loaded));
        env.library.create_playlist("Chill", "#00ff00").wait().unwrap();
        wait_until(cx, "playlist mới", |cx| page.read_with(cx, |p, _| names(&p.playlists) == ["Chill"]));
    }

    /// C6 — DB của bản mới hơn: chỉ báo, không vẽ lưới.
    #[::gpui::test]
    fn too_new_library_is_reported_without_grid(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        assert!(cx.debug_bounds("card-album-1").is_some(), "thư viện mở được thì có lưới");
        page.update(cx, |p, cx| {
            p.status = OpenStatus::Failed(OpenFailure::TooNew { path: "library.db".into(), found: 99 });
            cx.notify();
        });
        frame(cx);
        assert!(cx.debug_bounds("card-album-1").is_none(), "không vẽ lưới khi thư viện không mở được");
    }

    /// C7 — chuyển DB cũ hỏng: thử lại vẫn hỏng thì giữ trạng thái lỗi; "bắt đầu mới" mở thư viện trống.
    #[::gpui::test]
    fn legacy_failure_offers_retry_and_start_fresh(cx: &mut TestAppContext) {
        let env = env(Some(b"this is not a sqlite file"));
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        page.read_with(cx, |p, _| assert!(matches!(p.failure(), Some(OpenFailure::LegacyFailed { .. }))));

        page.update(cx, |p, cx| p.retry_open(cx));
        page.read_with(cx, |p, _| assert!(p.busy));
        wait_until(cx, "thử lại xong", |cx| page.read_with(cx, |p, _| !p.busy));
        page.read_with(cx, |p, _| assert!(matches!(p.failure(), Some(OpenFailure::LegacyFailed { .. })), "vẫn hỏng"));

        page.update(cx, |p, cx| p.start_fresh(cx));
        wait_until(cx, "bắt đầu mới xong", |cx| page.read_with(cx, |p, _| !p.busy && p.failure().is_none()));
        assert!(matches!(env.library.status(), OpenStatus::Ready(_)));
        wait_until(cx, "nạp lưới", |cx| page.read_with(cx, |p, _| p.loaded));
        page.read_with(cx, |p, _| assert!(p.albums.is_empty()));
    }

    /// C8 — không có thư viện (không khởi động được luồng nền): báo bằng chữ, không panic.
    #[::gpui::test]
    fn missing_library_renders_a_message(cx: &mut TestAppContext) {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.update(materiant::init);
        let (page, cx) = cx.add_window_view(|_, cx| LibraryPage::new(None, None, Route::MyMusic, cx));
        frame(cx);
        page.read_with(cx, |p, _| assert!(p.library.is_none() && p.loaded));
        assert!(cx.debug_bounds("card-album-1").is_none());
    }

    /// C9 — route Category gắn `SongList`, dùng lại một view khi đổi mục; mục biến mất thì báo `CategoryGone`.
    #[::gpui::test]
    fn category_route_shows_the_song_list(cx: &mut TestAppContext) {
        use groove_audio::engine::EngineOptions;
        use groove_audio::output::FakeOutput;

        let env = env(None);
        seed(&env.library);
        let library = env.library.clone();
        cx.update(|cx| {
            crate::player::install_for_test(
                crate::player::PlayerDeps {
                    library: Some(library),
                    output: Box::new(FakeOutput::new(48_000, 2)),
                    options: EngineOptions::default(),
                    session_file: None,
                },
                cx,
            );
        });
        let (page, events, cx) = open_page(cx, &env, Route::MyMusic);
        let (abbey, zed) = page.read_with(cx, |p, _| (p.albums[0].reference, p.albums[1].reference));
        page.update(cx, |p, cx| p.set_route(Route::Category(abbey), cx));
        frame(cx);
        let list = page.read_with(cx, |p, _| p.song_list.clone()).expect("đã dựng SongList");
        assert!(cx.debug_bounds("card-album-1").is_none(), "không vẽ lưới");
        wait_until(cx, "bài của album", |cx| {
            frame(cx);
            cx.debug_bounds("song-row-1").is_some()
        });

        page.update(cx, |p, cx| p.set_route(Route::Category(zed), cx));
        frame(cx);
        let again = page.read_with(cx, |p, _| p.song_list.clone()).unwrap();
        assert_eq!(list.entity_id(), again.entity_id(), "một view, đổi mục");
        assert_eq!(again.read_with(cx, |l, _| l.reference()), zed);

        // Mục không còn: `SongList` báo, view chuyển lên.
        let CategoryRef::Album(zed_id) = zed else { unreachable!() };
        env.library.writer().transaction(move |conn| Ok(queries::delete_album(conn, zed_id)?)).wait().unwrap();
        env.library.writer().emit(LibraryEvent::Synced(Default::default()));
        wait_until(cx, "CategoryGone", |_| events.borrow().contains(&LibraryPageEvent::CategoryGone(zed)));
    }

    /// C12 — Shift+F10 ở danh sách bài mở menu; playlist có mục xoá khỏi danh sách.
    #[::gpui::test]
    fn shift_f10_opens_the_song_menu(cx: &mut TestAppContext) {
        use groove_audio::engine::EngineOptions;
        use groove_audio::output::FakeOutput;

        let env = env(None);
        seed(&env.library);
        let library = env.library.clone();
        cx.update(|cx| {
            crate::player::install_for_test(
                crate::player::PlayerDeps {
                    library: Some(library),
                    output: Box::new(FakeOutput::new(48_000, 2)),
                    options: EngineOptions::default(),
                    session_file: None,
                },
                cx,
            );
        });
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        let abbey = page.read_with(cx, |p, _| p.albums[0].reference);
        page.update(cx, |p, cx| p.set_route(Route::Category(abbey), cx));
        frame(cx);
        wait_until(cx, "bài của album", |cx| {
            frame(cx);
            cx.debug_bounds("song-row-1").is_some()
        });
        let list = page.read_with(cx, |p, _| p.song_list.clone()).unwrap();
        assert!(list.read_with(cx, |l, _| l.menu_opens_on_first_song()), "chưa chọn dòng: nhắm dòng đầu");
        // Không có `LibraryCtl` trong test này: menu rỗng nhưng phím không được làm sập, focus phải ở trong danh sách.
        cx.simulate_keystrokes("shift-f10");
        frame(cx);
    }

    /// C13 — sửa thẻ cả nhóm xong (`MetadataBulk`, `done == total`) và `Moved` đều nạp lại lưới.
    #[::gpui::test]
    fn bulk_progress_and_moved_reload_the_grid(cx: &mut TestAppContext) {
        use groove_library::actor::Progress;

        let env = env(None);
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        let add_album = |library: &Library, name: &'static str| {
            library
                .writer()
                .transaction(move |conn| {
                    let folder = queries::get_or_create_folder(conn, "Rock", Path::new("/music/rock"))?;
                    let album = queries::get_or_create_album(conn, name, name, None)?;
                    let path = format!("/music/rock/{name}.mp3");
                    let song = SongUpsert {
                        path: path.into(),
                        folder_id: folder,
                        album_id: Some(album),
                        ..SongUpsert::default()
                    };
                    queries::upsert_song(conn, &song)?;
                    Ok(())
                })
                .wait()
                .unwrap();
        };
        add_album(&env.library, "Bulk");
        let bulk = |done, total| LibraryEvent::Progress(Progress { kind: ProgressKind::MetadataBulk, done, total });
        env.library.writer().emit(bulk(1, 2));
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
        page.read_with(cx, |p, _| assert!(p.albums.is_empty(), "chưa xong thì chưa nạp"));
        env.library.writer().emit(bulk(2, 2));
        wait_until(cx, "nạp sau bulk", |cx| page.read_with(cx, |p, _| names(&p.albums) == ["Bulk"]));

        add_album(&env.library, "Moved");
        env.library.writer().emit(LibraryEvent::Moved(Vec::new()));
        wait_until(cx, "nạp sau Moved", |cx| page.read_with(cx, |p, _| p.albums.len() == 2));
    }

    /// C10 — lùi từ danh sách về lưới cũ giữ tab và ô chọn, sang mục khác thì về Album và bỏ chọn.
    #[::gpui::test]
    fn back_from_a_list_keeps_the_tab(cx: &mut TestAppContext) {
        let env = env(None);
        seed(&env.library);
        let (page, _, cx) = open_page(cx, &env, Route::MyMusic);
        page.update(cx, |p, cx| {
            p.tab = Tab::Folders;
            p.selected = Some(0);
            p.set_route(Route::Category(CategoryRef::Folder(1)), cx);
            p.set_route(Route::MyMusic, cx);
        });
        page.read_with(cx, |p, _| assert_eq!((p.tab, p.selected), (Tab::Folders, Some(0))));
        page.update(cx, |p, cx| {
            p.set_route(Route::Playlists, cx);
            p.set_route(Route::MyMusic, cx);
        });
        page.read_with(cx, |p, _| assert_eq!((p.tab, p.selected), (Tab::Albums, None)));
    }

    /// C11 — `app::library(cx)` lấy lại đúng thư viện đã đặt.
    #[::gpui::test]
    fn library_global_round_trips(cx: &mut TestAppContext) {
        let env = env(None);
        cx.update(|cx| {
            assert!(app::try_library(cx).is_none());
            app::set_library(env.library.clone(), cx);
            assert!(matches!(app::library(cx).status(), OpenStatus::Opening | OpenStatus::Ready(_)));
        });
    }
}
