//! Chip đuôi tệp đứng cạnh ô tên tệp (hộp Sửa thông tin, Save as new — chủ dự án 09/11/2026): cao bằng ô nhập, chữ nằm giữa
//! theo chiều dọc. Hộp `line_height = 1` + `optical_lift` như `materiant::badge`: căn giữa hộp chữ thôi thì thân chữ trông
//! thấp (baseline theo ascent / descent, không theo nét chữ).

use materiant::field::shell::FIELD_HEIGHT;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{App, Div, SharedString, Window, div, px};
use materiant::style::optical_lift;
use materiant::theme::{ActiveTheme, Neutral};

/// `.mp3` trên nền lớp, bo như ô nhập.
pub fn extension_chip(extension: impl Into<SharedString>, window: &Window, cx: &App) -> Div {
    let c = cx.theme().colors();
    let size = cx.theme().font_size();
    let lift = optical_lift(window, size);
    div()
        .flex_none()
        .h(px(FIELD_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .rounded_md()
        .bg(c.layer(Neutral::N200))
        .text_color(c.muted)
        .text_size(size)
        .line_height(size)
        .whitespace_nowrap()
        .child(div().relative().top(-lift).child(extension.into()))
}
