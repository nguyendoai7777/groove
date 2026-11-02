//! groove-gd1_14: bốn thanh nhảy báo "đang phát" (đặc tả bảng "Hiệu ứng web không có ở GPUI", `playing-visualizer.vue`).
//!
//! Dựng lại đúng bản web: khung 14 x 14, bốn thanh rộng 2 (x = 1.5 / 4.5 / 7.5 / 10.5) cao 10 (y = 2..12), bo tròn hoàn
//! toàn, màu nhấn của theme (bản web: `--deco-visualizer-color` cyan, đổi theo G-4). Mỗi thanh co giãn theo `scaleY` quanh **đáy khung nhìn**
//! (`transform-origin: bottom` của SVG tính theo viewBox), nhảy qua lại (`alternate`, `ease-in-out`) với nhịp riêng:
//!
//! | thanh | nửa nhịp | trễ | scaleY |
//! | --- | --- | --- | --- |
//! | 1 | 300 ms | 0 | 0.25 → 0.8 |
//! | 2 | 380 ms | 75 ms | 0.15 → 1 |
//! | 3 | 280 ms | 25 ms | 0.3 → 0.7 |
//! | 4 | 340 ms | 125 ms | 0.2 → 0.9 |
//!
//! Thời điểm trên nhịp lấy từ [`Phase`] (toàn app, một bản, vòng [`LOOP`] = bội chung nhỏ nhất của bốn chu kỳ, ~45 phút):
//! mỗi thanh tự chạy đúng nhịp của nó, vòng lặp nối liền. Chỉ chạy khi `active`: một `canvas` vẽ bốn thanh và xin khung kế
//! theo nhịp màn hình. Tạm dừng thì [`Phase`] ghi lại thời gian đã chạy và bốn thanh đứng đúng khung đó (như
//! `animation-play-state: paused` của web), không xin khung hình; phát lại thì chạy tiếp từ pha đã giữ. OS bật giảm chuyển động
//! thì đứng yên. View khác (danh sách bài, hàng đợi, hộp lịch sử, bảng lệnh) dùng lại.
//!
//! ui-audit (02/11/2026): bản trước dựng bốn `div` qua `with_animation(..).with_max_fps(30)`: layout GPUI làm tròn theo pixel
//! nên thanh 14 px nhảy từng pixel nguyên, lại chỉ 30 khung / s, trông giật. Giờ vẽ thẳng bằng `paint_quad` toạ độ lẻ, mỗi nhịp
//! màn hình một lần như trình duyệt; chỉ khi cửa sổ active (cửa sổ nền, thu nhỏ thì đứng yên, không tốn khung).

use std::time::{Duration, Instant};

use materiant::gpui_kit as gpui;
use materiant::gpui_kit::base::animation::cubic_bezier;
use materiant::gpui_kit::{
    App, Bounds, Global, Hsla, IntoElement, Pixels, Styled, Window, canvas, fill, point, px, size,
};
use materiant::theme::ActiveTheme;

pub const BARS: usize = 4;
/// Cạnh khung vuông của SVG gốc.
const VIEW: f32 = 14.;
/// Thanh i: (x, rộng, cao) trong khung 14 của SVG gốc, đáy thanh ở y = 12.
const BAR_X: [f32; BARS] = [1.5, 4.5, 7.5, 10.5];
const BAR_W: f32 = 2.;
const BAR_H: f32 = 10.;
const BAR_BOTTOM: f32 = 12.;
/// Nửa nhịp (ms), trễ (ms), scaleY đầu / cuối của từng thanh.
const HALF_MS: [f32; BARS] = [300., 380., 280., 340.];
const DELAY_MS: [f32; BARS] = [0., 75., 25., 125.];
const FROM: [f32; BARS] = [0.25, 0.15, 0.3, 0.2];
const TO: [f32; BARS] = [0.8, 1., 0.7, 0.9];
/// Bội chung nhỏ nhất của 600, 760, 560, 680 ms: hết vòng thì cả bốn thanh cùng về pha đầu.
pub const LOOP: Duration = Duration::from_millis(2_713_200);

/// `scaleY` của thanh `index` sau `elapsed_ms` kể từ lúc bắt đầu chạy (`alternate`, `ease-in-out`, có trễ).
pub fn bar_scale(index: usize, elapsed_ms: f32) -> f32 {
    let index = index.min(BARS - 1);
    let half = HALF_MS[index];
    let local = (elapsed_ms - DELAY_MS[index]).rem_euclid(half * 2.) / half;
    let leg = if local > 1. { 2. - local } else { local };
    let eased = cubic_bezier(0.42, 0., 0.58, 1.)(leg);
    FROM[index] + (TO[index] - FROM[index]) * eased
}

/// Thời gian bốn thanh đã chạy: cộng dồn lúc dừng, tính tiếp từ `since` lúc đang chạy.
#[derive(Default)]
pub struct Phase {
    elapsed: Duration,
    since: Option<Instant>,
}

impl Phase {
    /// Báo trạng thái vẽ hiện tại; chỉ đổi khi chuyển giữa chạy và dừng.
    pub fn set_active(&mut self, active: bool, now: Instant) {
        match (active, self.since) {
            (true, None) => self.since = Some(now),
            (false, Some(_)) => {
                self.elapsed = self.total(now);
                self.since = None;
            }
            _ => {}
        }
    }

    fn total(&self, now: Instant) -> Duration {
        let running = self.since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        Duration::from_nanos(((self.elapsed + running).as_nanos() % LOOP.as_nanos()) as u64)
    }

    /// Mili giây trên nhịp [`bar_scale`] ở thời điểm `now`.
    pub fn elapsed_ms(&self, now: Instant) -> f32 {
        self.total(now).as_secs_f32() * 1000.
    }
}

impl Global for Phase {}

#[derive(IntoElement)]
pub struct PlayingBars {
    active: bool,
    size: Pixels,
}

impl PlayingBars {
    pub fn new(active: bool) -> PlayingBars {
        PlayingBars { active, size: px(14.) }
    }

    /// Cạnh khung vuông (mặc định 14 px như bản web).
    pub fn size(mut self, size: Pixels) -> PlayingBars {
        self.size = size;
        self
    }
}

/// Bốn thanh trong khung `bounds` (vuông), co `scale` quanh đáy khung nhìn: đỉnh ở `14 - 12 s`, đáy ở `14 - 2 s` (đơn vị khung 14).
pub fn bar_bounds(bounds: Bounds<Pixels>, scale: [f32; BARS]) -> [Bounds<Pixels>; BARS] {
    let unit = bounds.size.width / VIEW;
    std::array::from_fn(|index| {
        let s = scale[index];
        let top = VIEW - (VIEW - (BAR_BOTTOM - BAR_H)) * s;
        Bounds::new(
            point(bounds.origin.x + unit * BAR_X[index], bounds.origin.y + unit * top),
            size(unit * BAR_W, unit * BAR_H * s),
        )
    })
}

impl gpui::RenderOnce for PlayingBars {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color: Hsla = cx.theme().colors().primary;
        let now = cx.background_executor().now();
        // Như chữ chạy: mỗi khung là vẽ lại cả cửa sổ, nên chỉ chạy khi cửa sổ active; cửa sổ nền đứng ở khung đang có.
        let running = self.active && !cx.reduce_motion() && window.is_window_active();
        cx.default_global::<Phase>().set_active(running, now);
        if running {
            // Xin ở lúc dựng (view đang dựng là view chứa thanh), như `with_animation`.
            window.request_animation_frame();
        }
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let elapsed = cx.global::<Phase>().elapsed_ms(cx.background_executor().now());
                let scale: [f32; BARS] = std::array::from_fn(|ix| bar_scale(ix, elapsed));
                let radius = bounds.size.width / VIEW * (BAR_W / 2.);
                for bar in bar_bounds(bounds, scale) {
                    window.paint_quad(fill(bar, color).corner_radii(radius));
                }
            },
        )
        .flex_none()
        .w(self.size)
        .h(self.size)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use ::gpui::{Context, Render, TestAppContext};

    use super::*;

    /// View gốc chỉ chứa [PlayingBars] và đếm số lần vẽ.
    struct Probe {
        active: bool,
        renders: Rc<Cell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            PlayingBars::new(self.active)
        }
    }

    fn renders_after_a_second(active: bool, cx: &mut TestAppContext) -> usize {
        let renders = Rc::new(Cell::new(0));
        let counter = renders.clone();
        cx.update(materiant::init);
        let window = cx.add_window(move |_, _| Probe { active, renders: counter });
        window.update(cx, |_, window, _| window.activate_window()).unwrap();
        cx.run_until_parked();
        let first = renders.get();
        assert!(first >= 1, "vẽ lần đầu");
        for _ in 0..10 {
            cx.executor().advance_clock(Duration::from_millis(100));
            // App ảo không có vòng khung của nền tảng: giao khung kế bằng tay.
            window.update(cx, |_, window, cx| window.simulate_next_frame(cx)).unwrap();
            cx.run_until_parked();
        }
        renders.get() - first
    }

    /// Đang phát: tự xin khung hình theo nhịp màn hình, không trần `with_max_fps`.
    #[::gpui::test]
    fn playing_redraws_every_frame(cx: &mut TestAppContext) {
        let extra = renders_after_a_second(true, cx);
        assert!(extra >= 10, "hoạt ảnh phải chạy: {extra} lần vẽ thêm trong 1 giây");
    }

    /// Cửa sổ nền (không active): đứng yên dù đang phát.
    #[::gpui::test]
    fn inactive_window_never_redraws(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let counter = renders.clone();
        cx.update(materiant::init);
        let window = cx.add_window(move |_, _| Probe { active: true, renders: counter });
        window.update(cx, |_, window, _| window.activate_window()).unwrap();
        cx.run_until_parked();
        ::gpui::VisualTestContext::from_window(window.into(), cx).deactivate_window();
        cx.run_until_parked();
        // Khung đã xin trước khi mất active vẫn tới một lần.
        window.update(cx, |_, window, cx| window.simulate_next_frame(cx)).unwrap();
        cx.run_until_parked();
        let first = renders.get();
        for _ in 0..10 {
            cx.executor().advance_clock(Duration::from_millis(100));
            // App ảo không có vòng khung của nền tảng: giao khung kế bằng tay.
            window.update(cx, |_, window, cx| window.simulate_next_frame(cx)).unwrap();
            cx.run_until_parked();
        }
        assert_eq!(renders.get(), first, "cửa sổ nền không xin khung");
    }

    /// Tạm dừng: đứng yên, không xin khung hình nào (CPU là tính năng).
    #[::gpui::test]
    fn paused_never_redraws(cx: &mut TestAppContext) {
        assert_eq!(renders_after_a_second(false, cx), 0);
    }

    #[test]
    fn phase_freezes_on_pause_and_resumes_from_there() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut phase = Phase::default();
        assert_eq!(phase.elapsed_ms(at(500)), 0., "chưa chạy thì ở pha đầu");
        phase.set_active(true, at(0));
        assert!((phase.elapsed_ms(at(250)) - 250.).abs() < 0.5);
        phase.set_active(false, at(250));
        let frozen: Vec<f32> = (0..BARS).map(|ix| bar_scale(ix, phase.elapsed_ms(at(250)))).collect();
        for later in [300, 5_000, 90_000] {
            let now: Vec<f32> = (0..BARS).map(|ix| bar_scale(ix, phase.elapsed_ms(at(later)))).collect();
            assert_eq!(now, frozen, "dừng thì giữ nguyên khung ({later} ms sau)");
        }
        phase.set_active(true, at(10_000));
        assert!((phase.elapsed_ms(at(10_100)) - 350.).abs() < 0.5, "phát lại chạy tiếp từ 250 ms");
        phase.set_active(true, at(10_200));
        assert!((phase.elapsed_ms(at(10_200)) - 450.).abs() < 0.5, "set_active(true) lặp không đặt lại pha");
    }

    #[test]
    fn phase_wraps_at_the_loop() {
        let t0 = Instant::now();
        let mut phase = Phase::default();
        phase.set_active(true, t0);
        let ms = phase.elapsed_ms(t0 + LOOP + Duration::from_millis(100));
        assert!((ms - 100.).abs() < 1., "{ms}");
    }

    /// Dừng rồi phát lại trong view thật: khung lúc dừng đúng khung đang chạy, phát lại chạy tiếp từ đó.
    #[::gpui::test]
    fn pausing_in_a_view_keeps_the_running_frame(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let counter = renders.clone();
        cx.update(materiant::init);
        let window = cx.add_window(move |_, _| Probe { active: true, renders: counter });
        window.update(cx, |_, window, _| window.activate_window()).unwrap();
        let ms = |cx: &mut TestAppContext| {
            let now = cx.executor().now();
            cx.read(|cx| cx.global::<Phase>().elapsed_ms(now))
        };
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        window
            .update(cx, |probe, _, cx| {
                probe.active = false;
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        let frozen = ms(cx);
        assert!(frozen >= 350., "đã chạy khoảng 400 ms trước khi dừng: {frozen}");
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        assert_eq!(ms(cx), frozen, "dừng 3 giây vẫn đứng ở khung cũ");
        window
            .update(cx, |probe, _, cx| {
                probe.active = true;
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        let resumed = ms(cx);
        assert!((resumed - frozen - 200.).abs() < 30., "chạy tiếp từ {frozen}, được {resumed}");
    }

    #[test]
    fn scales_stay_between_the_keyframes() {
        for ix in 0..BARS {
            for ms in (0..5_000).step_by(7) {
                let s = bar_scale(ix, ms as f32);
                assert!((FROM[ix] - 1e-4..=TO[ix] + 1e-4).contains(&s), "thanh {ix} tại {ms} ms: {s}");
            }
        }
    }

    #[test]
    fn keyframes_land_on_the_half_period() {
        // Bắt đầu sau trễ ở `from`, nửa nhịp sau ở `to`, một nhịp sau về `from`.
        for ix in 0..BARS {
            let (delay, half) = (DELAY_MS[ix], HALF_MS[ix]);
            assert!((bar_scale(ix, delay) - FROM[ix]).abs() < 1e-3);
            assert!((bar_scale(ix, delay + half) - TO[ix]).abs() < 1e-3);
            assert!((bar_scale(ix, delay + half * 2.) - FROM[ix]).abs() < 1e-3);
        }
    }

    #[test]
    fn bars_run_at_their_own_pace_and_the_loop_is_seamless() {
        let at = |ms: f32| -> Vec<f32> { (0..BARS).map(|ix| bar_scale(ix, ms)).collect() };
        let mid = at(137.);
        assert!(mid.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 0.01), "các thanh lệch pha");
        let total = LOOP.as_millis() as f32;
        for (ix, half) in HALF_MS.iter().enumerate() {
            let cycles = total / (half * 2.);
            assert!((cycles - cycles.round()).abs() < 1e-3, "thanh {ix}: vòng lặp phải chứa số nguyên nhịp ({cycles})");
        }
    }

    #[test]
    fn geometry_matches_the_svg_at_full_scale() {
        // scaleY = 1: đỉnh y = 2, đáy y = 12 như `<rect y="2" height="10">`.
        let frame = Bounds::new(point(px(0.), px(0.)), size(px(14.), px(14.)));
        let bars = bar_bounds(frame, [1.; BARS]);
        assert_eq!(bars[0].origin, point(px(1.5), px(2.)));
        assert_eq!(bars[3].size, size(px(2.), px(10.)));
        // Co quanh đáy khung nhìn, toạ độ lẻ không làm tròn: scaleY 0,25 ở khung 15 px.
        let frame = Bounds::new(point(px(0.), px(0.)), size(px(15.), px(15.)));
        let bar = bar_bounds(frame, [0.25; BARS])[0];
        let unit = 15. / VIEW;
        assert!((f32::from(bar.origin.y) - unit * (VIEW - 12. * 0.25)).abs() < 1e-4);
        assert!((f32::from(bar.size.height) - unit * BAR_H * 0.25).abs() < 1e-4);
    }
}
