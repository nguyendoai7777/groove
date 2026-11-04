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
//!   (hộp Settings, task 24), `RouteChanged`. Lệnh ở `commands.rs` gọi [`open_search`]; Settings mở từ nút ở side nav.
//! - **Cài đặt**: [`settings`] đọc, [`update_settings`] sửa + lưu debounce + áp theme / ngôn ngữ nếu đổi. Một `JsonSaver`
//!   duy nhất cho cả app, đừng tạo cái thứ hai.
//! - **Tiêu đề cửa sổ**: [`set_title_override`] (Now Playing đặt `<bài> - <nghệ sĩ> | Groovex`, `None` để về tên trang).

use std::cell::OnceCell;
use std::time::Duration;

use groove_core::settings::{JsonSaver, LayoutStyle, Settings, ThemeChoice, UiFont};
use groove_core::{paths, t};
use groove_library::covers::CoverStore;
use groove_library::metadata_ops::MetadataOps;
use groove_library::store::Library;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    self, AnyElement, AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, DisplayId, Entity, EntityId,
    EventEmitter, FontWeight, Global, IntoElement, Render, SharedString, StyleRefinement, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, px, size,
};
use materiant::style::StyledExt;
use materiant::theme::picker::{ThemePreset, builtin_presets};
use materiant::theme::{ActiveTheme, MateriantTheme, ThemeMode, set_palette, set_theme};
use materiant::toast::ToastViewport;
use materiant::window::DragRegionExt;

use crate::dev::{self, DevArgs};
use crate::icons;
use crate::route::{History, Route};
use crate::views::app_bar::{self, AppBar};
use crate::views::bars_overlay::{self, BarsOverlay};
use crate::views::island::{self, Island};
use crate::views::layout;
use crate::views::library_page;
use crate::views::side_nav::{self, SideNav};

/// Cỡ mặc định của cửa sổ (đặc tả 1.1).
const WINDOW_SIZE: (f32, f32) = (800., 600.);
/// Cỡ nhỏ nhất: cột trái mở + ô tìm + ba nút cửa sổ còn chỗ. Đặc tả không ghi, xem `plan/confirm.md`.
const WINDOW_MIN_SIZE: (f32, f32) = (560., 360.);
/// Chiều cao thanh phát cố định ở đáy (đặc tả 1.1).
const PLAYER_BAR_HEIGHT: f32 = 90.;
/// Gom lần lưu `settings.json` lại: kéo thanh trượt âm lượng không ghi đĩa mỗi khung.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

pub fn run(dev: DevArgs) {
    // groove-gd2_01: AppUserModelID trước cửa sổ đầu, để taskbar gom với lối tắt của bộ cài.
    crate::platform::default_player::set_process_identity();
    // Chỗ đọc đĩa duy nhất của UI thread (N7): nạp cài đặt, đặt ngôn ngữ trước khi dựng bất cứ chữ nào.
    let settings = Settings::load();
    settings.apply_language();
    let saver = paths::settings_file()
        .and_then(|path| JsonSaver::spawn(path, SAVE_DEBOUNCE))
        .map_err(|err| log::warn!("settings saver: {err}"))
        .ok();

    // Task 25: bọc `run` để nhận tệp từ OS trước khi app chạy; bơm yêu cầu cài sau mọi khởi tạo dưới đây.
    crate::open_paths::run_app(gpui_kit::application().with_assets(icons::Assets), move |cx: &mut App| {
        materiant::init(cx);
        devtool::init(cx);
        install_settings(settings, saver, cx);

        let display_id = dev.display_id(cx);
        let start = Route::from_start_page(self::settings(cx).last_active_route);
        let bounds = Bounds::centered(display_id, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
        let options = main_window_options(bounds, display_id, !dev.background);
        // Task 27: vị trí đã lưu, phải có trước khi mở cửa sổ.
        let options = crate::window_state::restore(options, cx);
        let opened = gpui_kit::open_window(options, cx, |_, cx| cx.new(|cx| Shell::new(start, cx)));
        let (handle, shell) = match opened {
            Ok(opened) => opened,
            Err(err) => {
                // Bản phát hành không có console: panic ở đây là tắt câm, log mới là chỗ đọc được lý do.
                log::error!("cannot open the main window: {err:#}");
                cx.quit();
                return;
            }
        };
        cx.set_global(ShellHandle(shell));
        cx.set_global(MainWindow(handle));
        // drag-lag: kéo cửa sổ không vẽ lại cả cây mỗi nhịp (`platform::move_loop`); acrylic giật khi kéo trên Windows 10 ⇒
        // blur-behind (`platform::backdrop`).
        let _ = handle.update(cx, |_, window, _| {
            crate::platform::move_loop::install(window);
            crate::platform::backdrop::smooth_blur(window);
        });
        // Đóng cửa sổ chính là thoát app, không có khay (đặc tả 1.1); trình phát thu nhỏ còn mở cũng đóng theo.
        let main_id = handle.window_id();
        cx.on_window_closed(move |cx, closed| {
            if closed == main_id || cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
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
        // Task 24: hộp Settings vào `Slot::Settings`, nghe `ShellEvent::OpenSettings`; cần player và thư viện ở trên.
        crate::views::settings::dialog::install(handle, cx);
        // pip: trình phát thu nhỏ, menu dock; cần player ở trên. Mở ngay khi chạy bằng `--pip` (menu dock lúc app chưa chạy).
        crate::views::pip::install(cx);
        if crate::open_paths::wants_pip(&std::env::args_os().collect::<Vec<_>>()) {
            crate::views::pip::open(cx);
        }
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

/// Màu nhấn theo bảng màu đang chọn (Settings → Chung; mặc định là primary của materiant, xanh Material `#005cbb` — G-4);
/// view đọc `cx.theme().colors().primary`, không ghim mã màu. Chữ một độ đậm (G-6): nhãn và tiêu đề của mọi component
/// materiant về normal ngay ở theme, view không phải truyền `*_weight(NORMAL)` từng chỗ.
///
/// Nền (`choice`) và bảng màu (`preset`) là hai lựa chọn độc lập: `set_palette` chỉ thay màu (không sót màu preset trước),
/// bản vá của groove đặt nền, font, độ đậm. Nền `System` thì đổi sáng / tối theo OS với bảng màu đã chọn.
fn apply_theme(choice: ThemeChoice, preset: &str, font: UiFont, cx: &mut App) {
    set_palette(&theme_preset(preset).theme, cx);
    set_theme(&groove_theme(choice, font), cx);
}

/// Các preset bảng màu chọn được, nhãn đã dịch; thứ tự như gallery của materiant.
pub fn theme_presets() -> Vec<ThemePreset> {
    builtin_presets()
        .into_iter()
        .map(|preset| {
            let label = match preset.key.as_ref() {
                "default" => t!("settings.theme_preset.default"),
                "deeppink" => t!("settings.theme_preset.deeppink"),
                "rose-red" => t!("settings.theme_preset.rose_red"),
                "rebecca-purple" => t!("settings.theme_preset.rebecca_purple"),
                // Preset mới của materiant chưa có bản dịch: hiện khoá.
                _ => return preset,
            };
            preset.label(label)
        })
        .collect()
}

/// Preset theo khoá đã lưu; khoá lạ ⇒ mặc định.
fn theme_preset(key: &str) -> ThemePreset {
    let mut presets = builtin_presets();
    let ix = presets.iter().position(|p| p.key == key).unwrap_or(0);
    presets.swap_remove(ix)
}

/// Họ font nhúng (materiant `font`, D-20) của lựa chọn trong Settings.
pub fn font_family(font: UiFont) -> &'static str {
    match font {
        UiFont::MonaSans => materiant::font::MONA_SANS,
        UiFont::BeVietnamPro => materiant::font::BE_VIETNAM_PRO,
    }
}

fn groove_theme(choice: ThemeChoice, font: UiFont) -> MateriantTheme {
    MateriantTheme {
        mode: Some(theme_mode(choice)),
        // Font nhúng, không theo font hệ thống: Ubuntu Sans biến thiên làm lệch dấu tiếng Việt.
        font_family: Some(font_family(font).into()),
        label_weight: Some(FontWeight::NORMAL.0),
        title_weight: Some(FontWeight::NORMAL.0),
        ..Default::default()
    }
}

/// Đặt cài đặt toàn app, áp theme, và theo dõi để áp lại theme / ngôn ngữ khi chúng đổi.
fn install_settings(settings: Settings, saver: Option<JsonSaver>, cx: &mut App) {
    apply_theme(settings.theme, &settings.theme_preset, settings.font, cx);
    let mut applied = (
        settings.theme,
        settings.effective_language(),
        settings.font,
        settings.theme_preset.clone(),
        settings.layout_style,
    );
    let entity = cx.new(|_| settings);
    cx.observe(&entity, move |entity, cx| {
        let now = {
            let s = entity.read(cx);
            (s.theme, s.effective_language(), s.font, s.theme_preset.clone(), s.layout_style)
        };
        if now.0 != applied.0 || now.2 != applied.2 || now.3 != applied.3 {
            apply_theme(now.0, &now.3, now.2, cx);
        }
        if now.1 != applied.1 {
            groove_core::i18n::set_language(now.1);
            crate::views::pip::refresh_dock_menu(cx);
            cx.refresh_windows();
        } else if now.4 != applied.4 {
            // Kiểu chia vùng (`views::layout`): view `cached` (cột trái, trang thư viện, ba khung Now Playing) không nghe cài
            // đặt; `refresh` bỏ qua bộ đệm của chúng như khi đổi ngôn ngữ.
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

/// Đặt cài đặt toàn app cho test của view khác (`app::settings` cần global này), không lưu đĩa.
#[cfg(test)]
pub fn install_settings_for_test(settings: Settings, cx: &mut App) {
    install_settings(settings, None, cx);
}

/// Cài đặt hiện tại.
pub fn settings(cx: &App) -> &Settings {
    cx.global::<SettingsStore>().entity.read(cx)
}

/// Kiểu chia vùng đang chọn (`views::layout`). Chưa có cài đặt toàn app (test của một view lẻ) ⇒ mặc định.
pub fn layout_style(cx: &App) -> LayoutStyle {
    cx.try_global::<SettingsStore>().map(|store| store.entity.read(cx).layout_style).unwrap_or_default()
}

/// Như [`update_settings`] nhưng không báo view nào: cho giá trị không vẽ lên đâu (vị trí cửa sổ). Dời cửa sổ báo bounds mỗi
/// `WM_MOVE`; nếu `notify` thì `Shell` (nghe cài đặt) dựng và vẽ lại cả khung mỗi nhịp kéo, giữa vòng dời cửa sổ của OS —
/// kéo giật (drag-lag).
pub fn store_settings_quietly(cx: &mut App, change: impl FnOnce(&mut Settings)) {
    let entity = cx.global::<SettingsStore>().entity.clone();
    entity.update(cx, |settings, _| {
        change(settings);
        settings.sanitize();
    });
    let store = cx.global::<SettingsStore>();
    if let Some(saver) = &store.saver {
        saver.save(store.entity.read(cx));
    }
}

/// Sửa cài đặt: thông báo cho view đang quan sát, lưu (debounce), theme / ngôn ngữ / kiểu chia vùng đổi thì áp ngay.
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
    /// Hộp Settings (task 24): cũng là dialog tự vẽ lớp phủ.
    Settings,
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
    settings: Option<AnyView>,
}

impl Slots {
    fn get(&self, slot: Slot) -> Option<&AnyView> {
        match slot {
            Slot::PlayerBar => self.player_bar.as_ref(),
            Slot::Library => self.library.as_ref(),
            Slot::NowPlaying => self.now_playing.as_ref(),
            Slot::CommandPalette => self.command_palette.as_ref(),
            Slot::Settings => self.settings.as_ref(),
        }
    }

    fn set(&mut self, slot: Slot, view: AnyView) {
        match slot {
            Slot::PlayerBar => self.player_bar = Some(view),
            Slot::Library => self.library = Some(view),
            Slot::NowPlaying => self.now_playing = Some(view),
            Slot::CommandPalette => self.command_palette = Some(view),
            Slot::Settings => self.settings = Some(view),
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
///
/// cpu (03/11/2026): cột trái, thanh trên, trang thư viện nhúng `cached` (`views::island`); thanh phát và Now Playing nhúng
/// thường vì chứa chữ chạy / thanh nhảy — chúng tự giữ phần nặng của mình `cached`. Khung hình của hoạt ảnh nhờ vậy chỉ dựng
/// lại đường từ gốc tới hoạt ảnh.
pub struct Shell {
    history: History,
    slots: Slots,
    title_override: Option<SharedString>,
    window_title: SharedString,
    nav: Entity<Island<Shell>>,
    bar: Entity<Island<Shell>>,
    /// Bề rộng cột trái của khung đang vẽ (đang nội suy khi thu / mở).
    nav_width: f32,
    /// Lớp phủ vẽ thanh nhảy của view nội dung `cached` (`views::bars_overlay`).
    bars: Entity<BarsOverlay>,
    /// View nội dung của khung trước: đổi thì anchor của view cũ hết hiệu lực.
    content_view: Option<EntityId>,
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
            nav: Island::new(Shell::nav_part, cx),
            bar: Island::new(Shell::bar_part, cx),
            nav_width: side_nav::EXPANDED_WIDTH,
            bars: cx.new(|_| BarsOverlay),
            content_view: None,
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

    #[cfg(test)]
    pub fn slot(&self, slot: Slot) -> Option<&AnyView> {
        self.slots.get(slot)
    }

    pub fn set_title_override(&mut self, title: Option<SharedString>, cx: &mut Context<Self>) {
        if self.title_override != title {
            self.title_override = title;
            cx.notify();
        }
    }

    /// Tiêu đề cửa sổ: tiêu đề riêng nếu có, không thì `<trang> | Groovex`.
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

impl Shell {
    /// Phần cột trái (`Island`): dựng lại khi Shell `notify` (đổi màn, cài đặt) hay đổi bề rộng.
    fn nav_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity();
        SideNav::new(
            settings(cx).side_nav_collapsed,
            self.route().nav_section(),
            {
                let shell = shell.clone();
                move |target, _, cx| shell.update(cx, |this, cx| this.navigate(target, cx))
            },
            |_, cx| Shell::toggle_side_nav(cx),
            move |_, cx| shell.update(cx, |_, cx| cx.emit(ShellEvent::OpenSettings)),
        )
        .width(self.nav_width)
        .into_any_element()
    }

    /// Phần thanh trên (`Island`). Phóng to / khôi phục đổi cỡ cửa sổ, GPUI làm mới cả cửa sổ.
    fn bar_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity();
        AppBar::new(
            self.can_back(),
            window.is_maximized(),
            {
                let shell = shell.clone();
                move |_, cx| shell.update(cx, |this, cx| this.back(cx))
            },
            move |_, cx| shell.update(cx, |_, cx| cx.emit(ShellEvent::OpenSearch)),
        )
        .into_any_element()
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
        let c = cx.theme().colors().clone();
        // Bề rộng nội suy ở đây (không trong cột trái): view `cached` cần chủ đưa cỡ.
        self.nav_width = side_nav::animated_width(settings(cx).side_nav_collapsed, window, cx);
        // Vùng kéo cửa sổ (thanh trên, header cột trái — `data-tauri-drag-region` của bản web) nằm ở đây, sau phần `cached`:
        // GPUI (gpui-pre 0.3.10) dùng lại hitbox, listener của view `cached` nhưng không chép vùng `WindowControlArea`, nên
        // vùng kéo đặt trong view `cached` mất ngay khung đầu tiên view đó được dùng lại. Hitbox của khung bọc vẽ trước, nằm
        // sau mọi phần tử của thanh; nút `occlude` chặn nó, chỗ trống thì không.
        let nav = div()
            .relative()
            .w(px(self.nav_width))
            .h_full()
            .flex_none()
            .child(div().absolute().top_0().left_0().w_full().h(px(side_nav::HEADER_HEIGHT)).drag_region())
            .child(island::cached(&self.nav, |s| s.size_full()))
            // Kiểu lưới: đường dưới header cột trái nối tiếp đường dưới thanh trên (cùng cao 36 px). Phủ tuyệt đối để không
            // đổi bố cục của cột trái `cached`.
            .children(
                layout::hline(cx)
                    .map(|line| div().absolute().top(px(side_nav::HEADER_HEIGHT)).left_0().w_full().child(line)),
            );
        let bar = div()
            .w_full()
            .h(px(app_bar::HEIGHT))
            .flex_none()
            .drag_region()
            .child(island::cached(&self.bar, |s| s.size_full()));

        let content_view = self.slots.get(content_slot(route)).map(AnyView::entity_id);
        if content_view != self.content_view {
            self.content_view = content_view;
            bars_overlay::forget_all(cx);
        }
        let content = match (content_slot(route), self.slots.get(content_slot(route))) {
            // Trang thư viện không có hoạt ảnh lặp ngoài thanh nhảy của bài đang phát: nhúng `cached`, khung của chữ chạy
            // ở thanh phát không dựng lại lưới / danh sách.
            (Slot::Library, Some(view)) => {
                view.clone().cached(StyleRefinement::default().size_full()).into_any_element()
            }
            // Now Playing có tên bài chạy: nhúng thường, phần nặng bên trong tự `cached`.
            (_, Some(view)) => view.clone().into_any_element(),
            (_, None) => Shell::placeholder(route, cx).into_any_element(),
        };
        let player = self.slots.get(Slot::PlayerBar).map(|view| view.clone().into_any_element());

        div()
            .id("shell")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .text_ink(cx)
            .font_family(cx.theme().font_family())
            .text_size(cx.theme().font_size())
            .child(
                div().flex_1().min_h_0().flex().child(nav).children(layout::vline(cx)).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .bg(c.surface)
                        .child(bar)
                        .children(layout::hline(cx))
                        .child(
                            div().id("content").relative().flex_1().min_h_0().child(content).child(self.bars.clone()),
                        ),
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
            .children(self.slots.get(Slot::Settings).map(|view| view.clone().into_any_element()))
            .child(ToastViewport::new().inset_top(px(app_bar::HEIGHT)))
    }
}

// ---- Lối tắt cho view khác ----

/// `Global` trỏ tới khung đang chạy.
struct ShellHandle(Entity<Shell>);

impl Global for ShellHandle {}

/// Đặt khung cho test của view cần `app::shell` (`run` mới đặt global này).
#[cfg(test)]
pub fn set_shell_for_test(shell: Entity<Shell>, cx: &mut App) {
    cx.set_global(ShellHandle(shell));
}

/// `Global` trỏ tới cửa sổ chính (cửa sổ PiP là cửa sổ thứ hai).
struct MainWindow(AnyWindowHandle);

impl Global for MainWindow {}

/// Cửa sổ chính. `None` trong test không mở cửa sổ chính.
pub fn main_window(cx: &App) -> Option<AnyWindowHandle> {
    cx.try_global::<MainWindow>().map(|main| main.0)
}

/// Đưa cửa sổ chính lên trước. `activate_window` của GPUI khôi phục cửa sổ đang thu nhỏ (Windows: `IsIconic` + `SW_RESTORE`,
/// S22); Wayland không có token kích hoạt thì compositor chỉ báo cửa sổ cần chú ý (đặc tả 5.5).
pub fn bring_main_to_front(cx: &mut App) {
    cx.activate(true);
    if let Some(window) = main_window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
    }
}

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

/// Thư viện dùng chung (sao chép rẻ, gửi được giữa luồng). `None` khi không khởi động được hoặc chưa `set_library`.
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

    /// G-6: cài cài đặt là áp theme có nhãn / tiêu đề component độ đậm normal (hộp thoại, tab, nhãn ô nhập, nhãn nhóm menu).
    #[gpui::test]
    fn theme_sets_every_component_weight_to_normal(cx: &mut TestAppContext) {
        cx.update(|cx| {
            materiant::init(cx);
            install_settings_for_test(Settings::default(), cx);
            assert_eq!(cx.theme().label_weight(), FontWeight::NORMAL);
            assert_eq!(cx.theme().title_weight(), FontWeight::NORMAL);
        });
    }

    /// C2 — mở app về trang cuối đã lưu.
    #[gpui::test]
    fn opens_at_last_active_route(cx: &mut TestAppContext) {
        let settings = Settings { last_active_route: StartPage::Playlists, ..Settings::default() };
        let (shell, _, cx) = open(cx, settings);
        assert_eq!(route(&shell, cx), Route::Playlists);
        assert!(!shell.read_with(cx, |s, _| s.can_back()));
        assert_eq!(shell.read_with(cx, |s, _| s.title().to_string()), "Danh sách phát | Groovex");
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
        assert_eq!(title(cx), "Thư viện | Groovex");
        shell.update(cx, |s, cx| s.set_title_override(Some("Bài - Nghệ sĩ | Groovex".into()), cx));
        assert_eq!(title(cx), "Bài - Nghệ sĩ | Groovex");
        shell.update(cx, |s, cx| s.navigate(Route::NowPlaying, cx));
        assert_eq!(title(cx), "Đang phát | Groovex");
    }

    /// Đổi theme trong cài đặt thì theme materiant đổi theo.
    #[gpui::test]
    fn settings_change_applies_theme(cx: &mut TestAppContext) {
        let (_, _, cx) = open(cx, Settings { theme: ThemeChoice::Dark, ..Settings::default() });
        assert!(cx.update(|_, cx| cx.theme().is_dark()));
        let primary = cx.update(|_, cx| cx.theme().colors().primary);
        assert_eq!(
            primary,
            materiant::gpui_kit::Hsla::from(materiant::theme::palette::DEFAULT_SEEDS.primary.resolve(true)),
            "màu nhấn là primary của materiant (G-4)"
        );
        cx.update(|_, cx| update_settings(cx, |s| s.theme = ThemeChoice::Light));
        assert_eq!(cx.update(|_, cx| cx.theme().mode()), ThemeMode::Light);
        assert!(!cx.update(|_, cx| cx.theme().is_dark()));
    }

    /// Bảng màu: đổi preset là áp ngay, không sót màu preset trước, nền vẫn theo ô Nền, giữ font / độ đậm của groove; khoá lạ
    /// ⇒ mặc định.
    #[gpui::test]
    fn theme_preset_applies_on_top_of_the_default(cx: &mut TestAppContext) {
        use materiant::gpui_kit::Hsla;
        use materiant::theme::{DEEPPINK_THEME, REBECCA_PURPLE_THEME};
        let (_, _, cx) = open(cx, Settings { theme: ThemeChoice::Light, ..Settings::default() });
        let primary = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.theme().colors().primary);
        let default_light = primary(cx);

        cx.update(|_, cx| update_settings(cx, |s| s.theme_preset = "rebecca-purple".into()));
        assert_eq!(primary(cx), Hsla::from(REBECCA_PURPLE_THEME.primary.unwrap().resolve(false)));
        assert_eq!(cx.update(|_, cx| cx.theme().label_weight()), FontWeight::NORMAL, "bản vá groove áp sau preset");

        cx.update(|_, cx| update_settings(cx, |s| s.theme_preset = "deeppink".into()));
        assert!(!cx.update(|_, cx| cx.theme().is_dark()), "bảng màu không đổi nền");
        assert_eq!(primary(cx), Hsla::from(DEEPPINK_THEME.primary.unwrap().resolve(false)));
        // deeppink chỉ đổi màu nhấn: nền về như mặc định, không sót nền của Rebecca Purple.
        let surface = cx.update(|_, cx| cx.theme().colors().surface);
        let default_surface = materiant::theme::palette::DEFAULT_SEEDS.surface.resolve(false);
        assert_eq!(surface, Hsla::from(default_surface));

        cx.update(|_, cx| update_settings(cx, |s| s.theme_preset = "no-such-preset".into()));
        assert_eq!(primary(cx), default_light, "khoá lạ ⇒ mặc định");
        let seeds_default =
            cx.update(|_, cx| materiant::theme::picker::matching_preset(&builtin_presets(), cx.theme().seeds()));
        assert_eq!(seeds_default, Some(0), "không sót màu của preset trước");
    }

    #[test]
    fn every_preset_has_a_translated_label() {
        for preset in theme_presets() {
            assert_ne!(preset.label, preset.key, "{} chưa có bản dịch", preset.key);
        }
    }

    /// layout-style — đổi kiểu chia vùng thì trang thư viện (nhúng `cached`, không nghe cài đặt) dựng lại theo kiểu mới; đổi
    /// cài đặt khác (âm lượng) không dựng lại nó.
    #[gpui::test]
    fn layout_style_change_rebuilds_the_cached_page(cx: &mut TestAppContext) {
        use std::cell::Cell;

        struct Page {
            renders: Rc<Cell<usize>>,
            seen: Rc<Cell<LayoutStyle>>,
        }
        impl Render for Page {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.renders.set(self.renders.get() + 1);
                self.seen.set(crate::views::layout::style(cx));
                div().size_full().children(crate::views::layout::hline(cx))
            }
        }

        let (shell, _, cx) = open(cx, Settings::default());
        let (renders, seen) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(LayoutStyle::Grid)));
        let page = cx.new(|_| Page { renders: renders.clone(), seen: seen.clone() });
        shell.update(cx, |s, cx| s.set_slot(Slot::Library, page, cx));
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
        };
        draw(cx);
        assert_eq!(seen.get(), LayoutStyle::Cards, "mặc định thẻ");

        let before = renders.get();
        cx.update(|_, cx| update_settings(cx, |s| s.volume = 30));
        draw(cx);
        assert_eq!(renders.get(), before, "cài đặt khác không dựng lại trang `cached`");

        cx.update(|_, cx| update_settings(cx, |s| s.layout_style = LayoutStyle::Grid));
        draw(cx);
        assert_eq!(seen.get(), LayoutStyle::Grid, "trang `cached` vẽ theo kiểu mới");
        assert_eq!(renders.get(), before + 1);

        cx.update(|_, cx| update_settings(cx, |s| s.layout_style = LayoutStyle::Cards));
        draw(cx);
        assert_eq!(seen.get(), LayoutStyle::Cards, "về lại kiểu thẻ");
    }

    /// cpu — hoạt ảnh lặp ở thanh phát không dựng lại trang thư viện (nhúng `cached`); trang vẫn vẽ dữ liệu mới khi nó
    /// `notify`, cột trái vẫn theo màn mới khi Shell đổi màn.
    #[gpui::test]
    fn animation_frames_reuse_the_cached_page_but_data_changes_still_show(cx: &mut TestAppContext) {
        use std::cell::Cell;
        use std::time::Duration;

        use crate::views::playing_bars::PlayingBars;

        struct Page {
            value: u32,
            renders: Rc<Cell<usize>>,
            shown: Rc<Cell<u32>>,
        }
        impl Render for Page {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.renders.set(self.renders.get() + 1);
                self.shown.set(self.value);
                div().size_full().child(self.value.to_string())
            }
        }
        struct Bar;
        impl Render for Bar {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().child(PlayingBars::new(true))
            }
        }

        let (shell, _, cx) = open(cx, Settings::default());
        let (renders, shown) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let page = cx.new(|_| Page { value: 1, renders: renders.clone(), shown: shown.clone() });
        let bar = cx.new(|_| Bar);
        shell.update(cx, |s, cx| {
            s.set_slot(Slot::Library, page.clone(), cx);
            s.set_slot(Slot::PlayerBar, bar, cx);
        });
        cx.update(|window, _| window.activate_window());
        let frames = |n: usize, cx: &mut VisualTestContext| {
            for _ in 0..n {
                cx.executor().advance_clock(Duration::from_millis(16));
                let delivered = cx.update(|window, cx| {
                    let delivered = window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                    delivered
                });
                assert!(delivered > 0, "thanh nhảy phải xin khung kế");
                cx.run_until_parked();
            }
        };
        frames(2, cx);
        let before = renders.get();
        assert!(before >= 1 && shown.get() == 1);
        frames(20, cx);
        assert_eq!(renders.get(), before, "khung của hoạt ảnh không dựng lại trang");

        page.update(cx, |p, cx| {
            p.value = 2;
            cx.notify();
        });
        frames(1, cx);
        assert_eq!(shown.get(), 2, "trang `cached` vẫn vẽ dữ liệu mới khi nó `notify`");
        assert_eq!(renders.get(), before + 1);

        // Cột trái (phần `cached` của Shell) theo màn mới: vạch chỉ báo trượt về hàng Playlists.
        shell.update(cx, |s, cx| s.navigate(Route::Playlists, cx));
        cx.executor().advance_clock(Duration::from_millis(600));
        frames(3, cx);
        let ink = cx.debug_bounds("side-nav-ink").expect("vạch đã vẽ");
        assert_eq!(ink.origin.y, px(crate::views::side_nav::HEADER_HEIGHT + 2. * crate::views::side_nav::ITEM_HEIGHT));
    }

    /// cpu — thanh nhảy trong trang `cached` vẽ ở lớp phủ: khung của nó không dựng lại trang; hàng thôi phát hay đổi màn thì
    /// lớp phủ không vẽ ở chỗ cũ và thôi xin khung.
    #[gpui::test]
    fn bars_in_a_cached_page_animate_on_the_overlay_without_rebuilding_the_page(cx: &mut TestAppContext) {
        use std::cell::Cell;
        use std::time::Duration;

        use crate::views::bars_overlay;

        struct Page {
            playing: bool,
            renders: Rc<Cell<usize>>,
        }
        impl Render for Page {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                bars_overlay::forget(cx.entity_id(), cx);
                self.renders.set(self.renders.get() + 1);
                div().size_full().when(self.playing, |d| d.child(bars_overlay::anchor(px(14.))))
            }
        }

        let (shell, _, cx) = open(cx, Settings::default());
        let renders = Rc::new(Cell::new(0));
        let page = cx.new(|_| Page { playing: true, renders: renders.clone() });
        shell.update(cx, |s, cx| s.set_slot(Slot::Library, page.clone(), cx));
        cx.update(|window, _| window.activate_window());
        // Số khung đã xin được giao.
        let frames = |n: usize, cx: &mut VisualTestContext| {
            let mut delivered = 0;
            for _ in 0..n {
                cx.executor().advance_clock(Duration::from_millis(16));
                delivered += cx.update(|window, cx| {
                    let delivered = window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                    delivered
                });
                cx.run_until_parked();
            }
            delivered
        };
        frames(2, cx);
        let before = renders.get();
        assert_eq!(cx.update(|_, cx| bars_overlay::count(cx)), 1, "hàng đang phát đặt anchor");
        assert!(frames(20, cx) >= 20, "lớp phủ xin khung mỗi nhịp");
        assert_eq!(renders.get(), before, "khung của thanh nhảy không dựng lại trang `cached`");
        assert_eq!(cx.update(|_, cx| bars_overlay::count(cx)), 1, "trang dùng lại từ cache vẫn giữ anchor");

        page.update(cx, |p, cx| {
            p.playing = false;
            cx.notify();
        });
        frames(2, cx);
        assert_eq!(cx.update(|_, cx| bars_overlay::count(cx)), 0, "hàng thôi phát: anchor cũ bị bỏ");
        assert_eq!(frames(5, cx), 0, "không còn thanh chạy thì không xin khung");

        page.update(cx, |p, cx| {
            p.playing = true;
            cx.notify();
        });
        frames(2, cx);
        assert_eq!(cx.update(|_, cx| bars_overlay::count(cx)), 1);
        shell.update(cx, |s, cx| s.navigate(Route::NowPlaying, cx));
        frames(2, cx);
        assert_eq!(cx.update(|_, cx| bars_overlay::count(cx)), 0, "đổi màn: anchor của trang cũ hết hiệu lực");
    }
}
