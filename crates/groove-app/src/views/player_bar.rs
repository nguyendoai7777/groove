//! groove-gd1_14: thanh phát ở đáy cửa sổ (đặc tả 1.3, N5, S20).
//!
//! Trái: bìa vuông 90 px (hover phủ nút phát; bấm bìa hoặc chữ sang Now Playing), nghệ sĩ, tiêu đề chạy [`Marquee`] khi tràn.
//! Giữa: Shuffle, Prev, Play / Pause, Next, Repeat, dưới là [`SeekBar`]. Phải: Mute, thanh âm lượng (bánh xe ±1), Settings.
//! Không có bài thì cả thanh mờ và không bấm được. Mọi thao tác đi qua `player::player(cx)`; âm lượng qua `Player`, không
//! qua `app::update_settings`, để tới được engine.
//!
//! Lỗi phát (N5) chỉ phát sự kiện ở `Player`; chữ của toast thuộc vùng `player` nên do view này đẩy: `TrackFailed` là
//! cảnh báo (đã tự sang bài kế) hoặc lỗi (đã bỏ cuộc sau [`player::MAX_CONSECUTIVE_ERRORS`] lần), `OutputFailed` là lỗi kèm
//! cách gỡ.
//!
//! Nút Play đổi icon tức thì (đặc tả cho phép thay cho hiệu ứng biến hình 250 ms). Số giờ không nhấp nháy mỗi giây: tốn khung
//! hình liên tục (đặc tả mục 4).
//!
//! ```ignore
//! player_bar::install(window_handle, cx);   // trong `app::run`, sau `player::install`
//! ```

use std::sync::Arc;
use std::time::Duration;

use groove_audio::engine::AudioErrorKind;
use groove_core::model::RepeatMode;
use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use materiant::button::{IconButton, Size};
use materiant::gpui_kit::base::Disableable;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, ClickEvent, Context, Entity, FontWeight, Image, ImageFormat, ImageSource,
    IntoElement, ObjectFit, ParentElement, Render, ScrollEvent, Styled, Subscription, Window, black, div, img, px,
    relative,
};
use materiant::overlay::tooltip::TooltipExt;
use materiant::slider::Slider;
use materiant::theme::ActiveTheme;
use materiant::toast::{Toast, ToastKind, ToastSpec};
use materiant::virtual_grid::LruImageCache;

use crate::app::{self, Slot};
use crate::icons::GrooveIcon;
use crate::player::{self, Player, PlayerEvent};
use crate::route::Route;
use crate::views::cover_image::{CoverImage, CoverKind};
use crate::views::marquee::Marquee;
use crate::views::seek_bar::SeekBar;

/// Cạnh bìa = chiều cao thanh (`app::PLAYER_BAR_HEIGHT`).
const COVER_SIZE: f32 = 90.;
/// Số bìa giữ giải mã sẵn: chỉ bìa của bài đang phát.
const COVER_CACHE: usize = 4;
const VOLUME_WIDTH: f32 = 110.;
/// Toast lỗi thiết bị ở lại lâu hơn mặc định: người dùng cần thời gian đọc cách gỡ.
const OUTPUT_TOAST: Duration = Duration::from_secs(8);

/// Icon âm lượng: 0 hoặc tắt tiếng, 1-35, 36-65, trên 65 (đặc tả 1.3).
pub fn volume_icon(volume: u8, muted: bool) -> GrooveIcon {
    match volume {
        _ if muted => GrooveIcon::VolumeMute,
        0 => GrooveIcon::VolumeMute,
        1..=35 => GrooveIcon::VolumeMin,
        36..=65 => GrooveIcon::VolumeMedium,
        _ => GrooveIcon::VolumeMax,
    }
}

/// Toast cho `PlayerEvent::TrackFailed` (N5). Chưa biết tên bài thì nói "bài hát này". `gave_up` là lỗi, không thì cảnh báo
/// vì app đã tự sang bài kế.
pub fn track_failed_toast(kind: AudioErrorKind, title: Option<&str>, gave_up: bool) -> ToastSpec {
    let name = match title.map(str::trim).filter(|title| !title.is_empty()) {
        Some(title) => format!("“{title}”"),
        None => t!("player.toast.unknown_track").to_owned(),
    };
    let reason = match kind {
        AudioErrorKind::NotFound => t!("player.toast.kind.not_found", title = name),
        AudioErrorKind::Io => t!("player.toast.kind.io", title = name),
        AudioErrorKind::Unsupported => t!("player.toast.kind.unsupported", title = name),
        AudioErrorKind::Corrupt => t!("player.toast.kind.corrupt", title = name),
        AudioErrorKind::Output => t!("player.toast.kind.output", title = name),
    };
    let (toast_kind, message) = if gave_up {
        (ToastKind::Danger, format!("{reason} {}", t!("player.toast.gave_up")))
    } else {
        (ToastKind::Warning, reason)
    };
    ToastSpec::new(toast_kind, message).title(t!("player.toast.track_failed_title"))
}

/// Toast cho `PlayerEvent::OutputFailed`: không có thiết bị, Play thử lại.
pub fn output_failed_toast() -> ToastSpec {
    ToastSpec::new(ToastKind::Danger, t!("player.toast.output_failed"))
        .title(t!("player.toast.output_failed_title"))
        .duration(Some(OUTPUT_TOAST))
}

/// Âm lượng, tắt tiếng hiện tại. `Player` chưa có hàm đọc (ghi ở mục 8 của task), nên đọc cài đặt của app; test thay bằng bản giả.
fn app_prefs(cx: &App) -> (u8, bool) {
    let settings = app::settings(cx);
    (settings.volume, settings.is_muted)
}

type PrefsFn = fn(&App) -> (u8, bool);
type PlayerAction = fn(&mut Player, &mut Context<Player>);

pub struct PlayerBar {
    player: Entity<Player>,
    seek: Entity<SeekBar>,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    prefs: PrefsFn,
    /// Bìa của tệp ngoài, dựng một lần mỗi lần đổi bài: băm cả ảnh mỗi lần vẽ (4 lần / giây) là phí.
    external: Option<Arc<Image>>,
    _subscriptions: Vec<Subscription>,
}

impl PlayerBar {
    pub fn new(covers: Option<CoverStore>, window: &mut Window, cx: &mut Context<Self>) -> PlayerBar {
        PlayerBar::with_prefs(covers, app_prefs, window, cx)
    }

    fn with_prefs(
        covers: Option<CoverStore>,
        prefs: PrefsFn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PlayerBar {
        let player = player::player(cx);
        let seek = cx.new(SeekBar::new);
        let cache = LruImageCache::new(COVER_CACHE, cx);
        let subscriptions = vec![
            cx.observe(&player, |_, _, cx| cx.notify()),
            cx.subscribe_in(&player, window, |this, _, event: &PlayerEvent, window, cx| match event {
                PlayerEvent::TrackChanged => this.refresh_external(cx),
                PlayerEvent::TrackFailed { title, kind, gave_up, .. } => {
                    Toast::push(window, cx, track_failed_toast(*kind, title.as_deref(), *gave_up));
                }
                PlayerEvent::OutputFailed => {
                    Toast::push(window, cx, output_failed_toast());
                }
                PlayerEvent::DeviceChanged => {}
            }),
        ];
        let mut bar = PlayerBar { player, seek, covers, cache, prefs, external: None, _subscriptions: subscriptions };
        bar.refresh_external(cx);
        bar
    }

    /// Có bài đang chọn: không thì cả thanh mờ, không bấm được.
    pub fn is_enabled(&self, cx: &App) -> bool {
        self.player.read(cx).current_song().is_some()
    }

    fn refresh_external(&mut self, cx: &mut Context<Self>) {
        self.external = self.player.read(cx).external_cover().and_then(|cover| {
            Some(Arc::new(Image::from_bytes(ImageFormat::from_mime_type(&cover.mime)?, cover.data.clone())))
        });
        cx.notify();
    }

    fn cover(&self, cx: &App) -> AnyElement {
        if let Some(image) = &self.external {
            return div()
                .size_full()
                .overflow_hidden()
                .child(img(ImageSource::Image(image.clone())).size_full().object_fit(ObjectFit::Cover))
                .into_any_element();
        }
        let path = self.player.read(cx).current_song().and_then(|song| {
            let hash = song.cover_hash.as_deref()?;
            Some(self.covers.as_ref()?.path(hash, CoverSize::Small))
        });
        CoverImage::new(path, CoverKind::Album, None, &self.cache).into_any_element()
    }

    fn controls(&self, enabled: bool, cx: &App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let (playing, shuffle, repeat) = {
            let player = self.player.read(cx);
            (player.is_playing(), player.queue().shuffle(), player.queue().repeat())
        };
        let player = self.player.clone();
        let button = {
            let player = player.clone();
            move |id: &'static str, icon: GrooveIcon, label: &'static str, on: bool, action: PlayerAction| {
                let player = player.clone();
                let button = IconButton::new(id, icon)
                    .accessibility_label(label)
                    .disabled(!enabled)
                    .on_click(move |_: &ClickEvent, _, cx| player.update(cx, action));
                let button = if on { button.color(c.primary) } else { button };
                div().occlude().debug_selector(move || id.to_owned()).child(button.with_tooltip((id, 0usize), label))
            }
        };

        let (play_icon, play_label) = if playing {
            (GrooveIcon::Pause, t!("player.bar.pause"))
        } else {
            (GrooveIcon::Play, t!("player.bar.play"))
        };
        let (repeat_icon, repeat_label) = match repeat {
            RepeatMode::Off => (GrooveIcon::Loop, t!("player.bar.repeat_off")),
            RepeatMode::One => (GrooveIcon::LoopBadge, t!("player.bar.repeat_one")),
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .child(button(
                "player-shuffle",
                GrooveIcon::Shuffle,
                t!("player.bar.shuffle"),
                shuffle,
                Player::toggle_shuffle,
            ))
            .child(button("player-prev", GrooveIcon::Prev, t!("player.bar.prev"), false, Player::prev))
            .child(
                div().occlude().child(
                    IconButton::new("player-play", play_icon)
                        .filled()
                        .rounded_full()
                        .size(Size::Lg)
                        .accessibility_label(play_label)
                        .disabled(!enabled)
                        .on_click({
                            let player = player.clone();
                            move |_: &ClickEvent, _, cx| player.update(cx, |p, cx| p.toggle_play(cx))
                        })
                        .with_tooltip(("player-play", 0usize), play_label),
                ),
            )
            .child(button("player-next", GrooveIcon::Next, t!("player.bar.next"), false, Player::next))
            .child(button("player-repeat", repeat_icon, repeat_label, repeat == RepeatMode::One, Player::toggle_repeat))
    }
}

impl Render for PlayerBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let enabled = self.is_enabled(cx);
        let (volume, muted) = (self.prefs)(cx);
        let (artist, title) = match self.player.read(cx).current_song() {
            Some(song) => (
                song.display_artist().unwrap_or(t!("player.bar.unknown_artist")).to_owned(),
                song.display_title().to_owned(),
            ),
            None => (t!("player.bar.unknown_artist").to_owned(), t!("player.bar.no_song").to_owned()),
        };
        let player = self.player.clone();

        // Bấm bìa hoặc chữ sang Now Playing. Nút Play phủ trên bìa khi rê chuột.
        let cover = div()
            .id("player-cover")
            .group("player-cover")
            .relative()
            .size(px(COVER_SIZE))
            .flex_none()
            .cursor_pointer()
            .on_click(|_, _, cx| app::navigate(Route::NowPlaying, cx))
            .child(self.cover(cx))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(black().opacity(0.4))
                    .invisible()
                    .group_hover("player-cover", |style| style.visible())
                    .child(div().occlude().child({
                        let player = player.clone();
                        let (icon, label) = if player.read(cx).is_playing() {
                            (GrooveIcon::Pause, t!("player.bar.pause"))
                        } else {
                            (GrooveIcon::Play, t!("player.bar.play"))
                        };
                        IconButton::new("player-cover-play", icon)
                            .filled()
                            .rounded_full()
                            .size(Size::Lg)
                            .accessibility_label(label)
                            .on_click(move |_: &ClickEvent, _, cx| player.update(cx, |p, cx| p.toggle_play(cx)))
                    })),
            );

        let text = div()
            .id("player-text")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .justify_center()
            .gap_1()
            .cursor_pointer()
            .on_click(|_, _, cx| app::navigate(Route::NowPlaying, cx))
            .child(div().truncate().text_sm().text_color(colors.muted).child(artist))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(Marquee::new("player-title", title)));

        let left = div().w(relative(0.3)).min_w_0().flex().items_center().gap_3().child(cover).child(text);

        let center = div()
            .flex_1()
            .min_w_0()
            .max_w(px(640.))
            .flex()
            .flex_col()
            .justify_center()
            .child(self.controls(enabled, cx))
            .child(self.seek.clone());

        let volume_icon = volume_icon(volume, muted);
        let mute_label = if muted { t!("player.bar.unmute") } else { t!("player.bar.mute") };
        let right = div()
            .w(relative(0.3))
            .min_w_0()
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            .pr_3()
            .child(div().occlude().child({
                let player = player.clone();
                IconButton::new("player-mute", volume_icon)
                    .accessibility_label(mute_label)
                    .disabled(!enabled)
                    .on_click(move |_: &ClickEvent, _, cx| player.update(cx, |p, cx| p.toggle_mute(cx)))
                    .with_tooltip(("player-mute", 0usize), mute_label)
            }))
            .child(
                div()
                    .id("player-volume")
                    .occlude()
                    .on_scroll({
                        let player = player.clone();
                        move |event: &ScrollEvent, _, cx| {
                            // Bánh xe lên là tăng; một nấc cuộn là ±1 (đặc tả 1.3).
                            let dy = f32::from(event.delta.pixel_delta(px(16.)).y);
                            if dy != 0. {
                                let delta = if dy > 0. { 1 } else { -1 };
                                player.update(cx, |p, cx| p.adjust_volume(delta, cx));
                                cx.stop_propagation();
                            }
                        }
                    })
                    .child(
                        Slider::new("player-volume-slider")
                            .min(0.)
                            .max(100.)
                            .step(1.)
                            .value(f32::from(volume))
                            .w(px(VOLUME_WIDTH))
                            .accessibility_label(t!("player.bar.volume"))
                            .disabled(!enabled)
                            .on_change({
                                let player = player.clone();
                                move |value: &f32, _, cx| {
                                    let volume = value.round().clamp(0., 100.) as u8;
                                    player.update(cx, |p, cx| p.set_volume(volume, cx));
                                }
                            }),
                    ),
            )
            .child(
                div().occlude().child(
                    IconButton::new("player-settings", GrooveIcon::Settings)
                        .accessibility_label(t!("player.bar.settings"))
                        .disabled(!enabled)
                        .on_click(|_: &ClickEvent, _, cx| app::open_settings(cx))
                        .with_tooltip(("player-settings", 0usize), t!("player.bar.settings")),
                ),
            );

        let bar = div()
            .id("player-bar-view")
            .relative()
            .size_full()
            .flex()
            .items_center()
            .gap_4()
            .child(left)
            .child(center)
            .child(right);
        if enabled {
            bar
        } else {
            // Mờ và chặn mọi thao tác: `occlude` nuốt chuột, nút đã `disabled` nên Tab cũng bỏ qua.
            bar.opacity(0.5).child(div().absolute().top_0().left_0().size_full().occlude())
        }
    }
}

/// Dựng thanh phát vào `Slot::PlayerBar` của cửa sổ chính. Gọi một lần trong `app::run`, sau `player::install`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let covers = CoverStore::default_location().map_err(|err| log::warn!("cannot locate covers: {err}")).ok();
    let result = window.update(cx, |_, window, cx| {
        let bar = cx.new(|cx| PlayerBar::new(covers, window, cx));
        app::install_slot(Slot::PlayerBar, bar, cx);
    });
    if let Err(err) = result {
        log::error!("cannot install the player bar: {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, size};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_library::fixtures::{self, AudioKind};
    use materiant::gpui_kit::base::Root;
    use tempfile::TempDir;

    use super::*;
    use crate::player::PlayerDeps;

    #[test]
    fn volume_icon_follows_the_thresholds() {
        assert_eq!(volume_icon(0, false), GrooveIcon::VolumeMute);
        assert_eq!(volume_icon(50, true), GrooveIcon::VolumeMute, "tắt tiếng thắng mọi mức");
        assert_eq!(volume_icon(1, false), GrooveIcon::VolumeMin);
        assert_eq!(volume_icon(35, false), GrooveIcon::VolumeMin);
        assert_eq!(volume_icon(36, false), GrooveIcon::VolumeMedium);
        assert_eq!(volume_icon(65, false), GrooveIcon::VolumeMedium);
        assert_eq!(volume_icon(66, false), GrooveIcon::VolumeMax);
        assert_eq!(volume_icon(100, false), GrooveIcon::VolumeMax);
    }

    #[test]
    fn track_failure_toast_names_the_song_and_the_reason() {
        let kinds = [
            AudioErrorKind::NotFound,
            AudioErrorKind::Io,
            AudioErrorKind::Unsupported,
            AudioErrorKind::Corrupt,
            AudioErrorKind::Output,
        ];
        for kind in kinds {
            let spec = track_failed_toast(kind, Some("Bài A"), false);
            assert_eq!(spec.kind, ToastKind::Warning, "{kind:?}: đã tự sang bài kế");
            assert!(spec.message.contains("Bài A"), "{kind:?}: {}", spec.message);
            assert!(!spec.message.contains("player.toast"), "{kind:?}: thiếu khoá i18n: {}", spec.message);
            assert!(spec.title.is_some());
        }
        let gave_up = track_failed_toast(AudioErrorKind::Corrupt, None, true);
        assert_eq!(gave_up.kind, ToastKind::Danger);
        assert!(!gave_up.message.contains("{title}"), "chưa có tên thì dùng chữ thay: {}", gave_up.message);
        assert!(gave_up.message.len() > track_failed_toast(AudioErrorKind::Corrupt, None, false).message.len());
        assert_eq!(
            track_failed_toast(AudioErrorKind::Io, Some("  "), false).kind,
            ToastKind::Warning,
            "tên trống như không có tên"
        );
    }

    #[test]
    fn output_failure_toast_is_an_error_with_a_longer_stay() {
        let spec = output_failed_toast();
        assert_eq!(spec.kind, ToastKind::Danger);
        assert_eq!(spec.duration, Some(OUTPUT_TOAST));
        assert!(!spec.message.contains("player.toast"));
    }

    // ---- Test view: không phát tiếng (`FakeOutput`), không mở DB (thư viện `None`) ----

    struct TestPrefs(u8, bool);
    impl gpui::Global for TestPrefs {}

    fn test_prefs(cx: &App) -> (u8, bool) {
        cx.try_global::<TestPrefs>().map_or((70, false), |p| (p.0, p.1))
    }

    struct Rig {
        dir: TempDir,
        fake: FakeOutput,
        player: Entity<Player>,
    }

    fn rig(cx: &mut TestAppContext) -> Rig {
        // Engine chạy luồng thật, đánh thức foreground task từ luồng khác.
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
            cx.set_global(TestPrefs(70, false));
            player::install_for_test(deps, cx)
        });
        Rig { dir: TempDir::new().unwrap(), fake, player }
    }

    impl Rig {
        fn until(&self, cx: &mut VisualTestContext, what: &str, condition: impl Fn(&Player) -> bool) {
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

    fn open_bar(cx: &mut TestAppContext) -> (Entity<PlayerBar>, &mut VisualTestContext) {
        let window = cx.add_window(|window, cx| {
            let bar = cx.new(|cx| PlayerBar::with_prefs(None, test_prefs, window, cx));
            Root::new(bar, window, cx)
        });
        let bar = window.update(cx, |root, _, _| root.view().clone().downcast::<PlayerBar>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(1000.), px(90.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (bar, cx)
    }

    /// Không có bài: thanh mờ, không bấm được; vẽ được dù chưa có thư viện, bìa, cài đặt.
    #[gpui::test]
    fn bar_without_a_song_is_inert(cx: &mut TestAppContext) {
        let _rig = rig(cx);
        let (bar, cx) = open_bar(cx);
        assert!(!bar.read_with(cx, |bar, cx| bar.is_enabled(cx)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// N5: `TrackFailed` và `OutputFailed` thành toast, mỗi sự kiện một cái.
    #[gpui::test]
    fn failure_events_become_toasts(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (_bar, cx) = open_bar(cx);
        assert_eq!(cx.update(|window, cx| Toast::count(window, cx)), 0);

        let track = groove_core::model::TrackRef::External("x.wav".into());
        rig.player.update(cx, |_, cx| {
            cx.emit(PlayerEvent::TrackFailed {
                track,
                title: Some("Bài A".into()),
                kind: AudioErrorKind::NotFound,
                gave_up: false,
            })
        });
        cx.run_until_parked();
        assert_eq!(cx.update(|window, cx| Toast::count(window, cx)), 1);

        rig.player.update(cx, |_, cx| cx.emit(PlayerEvent::OutputFailed));
        cx.run_until_parked();
        assert_eq!(cx.update(|window, cx| Toast::count(window, cx)), 2);
    }

    /// Có bài: thanh sáng, nút Play đổi theo trạng thái phát.
    #[gpui::test]
    fn bar_follows_the_current_song(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (bar, cx) = open_bar(cx);
        let path = fixtures::write_sample(rig.dir.path(), "a", AudioKind::Wav).unwrap();
        rig.player.update(cx, |p, cx| p.play_paths(vec![path], cx));
        rig.until(cx, "song loaded", |p| p.current_song().is_some() && p.duration_sec() > 0.);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(bar.read_with(cx, |bar, cx| bar.is_enabled(cx)));
    }

    /// Bốn nút điều khiển chạy thật: Next, Prev đổi bài, Shuffle, Repeat đổi chế độ và tắt lẫn nhau (bảng 1.4).
    #[gpui::test]
    fn transport_buttons_drive_the_player(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (_bar, cx) = open_bar(cx);
        let first = fixtures::write_sample(rig.dir.path(), "a", AudioKind::Wav).unwrap();
        let second = fixtures::write_sample(rig.dir.path(), "b", AudioKind::Wav).unwrap();
        rig.player.update(cx, |p, cx| p.play_paths(vec![first.clone(), second.clone()], cx));
        rig.until(cx, "song loaded", |p| p.current_song().is_some() && p.duration_sec() > 0.);
        rig.player.update(cx, |p, cx| p.pause(cx));
        let click = |cx: &mut VisualTestContext, id: &'static str| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            });
            let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("không thấy {id}")).center();
            cx.simulate_mouse_move(at, None, Modifiers::none());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.simulate_click(at, Modifiers::none());
        };
        let is = |path: &std::path::Path| {
            let track = groove_core::model::TrackRef::External(path.to_owned());
            move |p: &Player| p.current_track() == Some(&track) && p.current_song().is_some()
        };

        click(cx, "player-next");
        rig.until(cx, "next", is(&second));
        click(cx, "player-prev");
        rig.until(cx, "prev", is(&first));

        click(cx, "player-shuffle");
        assert!(rig.player.read_with(cx, |p, _| p.queue().shuffle()));
        click(cx, "player-repeat");
        rig.player.read_with(cx, |p, _| {
            assert_eq!(p.queue().repeat(), RepeatMode::One);
            assert!(!p.queue().shuffle(), "bật repeat tắt shuffle");
        });
    }

    /// S20: kéo trên thanh chỉ đổi xem trước, `Player::seek` đúng một lần khi thả chuột.
    #[gpui::test]
    fn dragging_seeks_once_on_release(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (bar, cx) = open_bar(cx);
        let path = fixtures::write_sample(rig.dir.path(), "a", AudioKind::Wav).unwrap();
        rig.player.update(cx, |p, cx| p.play_paths(vec![path], cx));
        rig.until(cx, "song loaded", |p| p.current_song().is_some() && p.duration_sec() > 0.);
        rig.player.update(cx, |p, cx| p.pause(cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let duration = rig.player.read_with(cx, |p, _| p.duration_sec());
        let seek = bar.read_with(cx, |bar, _| bar.seek.clone());

        // Hàng seek nằm dưới hàng nút; tìm toạ độ y bằng cách thử vài chỗ rồi lấy chỗ nhận chuột.
        let none = Modifiers::none();
        // Cửa sổ 1000 px: khối giữa 316..684, thanh nằm giữa hai nhãn giờ, khoảng 372..628.
        let (x_press, x_drag, x_release) = (450., 520., 600.);
        let mut hit_y = None;
        for y in (54..80).step_by(2) {
            cx.simulate_mouse_move(point(px(x_press), px(y as f32)), None, none);
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.simulate_mouse_down(point(px(x_press), px(y as f32)), MouseButton::Left, none);
            if seek.read_with(cx, |s, _| s.is_dragging()) {
                hit_y = Some(y as f32);
                break;
            }
        }
        let y = hit_y.expect("thanh tua nhận chuột");
        let before = rig.player.read_with(cx, |p, _| p.position_sec());

        cx.simulate_mouse_move(point(px(x_drag), px(y)), Some(MouseButton::Left), none);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let during = rig.player.read_with(cx, |p, _| p.position_sec());
        assert!((during - before).abs() < 1e-6, "đang kéo thì chưa tua: {before} -> {during}");
        assert!(seek.read_with(cx, |s, _| s.is_dragging()));

        cx.simulate_mouse_up(point(px(x_release), px(y)), MouseButton::Left, none);
        assert!(!seek.read_with(cx, |s, _| s.is_dragging()), "thả chuột kết thúc kéo");
        rig.until(cx, "seeked", |p| p.position_sec() > duration * 0.5);
    }
}
