//! groove-gd2_05: thẻ Giới thiệu của Settings — tên + phiên bản, trạng thái cập nhật, Kiểm tra cập nhật / Cập nhật (hay Mở
//! trang tải với bản major / bản không tự cài được). Đọc `update::status` mỗi lần vẽ; đổi trạng thái thì `update` làm mới
//! cửa sổ.

use groove_core::t;
use materiant::button::Button;
use materiant::gpui_kit::{AnyElement, App, IntoElement, ParentElement, Styled, div, px};
use materiant::theme::ActiveTheme;

use crate::update::{self, Status};

pub fn panel(cx: &App) -> AnyElement {
    let c = cx.theme().colors();
    let status = update::status(cx);
    let text = match &status {
        Status::Idle => t!("update.about.status_idle").to_owned(),
        Status::Checking => t!("update.about.status_checking").to_owned(),
        Status::UpToDate => t!("update.about.status_up_to_date").to_owned(),
        Status::Available(m) | Status::Failed(Some(m)) => t!("update.about.status_available", version = &m.version),
        Status::Downloading { fraction, .. } => {
            t!("update.downloading", percent = (fraction * 100.).round() as u32)
        }
        Status::Installing(_) => t!("update.installing").to_owned(),
        Status::Failed(None) => t!("update.about.status_failed").to_owned(),
    };
    let busy = matches!(status, Status::Checking | Status::Downloading { .. } | Status::Installing(_));
    let action = match update::badge(cx) {
        Some(badge) if !busy => {
            let label = if badge.opens_page { t!("update.about.open_page") } else { t!("update.about.install") };
            Button::new("about-install", label)
                .primary()
                .on_click(|_, _, cx| update::start_install(cx))
                .into_any_element()
        }
        _ => Button::new("about-check", t!("update.about.check"))
            .outlined()
            .disabled(busy)
            .on_click(|_, _, cx| update::check_now(true, cx))
            .into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_xl().child(t!("common.app_name")))
                .child(div().text_color(c.muted).child(t!("update.about.version", version = update::SPEC.version))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(c.border)
                .child(div().flex_1().min_w_0().child(text))
                .child(div().flex_none().child(action)),
        )
        .into_any_element()
}
