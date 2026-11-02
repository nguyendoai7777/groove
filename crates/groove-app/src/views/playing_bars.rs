//! groove-gd1_14: bốn thanh nhảy báo "đang phát" (đặc tả bảng "Hiệu ứng web không có ở GPUI", `playing-visualizer.vue`).
//!
//! Bốn `div` cùng đáy; chiều cao lấy từ [`bar_height`] theo tiến độ của một `with_animation` lặp [`CYCLE`], lệch pha từng
//! thanh. Chỉ chạy khi `active`: tạm dừng thì bốn thanh đứng ở [`REST`], không xin khung hình. OS bật giảm chuyển động thì
//! `with_animation` tự đứng yên. View khác (danh sách bài, task 16) dùng lại.

use std::f32::consts::TAU;
use std::time::Duration;

use materiant::gpui_kit as gpui;
use materiant::gpui_kit::{
    Animation, AnimationExt, App, Hsla, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, div, px,
};
use materiant::theme::ActiveTheme;

pub const BARS: usize = 4;
/// Một nhịp nhảy.
pub const CYCLE: Duration = Duration::from_millis(1200);
/// Chiều cao thấp nhất / khi đứng, tỉ lệ so với khung.
pub const REST: f32 = 0.3;
const MAX_FPS: f32 = 20.;
/// Mỗi thanh nhảy chậm / nhanh khác nhau (số chu kỳ trong một nhịp, nguyên để nối liền khi lặp), lệch pha.
const SPEED: [f32; BARS] = [1., 2., 3., 2.];
const PHASE: [f32; BARS] = [0., 0.35, 0.7, 0.15];

/// Chiều cao thanh `index` (`REST..=1`) ở tiến độ `progress` của một nhịp.
pub fn bar_height(index: usize, progress: f32) -> f32 {
    let index = index.min(BARS - 1);
    let wave = 0.5 + 0.5 * (TAU * (progress * SPEED[index] + PHASE[index])).sin();
    REST + (1. - REST) * wave
}

#[derive(IntoElement)]
pub struct PlayingBars {
    active: bool,
    size: Pixels,
    color: Option<Hsla>,
}

impl PlayingBars {
    pub fn new(active: bool) -> PlayingBars {
        PlayingBars { active, size: px(16.), color: None }
    }

    /// Cạnh khung vuông (mặc định 16 px).
    pub fn size(mut self, size: Pixels) -> PlayingBars {
        self.size = size;
        self
    }

    /// Màu thanh (mặc định màu nhấn của theme).
    pub fn color(mut self, color: Hsla) -> PlayingBars {
        self.color = Some(color);
        self
    }
}

fn row(size: Pixels, color: Hsla, heights: [f32; BARS]) -> gpui::Div {
    let bar_width = size * 0.18;
    let mut row = div().flex().items_end().justify_between().w(size).h(size).flex_none();
    for height in heights {
        row = row.child(div().w(bar_width).h(size * height).rounded_sm().bg(color));
    }
    row
}

impl RenderOnce for PlayingBars {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(cx.theme().colors().primary);
        let size = self.size;
        if self.active {
            row(size, color, [REST; BARS])
                .with_animation(
                    "playing-bars",
                    Animation::new(CYCLE).repeat().with_max_fps(MAX_FPS),
                    move |_, progress| row(size, color, std::array::from_fn(|ix| bar_height(ix, progress))),
                )
                .into_any_element()
        } else {
            row(size, color, [REST; BARS]).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_stay_within_the_frame() {
        for ix in 0..BARS {
            for step in 0..=100 {
                let h = bar_height(ix, step as f32 / 100.);
                assert!((REST - 1e-4..=1. + 1e-4).contains(&h), "thanh {ix} bước {step}: {h}");
            }
        }
    }

    #[test]
    fn bars_are_out_of_phase_and_loop() {
        let at = |p: f32| -> Vec<f32> { (0..BARS).map(|ix| bar_height(ix, p)).collect() };
        let mid = at(0.1);
        assert!(mid.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 0.01), "các thanh lệch pha");
        let (start, end) = (at(0.), at(1.));
        for ix in 0..BARS {
            assert!((start[ix] - end[ix]).abs() < 1e-4, "thanh {ix} nối liền khi lặp");
        }
    }
}
