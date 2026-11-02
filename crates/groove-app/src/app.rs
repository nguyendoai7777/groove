//! Vòng đời app. groove-gd1_03 dựng khung cửa sổ ở đây; các task sau nối vào đúng một lời gọi khởi tạo trong `run`
//! (xem `CLAUDE.md`, "Ngoại lệ có chủ ý").
//!
//! Bố cục (đặc tả 1.1): cột điều hướng trái (nền alpha, lộ desktop đã nhoè), bên phải là thanh trên 36 px + vùng nội dung
//! đục, đáy là thanh phát cao 90 px đục. Cửa sổ không viền, mở bằng `translucent_window_options`.
//!
//! # Cho task sau
//!
//! - **Slot**: mỗi vùng chưa dựng là một [`Slot`]; task sở hữu view tạo `Entity` rồi gọi một lần trong `run`:
//!   `app::install_slot(Slot::PlayerBar, view, cx)`. Slot trống vẽ chữ giữ chỗ. `Slot::Library` phục vụ cả My Music,
//!   Playlists và danh sách bài của một category (bản cũ: hai route chung một màn, đặc tả 1.2): view đọc [`current_route`]
//!   và nghe [`ShellEvent::RouteChanged`].
//! - **Điều hướng**: [`navigate`], [`go_back`], [`current_route`]. Từ view: `app::navigate(Route::Category(..), cx)`.
//! - **Sự kiện**: `app::shell(cx).subscribe(..)` nhận [`ShellEvent`]: `OpenSearch` (bảng lệnh, task 23), `OpenSettings`
//!   (hộp Settings, task 24), `RouteChanged`. Lệnh ở `commands.rs` gọi [`open_search`], [`open_settings`].
//! - **Cài đặt**: [`settings`] đọc, [`update_settings`] sửa + lưu debounce + áp theme / ngôn ngữ nếu đổi. Một `JsonSaver`
//!   duy nhất cho cả app, đừng tạo cái thứ hai.
//! - **Tiêu đề cửa sổ**: [`set_title_override`] (Now Playing đặt `<bài> - <nghệ sĩ> | Groove X`, `None` để về tên trang).

use std::cell::OnceCell;
use std::time::Duration;

use groove_core::settings::{JsonSaver, Settings, ThemeChoice};
use groove_core::{paths, t};
use groove_library::covers::CoverStore;
use groove_library::metadata_ops::MetadataOps;
use groove_library::store::Library;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    self, AnyView, App, AppContext, Bounds, Context, DisplayId, Entity, EventEmitter, Global, IntoElement, Render,
    SharedString, Styled, Window, WindowBounds, WindowDecorations, WindowOptions, div, px, size,
};
use materiant::style::StyledExt;
use materiant::theme::{ActiveTheme, MateriantTheme, ThemeMode, set_theme};
use materiant::toast::ToastViewport;

use crate::dev::{self, DevArgs};
use crate::icons;
use crate::route::{History, Route};
use crate::views::app_bar::{self, AppBar};
use crate::views::library_page;
use crate::views::side_nav::SideNav;

/// Cỡ mặc định của cửa sổ (đặc tả 1.1).
const WINDOW_SIZE: (f32, f32) = (800., 600.);
/// Cỡ nhỏ nhất: cột trái mở + ô tìm + ba nút cửa sổ còn chỗ. Đặc tả không ghi, xem `plan/confirm.md`.
const WINDOW_MIN_SIZE: (f32, f32) = (560., 360.);
/// Chiều cao thanh phát cố định ở đáy (đặc tả 1.1).
const PLAYER_BAR_HEIGHT: f32 = 90.;
/// Gom lần lưu `settings.json` lại: kéo thanh trượt âm lượng không ghi đĩa mỗi khung.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

pub fn run(dev: DevArgs) {
    // Chỗ đọc đĩa duy nhất của UI thread (N7): nạp cài đặt, đặt ngôn ngữ trước khi dựng bất cứ chữ nào.
    let settings = Settings::load();
    settings.apply_language();
    let saver = paths::settings_file()
        .and_then(|path| JsonSaver::spawn(path, SAVE_DEBOUNCE))
        .map_err(|err| log::warn!("settings saver: {err}"))
        .ok();

    gpui_kit::application().with_assets(icons::Assets).run(move |cx: &mut App| {
        materiant::init(cx);
        install_settings(settings, saver, cx);
        // Đóng cửa sổ là thoát app, không có khay (đặc tả 1.1).
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let display_id = dev.display_id(cx);
        let start = Route::from_start_page(self::settings(cx).last_active_route);
        let bounds = Bounds::centered(display_id, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
        let options = main_window_options(bounds, display_id, !dev.background);
        let (handle, shell) =
            gpui_kit::open_window(options, cx, |_, cx| cx.new(|cx| Shell::new(start, cx))).expect("open main window");
        cx.set_global(ShellHandle(shell));
        if !dev.background {
            cx.activate(true);
        }
        // Task sau thêm đúng một lời gọi `install_slot(..)` / khởi tạo của mình ở đây.
        // Task 15: mở thư viện chuẩn, đặt global `library(cx)`, dựng màn lưới vào `Slot::Library`.
        library_page::install(cx);
        // Task 13: player cần thư viện ở trên.
        crate::player::install(cx);
        // Task 22: phím tắt, cần player ở trên.
        crate::commands::install(cx);
        // Task 14: thanh phát vào `Slot::PlayerBar`, cần player ở trên.
        crate::views::player_bar::install(handle, cx);
        // Task 26: media session, nút taskbar, cần phím tắt ở trên.
        crate::platform::media_session::install(handle, cx);
        // Task 23: bảng lệnh vào `Slot::CommandPalette`, nghe `ShellEvent::OpenSearch`.
        crate::views::command_palette::install(handle, cx);
        // Task 21: màn Now Playing vào `Slot::NowPlaying`, cần player và thư viện ở trên.
        crate::views::now_playing::install(handle, cx);
        dev::replay(handle, dev.keys, cx);
    });
}

/// Cửa sổ chính: không viền (thanh trên tự vẽ), nền mờ, cỡ nhỏ nhất.
fn main_window_options(
    bounds: Bounds<materiant::gpui_kit::Pixels>,
    display_id: Option<DisplayId>,
    focus: bool,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Không `titlebar`: bỏ thanh tiêu đề OS. Tên cửa sổ (taskbar, Alt+Tab) đặt bằng `set_window_title` ở lần vẽ đầu.
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        window_min_size: Some(size(px(WINDOW_MIN_SIZE.0), px(WINDOW_MIN_SIZE.1))),
        display_id,
        focus,
        ..materiant::window::translucent_window_options()
    }
}

// ---- Cài đặt dùng chung ----

/// `Global` giữ cài đặt đang áp và bộ lưu debounce duy nhất.
struct SettingsStore {
    entity: Entity<Settings>,
    saver: Option<JsonSaver>,
}

impl Global for SettingsStore {}

fn theme_mode(choice: ThemeChoice) -> ThemeMode {
    match choice {
        ThemeChoice::Light => ThemeMode::Light,
        ThemeChoice::Dark => ThemeMode::Dark,
        ThemeChoice::System => ThemeMode::System,
    }
}

fn apply_theme(choice: ThemeChoice, cx: &mut App) {
    set_theme(&MateriantTheme { mode: Some(theme_mode(choice)), ..Default::default() }, cx);
}

/// Đặt cài đặt toàn app, áp theme, và theo dõi để áp lại theme / ngôn ngữ khi chúng đổi.
fn install_settings(settings: Settings, saver: Option<JsonSaver>, cx: &mut App) {
    apply_theme(settings.theme, cx);
    let mut applied = (settings.theme, settings.effective_language());
    let entity = cx.new(|_| settings);
    cx.observe(&entity, move |entity, cx| {
        let now = {
            let s = entity.read(cx);
            (s.theme, s.effective_language())
        };
        if now.0 != applied.0 {
            apply_theme(now.0, cx);
        }
        if now.1 != applied.1 {
            groove_core::i18n::set_language(now.1);
            cx.refresh_windows();
        }
        applied = now;
    })
    .detach();
    cx.set_global(SettingsStore { entity, saver });
    // Lần lưu đang chờ debounce phải xuống đĩa trước khi thoát.
    cx.on_app_quit(|cx| {
        if let Some(saver) = cx.try_global::<SettingsStore>().and_then(|s| s.saver.as_ref()) {
            saver.flush();
        }
        async {}
    })
    .detach();
}

/// Cài đặt hiện tại.
pub fn settings(cx: &App) -> &Settings {
    cx.global::<SettingsStore>().entity.read(cx)
}

/// Sửa cài đặt: thông báo cho view đang quan sát, lưu (debounce), theme / ngôn ngữ đổi thì áp ngay.
pub fn update_settings(cx: &mut App, change: impl FnOnce(&mut Settings)) {
    let entity = cx.global::<SettingsStore>().entity.clone();
    entity.update(cx, |settings, cx| {
        change(settings);
        settings.sanitize();
        cx.notify();
    });
    let store = cx.global::<SettingsStore>();
    if let Some(saver) = &store.saver {
        saver.save(store.entity.read(cx));
    }
}

// ---- Khung ----

/// Vùng chờ task sau điền vào.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Thanh phát đáy (task 14).
    PlayerBar,
    /// My Music, Playlists và danh sách bài của một category (task 15, 16).
    Library,
    /// Now Playing (task 21).
    NowPlaying,
    /// Bảng lệnh (task 23): dialog tự vẽ lớp phủ, nên chỉ cần nằm trong cây của khung.
    CommandPalette,
}

/// Sự kiện của khung cho view khác nghe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellEvent {
    RouteChanged(Route),
    /// Ô tìm trên thanh trên hay lệnh `open_search`: mở bảng lệnh (task 23).
    OpenSearch,
    /// Nút Settings ở đáy cột trái: mở hộp Settings (task 24).
    OpenSettings,
}

#[derive(Default)]
struct Slots {
    player_bar: Option<AnyView>,
    library: Option<AnyView>,
    now_playing: Option<AnyView>,
    command_palette: Option<AnyView>,
}

impl Slots {
    fn get(&self, slot: Slot) -> Option<&AnyView> {
        match slot {
            Slot::PlayerBar => self.player_bar.as_ref(),
            Slot::Library => self.library.as_ref(),
            Slot::NowPlaying => self.now_playing.as_ref(),
            Slot::CommandPalette => self.command_palette.as_ref(),
        }
    }

    fn set(&mut self, slot: Slot, view: AnyView) {
        match slot {
            Slot::PlayerBar => self.player_bar = Some(view),
            Slot::Library => self.library = Some(view),
            Slot::NowPlaying => self.now_playing = Some(view),
            Slot::CommandPalette => self.command_palette = Some(view),
        }
    }
}

/// Slot hiện nội dung của một màn.
fn content_slot(route: Route) -> Slot {
    match route {
        Route::NowPlaying => Slot::NowPlaying,
        _ => Slot::Library,
    }
}

/// Nội dung cửa sổ.
pub struct Shell {
    history: History,
    slots: Slots,
    title_override: Option<SharedString>,
    window_title: SharedString,
}

impl EventEmitter<ShellEvent> for Shell {}

impl Shell {
    pub fn new(start: Route, cx: &mut Context<Self>) -> Shell {
        // Cài đặt đổi (cột trái thu / mở từ nơi khác) thì vẽ lại.
        let settings = cx.global::<SettingsStore>().entity.clone();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        Shell {
            history: History::new(start),
            slots: Slots::default(),
            title_override: None,
            window_title: SharedString::default(),
        }
    }

    pub fn route(&self) -> Route {
        self.history.current()
    }

    pub fn can_back(&self) -> bool {
        self.history.can_back()
    }

    pub fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        if self.history.push(route) {
            self.route_changed(cx);
        }
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.history.back().is_some() {
            self.route_changed(cx);
        }
    }

    fn route_changed(&mut self, cx: &mut Context<Self>) {
        let route = self.history.current();
        // Tiêu đề riêng (bài đang phát) chỉ đúng ở màn đặt nó; đổi màn là về tên trang, view sẽ đặt lại nếu cần.
        self.title_override = None;
        let page = route.start_page();
        if settings(cx).last_active_route != page {
            update_settings(cx, |s| s.last_active_route = page);
        }
        cx.emit(ShellEvent::RouteChanged(route));
        cx.notify();
    }

    pub fn set_slot(&mut self, slot: Slot, view: impl Into<AnyView>, cx: &mut Context<Self>) {
        self.slots.set(slot, view.into());
        cx.notify();
    }

    pub fn slot(&self, slot: Slot) -> Option<&AnyView> {
        self.slots.get(slot)
    }

    pub fn set_title_override(&mut self, title: Option<SharedString>, cx: &mut Context<Self>) {
        if self.title_override != title {
            self.title_override = title;
            cx.notify();
        }
    }

    /// Tiêu đề cửa sổ: tiêu đề riêng nếu có, không thì `<trang> | Groove X`.
    pub fn title(&self) -> SharedString {
        match &self.title_override {
            Some(title) => title.clone(),
            None => format!("{} | {}", self.route().title(), t!("common.app_name")).into(),
        }
    }

    fn toggle_side_nav(cx: &mut App) {
        update_settings(cx, |s| s.side_nav_collapsed = !s.side_nav_collapsed);
    }

    fn placeholder(route: Route, cx: &App) -> impl IntoElement {
        let c = cx.theme().colors();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(c.muted)
            .child(t!("shell.slot.empty", page = route.title()))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title();
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }

        let route = self.route();
        let shell = cx.entity();
        let c = cx.theme().colors().clone();
        let collapsed = settings(cx).side_nav_collapsed;

        let nav = SideNav::new(
            collapsed,
            route.nav_section(),
            {
                let shell = shell.clone();
                move |target, _, cx| shell.update(cx, |this, cx| this.navigate(target, cx))
            },
            |_, cx| Shell::toggle_side_nav(cx),
            {
                let shell = shell.clone();
                move |_, cx| shell.update(cx, |_, cx| cx.emit(ShellEvent::OpenSettings))
            },
        );
        let bar = AppBar::new(
            self.can_back(),
            window.is_maximized(),
            {
                let shell = shell.clone();
                move |_, cx| shell.update(cx, |this, cx| this.back(cx))
            },
            {
                let shell = shell.clone();
                move |_, cx| shell.update(cx, |_, cx| cx.emit(ShellEvent::OpenSearch))
            },
        );

        let content = match self.slots.get(content_slot(route)) {
            Some(view) => view.clone().into_any_element(),
            None => Shell::placeholder(route, cx).into_any_element(),
        };
        let player = self.slots.get(Slot::PlayerBar).map(|view| view.clone().into_any_element());

        div()
            .id("shell")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .text_ink(cx)
            .text_size(cx.theme().font_size())
            .child(
                div().flex_1().min_h_0().flex().child(nav).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .bg(c.surface)
                        .child(bar)
                        .child(div().id("content").flex_1().min_h_0().child(content)),
                ),
            )
            .child(
                div()
                    .id("player-bar")
                    .h(px(PLAYER_BAR_HEIGHT))
                    .flex_none()
                    .w_full()
                    .bg(c.surface)
                    .border_t_1()
                    .border_color(c.border)
                    .children(player),
            )
            .children(self.slots.get(Slot::CommandPalette).map(|view| view.clone().into_any_element()))
            .child(ToastViewport::new().inset_top(px(app_bar::HEIGHT)))
    }
}

// ---- Lối tắt cho view khác ----

/// `Global` trỏ tới khung đang chạy.
struct ShellHandle(Entity<Shell>);

impl Global for ShellHandle {}

pub fn shell(cx: &App) -> Entity<Shell> {
    cx.global::<ShellHandle>().0.clone()
}

pub fn navigate(route: Route, cx: &mut App) {
    shell(cx).update(cx, |this, cx| this.navigate(route, cx));
}

pub fn go_back(cx: &mut App) {
    shell(cx).update(cx, |this, cx| this.back(cx));
}

pub fn current_route(cx: &App) -> Route {
    shell(cx).read(cx).route()
}

/// `Global` giữ thư viện đang mở (groove-gd1_15 đặt trong `library_page::install`).
struct LibraryHandle(Library);

impl Global for LibraryHandle {}

/// Đặt thư viện dùng chung. Gọi một lần lúc khởi động.
pub fn set_library(library: Library, cx: &mut App) {
    cx.set_global(LibraryHandle(library));
    cx.set_global(MetadataOpsHandle::default());
}

/// `Global` giữ `MetadataOps` dùng chung (mỗi bản một luồng ghi tệp nên dựng một lần, lười ở lần gọi đầu).
#[derive(Default)]
struct MetadataOpsHandle(OnceCell<MetadataOps>);

impl Global for MetadataOpsHandle {}

/// `MetadataOps` dùng chung, dựng lười từ [`library`] và `CoverStore::default_location()`; `None` khi chưa có thư viện hoặc
/// không dựng được (đã ghi log). Sao chép rẻ, gửi được giữa luồng.
pub fn try_metadata_ops(cx: &App) -> Option<MetadataOps> {
    let library = try_library(cx)?;
    let handle = cx.try_global::<MetadataOpsHandle>()?;
    if let Some(ops) = handle.0.get() {
        return Some(ops.clone());
    }
    let covers = CoverStore::default_location().map_err(|err| log::warn!("cannot locate covers: {err}")).ok()?;
    let ops = MetadataOps::new(library.clone(), covers)
        .map_err(|err| log::error!("cannot start metadata ops: {err}"))
        .ok()?;
    Some(handle.0.get_or_init(|| ops).clone())
}

/// Như [`try_metadata_ops`], panic nếu chưa `set_library`. Dùng ở chỗ chắc chắn đã khởi động xong.
pub fn metadata_ops(cx: &App) -> MetadataOps {
    try_metadata_ops(cx).expect("metadata ops need the library: call set_library first")
}

/// Thư viện dùng chung (sao chép rẻ, gửi được giữa luồng). Panic nếu chưa `set_library`; `try_library` cho chỗ có thể
/// chạy trước lúc khởi động xong.
pub fn library(cx: &App) -> &Library {
    &cx.global::<LibraryHandle>().0
}

pub fn try_library(cx: &App) -> Option<&Library> {
    cx.try_global::<LibraryHandle>().map(|handle| &handle.0)
}

/// Đặt sẵn `MetadataOps` trỏ thư mục tạm. Gọi sau `set_library`.
#[cfg(test)]
pub fn set_metadata_ops(ops: MetadataOps, cx: &mut App) {
    assert!(cx.global::<MetadataOpsHandle>().0.set(ops).is_ok(), "metadata ops already built");
}

pub fn install_slot(slot: Slot, view: impl Into<AnyView>, cx: &mut App) {
    shell(cx).update(cx, |this, cx| this.set_slot(slot, view, cx));
}

pub fn open_search(cx: &mut App) {
    shell(cx).update(cx, |_, cx| cx.emit(ShellEvent::OpenSearch));
}

pub fn open_settings(cx: &mut App) {
    shell(cx).update(cx, |_, cx| cx.emit(ShellEvent::OpenSettings));
}

pub fn set_title_override(title: Option<SharedString>, cx: &mut App) {
    shell(cx).update(cx, |this, cx| this.set_title_override(title, cx));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{TestAppContext, VisualTestContext};
    use groove_core::model::CategoryRef;
    use groove_core::settings::StartPage;
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::{WindowBackgroundAppearance, point};

    use super::*;

    type Events = Rc<RefCell<Vec<ShellEvent>>>;

    /// Cửa sổ ảo chứa `Shell`, cài đặt mặc định, không lưu đĩa. Ghi lại mọi `ShellEvent`.
    fn open(cx: &mut TestAppContext, settings: Settings) -> (Entity<Shell>, Events, &mut VisualTestContext) {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.update(|cx| {
            materiant::init(cx);
            install_settings(settings, None, cx);
        });
        let start = Route::from_start_page(cx.read(|cx| self::settings(cx).last_active_route));
        let window = cx.add_window(|window, cx| {
            let shell = cx.new(|cx| Shell::new(start, cx));
            Root::new(shell, window, cx)
        });
        let shell = window.update(cx, |root, _, _| root.view().clone().downcast::<Shell>().unwrap()).unwrap();
        let events: Events = Rc::default();
        let log = events.clone();
        cx.update(|cx| cx.subscribe(&shell, move |_, event: &ShellEvent, _| log.borrow_mut().push(*event)).detach());
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (shell, events, cx)
    }

    fn route(shell: &Entity<Shell>, cx: &VisualTestContext) -> Route {
        shell.read_with(cx, |s, _| s.route())
    }

    /// Chữ ở tâm một mục điều hướng (cột trái mở: mục i ở y = 36 + 42 * i + 21).
    fn nav_item_center(ix: usize) -> gpui::Point<gpui::Pixels> {
        point(
            px(100.),
            px(crate::views::side_nav::HEADER_HEIGHT + crate::views::side_nav::ITEM_HEIGHT * ix as f32 + 21.),
        )
    }

    /// C1 — cửa sổ không viền, nền mờ, cỡ mặc định.
    #[test]
    fn main_window_is_frameless_and_blurred() {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)));
        let options = main_window_options(bounds, None, true);
        assert!(options.titlebar.is_none(), "không thanh tiêu đề OS");
        assert_eq!(options.window_background, WindowBackgroundAppearance::Blurred);
        assert_eq!(options.window_decorations, Some(WindowDecorations::Client));
        assert!(options.is_movable && options.is_resizable && options.focus);
        assert_eq!(options.inactive_frame_interval, None, "giữ trần khung hình của materiant::window_options");
        let background = main_window_options(bounds, None, false);
        assert!(!background.focus, "`--background` không giành focus");
    }

    #[test]
    fn theme_choice_maps_to_the_same_mode() {
        for choice in [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::System] {
            assert_eq!(serde_json::to_string(&choice).unwrap(), serde_json::to_string(&theme_mode(choice)).unwrap());
        }
    }

    /// C2 — mở app về trang cuối đã lưu.
    #[gpui::test]
    fn opens_at_last_active_route(cx: &mut TestAppContext) {
        let settings = Settings { last_active_route: StartPage::Playlists, ..Settings::default() };
        let (shell, _, cx) = open(cx, settings);
        assert_eq!(route(&shell, cx), Route::Playlists);
        assert!(!shell.read_with(cx, |s, _| s.can_back()));
        assert_eq!(shell.read_with(cx, |s, _| s.title().to_string()), "Danh sách phát | Groove X");
    }

    /// C3, C4 — bấm mục điều hướng đổi màn, lưu trang cuối; nút lùi quay về.
    #[gpui::test]
    fn nav_click_navigates_and_persists(cx: &mut TestAppContext) {
        let (shell, events, cx) = open(cx, Settings::default());
        cx.simulate_click(nav_item_center(1), Default::default());
        assert_eq!(route(&shell, cx), Route::NowPlaying);
        assert_eq!(cx.update(|_, cx| settings(cx).last_active_route), StartPage::NowPlaying);
        assert_eq!(*events.borrow(), [ShellEvent::RouteChanged(Route::NowPlaying)]);
        assert!(shell.read_with(cx, |s, _| s.can_back()));

        shell.update(cx, |s, cx| s.back(cx));
        assert_eq!(route(&shell, cx), Route::MyMusic);
        assert_eq!(cx.update(|_, cx| settings(cx).last_active_route), StartPage::MyMusic);
    }

    /// C4 — xem danh sách một category thì tô sáng mục cha, và không được nhớ lại ở lần mở sau.
    #[gpui::test]
    fn category_route_keeps_parent_highlight_and_is_not_saved(cx: &mut TestAppContext) {
        let (shell, events, cx) = open(cx, Settings::default());
        let list = Route::Category(CategoryRef::Playlist(4));
        shell.update(cx, |s, cx| s.navigate(list, cx));
        assert_eq!(route(&shell, cx), list);
        assert_eq!(cx.update(|_, cx| settings(cx).last_active_route), StartPage::Playlists);
        assert_eq!(events.borrow().last(), Some(&ShellEvent::RouteChanged(list)));
        // Điều hướng tới đúng màn đang ở thì không thêm bước, không phát sự kiện.
        let count = events.borrow().len();
        shell.update(cx, |s, cx| s.navigate(list, cx));
        assert_eq!(events.borrow().len(), count);
    }

    /// C5 — nút thu cột trái đổi `side_nav_collapsed`.
    #[gpui::test]
    fn toggling_side_nav_updates_settings(cx: &mut TestAppContext) {
        let (_, _, cx) = open(cx, Settings::default());
        assert!(!cx.update(|_, cx| settings(cx).side_nav_collapsed));
        // Nút thu nằm sát mép phải đầu cột trái (rộng 230 px).
        cx.simulate_click(point(px(crate::views::side_nav::EXPANDED_WIDTH - 18.), px(18.)), Default::default());
        assert!(cx.update(|_, cx| settings(cx).side_nav_collapsed));
    }

    /// C6 — ô tìm và nút Settings phát sự kiện cho task 23, 24.
    #[gpui::test]
    fn search_and_settings_emit_events(cx: &mut TestAppContext) {
        let (_, events, cx) = open(cx, Settings::default());
        cx.simulate_click(point(px(WINDOW_SIZE.0 / 2. + 60.), px(18.)), Default::default());
        assert_eq!(*events.borrow(), [ShellEvent::OpenSearch]);
        // Nút Settings ở đáy cột trái, phía trên thanh phát.
        cx.simulate_click(point(px(100.), px(WINDOW_SIZE.1 - PLAYER_BAR_HEIGHT - 20.)), Default::default());
        assert_eq!(events.borrow().last(), Some(&ShellEvent::OpenSettings));
    }

    /// C8 — slot trống vẽ được; slot điền thì giữ view và vẽ nó; `Library` dùng chung cho ba màn.
    #[gpui::test]
    fn slots_start_empty_and_accept_views(cx: &mut TestAppContext) {
        struct Marker;
        impl Render for Marker {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().id("marker").size_full()
            }
        }

        let (shell, _, cx) = open(cx, Settings::default());
        assert!(shell.read_with(cx, |s, _| s.slot(Slot::Library).is_none()));
        let view = cx.new(|_| Marker);
        shell.update(cx, |s, cx| s.set_slot(Slot::Library, view.clone(), cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(shell.read_with(cx, |s, _| s.slot(Slot::Library).is_some()));
        assert!(shell.read_with(cx, |s, _| s.slot(Slot::NowPlaying).is_none()));
        assert_eq!(content_slot(Route::MyMusic), Slot::Library);
        assert_eq!(content_slot(Route::Playlists), Slot::Library);
        assert_eq!(content_slot(Route::Category(CategoryRef::Album(1))), Slot::Library);
        assert_eq!(content_slot(Route::NowPlaying), Slot::NowPlaying);
    }

    /// Tiêu đề cửa sổ theo trang, đặt riêng được, đổi màn thì về tên trang.
    #[gpui::test]
    fn window_title_follows_route_and_override(cx: &mut TestAppContext) {
        let (shell, _, cx) = open(cx, Settings::default());
        let title = |cx: &VisualTestContext| shell.read_with(cx, |s, _| s.title().to_string());
        assert_eq!(title(cx), "Thư viện | Groove X");
        shell.update(cx, |s, cx| s.set_title_override(Some("Bài - Nghệ sĩ | Groove X".into()), cx));
        assert_eq!(title(cx), "Bài - Nghệ sĩ | Groove X");
        shell.update(cx, |s, cx| s.navigate(Route::NowPlaying, cx));
        assert_eq!(title(cx), "Đang phát | Groove X");
    }

    /// Đổi theme trong cài đặt thì theme materiant đổi theo.
    #[gpui::test]
    fn settings_change_applies_theme(cx: &mut TestAppContext) {
        let (_, _, cx) = open(cx, Settings { theme: ThemeChoice::Dark, ..Settings::default() });
        assert!(cx.update(|_, cx| cx.theme().is_dark()));
        cx.update(|_, cx| update_settings(cx, |s| s.theme = ThemeChoice::Light));
        assert_eq!(cx.update(|_, cx| cx.theme().mode()), ThemeMode::Light);
        assert!(!cx.update(|_, cx| cx.theme().is_dark()));
    }
}
