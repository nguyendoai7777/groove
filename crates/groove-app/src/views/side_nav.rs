//! groove-gd1_03: cột điều hướng trái (đặc tả 1.1).
//!
//! Rộng 230 px, thu còn 56 px; đầu cột cao 36 px kéo được cửa sổ, có tên app + nút tròn thu / mở; ba mục cao 42 px, vạch trái
//! 3 px của mục đang chọn trượt lên xuống khi đổi mục (`materiant::tabs::InkBar`, cùng nhịp với tab dọc); Settings ở đáy.
//! Nền là tầng alpha của cửa sổ mờ (`translucent_surface`; OS không làm mờ được thì alpha 0,94, `platform::backdrop`), phần nội
//! dung bên phải đục. Bề rộng nội suy 300 ms, OS bật giảm
//! chuyển động thì nhảy (`transition` của gpui-base tự xử lý).
//!
//! chrome-nav (02/11/2026): nút thu là nút tròn 28 px icon khung có vách (khác hẳn nút lùi `<` của thanh trên — trước đây hai
//! nút `<` đứng sát nhau trông như trùng); thu lại thì nút đứng giữa đầu cột, vẫn thấy và bấm được. Mỗi hàng vẫn là một nút
//! riêng: Tab đi qua từng hàng, Enter / Space chọn, vòng focus khi đi bằng phím.

use std::rc::Rc;
use std::time::Duration;

use groove_core::t;
use materiant::button::IconButton;
use materiant::effect::ripple::{RippleConfig, RippleExt};
use materiant::gpui_kit::base::Button;
use materiant::gpui_kit::base::motion::{Transition, transition};
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px, svg, transparent_black,
};
use materiant::overlay::tooltip::TooltipExt;
use materiant::style::ActiveTokens;
use materiant::tabs::InkBar;
use materiant::theme::color::hex;
use materiant::theme::{ActiveTheme, Neutral};

use crate::icons::GrooveIcon;
use crate::platform::backdrop;
use crate::route::Route;

pub const EXPANDED_WIDTH: f32 = 230.;
pub const COLLAPSED_WIDTH: f32 = 56.;
pub const HEADER_HEIGHT: f32 = 36.;
pub const ITEM_HEIGHT: f32 = 42.;
const ACTIVE_BAR: f32 = 3.;
/// Nút thu / mở: tròn 28 px, icon 18 px; SVG vẽ trên lưới 18 đơn vị, nét 1 px canh nửa điểm ảnh cho sắc, mảnh (chủ dự án).
const TOGGLE_SIZE: f32 = 28.;
const ICON_SIZE: f32 = 18.;
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

/// Bề rộng cột trái đang nội suy (300 ms) về đích theo `collapsed`. Shell tính để đặt cỡ cho phần cột trái `cached` (cpu):
/// view `cached` bố cục theo cỡ chủ đưa, không đo nội dung.
pub fn animated_width(collapsed: bool, window: &mut Window, cx: &mut App) -> f32 {
    let target = if collapsed { COLLAPSED_WIDTH } else { EXPANDED_WIDTH };
    transition("side-nav-width", target, Transition::new(WIDTH_TRANSITION), window, cx)
}

#[derive(IntoElement)]
pub struct SideNav {
    /// Bề rộng chủ đã tính ([`animated_width`]); `None` thì tự nội suy.
    width: Option<f32>,
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
            width: None,
            collapsed,
            active,
            on_select: Rc::new(on_select),
            on_toggle: Rc::new(move |(), window, cx| on_toggle(window, cx)),
            on_settings: Rc::new(move |(), window, cx| on_settings(window, cx)),
        }
    }
}

impl SideNav {
    /// Bề rộng do chủ nội suy ([`animated_width`]).
    pub fn width(mut self, width: f32) -> SideNav {
        self.width = Some(width);
        self
    }
}

impl RenderOnce for SideNav {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let width = match self.width {
            Some(width) => width,
            None => animated_width(self.collapsed, window, cx),
        };
        let wide = width > LABEL_THRESHOLD;
        let c = cx.theme().colors().clone();

        let toggle_label = if self.collapsed { t!("shell.side_nav.expand") } else { t!("shell.side_nav.collapse") };
        let nav_ink = NavInk::resolve(false, cx);
        let toggle = {
            let on_toggle = self.on_toggle.clone();
            div().flex_none().occlude().child(
                IconButton::new(
                    "side-nav-toggle",
                    if self.collapsed { GrooveIcon::SidebarExpand } else { GrooveIcon::SidebarCollapse },
                )
                .base()
                // Cùng màu icon mục nav chưa chọn (xám `muted`, rê sáng lên) — trắng đặc trông đậm hơn icon bên dưới.
                .ink(nav_ink.icon)
                .hover_ink(nav_ink.hover)
                .accessibility_label(toggle_label)
                .on_click(move |_: &ClickEvent, window, cx| on_toggle((), window, cx))
                .rounded_full()
                .w(px(TOGGLE_SIZE))
                .h(px(TOGGLE_SIZE))
                .icon_size(px(ICON_SIZE)),
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
                d.justify_between().pl_3().pr(px(6.)).gap_2().child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(c.text)
                        .child(t!("common.app_name")),
                )
            })
            .when(!wide, |d| d.justify_center())
            // Vùng kéo cửa sổ của header nằm ở `Shell` (dải sau cột trái): cột trái nhúng `cached`, vùng điều khiển cửa sổ
            // của view `cached` mất khi GPUI dùng lại khung trước.
            .child(toggle);

        let items: Vec<AnyElement> = Route::NAV
            .iter()
            .enumerate()
            .map(|(ix, &route)| nav_item(ix, route, route == self.active, wide, self.on_select.clone(), window, cx))
            .collect();
        // Vạch của mục đang chọn: một phần tử trượt giữa các hàng (hàng cao đều nên khỏi đo).
        let ink = Route::NAV.iter().position(|&route| route == self.active).map(|ix| {
            InkBar::new("side-nav-ink", ix as f32 * ITEM_HEIGHT, ITEM_HEIGHT)
                .vertical()
                .thickness(ACTIVE_BAR)
                .color(c.primary)
                .build(window, cx)
                .debug_selector(|| "side-nav-ink".into())
        });

        let settings = nav_button(
            "side-nav-settings",
            (GrooveIcon::Settings, t!("shell.nav.settings")),
            wide,
            false,
            self.on_settings.clone(),
            window,
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
            .bg(backdrop::side_nav_surface(cx))
            .child(header)
            .child(div().relative().flex_1().flex().flex_col().overflow_hidden().children(items).children(ink))
            .child(div().flex_none().border_t_1().border_color(c.border.opacity(0.3)).child(settings))
    }
}

/// Màu một hàng theo bản web (`side-nav.vue`): đang chọn = icon accent-light + chữ trắng (độ đậm normal, G-6); còn lại =
/// muted, hover sáng lên text-secondary, nền và vạch trái nhạt.
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
        let accent = c.primary;
        let secondary: Hsla = if dark { hex(0xd4d4d8).into() } else { c.text };
        NavInk {
            icon: if active { accent } else { c.muted },
            label: if active { c.text } else { c.muted },
            hover: secondary,
            // Mục đang chọn không tô viền: vạch trượt [`InkBar`] đứng đè đúng chỗ đó.
            bar: transparent_black(),
            hover_bar: c.border.opacity(0.7),
            active_bg: c.layer(Neutral::N200),
            hover_bg: c.layer(Neutral::N100),
        }
    }
}

/// Một hàng cao 42 px toàn bề ngang, vạch trái 3 px: icon + chữ khi rộng, chỉ icon + tooltip khi thu.
fn nav_button(
    id: &'static str,
    (icon, label): (GrooveIcon, &'static str),
    wide: bool,
    active: bool,
    on_click: Handler<()>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ink = NavInk::resolve(active, cx);
    let focus = window.use_keyed_state(ElementId::from((id, 1u32)), cx, |_, cx| cx.focus_handle()).read(cx).clone();
    let ring = materiant::style::focus_visible(&focus, window, cx);
    let group = SharedString::from(format!("{id}-group"));
    let glyph = svg()
        .path(icon.path())
        .flex_none()
        .size(px(ICON_SIZE))
        .text_color(ink.icon)
        .when(!active, |s| s.group_hover(group.clone(), move |s| s.text_color(ink.hover)));
    let row = Button::new(id)
        .track_focus(&focus)
        .selected(active)
        .accessibility_label(label)
        .relative()
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
        // Ripple như mọi mục bấm được (hàng không bo góc, lớp phủ kín hàng).
        .child(div().id(ElementId::from((id, 2u32))).absolute().inset_0().ripple(RippleConfig::default()))
        .child(glyph)
        .when(ring, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(2.))
                    .bottom(px(2.))
                    .left(px(2.))
                    .right(px(2.))
                    .border_2()
                    .rounded(px(4.))
                    .border_color(cx.focus_ring_color()),
            )
        })
        .when(wide, |d| {
            d.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(ink.label)
                    .when(!active, |t| t.group_hover(group.clone(), move |s| s.text_color(ink.hover)))
                    .child(label),
            )
        });
    if wide { row.into_any_element() } else { row.with_tooltip((id, 0usize), label).into_any_element() }
}

fn nav_item(
    ix: usize,
    route: Route,
    active: bool,
    wide: bool,
    on_select: Handler<Route>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id: &'static str = ["side-nav-0", "side-nav-1", "side-nav-2"][ix];
    nav_button(
        id,
        (nav_icon(route), route.title()),
        wide,
        active,
        Rc::new(move |(), window, cx| on_select(route, window, cx)),
        window,
        cx,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use ::gpui::{Context, Render, TestAppContext, VisualTestContext};
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::{KeyDownEvent, KeyUpEvent, Keystroke, point, size};

    use super::*;

    struct Probe {
        picked: Rc<RefCell<Vec<Route>>>,
        settings: Rc<RefCell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let (picked, settings) = (self.picked.clone(), self.settings.clone());
            SideNav::new(
                false,
                Route::MyMusic,
                move |route, _, _| picked.borrow_mut().push(route),
                |_, _| {},
                move |_, _| *settings.borrow_mut() += 1,
            )
        }
    }

    type Opened<'a> = (Rc<RefCell<Vec<Route>>>, Rc<RefCell<usize>>, &'a mut VisualTestContext);

    fn open(cx: &mut TestAppContext) -> Opened<'_> {
        groove_core::i18n::set_language(groove_core::i18n::Language::En);
        cx.update(materiant::init);
        let picked = Rc::new(RefCell::new(Vec::new()));
        let settings = Rc::new(RefCell::new(0));
        let (p, s) = (picked.clone(), settings.clone());
        let window = cx.add_window(move |window, cx| {
            let probe = cx.new(|_| Probe { picked: p, settings: s });
            Root::new(probe, window, cx)
        });
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (picked, settings, cx)
    }

    fn press(cx: &mut VisualTestContext, keys: &str) {
        let keystroke = cx.update(|_, cx| Keystroke::parse_with_os(keys, cx.platform_info().os).unwrap());
        cx.simulate_event(KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Cửa sổ chưa có gì focus thì Tab của `Root` không chạy: bấm chuột vào My Music trước (như người dùng).
    fn click_my_music(cx: &mut VisualTestContext) {
        cx.simulate_click(point(px(100.), px(HEADER_HEIGHT + ITEM_HEIGHT / 2.)), Default::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Tab dẫn qua ba mục rồi Settings; Enter và Space chọn mục đang focus.
    #[::gpui::test]
    fn tab_reaches_every_row_and_enter_space_activate(cx: &mut TestAppContext) {
        let (picked, settings, cx) = open(cx);
        click_my_music(cx);
        assert_eq!(*picked.borrow(), vec![Route::MyMusic]);
        press(cx, "tab");
        press(cx, "enter");
        press(cx, "tab");
        press(cx, "space");
        assert_eq!(*picked.borrow(), vec![Route::MyMusic, Route::NowPlaying, Route::Playlists]);
        press(cx, "tab");
        press(cx, "enter");
        assert_eq!(*settings.borrow(), 1, "mục Settings cũng vào được bằng bàn phím");
        press(cx, "shift-tab");
        press(cx, "enter");
        assert_eq!(picked.borrow().last(), Some(&Route::Playlists), "Shift+Tab quay lại mục trước");
    }

    struct Moving {
        active: Route,
    }

    impl Render for Moving {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            SideNav::new(false, self.active, |_, _, _| {}, |_, _| {}, |_, _| {})
        }
    }

    /// Đổi mục thì vạch trái trượt từ hàng cũ qua giữa rồi dừng đúng hàng mới; lần vẽ đầu đứng ngay ở hàng đang chọn.
    #[::gpui::test]
    fn ink_slides_to_the_new_row(cx: &mut TestAppContext) {
        groove_core::i18n::set_language(groove_core::i18n::Language::En);
        cx.update(materiant::init);
        let view = cx.new(|_| Moving { active: Route::NowPlaying });
        let window = cx.add_window({
            let view = view.clone();
            move |window, cx| Root::new(view, window, cx)
        });
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(400.), px(400.)));
        let frame = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
        };
        frame(cx);
        let ink = |cx: &mut VisualTestContext| cx.debug_bounds("side-nav-ink").expect("vạch đã vẽ");
        let first = ink(cx);
        assert_eq!(first.origin.y, px(HEADER_HEIGHT + ITEM_HEIGHT));
        assert_eq!((first.size.width, first.size.height), (px(ACTIVE_BAR), px(ITEM_HEIGHT)));

        view.update(cx, |v, cx| {
            v.active = Route::Playlists;
            cx.notify();
        });
        frame(cx);
        cx.executor().advance_clock(Duration::from_millis(120));
        frame(cx);
        let mid = ink(cx).origin.y;
        assert!(
            mid > px(HEADER_HEIGHT + ITEM_HEIGHT) && mid < px(HEADER_HEIGHT + 2. * ITEM_HEIGHT),
            "đang trượt: {mid:?}"
        );
        cx.executor().advance_clock(Duration::from_millis(400));
        frame(cx);
        frame(cx);
        assert_eq!(ink(cx).origin.y, px(HEADER_HEIGHT + 2. * ITEM_HEIGHT));
    }

    /// Đi bằng bàn phím thì hàng đang focus có vòng focus.
    #[::gpui::test]
    fn keyboard_focus_is_visible(cx: &mut TestAppContext) {
        let (_, _, cx) = open(cx);
        click_my_music(cx);
        press(cx, "tab");
        assert!(cx.update(|window, cx| window.focused(cx).is_some()), "có phần tử đang giữ focus");
        assert!(cx.update(|window, _| window.last_input_was_keyboard()), "vòng focus chỉ hiện khi đi bằng phím");
    }
}
