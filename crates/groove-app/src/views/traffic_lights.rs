//! macos-captions (09/11/2026): ba đèn giao thông tự vẽ ở góc trái trên cửa sổ chính trên macOS (chủ dự án: thanh trên tự vẽ,
//! không dùng thanh tiêu đề / nút native). Chấm 12 px cách nhau 8 px như AppKit; rê vào cụm thì cả ba hiện ký hiệu (đóng,
//! thu nhỏ, phóng to / khôi phục); cửa sổ nền thì ba chấm xám. Bấm gọi [`window_controls`] như ba nút kiểu Windows.
//!
//! `Shell` vẽ cụm này phủ tuyệt đối sau các phần `cached` (cột trái, thanh trên), như vùng kéo cửa sổ: phần tử bấm được trong
//! view `cached` vẫn ăn, nhưng đặt ở `Shell` thì một chỗ cho cả lúc cột trái mở lẫn thu.

// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{App, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, black, div, px, svg};
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::ActiveTheme;
use materiant::theme::color::hex;

use crate::icons::GrooveIcon;
use crate::platform::window_controls;
use crate::views::app_bar::Caption;

/// Đường kính một chấm.
pub const DOT: f32 = 12.;
/// Khoảng giữa hai chấm.
pub const GAP: f32 = 8.;
/// Chấm đầu cách mép trái cửa sổ.
pub const LEFT: f32 = 14.;
/// Mép phải cụm đèn + khoảng thở: chữ / nút đầu tiên của hàng trên bắt đầu từ đây.
pub const END: f32 = LEFT + 3. * DOT + 2. * GAP + 12.;
const _: () = assert!(END >= LEFT + 3. * DOT + 2. * GAP + 8., "chữ sau cụm đèn không đè chấm thứ ba");
/// Ký hiệu trong chấm khi rê.
const GLYPH: f32 = 8.;
const GROUP: &str = "traffic-lights";

const CLOSE: u32 = 0xff5f57;
const MINIMIZE: u32 = 0xfebc2e;
const ZOOM: u32 = 0x28c840;

/// Màu nền của một chấm: cửa sổ active thì màu của AppKit, nền thì xám theo màu chữ.
pub fn fill(caption: Caption, active: bool, text: Hsla) -> Hsla {
    if !active {
        return text.opacity(0.22);
    }
    hex(match caption {
        Caption::Close => CLOSE,
        Caption::Minimize => MINIMIZE,
        Caption::Maximize => ZOOM,
    })
    .into()
}

#[derive(IntoElement)]
pub struct TrafficLights {
    /// Cao của hàng chứa (thanh trên / header cột trái): cụm đèn canh giữa theo chiều dọc trong đó.
    height: f32,
}

impl TrafficLights {
    pub fn new(height: f32) -> TrafficLights {
        TrafficLights { height }
    }
}

fn light(
    id: &'static str,
    caption: Caption,
    maximized: bool,
    label: &'static str,
    active: bool,
    cx: &App,
) -> impl IntoElement {
    let text = cx.theme().colors().text;
    let icon = match caption {
        Caption::Minimize => GrooveIcon::Minus,
        Caption::Close => GrooveIcon::Close,
        Caption::Maximize => caption.icon(maximized),
    };
    let glyph = black().opacity(0.6);
    div()
        .id(id)
        .flex_none()
        .size(px(DOT))
        .rounded_full()
        .bg(fill(caption, active, text))
        .border_1()
        .border_color(black().opacity(0.12))
        .flex()
        .items_center()
        .justify_center()
        .active(|s| s.opacity(0.75))
        .on_click(move |_, window, _| match caption {
            Caption::Minimize => window_controls::minimize(window),
            Caption::Maximize => window_controls::toggle_maximize(window),
            Caption::Close => window_controls::close(window),
        })
        .child(
            svg()
                .path(icon.path())
                .size(px(GLYPH))
                .flex_none()
                .text_color(glyph.opacity(0.))
                .group_hover(GROUP, move |s| s.text_color(glyph)),
        )
        .with_tooltip((id, 0usize), label)
}

impl RenderOnce for TrafficLights {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active = window.is_window_active();
        let maximized = window.is_maximized();
        let toggle_label = if maximized {
            groove_core::t!("shell.app_bar.restore")
        } else {
            groove_core::t!("shell.app_bar.maximize")
        };
        div()
            .id(GROUP)
            .group(GROUP)
            .occlude()
            .absolute()
            .top(px((self.height - DOT) / 2.))
            .left(px(LEFT))
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(light(
                "traffic-close",
                Caption::Close,
                maximized,
                groove_core::t!("shell.app_bar.close"),
                active,
                cx,
            ))
            .child(light(
                "traffic-minimize",
                Caption::Minimize,
                maximized,
                groove_core::t!("shell.app_bar.minimize"),
                active,
                cx,
            ))
            .child(light("traffic-zoom", Caption::Maximize, maximized, toggle_label, active, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_use_appkit_colors_and_grey_out_in_the_background() {
        let text: Hsla = hex(0xf5f5f5).into();
        assert_eq!(fill(Caption::Close, true, text), Hsla::from(hex(CLOSE)));
        assert_eq!(fill(Caption::Minimize, true, text), Hsla::from(hex(MINIMIZE)));
        assert_eq!(fill(Caption::Maximize, true, text), Hsla::from(hex(ZOOM)));
        for caption in [Caption::Close, Caption::Minimize, Caption::Maximize] {
            assert!(fill(caption, false, text).a < 0.3);
        }
    }
}
