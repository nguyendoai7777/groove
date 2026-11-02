//! groove-gd1_03: cột điều hướng trái (đặc tả 1.1).
//!
//! Rộng 230 px, thu còn 56 px; đầu cột cao 36 px kéo được cửa sổ, có tên app + nút thu; ba mục cao 42 px với vạch trái 3 px
//! khi active; Settings ở đáy. Nền là tầng alpha của cửa sổ mờ (`translucent_surface`), phần nội dung bên phải đục.
//! Bề rộng nội suy 300 ms, OS bật giảm chuyển động thì nhảy (`transition` của gpui-base tự xử lý).

use std::rc::Rc;
use std::time::Duration;

use groove_core::t;
use materiant::button::{Button, IconButton};
use materiant::gpui_kit::base::motion::{Transition, transition};
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, FontWeight, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, transparent_black,
};
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::window::{DragRegionExt, translucent_surface};

use crate::icons::GrooveIcon;
use crate::route::Route;

pub const EXPANDED_WIDTH: f32 = 230.;
pub const COLLAPSED_WIDTH: f32 = 56.;
pub const HEADER_HEIGHT: f32 = 36.;
pub const ITEM_HEIGHT: f32 = 42.;
const ACTIVE_BAR: f32 = 3.;
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
            nav_item(ix, route, active, wide, self.on_select.clone(), &c)
        });

        let settings = nav_button(
            "side-nav-settings",
            GrooveIcon::Settings,
            t!("shell.nav.settings"),
            wide,
            false,
            self.on_settings.clone(),
            &c,
        )
        .border_l(px(ACTIVE_BAR))
        .border_color(transparent_black());

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

/// Một nút cao 42 px toàn bề ngang: icon + chữ khi rộng, chỉ icon + tooltip khi thu.
fn nav_button(
    id: &'static str,
    icon: GrooveIcon,
    label: &'static str,
    wide: bool,
    active: bool,
    on_click: Handler<()>,
    c: &materiant::theme::ThemeColors,
) -> materiant::gpui_kit::Div {
    let ink = if active { c.primary_ink } else { c.muted };
    let click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| on_click((), window, cx);
    let button = if wide {
        Button::new(id, label)
            .icon(icon)
            .text()
            .color(ink)
            .on_click(click)
            .w_full()
            .h(px(ITEM_HEIGHT))
            .rounded_none()
            .justify_start()
            .px_4()
            .when(active, |b| b.font_weight(FontWeight::SEMIBOLD))
            .into_any_element()
    } else {
        IconButton::new(id, icon)
            .accessibility_label(label)
            .color(ink)
            .on_click(click)
            .w_full()
            .h(px(ITEM_HEIGHT))
            .rounded_none()
            .into_any_element()
    };
    let tipped = if wide { button } else { button.with_tooltip((id, 0usize), label).into_any_element() };
    div().w_full().child(tipped)
}

fn nav_item(
    ix: usize,
    route: Route,
    active: bool,
    wide: bool,
    on_select: Handler<Route>,
    c: &materiant::theme::ThemeColors,
) -> impl IntoElement {
    let id: &'static str = ["side-nav-0", "side-nav-1", "side-nav-2"][ix];
    let hover_bg = c.layer(Neutral::N100);
    nav_button(
        id,
        nav_icon(route),
        route.title(),
        wide,
        active,
        Rc::new(move |(), window, cx| on_select(route, window, cx)),
        c,
    )
    .border_l(px(ACTIVE_BAR))
    .border_color(if active { c.primary_ink } else { transparent_black() })
    .when(active, |d| d.bg(c.layer(Neutral::N200)))
    .when(!active, |d| d.hover(move |s| s.bg(hover_bg)))
}
