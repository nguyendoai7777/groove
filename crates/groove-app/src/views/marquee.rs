//! groove-gd1_14: tiêu đề chạy ngang khi tràn (đặc tả 1.3, bảng "Hiệu ứng web không có ở GPUI").
//!
//! Phần tử cắt `overflow_hidden`; chữ nhân đôi cách nhau [`GAP`] và lệch trái theo `with_animation` lặp [`CYCLE`] (12 s).
//! Chỉ chạy khi chữ rộng hơn khung và chuột không đè lên; ngược lại là một dòng cắt `…`, không xin khung hình nào. Đo chữ ở
//! `prepaint` của một `canvas` phủ khung (lúc đó kiểu chữ của cha đã áp), nên nhịp vẽ đầu tiên còn là dòng tĩnh. OS bật giảm
//! chuyển động thì `with_animation` tự đứng yên ở đầu dòng.
//!
//! Kiểu chữ (cỡ, đậm, màu) do phần tử cha đặt.

use std::time::Duration;

// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    Animation, AnimationExt, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    canvas, div, px,
};

/// Một vòng chạy hết (đặc tả: 12 s).
pub const CYCLE: Duration = Duration::from_secs(12);
/// Khoảng trống giữa hai bản chữ.
pub const GAP: f32 = 48.;
/// Số lần vẽ lại mỗi giây khi chạy: đủ mượt cho chữ trượt chậm, nhẹ hơn vsync.
const MAX_FPS: f32 = 30.;

/// Chữ rộng hơn khung thì chạy. Khung chưa đo (0) thì chưa chạy.
pub fn overflows(text_width: f32, box_width: f32) -> bool {
    box_width > 0. && text_width > box_width + 0.5
}

/// Độ lệch trái (px, âm) ở tiến độ `progress` của một vòng: cuối vòng đúng một bản chữ + khoảng trống, nối liền đầu vòng.
pub fn offset(progress: f32, text_width: f32) -> f32 {
    -progress.clamp(0., 1.) * (text_width + GAP)
}

#[derive(Default)]
struct State {
    box_width: f32,
    text_width: f32,
    hovered: bool,
}

#[derive(IntoElement)]
pub struct Marquee {
    id: ElementId,
    text: SharedString,
}

impl Marquee {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Marquee {
        Marquee { id: id.into(), text: text.into() }
    }
}

impl RenderOnce for Marquee {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let (box_width, text_width, hovered) = {
            let s = state.read(cx);
            (s.box_width, s.text_width, s.hovered)
        };
        let scrolling = overflows(text_width, box_width) && !hovered;

        let measure = {
            let (state, text) = (state.clone(), self.text.clone());
            canvas(
                move |bounds, window, cx| {
                    let style = window.text_style();
                    let size = style.font_size.to_pixels(window.rem_size());
                    let run = style.to_run(text.len());
                    let line = window.text_system().shape_line(text, size, &[run], None);
                    let (box_width, text_width) = (f32::from(bounds.size.width), f32::from(line.width));
                    let changed = {
                        let s = state.read(cx);
                        (s.box_width - box_width).abs() > 0.5 || (s.text_width - text_width).abs() > 0.5
                    };
                    if changed {
                        state.update(cx, |s, cx| {
                            s.box_width = box_width;
                            s.text_width = text_width;
                            cx.notify();
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
        };

        let content = if scrolling {
            div()
                .flex()
                .flex_none()
                .whitespace_nowrap()
                .child(self.text.clone())
                .child(div().w(px(GAP)).flex_none())
                .child(self.text.clone())
                .with_animation(
                    (self.id.clone(), "marquee-run"),
                    Animation::new(CYCLE).repeat().with_max_fps(MAX_FPS),
                    move |el, progress| el.ml(px(offset(progress, text_width))),
                )
                .into_any_element()
        } else {
            div().truncate().child(self.text.clone()).into_any_element()
        };

        div()
            .id(self.id)
            .relative()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .on_hover(move |hovered: &bool, _, cx| {
                state.update(cx, |s, cx| {
                    if s.hovered != *hovered {
                        s.hovered = *hovered;
                        cx.notify();
                    }
                });
            })
            .child(measure)
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_only_when_text_is_wider_than_the_box() {
        assert!(overflows(300., 200.));
        assert!(!overflows(150., 200.));
        assert!(!overflows(200.2, 200.), "lệch nửa px không tính là tràn");
        assert!(!overflows(300., 0.), "khung chưa đo");
    }

    #[test]
    fn offset_loops_seamlessly() {
        assert_eq!(offset(0., 300.), 0.);
        // Cuối vòng đúng một bản chữ cộng khoảng trống: bản thứ hai nằm chỗ bản thứ nhất lúc đầu.
        assert_eq!(offset(1., 300.), -(300. + GAP));
        assert_eq!(offset(2., 300.), offset(1., 300.), "kẹp ở cuối vòng");
        assert_eq!(offset(-1., 300.), 0.);
        assert!(offset(0.5, 300.) < 0.);
    }
}
