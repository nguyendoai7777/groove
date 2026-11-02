//! groove-gd1_03: thanh trên cao 36 px (đặc tả 1.1).
//!
//! Nút lùi theo lịch sử (mờ khi không lùi được); ô tìm giữa rộng 40 % bấm để mở bảng lệnh, kèm chip `Ctrl` `K`; thu nhỏ,
//! phóng to / khôi phục, đóng (hover đỏ). Vùng trống kéo được cửa sổ; phần tử bấm được `occlude` để không kéo theo.

use std::rc::Rc;

use groove_core::t;
use materiant::button::IconButton;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{App, ClickEvent, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative};
use materiant::icon::Icon;
use materiant::kbd::Kbd;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::window::DragRegionExt;

use crate::icons::GrooveIcon;

pub const HEIGHT: f32 = 36.;
/// Ô tìm rộng 40 % thanh, canh giữa.
const SEARCH_WIDTH: f32 = 0.4;
const CONTROL_WIDTH: f32 = 46.;

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

/// Nút điều khiển cửa sổ: rộng 46 px, không bo, cao bằng thanh.
fn control(
    id: &'static str,
    icon: GrooveIcon,
    label: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    IconButton::new(id, icon)
        .accessibility_label(label)
        .on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
        .w(px(CONTROL_WIDTH))
        .h(px(HEIGHT))
        .rounded_none()
        .with_tooltip((id, 0usize), label)
}

impl RenderOnce for AppBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let mac = cfg!(target_os = "macos");

        let back = {
            let on_back = self.on_back.clone();
            div().occlude().child(
                IconButton::new("app-bar-back", GrooveIcon::AngleLeft)
                    .accessibility_label(t!("shell.app_bar.back"))
                    .disabled(!self.can_back)
                    .on_click(move |_: &ClickEvent, window, cx| on_back(window, cx))
                    .rounded_none()
                    .h(px(HEIGHT)),
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

        // Nền đỏ nằm ở lớp bọc: nút bên trong giữ màu trung tính, chỉ phủ thêm tầng hover của nó.
        let close = div()
            .occlude()
            .hover({
                let danger = c.danger;
                move |s| s.bg(danger)
            })
            .child(control("app-bar-close", GrooveIcon::Close, t!("shell.app_bar.close"), |window, _| {
                window.remove_window()
            }));

        let maximized = self.maximized;
        let (toggle_icon, toggle_label) = if maximized {
            (GrooveIcon::ArrowsCompress, t!("shell.app_bar.restore"))
        } else {
            (GrooveIcon::ArrowsExpand, t!("shell.app_bar.maximize"))
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
                    .child(control("app-bar-minimize", GrooveIcon::Minus, t!("shell.app_bar.minimize"), |window, _| {
                        window.minimize_window()
                    }))
                    .child(control("app-bar-maximize", toggle_icon, toggle_label, |window, _| window.zoom_window()))
                    .child(close),
            )
    }
}
