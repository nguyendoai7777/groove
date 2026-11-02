//! groove-gd1_03: thanh trên cao 36 px (đặc tả 1.1).
//!
//! Nút lùi theo lịch sử (mờ khi không lùi được); ô tìm giữa rộng 40 % bấm để mở bảng lệnh, kèm chip `Ctrl` `K`; thu nhỏ,
//! phóng to / khôi phục, đóng kiểu nút tiêu đề Windows ([`caption`]: rê xám nhạt, Đóng rê đỏ `#c42b1c` icon trắng). Vùng
//! trống kéo được cửa sổ, nhấp đúp phóng to / khôi phục (OS lo qua `HTCAPTION`); phần tử bấm được `occlude` để không kéo
//! theo.
//!
//! chrome-nav (02/11/2026): thu nhỏ / phóng to từng không chạy vì gọi `minimize_window` / `zoom_window` từ `on_click` của
//! GPUI; `zoom_window` của gpui-pre-windows chỉ `SW_MAXIMIZE`, không bao giờ khôi phục. Nay nút là vùng điều khiển của OS.

use std::rc::Rc;

use groove_core::t;
use materiant::button::IconButton;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, WindowControlArea, div, px,
    relative, svg, white,
};
use materiant::icon::Icon;
use materiant::kbd::Kbd;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::color::hex;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::window::DragRegionExt;

use crate::icons::GrooveIcon;

pub const HEIGHT: f32 = 36.;
/// Ô tìm rộng 40 % thanh, canh giữa.
const SEARCH_WIDTH: f32 = 0.4;
const CONTROL_WIDTH: f32 = 46.;
/// Nút Lùi tròn 32 px cách mép trái bằng khoảng nó cách mép trên / dưới thanh 36 px.
const BACK_INSET: f32 = 2.;
/// Icon nút cửa sổ: 10 px nét 1 px như Segoe Fluent Icons.
const CAPTION_ICON: f32 = 10.;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AppBar {
    can_back: bool,
    maximized: bool,
    on_back: Handler,
    on_search: Handler,
}

impl AppBar {
    pub fn new(
        can_back: bool,
        maximized: bool,
        on_back: impl Fn(&mut Window, &mut App) + 'static,
        on_search: impl Fn(&mut Window, &mut App) + 'static,
    ) -> AppBar {
        AppBar { can_back, maximized, on_back: Rc::new(on_back), on_search: Rc::new(on_search) }
    }
}

/// Ba nút cửa sổ ở góc phải.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caption {
    Minimize,
    Maximize,
    Close,
}

impl Caption {
    /// Vùng điều khiển cửa sổ báo cho OS qua hit-test (Windows: `HTMINBUTTON`, `HTMAXBUTTON`, `HTCLOSE`).
    fn area(self) -> WindowControlArea {
        match self {
            Caption::Minimize => WindowControlArea::Min,
            Caption::Maximize => WindowControlArea::Max,
            Caption::Close => WindowControlArea::Close,
        }
    }
}

/// Đỏ của nút Đóng khi rê chuột (Windows 11 `#c42b1c`), chữ trắng.
pub const CLOSE_HOVER: u32 = 0xc42b1c;

/// Màu một nút cửa sổ như Windows: thu nhỏ / phóng to rê là lớp xám mờ theo màu chữ, nhấn nhạt hơn; Đóng rê nền đỏ, icon
/// trắng, nhấn đỏ hơi trong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptionPaint {
    pub icon: Hsla,
    pub hover_bg: Hsla,
    pub hover_icon: Hsla,
    pub active_bg: Hsla,
}

impl CaptionPaint {
    pub fn resolve(caption: Caption, text: Hsla) -> CaptionPaint {
        match caption {
            Caption::Close => {
                let red: Hsla = hex(CLOSE_HOVER).into();
                CaptionPaint { icon: text, hover_bg: red, hover_icon: white(), active_bg: red.opacity(0.9) }
            }
            _ => CaptionPaint {
                icon: text,
                hover_bg: text.opacity(0.09),
                hover_icon: text,
                active_bg: text.opacity(0.06),
            },
        }
    }
}

/// Nút cửa sổ rộng 46 px, cao bằng thanh, icon nét mảnh 10 px.
///
/// Windows: nút là vùng `WindowControlArea` — OS nhận nhấp như nút tiêu đề thật (`HTMINBUTTON`…): thu nhỏ, phóng to / khôi
/// phục (`SW_NORMAL` khi đang phóng to), đóng, Snap Layouts của Windows 11 khi rê nút phóng to. Không gắn `on_click`: GPUI
/// nhận nhấp trước, phần tử nào dừng sự kiện là OS bỏ qua. Nền khác: `on_click` gọi thẳng API cửa sổ của GPUI.
fn caption(id: &'static str, caption: Caption, icon: GrooveIcon, label: &'static str, cx: &App) -> impl IntoElement {
    let paint = CaptionPaint::resolve(caption, cx.theme().colors().text);
    div()
        .id(id)
        .group(id)
        .occlude()
        .flex_none()
        .w(px(CONTROL_WIDTH))
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(caption.area())
        .hover(move |s| s.bg(paint.hover_bg))
        .active(move |s| s.bg(paint.active_bg))
        .when(!cfg!(target_os = "windows"), |d| {
            d.on_click(move |_, window, _| match caption {
                Caption::Minimize => window.minimize_window(),
                Caption::Maximize => window.zoom_window(),
                Caption::Close => window.remove_window(),
            })
        })
        .child(
            svg()
                .path(icon.path())
                .size(px(CAPTION_ICON))
                .flex_none()
                .text_color(paint.icon)
                .group_hover(id, move |s| s.text_color(paint.hover_icon)),
        )
        .with_tooltip((id, 0usize), label)
}

impl RenderOnce for AppBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let mac = cfg!(target_os = "macos");

        let back = {
            let on_back = self.on_back.clone();
            // Nút icon tròn như mọi nút icon (ui-audit), không phải ô chữ nhật cao bằng thanh; nút cửa sổ bên phải mới theo
            // kiểu Windows.
            div().occlude().ml(px(BACK_INSET)).child(
                IconButton::new("app-bar-back", GrooveIcon::AngleLeft)
                    .accessibility_label(t!("shell.app_bar.back"))
                    .disabled(!self.can_back)
                    .on_click(move |_: &ClickEvent, window, cx| on_back(window, cx))
                    .rounded_full(),
            )
        };

        let search = {
            let on_search = self.on_search.clone();
            div()
                .id("app-bar-search")
                .occlude()
                .absolute()
                .top(px(5.))
                .h(px(HEIGHT - 10.))
                .left(relative((1. - SEARCH_WIDTH) / 2.))
                .w(relative(SEARCH_WIDTH))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .rounded_full()
                .border_1()
                .border_color(materiant::gpui_kit::transparent_black())
                .bg(c.layer(Neutral::N100))
                .cursor_pointer()
                .hover({
                    let (border, bg) = (c.input_hover, c.layer(Neutral::N200));
                    move |s| s.border_color(border).bg(bg)
                })
                .on_click(move |_, window, cx| on_search(window, cx))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(GrooveIcon::Search).size(px(14.)).color(c.muted))
                        .child(div().truncate().text_color(c.muted).child(t!("shell.app_bar.search_placeholder"))),
                )
                .child(Kbd::new("Ctrl+K").mac_symbols(mac))
        };

        let (toggle_icon, toggle_label) = if self.maximized {
            (GrooveIcon::WindowRestore, t!("shell.app_bar.restore"))
        } else {
            (GrooveIcon::WindowMaximize, t!("shell.app_bar.maximize"))
        };

        div()
            .id("app-bar")
            .relative()
            .h(px(HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .drag_region()
            .child(back)
            .child(search)
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .h_full()
                    .occlude()
                    .child(caption(
                        "app-bar-minimize",
                        Caption::Minimize,
                        GrooveIcon::WindowMinimize,
                        t!("shell.app_bar.minimize"),
                        cx,
                    ))
                    .child(caption("app-bar-maximize", Caption::Maximize, toggle_icon, toggle_label, cx))
                    .child(caption(
                        "app-bar-close",
                        Caption::Close,
                        GrooveIcon::WindowClose,
                        t!("shell.app_bar.close"),
                        cx,
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Như nút tiêu đề Windows: Đóng rê đỏ `#c42b1c` icon trắng; thu nhỏ / phóng to rê xám nhạt, giữ màu icon, nhấn nhạt hơn rê.
    #[test]
    fn caption_paint_matches_windows() {
        let text: Hsla = hex(0xf5f5f5).into();
        let close = CaptionPaint::resolve(Caption::Close, text);
        assert_eq!(close.hover_bg, Hsla::from(hex(CLOSE_HOVER)));
        assert_eq!(close.hover_icon, white());
        assert_eq!(close.icon, text);
        for caption in [Caption::Minimize, Caption::Maximize] {
            let p = CaptionPaint::resolve(caption, text);
            assert_eq!(p.hover_icon, text);
            assert!(p.hover_bg.a > 0. && p.hover_bg.a < 0.15, "xám nhạt: {:?}", p.hover_bg);
            assert!(p.active_bg.a < p.hover_bg.a);
        }
        assert_eq!(Caption::Minimize.area(), WindowControlArea::Min);
        assert_eq!(Caption::Maximize.area(), WindowControlArea::Max);
        assert_eq!(Caption::Close.area(), WindowControlArea::Close);
    }
}
