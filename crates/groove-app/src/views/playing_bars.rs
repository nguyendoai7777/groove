//! groove-gd1_14: bốn thanh nhảy báo "đang phát" (đặc tả bảng "Hiệu ứng web không có ở GPUI", `playing-visualizer.vue`).
//!
//! Dựng lại đúng bản web: khung 14 x 14, bốn thanh rộng 2 (x = 1.5 / 4.5 / 7.5 / 10.5) cao 10 (y = 2..12), bo tròn hoàn
//! toàn, màu `--deco-visualizer-color` (cyan-400 `#22d3ee`). Mỗi thanh co giãn theo `scaleY` quanh **đáy khung nhìn**
//! (`transform-origin: bottom` của SVG tính theo viewBox), nhảy qua lại (`alternate`, `ease-in-out`) với nhịp riêng:
//!
//! | thanh | nửa nhịp | trễ | scaleY |
//! | --- | --- | --- | --- |
//! | 1 | 300 ms | 0 | 0.25 → 0.8 |
//! | 2 | 380 ms | 75 ms | 0.15 → 1 |
//! | 3 | 280 ms | 25 ms | 0.3 → 0.7 |
//! | 4 | 340 ms | 125 ms | 0.2 → 0.9 |
//!
//! Bốn nhịp không chung bội nhỏ, nên một `with_animation` lặp [`LOOP`] (bội chung nhỏ nhất của bốn chu kỳ, ~45 phút) cho
//! mỗi thanh tự chạy đúng nhịp của nó mà vòng lặp vẫn nối liền. Chỉ chạy khi `active`: tạm dừng thì bốn thanh đứng ở
//! [`PAUSED_SCALE`] (web dừng đúng khung đang chạy, GPUI không giữ pha qua lần vẽ lại), không xin khung hình. OS bật giảm
//! chuyển động thì `with_animation` tự đứng yên. View khác (danh sách bài, hàng đợi, hộp lịch sử, bảng lệnh) dùng lại.

use std::time::Duration;

use materiant::gpui_kit as gpui;
use materiant::gpui_kit::base::animation::cubic_bezier;
use materiant::gpui_kit::{
    Animation, AnimationExt, App, Hsla, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div, px,
};
use materiant::theme::color::hex;

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
/// Hình dừng: giữa hai đầu mút của từng thanh.
pub const PAUSED_SCALE: [f32; BARS] = [0.525, 0.575, 0.5, 0.55];
/// 60 khung/s như trình duyệt thì thừa với bốn thanh 14 px; 30 vẫn mượt, nhẹ CPU.
const MAX_FPS: f32 = 30.;

/// `scaleY` của thanh `index` sau `elapsed_ms` kể từ lúc bắt đầu chạy (`alternate`, `ease-in-out`, có trễ).
pub fn bar_scale(index: usize, elapsed_ms: f32) -> f32 {
    let index = index.min(BARS - 1);
    let half = HALF_MS[index];
    let local = (elapsed_ms - DELAY_MS[index]).rem_euclid(half * 2.) / half;
    let leg = if local > 1. { 2. - local } else { local };
    let eased = cubic_bezier(0.42, 0., 0.58, 1.)(leg);
    FROM[index] + (TO[index] - FROM[index]) * eased
}

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

/// Khung 14 x 14 với bốn thanh co `scale` quanh đáy khung nhìn: đỉnh ở `14 - 12 s`, đáy ở `14 - 2 s`.
fn frame(size: Pixels, color: Hsla, scale: [f32; BARS]) -> gpui::Div {
    let unit = size / VIEW;
    let mut frame = div().relative().flex_none().w(size).h(size);
    for (index, s) in scale.into_iter().enumerate() {
        let top = VIEW - (VIEW - (BAR_BOTTOM - BAR_H)) * s;
        frame = frame.child(
            div()
                .absolute()
                .left(unit * BAR_X[index])
                .top(unit * top)
                .w(unit * BAR_W)
                .h(unit * BAR_H * s)
                .rounded(unit * (BAR_W / 2.))
                .bg(color),
        );
    }
    frame
}

impl gpui::RenderOnce for PlayingBars {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let color: Hsla = hex(0x22d3ee).into();
        let size = self.size;
        if self.active {
            frame(size, color, PAUSED_SCALE)
                .with_animation(
                    SharedString::from("playing-bars"),
                    Animation::new(LOOP).repeat().with_max_fps(MAX_FPS),
                    move |_, progress| {
                        let elapsed = progress * LOOP.as_millis() as f32;
                        frame(size, color, std::array::from_fn(|ix| bar_scale(ix, elapsed)))
                    },
                )
                .into_any_element()
        } else {
            frame(size, color, PAUSED_SCALE).into_any_element()
        }
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
        let _window = cx.add_window(move |_, _| Probe { active, renders: counter });
        cx.run_until_parked();
        let first = renders.get();
        assert!(first >= 1, "vẽ lần đầu");
        for _ in 0..10 {
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
        }
        renders.get() - first
    }

    /// Đang phát: hoạt ảnh tự xin khung hình (có nhịp, không vẽ lại ào ạt: <= MAX_FPS).
    #[::gpui::test]
    fn playing_redraws_at_a_capped_rate(cx: &mut TestAppContext) {
        let extra = renders_after_a_second(true, cx);
        assert!(extra >= 10, "hoạt ảnh phải chạy: {extra} lần vẽ thêm trong 1 giây");
        assert!(extra <= MAX_FPS as usize + 5, "không vượt trần khung hình: {extra}");
    }

    /// Tạm dừng: đứng yên, không xin khung hình nào (CPU là tính năng).
    #[::gpui::test]
    fn paused_never_redraws(cx: &mut TestAppContext) {
        assert_eq!(renders_after_a_second(false, cx), 0);
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
        let top = VIEW - (VIEW - (BAR_BOTTOM - BAR_H)) * 1.;
        assert_eq!(top, 2.);
        assert_eq!(top + BAR_H, BAR_BOTTOM);
    }
}
