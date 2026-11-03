//! pip (03/11/2026): trình phát thu nhỏ — cửa sổ nhỏ kiểu Picture in picture của trình duyệt.
//!
//! Cửa sổ 420 × 116 không viền, cỡ cố định, đệm 8 px: bìa vuông 100 px bên trái; bên phải tên bài (một dòng, [`Marquee`]) + nút quay lại cửa
//! sổ chính ở góc trên phải, thanh tua ([`SeekBar::without_times`], giãn hết bề ngang), hàng nút Shuffle / Lùi / Phát-Dừng /
//! Tới / Repeat ([`Transport`] của thanh phát); ba khối cách nhau và cách mép 8 px. Chỗ trống là vùng kéo cửa sổ, thanh
//! tua `occlude` để kéo là tua. Dùng chung `Player` với cửa sổ chính (`player::player`).
//!
//! - **Mở / đóng**: [`toggle`] từ nút PiP và menu chuột phải của thanh phát; [`open`] từ menu dock (`groove --pip`, jump list
//!   Windows, dock macOS — `open_paths`, [`install`]). Nút quay lại ([`back_to_main`]) đóng PiP và đưa cửa sổ chính lên. Mở PiP
//!   để yên cửa sổ chính như trình duyệt (`plan/confirm.md` mục `pip`); đóng cửa sổ chính là thoát app, PiP đóng theo.
//! - **Trên cùng**: `platform::always_on_top` (Windows, macOS); Linux là cửa sổ thường.
//! - **Vị trí**: lưu ở `Settings::pip_window` mỗi lần dời; mở lại đúng chỗ ở nền đặt được vị trí (`window_state::restore_fixed`),
//!   lần đầu ở góc dưới phải màn của cửa sổ chính. Wayland: compositor tự đặt.
//! - **CPU** theo bố cục của thanh phát (`groove/CLAUDE.md` mục "Hoạt ảnh lặp"): tên chạy là phần duy nhất dựng lại mỗi khung;
//!   bìa và hàng nút là `Island` nhúng `cached`, thanh tua là view `cached` riêng nghe nhịp vị trí. Tên chỉ chạy khi cửa sổ PiP
//!   active và đang phát (`Marquee`).

use std::sync::Arc;

use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    self, AnyElement, AnyWindowHandle, App, Bounds, ClickEvent, Context, DisplayId, Entity, FontWeight, Global,
    IntoElement, ParentElement, Pixels, Render, RenderImage, SharedString, StyleRefinement, Styled, Subscription,
    Window, WindowBounds, WindowDecorations, WindowOptions, div, point, px, size,
};
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::ActiveTheme;
use materiant::virtual_grid::LruImageCache;
use materiant::window::DragRegionExt;

use crate::app;
use crate::icons::GrooveIcon;
use crate::player::{self, Player, PlayerEvent};
use crate::views::cover_image::{CoverImage, CoverKind, PlaceholderLook};
use crate::views::island::{self, Island};
use crate::views::marquee::Marquee;
use crate::views::player_bar::{BarState, ControlSet, Transport, web_icon_button};
use crate::views::seek_bar::{self, SeekBar};

/// Cỡ cửa sổ (như PiP của trình duyệt cho một dòng nhạc): cao vừa đủ ba khối của cột phải cộng đệm.
pub const SIZE: (f32, f32) = (420., PADDING * 2. + COVER);
/// Bìa vuông cao bằng cột phải (ba khối + hai khe).
const COVER: f32 = HEADER_HEIGHT + seek_bar::HIT_HEIGHT + CONTROLS_HEIGHT + GAP * 2.;
/// Lần đầu (chưa lưu vị trí): cách mép phải, mép dưới của màn.
const SCREEN_MARGIN: f32 = 32.;
/// Đệm cửa sổ bốn phía; khe giữa bìa và cột phải, giữa ba khối của cột.
const PADDING: f32 = 8.;
const GAP: f32 = 8.;
/// Hàng tên bài: cao bằng nút quay lại.
const HEADER_HEIGHT: f32 = 28.;
const BACK_BOX: f32 = 28.;
const BACK_GLYPH: f32 = 18.;
/// Hàng nút: cao bằng nút Phát.
const CONTROLS_HEIGHT: f32 = PIP_CONTROLS.play_size.1;
const COVER_CACHE: usize = 2;

const PIP_CONTROLS: ControlSet = ControlSet {
    play: "pip-play",
    shuffle: "pip-shuffle",
    prev: "pip-prev",
    next: "pip-next",
    repeat: "pip-repeat",
    icon: (30., 22.),
    play_size: (28., 40.),
    gap: 8.,
};

/// Âm lượng không vẽ ở PiP: `BarState` của nó không đọc cài đặt.
fn no_prefs(_: &App) -> (u8, bool) {
    (0, false)
}

// ---- Trạng thái mở / đóng ----

/// Cửa sổ PiP đang mở (nếu có). Thanh phát nghe entity này để đổi nhãn nút PiP.
#[derive(Default)]
pub struct PipState {
    window: Option<AnyWindowHandle>,
}

struct PipGlobal(Entity<PipState>);

impl Global for PipGlobal {}

/// Entity trạng thái, tạo ở lần gọi đầu.
pub fn state(cx: &mut App) -> Entity<PipState> {
    if let Some(global) = cx.try_global::<PipGlobal>() {
        return global.0.clone();
    }
    let entity = cx.new(|_| PipState::default());
    // Cửa sổ PiP đóng bằng đường nào (nút quay lại, Alt+F4, OS) cũng về trạng thái đóng.
    let watched = entity.clone();
    cx.on_window_closed(move |cx, closed| {
        watched.update(cx, |state, cx| {
            if state.window.is_some_and(|window| window.window_id() == closed) {
                state.window = None;
                cx.notify();
            }
        });
    })
    .detach();
    cx.set_global(PipGlobal(entity.clone()));
    entity
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<PipGlobal>().is_some_and(|global| global.0.read(cx).window.is_some())
}

fn window(cx: &App) -> Option<AnyWindowHandle> {
    cx.try_global::<PipGlobal>().and_then(|global| global.0.read(cx).window)
}

/// Nút PiP, menu chuột phải: mở nếu đang đóng, đóng nếu đang mở.
pub fn toggle(cx: &mut App) {
    if is_open(cx) { close(cx) } else { open(cx) }
}

/// Mở PiP; đang mở thì đưa nó lên trước.
pub fn open(cx: &mut App) {
    if let Some(window) = window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let options = window_options(cx);
    let covers = CoverStore::default_location().map_err(|err| log::warn!("pip: cannot locate covers: {err}")).ok();
    let opened = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| PipView::new(covers, window, cx)));
    match opened {
        Ok((handle, _)) => {
            let entity = state(cx);
            entity.update(cx, |state, cx| {
                state.window = Some(handle);
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, cx| {
                if !crate::platform::always_on_top::keep_on_top(window, cx) {
                    log::info!("pip: always on top is up to the window manager on this platform");
                }
            });
        }
        Err(err) => log::error!("cannot open the picture-in-picture window: {err:#}"),
    }
}

/// Đóng PiP (không làm gì nếu đang đóng). Hoãn tới sau sự kiện đang chạy: nút quay lại nằm trong chính cửa sổ PiP, mà GPUI
/// từ chối `update` một cửa sổ đang xử lý sự kiện của nó.
pub fn close(cx: &mut App) {
    if let Some(window) = window(cx) {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        });
    }
}

/// Nút quay lại: đóng PiP, đưa cửa sổ chính lên (khôi phục nếu đang thu nhỏ).
pub fn back_to_main(cx: &mut App) {
    close(cx);
    app::bring_main_to_front(cx);
}

/// Không viền, cỡ cố định, nền đục (chữ giữ ClearType). Vị trí: đã lưu, không thì góc dưới phải màn của cửa sổ chính.
fn window_options(cx: &mut App) -> WindowOptions {
    let pip_size = size(px(SIZE.0), px(SIZE.1));
    let (bounds, display_id) = crate::window_state::restore_fixed(app::settings(cx).pip_window.as_ref(), pip_size, cx)
        .unwrap_or_else(|| default_bounds(pip_size, cx));
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        is_resizable: false,
        is_minimizable: false,
        // Như trình duyệt: mở PiP không lấy focus của cửa sổ đang dùng.
        focus: false,
        display_id,
        // Trùng `app_id` của cửa sổ chính: dock Linux gom về một icon.
        app_id: Some(crate::window_state::APP_ID.into()),
        window_min_size: Some(pip_size),
        ..materiant::window::window_options()
    }
}

fn default_bounds(pip_size: gpui_kit::Size<Pixels>, cx: &mut App) -> (Bounds<Pixels>, Option<DisplayId>) {
    let display = app::main_window(cx)
        .and_then(|main| main.update(cx, |_, window, cx| window.display(cx)).ok().flatten())
        .or_else(|| cx.primary_display());
    match display {
        Some(display) => {
            let area = display.bounds();
            let origin = point(
                area.origin.x + area.size.width - pip_size.width - px(SCREEN_MARGIN),
                area.origin.y + area.size.height - pip_size.height - px(SCREEN_MARGIN),
            );
            (Bounds::new(origin, pip_size), Some(display.id()))
        }
        None => (Bounds::new(point(px(0.), px(0.)), pip_size), None),
    }
}

// ---- View ----

pub struct PipView {
    player: Entity<Player>,
    seek: Entity<SeekBar>,
    transport: Transport,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    /// Bìa của tệp ngoài, giải mã sẵn ở `Player`.
    external: Option<Arc<RenderImage>>,
    /// Lần `notify` gần nhất vẽ theo trạng thái này (phần `cached` chỉ dựng lại khi nó đổi).
    state: BarState,
    window_title: SharedString,
    cover_island: Entity<Island<PipView>>,
    controls_island: Entity<Island<PipView>>,
    /// Số lần dựng hàng nút (test cpu).
    #[cfg(test)]
    controls_built: usize,
    _subscriptions: Vec<Subscription>,
}

impl PipView {
    pub fn new(covers: Option<CoverStore>, window: &mut Window, cx: &mut Context<Self>) -> PipView {
        let player = player::player(cx);
        let seek = cx.new(|cx| SeekBar::new(window, cx).without_times());
        let subscriptions = vec![
            cx.observe(&player, |this, player, cx| {
                let now = BarState::of(player.read(cx), no_prefs, cx);
                if now != this.state {
                    this.state = now;
                    cx.notify();
                }
            }),
            cx.subscribe(&player, |this, _, event: &PlayerEvent, cx| {
                if matches!(event, PlayerEvent::TrackChanged) {
                    this.refresh_external(cx);
                }
            }),
            // Dời cửa sổ là lưu vị trí (`JsonSaver` gom lần ghi đĩa).
            cx.observe_window_bounds(window, |_, window, cx| {
                let placement = crate::window_state::current(window, cx);
                if app::settings(cx).pip_window.as_ref() != Some(&placement) {
                    app::update_settings(cx, |s| s.pip_window = Some(placement));
                }
            }),
        ];
        let state = BarState::of(player.read(cx), no_prefs, cx);
        let mut view = PipView {
            transport: Transport::new(player.clone(), cx),
            player,
            seek,
            covers,
            cache: LruImageCache::new(COVER_CACHE, cx),
            external: None,
            state,
            window_title: SharedString::default(),
            cover_island: Island::new(PipView::cover_part, cx),
            controls_island: Island::new(PipView::controls_part, cx),
            #[cfg(test)]
            controls_built: 0,
            _subscriptions: subscriptions,
        };
        view.refresh_external(cx);
        view
    }

    fn refresh_external(&mut self, cx: &mut Context<Self>) {
        self.external = self.player.read(cx).external_cover().cloned();
        cx.notify();
    }

    fn is_enabled(&self, cx: &App) -> bool {
        self.player.read(cx).current_song().is_some()
    }

    /// Bìa vuông cao bằng cửa sổ (`Island`, nhúng `cached`).
    fn cover_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let path = self.player.read(cx).current_song().and_then(|song| {
            let hash = song.cover_hash.as_deref()?;
            Some(self.covers.as_ref()?.path(hash, CoverSize::Small))
        });
        CoverImage::new(path, CoverKind::Album, None, &self.cache)
            .embedded(self.external.clone())
            .radius(px(4.))
            // Đĩa giữ chỗ lấp cả ô bìa: `Thumb` thụt icon 8 px mỗi bên, đĩa trong `album.svg` rộng 40 / 46 ô icon; mép đĩa
            // cách cột phải đúng khe 8 px.
            .look(PlaceholderLook::Thumb(px(COVER * 46. / 40. + 16.)))
            .into_any_element()
    }

    /// Hàng nút (`Island`, nhúng `cached`).
    fn controls_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        #[cfg(test)]
        {
            self.controls_built += 1;
        }
        let enabled = self.is_enabled(cx);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(self.transport.controls(&PIP_CONTROLS, enabled, window, cx))
            .into_any_element()
    }
}

/// Nút góc trên phải: quay lại cửa sổ chính.
fn back_button(cx: &App) -> impl IntoElement {
    let label = t!("player.pip.back");
    div().occlude().debug_selector(|| "pip-back".to_owned()).child(
        web_icon_button("pip-back", GrooveIcon::PipExit, BACK_BOX, BACK_GLYPH, false, cx)
            .accessibility_label(label)
            .on_click(|_: &ClickEvent, _, cx| back_to_main(cx))
            .with_tooltip(("pip-back", 0usize), label),
    )
}

impl Render for PipView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = SharedString::from(t!("player.pip.window_title"));
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
        self.transport.sync(cx);
        let colors = cx.theme().colors().clone();
        let playing = self.player.read(cx).is_playing();
        let song_title = match self.player.read(cx).current_song() {
            Some(song) => song.display_title().to_owned(),
            None => t!("player.bar.no_song").to_owned(),
        };

        let header = div()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            // Một dòng, cao cố định: tên dài chạy chữ, không cắt "…".
            .child(div().flex_1().min_w_0().child(Marquee::new("pip-title", song_title).paused(!playing)))
            .child(back_button(cx));
        // `occlude`: thanh tua không thuộc vùng kéo cửa sổ, kéo thanh là tua chứ không dời cửa sổ (xem `seek_bar`).
        let seek = div()
            .w_full()
            .h(px(seek_bar::HIT_HEIGHT))
            .flex_none()
            .occlude()
            .child(self.seek.clone().cached(StyleRefinement::default().size_full()));
        let controls = island::cached(&self.controls_island, |s| s.w_full().h(px(CONTROLS_HEIGHT)).flex_none());

        // Vùng kéo là khung gốc (không `cached`, xem bẫy `WindowControlArea` ở `groove/CLAUDE.md`); nút `occlude` chặn nó.
        div()
            .id("pip")
            .size_full()
            .flex()
            .bg(colors.surface)
            .text_color(colors.text)
            .font_family(cx.theme().font_family())
            .text_size(cx.theme().font_size())
            .font_weight(FontWeight::NORMAL)
            .drag_region()
            .p(px(PADDING))
            .gap(px(GAP))
            .child(island::cached(&self.cover_island, |s| s.size(px(COVER)).flex_none()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    // Ba khối cách nhau 8 px, cửa sổ cao vừa khít (xem `SIZE`).
                    .gap(px(GAP))
                    .child(header)
                    .child(seek)
                    .child(controls),
            )
    }
}

/// Mục "Trình phát thu nhỏ" ở menu dock macOS / jump list Windows. Action phải đăng ký trước khi đặt menu.
mod dock {
    use materiant::gpui_kit as gpui;
    use materiant::gpui_kit::{App, MenuItem, actions};

    actions!(groove, [OpenPictureInPicture]);

    pub fn install(cx: &mut App) {
        cx.on_action(|_: &OpenPictureInPicture, cx| super::open(cx));
        refresh(cx);
    }

    /// Đặt lại nhãn theo ngôn ngữ đang dùng. Linux: menu dock là `Actions=` của `groove.desktop`, GPUI bỏ qua lời gọi này.
    pub fn refresh(cx: &mut App) {
        cx.set_dock_menu(vec![MenuItem::action(groove_core::t!("player.bar.pip"), OpenPictureInPicture)]);
    }
}

/// Đăng ký action, menu dock. Gọi một lần trong `app::run`.
pub fn install(cx: &mut App) {
    state(cx);
    dock::install(cx);
}

/// Ngôn ngữ đổi: nhãn menu dock theo.
pub fn refresh_dock_menu(cx: &mut App) {
    dock::refresh(cx);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui::{Modifiers, TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::model::TrackRef;
    use groove_core::settings::Settings;
    use groove_library::fixtures::{self, AudioKind};
    use tempfile::TempDir;

    use super::*;
    use crate::player::PlayerDeps;

    struct Rig {
        dir: TempDir,
        fake: FakeOutput,
        player: Entity<Player>,
    }

    /// Player không phát tiếng (`FakeOutput`), không thư viện; cài đặt mặc định không lưu đĩa.
    fn rig(cx: &mut TestAppContext) -> Rig {
        cx.executor().allow_parking();
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let fake = FakeOutput::new(48_000, 2);
        let deps = PlayerDeps {
            library: None,
            output: Box::new(fake.clone()),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: None,
        };
        let player = cx.update(|cx| {
            materiant::init(cx);
            app::install_settings_for_test(Settings::default(), cx);
            let player = player::install_for_test(deps, cx);
            install(cx);
            player
        });
        Rig { dir: TempDir::new().unwrap(), fake, player }
    }

    impl Rig {
        fn until(&self, cx: &mut TestAppContext, what: &str, condition: impl Fn(&Player) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                cx.run_until_parked();
                if self.player.read_with(cx, |p, _| condition(p)) {
                    return;
                }
                assert!(Instant::now() < deadline, "timed out: {what}");
                let _ = self.fake.pull(480);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn open_pip(cx: &mut TestAppContext) -> &mut VisualTestContext {
        cx.update(open);
        cx.run_until_parked();
        let handle = cx.read(window).expect("PiP đã mở");
        let cx = VisualTestContext::from_window(handle, cx).into_mut();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx
    }

    fn click(cx: &mut VisualTestContext, id: &'static str) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("không thấy {id}")).center();
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_click(at, Modifiers::none());
    }

    /// Mở hai lần vẫn một cửa sổ; cửa sổ cỡ cố định, chưa có bài thì hàng nút tắt nhưng vẽ được.
    #[gpui::test]
    fn opens_once_with_the_fixed_size(cx: &mut TestAppContext) {
        let _rig = rig(cx);
        assert!(!cx.read(is_open));
        let vcx = open_pip(cx);
        assert!(vcx.debug_bounds("pip-play").is_some());
        assert!(vcx.debug_bounds("pip-back").is_some());
        let bounds = vcx.update(|window, _| window.bounds());
        assert_eq!(bounds.size, size(px(SIZE.0), px(SIZE.1)));
        cx.update(open);
        cx.run_until_parked();
        assert!(cx.read(is_open));
        assert_eq!(cx.read(|cx| cx.windows().len()), 1, "mở lại chỉ đưa PiP lên, không mở cửa sổ thứ hai");
    }

    /// Hai cửa sổ chung một `Player`: nút của PiP điều khiển đúng player của app.
    #[gpui::test]
    fn controls_drive_the_shared_player(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let first = fixtures::write_sample(rig.dir.path(), "a", AudioKind::Wav).unwrap();
        let second = fixtures::write_sample(rig.dir.path(), "b", AudioKind::Wav).unwrap();
        rig.player.update(cx, |p, cx| p.play_paths(vec![first, second.clone()], cx));
        rig.until(cx, "song loaded", |p| p.current_song().is_some() && p.duration_sec() > 0.);
        rig.player.update(cx, |p, cx| p.pause(cx));
        let vcx = open_pip(cx);
        click(vcx, "pip-next");
        let track = TrackRef::External(second);
        rig.until(cx, "next", |p| p.current_track() == Some(&track) && p.current_song().is_some());
        let vcx = VisualTestContext::from_window(cx.read(window).unwrap(), cx).into_mut();
        click(vcx, "pip-shuffle");
        assert!(rig.player.read_with(cx, |p, _| p.queue().shuffle()));
    }

    /// Nút quay lại đóng PiP; trạng thái mở / đóng theo cả khi cửa sổ đóng bằng đường khác.
    #[gpui::test]
    fn back_closes_the_window_and_toggle_reopens(cx: &mut TestAppContext) {
        let _rig = rig(cx);
        let vcx = open_pip(cx);
        click(vcx, "pip-back");
        cx.run_until_parked();
        assert!(!cx.read(is_open));
        assert_eq!(cx.read(|cx| cx.windows().len()), 0);

        cx.update(toggle);
        cx.run_until_parked();
        assert!(cx.read(is_open));
        let handle = cx.read(window).unwrap();
        handle.update(cx, |_, window, _| window.remove_window()).unwrap();
        cx.run_until_parked();
        assert!(!cx.read(is_open), "đóng bằng OS (Alt+F4) cũng về trạng thái đóng");
    }

    /// cpu — tên dài đang chạy ở PiP active: mỗi khung chỉ dựng lại tên, hàng nút `cached` không dựng lại.
    #[gpui::test]
    fn running_title_does_not_rebuild_the_controls(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let name = "a very long song title that never fits in the picture in picture window at all";
        let path = fixtures::write_sample(rig.dir.path(), name, AudioKind::Wav).unwrap();
        rig.player.update(cx, |p, cx| p.play_paths(vec![path], cx));
        rig.until(cx, "playing", |p| p.current_song().is_some() && p.is_playing());
        let vcx = open_pip(cx);
        vcx.update(|window, _| window.activate_window());
        let built = |vcx: &mut VisualTestContext| {
            let view = vcx.update(|window, _| window.root::<materiant::gpui_kit::base::Root>()).flatten().unwrap();
            vcx.update(|_, cx| view.read(cx).view().clone().downcast::<PipView>().unwrap().read(cx).controls_built)
        };
        let mut delivered_total = 0;
        let mut frame = |vcx: &mut VisualTestContext| {
            vcx.executor().advance_clock(Duration::from_millis(16));
            let delivered = vcx.update(|window, cx| {
                let delivered = window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
                delivered
            });
            delivered_total += delivered;
        };
        for _ in 0..3 {
            frame(vcx);
        }
        let before = built(vcx);
        for _ in 0..10 {
            frame(vcx);
        }
        assert!(delivered_total > 0, "tên dài phải chạy (xin khung)");
        assert_eq!(built(vcx), before, "khung của chữ chạy dựng lại hàng nút");
    }

    /// Dời / đổi cửa sổ là lưu vị trí vào cài đặt.
    #[gpui::test]
    fn moving_the_window_saves_its_place(cx: &mut TestAppContext) {
        let _rig = rig(cx);
        let vcx = open_pip(cx);
        vcx.simulate_resize(size(px(SIZE.0), px(SIZE.1 + 1.)));
        vcx.run_until_parked();
        let saved = cx.read(|cx| app::settings(cx).pip_window.clone()).expect("đã lưu vị trí");
        assert_eq!(saved.width, SIZE.0);
        assert!(!saved.maximized);
    }
}
