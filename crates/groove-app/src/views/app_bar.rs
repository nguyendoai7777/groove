//! groove-gd1_03: thanh trên cao 36 px (đặc tả 1.1).
//!
//! Nút lùi theo lịch sử (mờ khi không lùi được); ô tìm giữa rộng 40 % bấm để mở bảng lệnh, kèm chip `Ctrl` `K`; thu nhỏ,
//! phóng to / khôi phục, đóng như bản web ([`caption`]: icon `Minus`, `ArrowsExpand` / `ArrowsCompress`, `Close` 20 px trong ô
//! 36 px; rê xám nhạt, Đóng rê đỏ `red-600` icon trắng). Vùng trống kéo được cửa sổ, nhấp đúp phóng to / khôi phục (OS lo
//! qua `HTCAPTION`): vùng kéo là khung bọc ở `Shell` nằm sau thanh (thanh nhúng `cached`, xem `app::Shell::render`);
//! phần tử bấm được `occlude` để không kéo theo, phần còn lại của thanh không chặn chuột.
//!
//! window-buttons (03/11/2026): nút cửa sổ là nút bấm thường gọi [`window_controls`], không còn vùng `WindowControlArea`
//! cho OS hit-test (bấm trên màn thật không ăn) — lý do ở đầu module đó.

use std::rc::Rc;

use groove_core::t;
use materiant::button::IconButton;
use materiant::effect::ripple::{RippleConfig, RippleExt};
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative, svg, white,
};
use materiant::icon::Icon;
use materiant::kbd::Kbd;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::color::hex;
use materiant::theme::{ActiveTheme, Neutral};

use crate::icons::GrooveIcon;
use crate::platform::window_controls;
use crate::views::{side_nav, traffic_lights};

pub const HEIGHT: f32 = 36.;
/// Ô tìm rộng 40 % thanh, canh giữa.
const SEARCH_WIDTH: f32 = 0.4;
/// Nút cửa sổ như `icon-btn` của bản web: ô 36 px (`w-9`), icon 20 px (`sm`).
/// macos-caption-inset: macOS bo góc cửa sổ lớn (cửa sổ `Resizable`) ⇒ nút nhỏ hơn (32 × 28, bo góc, icon 18 px) và cụm nút
/// lùi khỏi mép phải [`MAC_RIGHT_INSET`] để nút Đóng không chạm góc bo.
const CONTROL_WIDTH: f32 = if cfg!(target_os = "macos") { 32. } else { 36. };
const CONTROL_HEIGHT: f32 = if cfg!(target_os = "macos") { 28. } else { HEIGHT };
const MAC_RIGHT_INSET: f32 = 8.;
/// Nút Lùi tròn 32 px cách mép trái bằng khoảng nó cách mép trên / dưới thanh 36 px.
const BACK_INSET: f32 = 2.;
const CAPTION_ICON: f32 = if cfg!(target_os = "macos") { 18. } else { 20. };

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AppBar {
    can_back: bool,
    maximized: bool,
    on_back: Handler,
    on_search: Handler,
    /// macOS, cột trái đang thu: nút mở cột đứng đầu thanh, sau cụm đèn giao thông (`side_nav::toggle_button`).
    nav_toggle: Option<side_nav::Handler<()>>,
}

impl AppBar {
    pub fn new(
        can_back: bool,
        maximized: bool,
        on_back: impl Fn(&mut Window, &mut App) + 'static,
        on_search: impl Fn(&mut Window, &mut App) + 'static,
    ) -> AppBar {
        AppBar { can_back, maximized, on_back: Rc::new(on_back), on_search: Rc::new(on_search), nav_toggle: None }
    }
}

impl AppBar {
    /// Nút mở cột trái ở đầu thanh (macOS lúc cột thu: đầu cột hẹp đã có đèn giao thông).
    pub fn nav_toggle(mut self, on_toggle: impl Fn(&mut Window, &mut App) + 'static) -> AppBar {
        self.nav_toggle = Some(Rc::new(move |(), window, cx| on_toggle(window, cx)));
        self
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
    /// Icon như bản web (`app-bar.vue`): phóng to `ArrowsExpand`, đang phóng to thì `ArrowsCompress` (khôi phục).
    pub fn icon(self, maximized: bool) -> GrooveIcon {
        match self {
            Caption::Minimize => GrooveIcon::Minus,
            Caption::Maximize if maximized => GrooveIcon::ArrowsCompress,
            Caption::Maximize => GrooveIcon::ArrowsExpand,
            Caption::Close => GrooveIcon::Close,
        }
    }
}

/// Đỏ của nút Đóng khi rê chuột: `hover:bg-red-600` của bản web, icon trắng.
pub const CLOSE_HOVER: u32 = 0xdc2626;

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

/// Nút cửa sổ rộng 36 px, cao bằng thanh, icon 20 px (macOS 32 × 28 bo góc, icon 18 px), ripple từ tâm. Bấm gọi [`window_controls`] (phóng to bấm lần hai là khôi phục).
fn caption(id: &'static str, caption: Caption, icon: GrooveIcon, label: &'static str, cx: &App) -> impl IntoElement {
    let paint = CaptionPaint::resolve(caption, cx.theme().colors().text);
    div()
        .id(id)
        .group(id)
        .occlude()
        .flex_none()
        .w(px(CONTROL_WIDTH))
        .h(px(CONTROL_HEIGHT))
        .when(cfg!(target_os = "macos"), |d| d.rounded_md().overflow_hidden())
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(paint.hover_bg))
        .active(move |s| s.bg(paint.active_bg))
        // Ripple loang từ tâm như nút icon của materiant; màu theo icon lúc rê (Đóng: trắng trên nền đỏ).
        .ripple(RippleConfig::centered().color(paint.hover_icon))
        .on_click(move |_, window, _| match caption {
            Caption::Minimize => window_controls::minimize(window),
            Caption::Maximize => window_controls::toggle_maximize(window),
            Caption::Close => window_controls::close(window),
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

        // macos-captions: cụm đèn giao thông (`Shell` vẽ từ mép trái cửa sổ) tràn qua cột trái đang thu ⇒ chừa phần tràn.
        let nav_toggle = self.nav_toggle.clone().map(|on_toggle| {
            div()
                .flex()
                .items_center()
                .pl(px((traffic_lights::END - side_nav::COLLAPSED_WIDTH).max(0.)))
                .child(side_nav::toggle_button("app-bar-nav-toggle", true, on_toggle, cx))
        });

        let toggle_label = if self.maximized { t!("shell.app_bar.restore") } else { t!("shell.app_bar.maximize") };

        div()
            .id("app-bar")
            .relative()
            .h(px(HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .children(nav_toggle)
            .child(back)
            .child(search)
            // Ba nút cửa sổ ở góc phải có trên mọi nền (macOS có thêm đèn giao thông tự vẽ ở góc trái, chủ dự án giữ cả hai).
            .child(
                div()
                    .ml_auto()
                    .when(cfg!(target_os = "macos"), |d| d.mr(px(MAC_RIGHT_INSET)).gap(px(2.)))
                    .flex()
                    .items_center()
                    .h_full()
                    .occlude()
                    .child(caption(
                        "app-bar-minimize",
                        Caption::Minimize,
                        Caption::Minimize.icon(self.maximized),
                        t!("shell.app_bar.minimize"),
                        cx,
                    ))
                    .child(caption(
                        "app-bar-maximize",
                        Caption::Maximize,
                        Caption::Maximize.icon(self.maximized),
                        toggle_label,
                        cx,
                    ))
                    .child(caption(
                        "app-bar-close",
                        Caption::Close,
                        Caption::Close.icon(self.maximized),
                        t!("shell.app_bar.close"),
                        cx,
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Đóng rê đỏ `red-600` icon trắng như bản web; thu nhỏ / phóng to rê xám nhạt, giữ màu icon, nhấn nhạt hơn rê.
    #[test]
    fn caption_paint_matches_web() {
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
    }

    /// Icon như `app-bar.vue` của bản web: `Minus`, `ArrowsExpand` / `ArrowsCompress` theo trạng thái phóng to, `Close`.
    #[test]
    fn caption_icons_match_web() {
        for maximized in [false, true] {
            assert_eq!(Caption::Minimize.icon(maximized), GrooveIcon::Minus);
            assert_eq!(Caption::Close.icon(maximized), GrooveIcon::Close);
        }
        assert_eq!(Caption::Maximize.icon(false), GrooveIcon::ArrowsExpand);
        assert_eq!(Caption::Maximize.icon(true), GrooveIcon::ArrowsCompress);
    }
}
