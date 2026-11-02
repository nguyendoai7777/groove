//! Vòng đời app. groove-gd1_01 chỉ mở cửa sổ trống; groove-gd1_03 dựng khung thật ở đây, các task sau nối vào đúng một
//! lời gọi khởi tạo trong `run`.

use groove_core::t;
use materiant::gpui_kit::{
    self, App, AppContext, Bounds, Context, IntoElement, Render, Styled, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, size,
};
use materiant::theme::ActiveTheme;

use crate::dev::{self, DevArgs};

/// Cỡ mặc định của cửa sổ (đặc tả 1.1).
const WINDOW_SIZE: (f32, f32) = (800., 600.);

pub fn run(dev: DevArgs) {
    gpui_kit::application().run(move |cx: &mut App| {
        materiant::init(cx);
        // Đóng cửa sổ là thoát app, không có khay (đặc tả 1.1).
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let display_id = dev.display_id(cx);
        let bounds = Bounds::centered(display_id, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(t!("common.app_name").into()), ..Default::default() }),
            display_id,
            focus: !dev.background,
            ..materiant::window::window_options()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Shell)).expect("open main window");
        if !dev.background {
            cx.activate(true);
        }
        dev::replay(handle, dev.keys, cx);
    });
}

/// Nội dung cửa sổ, chưa có gì.
struct Shell;

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.theme().colors().surface)
    }
}
