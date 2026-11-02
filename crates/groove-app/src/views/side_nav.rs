//! groove-gd1_03: cột điều hướng trái (đặc tả 1.1).
//!
//! Rộng 230 px, thu còn 56 px; đầu cột cao 36 px kéo được cửa sổ, có tên app + nút thu; ba mục cao 42 px với vạch trái 3 px
//! khi active; Settings ở đáy. Nền là tầng alpha của cửa sổ mờ (`translucent_surface`), phần nội dung bên phải đục.
//! Bề rộng nội suy 300 ms, OS bật giảm chuyển động thì nhảy (`transition` của gpui-base tự xử lý).

use std::rc::Rc;
use std::time::Duration;

use groove_core::t;
use materiant::button::IconButton;
use materiant::gpui_kit::base::motion::{Transition, transition};
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, Rgba, SharedString, Styled,
    Window, div, px, svg, transparent_black,
};
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::color::hex;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::window::{DragRegionExt, translucent_surface};

use crate::icons::GrooveIcon;
use crate::route::Route;

pub const EXPANDED_WIDTH: f32 = 230.;
pub const COLLAPSED_WIDTH: f32 = 56.;
pub const HEADER_HEIGHT: f32 = 36.;
pub const ITEM_HEIGHT: f32 = 42.;
const ACTIVE_BAR: f32 = 3.;
const ICON_SIZE: f32 = 16.;
const WIDTH_TRANSITION: Duration = Duration::from_millis(300);
/// Quá ngưỡng này (đang nội suy) mới vẽ chữ; thấp hơn chỉ còn icon, tránh chữ bị cắt giữa chừng.
const LABEL_THRESHOLD: f32 = COLLAPSED_WIDTH + 64.;

type Handler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// Icon của từng mục điều hướng.
pub fn nav_icon(route: Route) -> GrooveIcon {
    match route.nav_section() {
        Route::NowPlaying => GrooveIcon::NowPlaying,
        Route::Playlists => GrooveIcon::Album,
        _ => GrooveIcon::Song,
    }
}

#[derive(IntoElement)]
pub struct SideNav {
    collapsed: bool,
    active: Route,
    on_select: Handler<Route>,
    on_toggle: Handler<()>,
    on_settings: Handler<()>,
}

impl SideNav {
    /// `active` là mục đang tô sáng (`Route::nav_section` của màn hiện tại).
    pub fn new(
        collapsed: bool,
        active: Route,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
        on_settings: impl Fn(&mut Window, &mut App) + 'static,
    ) -> SideNav {
        SideNav {
            collapsed,
            active,
            on_select: Rc::new(on_select),
            on_toggle: Rc::new(move |(), window, cx| on_toggle(window, cx)),
            on_settings: Rc::new(move |(), window, cx| on_settings(window, cx)),
        }
    }
}

impl RenderOnce for SideNav {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let target = if self.collapsed { COLLAPSED_WIDTH } else { EXPANDED_WIDTH };
        let width = transition("side-nav-width", target, Transition::new(WIDTH_TRANSITION), window, cx);
        let wide = width > LABEL_THRESHOLD;
        let c = cx.theme().colors().clone();

        let toggle_label = if self.collapsed { t!("shell.side_nav.expand") } else { t!("shell.side_nav.collapse") };
        let toggle = {
            let on_toggle = self.on_toggle.clone();
            div().occlude().child(
                IconButton::new(
                    "side-nav-toggle",
                    if self.collapsed { GrooveIcon::AngleRight } else { GrooveIcon::AngleLeft },
                )
                .accessibility_label(toggle_label)
                .on_click(move |_: &ClickEvent, window, cx| on_toggle((), window, cx))
                .rounded_none()
                .h(px(HEADER_HEIGHT)),
            )
        }
        .with_tooltip("side-nav-toggle-tip", toggle_label);

        let header = div()
            .id("side-nav-header")
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .overflow_hidden()
            .when(wide, |d| {
                d.justify_between().pl_3().child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text)
                        .child(t!("common.app_name")),
                )
            })
            .when(!wide, |d| d.justify_center())
            .drag_region()
            .child(toggle);

        let items = Route::NAV.iter().enumerate().map(|(ix, &route)| {
            let active = route == self.active;
            nav_item(ix, route, active, wide, self.on_select.clone(), cx)
        });

        let settings = nav_button(
            "side-nav-settings",
            GrooveIcon::Settings,
            t!("shell.nav.settings"),
            wide,
            false,
            self.on_settings.clone(),
            cx,
        );

        div()
            .id("side-nav")
            .w(px(width))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(translucent_surface(cx))
            .child(header)
            .child(div().flex_1().flex().flex_col().overflow_hidden().children(items))
            .child(div().flex_none().border_t_1().border_color(c.border.opacity(0.3)).child(settings))
    }
}

/// Màu một hàng theo bản web (`side-nav.vue`): đang chọn = icon accent-light + chữ trắng đậm; còn lại = muted, hover sáng
/// lên text-secondary, nền và vạch trái nhạt.
#[derive(Clone, Copy)]
struct NavInk {
    icon: Hsla,
    label: Hsla,
    hover: Hsla,
    bar: Hsla,
    hover_bar: Hsla,
    active_bg: Hsla,
    hover_bg: Hsla,
}

impl NavInk {
    fn resolve(active: bool, cx: &App) -> NavInk {
        let theme = cx.theme();
        let c = theme.colors();
        let dark = theme.is_dark();
        let accent: Hsla = if dark { Rgba::from(hex(0x22d3ee)).into() } else { c.primary };
        let secondary: Hsla = if dark { Rgba::from(hex(0xd4d4d8)).into() } else { c.text };
        NavInk {
            icon: if active { accent } else { c.muted },
            label: if active { c.text } else { c.muted },
            hover: secondary,
            bar: if active { accent } else { transparent_black() },
            hover_bar: c.border.opacity(0.7),
            active_bg: c.layer(Neutral::N200),
            hover_bg: c.layer(Neutral::N100),
        }
    }
}

/// Một hàng cao 42 px toàn bề ngang, vạch trái 3 px: icon + chữ khi rộng, chỉ icon + tooltip khi thu.
fn nav_button(
    id: &'static str,
    icon: GrooveIcon,
    label: &'static str,
    wide: bool,
    active: bool,
    on_click: Handler<()>,
    cx: &App,
) -> AnyElement {
    let ink = NavInk::resolve(active, cx);
    let group = SharedString::from(format!("{id}-group"));
    let glyph = svg()
        .path(icon.path())
        .flex_none()
        .size(px(ICON_SIZE))
        .text_color(ink.icon)
        .when(!active, |s| s.group_hover(group.clone(), move |s| s.text_color(ink.hover)));
    let row = div()
        .id(id)
        .group(group.clone())
        .w_full()
        .h(px(ITEM_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .cursor_pointer()
        .border_l(px(ACTIVE_BAR))
        .border_color(ink.bar)
        .when(wide, |d| d.justify_start().gap(px(12.)).pl(px(13.)).pr_4())
        .when(!wide, |d| d.justify_center())
        .when(active, |d| d.bg(ink.active_bg))
        .when(!active, |d| d.hover(move |s| s.bg(ink.hover_bg).border_color(ink.hover_bar)))
        .on_click(move |_: &ClickEvent, window, cx| on_click((), window, cx))
        .child(glyph)
        .when(wide, |d| {
            d.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(ink.label)
                    .when(active, |t| t.font_weight(FontWeight::SEMIBOLD))
                    .when(!active, |t| t.group_hover(group.clone(), move |s| s.text_color(ink.hover)))
                    .child(label),
            )
        });
    if wide { row.into_any_element() } else { row.with_tooltip((id, 0usize), label).into_any_element() }
}

fn nav_item(ix: usize, route: Route, active: bool, wide: bool, on_select: Handler<Route>, cx: &App) -> AnyElement {
    let id: &'static str = ["side-nav-0", "side-nav-1", "side-nav-2"][ix];
    nav_button(
        id,
        nav_icon(route),
        route.title(),
        wide,
        active,
        Rc::new(move |(), window, cx| on_select(route, window, cx)),
        cx,
    )
}
