//! groove-gd2_05: hộp "Có gì mới" — hiện một lần khi Groovex mở lần đầu sau khi lên bản mới (tự cập nhật hay cài tay). Quyết
//! định + chữ ở `updater::notes::on_start` (sổ `<data>/last-version`, note server gửi kèm lúc cài, hoặc
//! `release-notes/v<version>.md` nhúng lúc build). Hộp tự vẽ lớp phủ trong cây của khung (`Slot::WhatsNew`); Enter / Esc /
//! nút OK đóng. Chạy thử: `GROOVE_WHATS_NEW=1` (bản debug) mở hộp bất kể sổ.

use groove_core::{paths, t};
use materiant::button::Button;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    Render, Styled, Window, div, px,
};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::scroll::ScrollArea;
use materiant::theme::ActiveTheme;
use updater::ReleaseNote;
use updater::notes::{self, Block};

use crate::app::{self, Slot};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/release_note.rs"));
}

const FORCE_VAR: &str = "GROOVE_WHATS_NEW";
const DIALOG_WIDTH: f32 = 480.;
const BODY_MAX_HEIGHT: f32 = 360.;

pub struct WhatsNewDialog {
    handle: DialogHandle,
    focus: FocusHandle,
    blocks: Vec<Block>,
}

impl WhatsNewDialog {
    fn new(cx: &mut Context<Self>) -> WhatsNewDialog {
        WhatsNewDialog { handle: DialogHandle::new(cx), focus: cx.focus_handle(), blocks: Vec::new() }
    }

    pub fn open(&mut self, notes: &[ReleaseNote], window: &mut Window, cx: &mut Context<Self>) {
        self.blocks = notes::blocks(notes);
        if !self.handle.is_open() {
            self.handle.open(window, cx);
        }
        // Focus vào thân hộp để Enter đóng (Esc là `Cancel` của Dialog, vẫn trong ngữ cảnh hộp).
        window.focus(&self.focus, cx);
        cx.notify();
    }

    #[cfg(test)]
    pub fn new_for_test(cx: &mut Context<Self>) -> WhatsNewDialog {
        WhatsNewDialog::new(cx)
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    fn close(&mut self, how: &str, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("whats new: closed by {how}");
        self.handle.close(window, cx);
        cx.notify();
    }

    fn body(&self, cx: &Context<Self>) -> AnyElement {
        let c = cx.theme().colors();
        let (muted, primary) = (c.muted, c.primary);
        let mut list = div().w_full().flex().flex_col().gap(px(6.)).pb(px(4.));
        for block in &self.blocks {
            list = match block {
                Block::Title(text) => list.child(div().pt(px(12.)).text_lg().child(text.clone())),
                Block::Heading(text) => list.child(div().pt(px(8.)).text_color(primary).child(text.clone())),
                Block::Bullet(text) => list.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(div().flex_none().text_color(muted).child("•"))
                        .child(div().flex_1().min_w_0().child(text.clone())),
                ),
                Block::Paragraph(text) => list.child(div().child(text.clone())),
            };
        }
        div()
            .id("whats-new-body")
            .track_focus(&self.focus)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().text_size(cx.theme().caption_size()).text_color(muted).child(format!(
                "{} {}",
                t!("common.app_name"),
                super::SPEC.version
            )))
            .child(ScrollArea::new("whats-new-scroll").vertical().w_full().max_h(px(BODY_MAX_HEIGHT)).child(list))
            .into_any_element()
    }
}

impl Render for WhatsNewDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ok = Button::new("whats-new-ok", t!("update.whats_new.ok"))
            .primary()
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close("button", window, cx)))
            .into_any_element();
        div()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    this.close("enter", window, cx);
                }
            }))
            .child(
                Dialog::new("whats-new", &self.handle)
                    .max_width(px(DIALOG_WIDTH))
                    .title(t!("update.whats_new.title"))
                    .title_weight(materiant::gpui_kit::FontWeight::NORMAL)
                    .actions([ok])
                    .child(self.body(cx)),
            )
    }
}

/// Dựng hộp vào `Slot::WhatsNew`; đọc sổ trên luồng nền, phải hiện thì mở. Gọi một lần trong `app::run`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let created = window.update(cx, |_, _, cx| {
        let dialog = cx.new(WhatsNewDialog::new);
        app::install_slot(Slot::WhatsNew, dialog.clone(), cx);
        dialog
    });
    let Ok(dialog) = created else { return };
    let forced = cfg!(debug_assertions) && std::env::var_os(FORCE_VAR).is_some_and(|v| v == "1");
    let decide = cx.background_executor().spawn(async move {
        let data = paths::data_dir().ok()?;
        let embedded = embedded::RELEASE_NOTE;
        if forced {
            let markdown = embedded.unwrap_or("## Mới\n\n- Bản chạy thử hộp \"Có gì mới\".").to_owned();
            return Some(vec![ReleaseNote { version: super::SPEC.version.into(), markdown }]);
        }
        notes::on_start(&data, &data.join("update"), super::SPEC.version, embedded)
    });
    cx.spawn(async move |cx| {
        let Some(notes) = decide.await else { return };
        log::info!("whats new: {} ({} notes)", super::SPEC.version, notes.len());
        let _ = window.update(cx, |_, window, cx| dialog.update(cx, |d, cx| d.open(&notes, window, cx)));
    })
    .detach();
}
