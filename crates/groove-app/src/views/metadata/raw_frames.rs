//! groove-gd1_19: tab Raw Frames — mọi khung ID3 trong tệp, khung lạ tô vàng (đặc tả 1.8, `tags.rs:378-408`).
//!
//! Nhãn khung tra theo `id` qua i18n vùng `metadata` (`frame.<id>`); khung chưa có nhãn hiện mỗi `id`. `TXXX` hiện
//! thêm mô tả của nó.

use groove_core::t;
use groove_library::tags::RawFrame;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use materiant::icon::{Icon, IconName};
use materiant::theme::{ActiveTheme, Neutral};

/// Nhãn dễ đọc của một mã khung, `None` khi chưa có trong catalog.
pub fn frame_label(id: &str) -> Option<&'static str> {
    Some(match id {
        "TIT2" => t!("metadata.frame.TIT2"),
        "TPE1" => t!("metadata.frame.TPE1"),
        "TPE2" => t!("metadata.frame.TPE2"),
        "TALB" => t!("metadata.frame.TALB"),
        "TRCK" => t!("metadata.frame.TRCK"),
        "TPOS" => t!("metadata.frame.TPOS"),
        "TYER" => t!("metadata.frame.TYER"),
        "TDRC" => t!("metadata.frame.TDRC"),
        "TCON" => t!("metadata.frame.TCON"),
        "TCOM" => t!("metadata.frame.TCOM"),
        "TPUB" => t!("metadata.frame.TPUB"),
        "TCOP" => t!("metadata.frame.TCOP"),
        "TBPM" => t!("metadata.frame.TBPM"),
        "TIT1" => t!("metadata.frame.TIT1"),
        "TCMP" => t!("metadata.frame.TCMP"),
        "TLEN" => t!("metadata.frame.TLEN"),
        "TSSE" => t!("metadata.frame.TSSE"),
        "TENC" => t!("metadata.frame.TENC"),
        "USLT" => t!("metadata.frame.USLT"),
        "COMM" => t!("metadata.frame.COMM"),
        "APIC" => t!("metadata.frame.APIC"),
        "TXXX" => t!("metadata.frame.TXXX"),
        "PRIV" => t!("metadata.frame.PRIV"),
        "UFID" => t!("metadata.frame.UFID"),
        "POPM" => t!("metadata.frame.POPM"),
        _ => return None,
    })
}

/// Số khung rác (`known == false`): hiện ở tiêu đề dialog.
pub fn junk_count(frames: &[RawFrame]) -> usize {
    frames.iter().filter(|frame| !frame.known).count()
}

/// Cột "Khung": `TXXX` kèm mô tả, `TXXX:TIMELINE`.
pub fn frame_id_text(frame: &RawFrame) -> String {
    match frame.description.as_deref().filter(|d| !d.is_empty()) {
        Some(description) => format!("{}:{description}", frame.id),
        None => frame.id.clone(),
    }
}

/// Bảng khung, cuộn dọc khi dài. `is_mp3` quyết định câu giải thích khi bảng trống.
#[derive(IntoElement)]
pub struct RawFrames {
    frames: Vec<RawFrame>,
    is_mp3: bool,
}

impl RawFrames {
    pub fn new(frames: Vec<RawFrame>, is_mp3: bool) -> RawFrames {
        RawFrames { frames, is_mp3 }
    }
}

impl RenderOnce for RawFrames {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        if self.frames.is_empty() {
            let text = if self.is_mp3 { t!("metadata.raw.empty") } else { t!("metadata.raw.only_mp3") };
            return div().py_6().text_color(c.muted).child(text).into_any_element();
        }
        let cell = |text: String, width: Option<f32>| -> AnyElement {
            let base = div().min_w_0().overflow_hidden().whitespace_normal().child(text);
            match width {
                Some(width) => base.w(px(width)).flex_none().into_any_element(),
                None => base.flex_1().into_any_element(),
            }
        };
        let header = div()
            .flex()
            .gap_3()
            .px_2()
            .pb_1()
            .text_color(c.muted)
            .child(cell(t!("metadata.raw.column_id").to_owned(), Some(150.)))
            .child(cell(t!("metadata.raw.column_name").to_owned(), Some(150.)))
            .child(cell(t!("metadata.raw.column_value").to_owned(), None));
        let rows = self.frames.iter().enumerate().map(|(ix, frame)| {
            let junk = !frame.known;
            let name = frame_label(&frame.id).unwrap_or_default().to_owned();
            let row = div()
                .id(("metadata-raw-frame", ix))
                .flex()
                .gap_3()
                .px_2()
                .py_1()
                .rounded_md()
                .items_start()
                .child(cell(frame_id_text(frame), Some(150.)))
                .child(cell(name, Some(150.)))
                .child(cell(frame.value.clone(), None));
            if junk {
                row.bg(c.warning.opacity(0.18))
                    .child(div().flex_none().child(Icon::new(IconName::Warning).size(px(14.)).color(c.warning)))
                    .into_any_element()
            } else {
                row.when(ix % 2 == 0, |row| row.bg(c.layer(Neutral::N100))).into_any_element()
            }
        });
        div()
            .id("metadata-raw-frames")
            .flex()
            .flex_col()
            .when(junk_count(&self.frames) > 0, |table| {
                table.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .pb_2()
                        .text_color(c.warning)
                        .child(Icon::new(IconName::Warning).size(px(14.)).color(c.warning))
                        .child(t!("metadata.raw.junk")),
                )
            })
            .child(header)
            .children(rows)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: &str, description: Option<&str>, known: bool) -> RawFrame {
        RawFrame { id: id.into(), description: description.map(Into::into), value: "v".into(), known }
    }

    #[test]
    fn counts_only_unknown_frames() {
        let frames =
            [frame("TIT2", None, true), frame("TXXX", Some("TIMELINE"), true), frame("TXXX", Some("X"), false)];
        assert_eq!(junk_count(&frames), 1, "TXXX:TIMELINE không phải rác");
        assert_eq!(junk_count(&[]), 0);
    }

    #[test]
    fn txxx_shows_its_description() {
        assert_eq!(frame_id_text(&frame("TXXX", Some("TIMELINE"), true)), "TXXX:TIMELINE");
        assert_eq!(frame_id_text(&frame("TIT2", None, true)), "TIT2");
        assert_eq!(frame_id_text(&frame("TXXX", Some(""), false)), "TXXX");
    }

    #[test]
    fn labels_come_from_the_catalog_by_id() {
        use groove_core::i18n::{Language, set_language};
        set_language(Language::Vi);
        assert_eq!(frame_label("TIT2"), Some("Tiêu đề"));
        assert_eq!(frame_label("ZZZZ"), None);
        assert_eq!(frame_label("TIT2"), Some("Tiêu đề"));
    }
}
