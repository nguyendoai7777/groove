//! groove-gd1_19: ô bìa 200 px của dialog metadata (đặc tả 1.8): nhấp chọn tệp, kéo thả, dán khi rê chuột trên ô, nút bỏ
//! bìa.
//!
//! Ô giữ lựa chọn (`CoverEdit`) cho tới khi lưu. Đọc tệp ảnh chạy ở luồng nền (N7), kiểm định dạng bằng chữ ký đầu tệp
//! trước khi nhận, trần 32 MB như `tags`. Ảnh hiện qua `ImageSource::Image`; asset của nó bị gỡ khi đổi ảnh hoặc đóng ô
//! để bìa lớn không nằm lại trong bộ nhớ.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use groove_core::t;
use groove_library::tags::{CoverArt, CoverEdit};
use materiant::button::{Button, Size};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    ClipboardEntry, Context, EventEmitter, ExternalPaths, Image, ImageFormat, ImageSource, IntoElement, ObjectFit,
    ParentElement, PathPromptOptions, Render, SharedString, Styled, StyledImage, Window, div, img, px,
};
use materiant::icon::{Icon, IconName};
use materiant::theme::{ActiveTheme, Neutral};

use crate::icons::GrooveIcon;

/// Trần cỡ ảnh bìa, đúng với `tags` (`MAX_COVER_BYTES`).
pub const MAX_COVER_BYTES: usize = 32 * 1024 * 1024;
/// Cạnh ô bìa.
pub const SIDE: f32 = 200.;

/// Vì sao không nhận được ảnh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverProblem {
    /// Không phải JPEG, PNG, WebP, GIF, BMP.
    Unsupported,
    TooLarge,
    ReadFailed,
}

impl CoverProblem {
    pub fn text(self) -> &'static str {
        match self {
            CoverProblem::Unsupported => t!("metadata.cover.unsupported"),
            CoverProblem::TooLarge => t!("metadata.cover.too_large"),
            CoverProblem::ReadFailed => t!("metadata.cover.read_failed"),
        }
    }
}

/// Định dạng ảnh theo chữ ký đầu tệp; `None` khi không thuộc năm loại `tags` nhận.
pub fn sniff_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else if bytes.starts_with(b"BM") {
        Some(ImageFormat::Bmp)
    } else {
        None
    }
}

fn format_of_mime(mime: &str) -> Option<ImageFormat> {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::Webp),
        "image/bmp" => Some(ImageFormat::Bmp),
        _ => None,
    }
}

/// Đọc tệp ảnh, kiểm cỡ và định dạng. Chạm đĩa: chỉ gọi ở luồng nền.
pub fn read_cover_file(path: &Path) -> Result<Vec<u8>, CoverProblem> {
    let file = File::open(path).map_err(|_| CoverProblem::ReadFailed)?;
    let mut bytes = Vec::new();
    file.take(MAX_COVER_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|_| CoverProblem::ReadFailed)?;
    check_cover_bytes(&bytes).map(|_| bytes)
}

/// Cỡ trước, định dạng sau.
pub fn check_cover_bytes(bytes: &[u8]) -> Result<ImageFormat, CoverProblem> {
    if bytes.len() > MAX_COVER_BYTES {
        return Err(CoverProblem::TooLarge);
    }
    sniff_format(bytes).ok_or(CoverProblem::Unsupported)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Choice {
    Keep,
    Remove,
    Set(Vec<u8>),
}

/// Báo cho dialog đọc lại `edit()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverPaneEvent {
    Changed,
}

pub struct CoverPane {
    original: Option<CoverArt>,
    choice: Choice,
    problem: Option<CoverProblem>,
    hovered: bool,
    /// Ảnh đang hiện, kèm nguồn để gỡ asset khi bỏ.
    shown: Option<Arc<Image>>,
}

impl EventEmitter<CoverPaneEvent> for CoverPane {}

impl CoverPane {
    pub fn new(original: Option<CoverArt>, cx: &mut Context<Self>) -> CoverPane {
        let mut pane = CoverPane { original, choice: Choice::Keep, problem: None, hovered: false, shown: None };
        pane.shown = pane.image_for_choice();
        cx.on_release(|pane, cx| {
            if let Some(image) = pane.shown.take() {
                ImageSource::from(image).remove_asset(cx);
            }
        })
        .detach();
        pane
    }

    fn image_for_choice(&self) -> Option<Arc<Image>> {
        match &self.choice {
            Choice::Keep => {
                let art = self.original.as_ref()?;
                let format = sniff_format(&art.data).or_else(|| format_of_mime(&art.mime))?;
                Some(Arc::new(Image::from_bytes(format, art.data.clone())))
            }
            Choice::Remove => None,
            Choice::Set(bytes) => Some(Arc::new(Image::from_bytes(sniff_format(bytes)?, bytes.clone()))),
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.shown.take() {
            ImageSource::from(old).remove_asset(cx);
        }
        self.shown = self.image_for_choice();
        cx.emit(CoverPaneEvent::Changed);
        cx.notify();
    }

    /// Phần sửa bìa để đưa cho `MetadataOps`.
    pub fn edit(&self) -> CoverEdit {
        match &self.choice {
            Choice::Keep => CoverEdit::Keep,
            Choice::Remove => CoverEdit::Remove,
            Choice::Set(bytes) => CoverEdit::Set(bytes.clone()),
        }
    }

    pub fn is_changed(&self) -> bool {
        self.choice != Choice::Keep
    }

    /// Có ảnh bìa nào đang hiện (gốc hoặc mới).
    pub fn has_image(&self) -> bool {
        self.shown.is_some()
    }

    /// Chuột đang ở trên ô: dialog dùng để quyết định Ctrl+V là dán ảnh hay dán chữ.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    #[cfg(test)]
    pub fn problem(&self) -> Option<CoverProblem> {
        self.problem
    }

    /// Nhận ảnh từ bộ nhớ (đã đọc ở nơi khác). Sai định dạng hoặc quá lớn thì báo lỗi, giữ lựa chọn cũ.
    pub fn set_image(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) -> bool {
        match check_cover_bytes(&bytes) {
            Ok(_) => {
                self.problem = None;
                self.choice = Choice::Set(bytes);
                self.refresh(cx);
                true
            }
            Err(problem) => {
                self.problem = Some(problem);
                cx.notify();
                false
            }
        }
    }

    pub fn remove(&mut self, cx: &mut Context<Self>) {
        self.problem = None;
        self.choice = Choice::Remove;
        self.refresh(cx);
    }

    /// Về bìa gốc của tệp.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.problem = None;
        self.choice = Choice::Keep;
        self.refresh(cx);
    }

    /// Đọc tệp ở luồng nền rồi nhận ảnh.
    pub fn load_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let read = cx.background_spawn(async move { read_cover_file(&path) });
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(bytes) => {
                    this.set_image(bytes, cx);
                }
                Err(problem) => {
                    this.problem = Some(problem);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Hộp thoại chọn tệp của OS.
    pub fn choose_file(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t!("metadata.cover.choose"))),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.load_file(path, cx));
            }
        })
        .detach();
    }

    /// Dán từ clipboard: ảnh thì nhận ngay, tệp ảnh được chép thì đọc ở nền. `false` khi clipboard không có ảnh, để
    /// Ctrl+V tiếp tục dán chữ như thường.
    pub fn paste(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else { return false };
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    self.set_image(image.bytes.clone(), cx);
                    return true;
                }
                ClipboardEntry::ExternalPaths(paths) => {
                    if let Some(path) = paths.paths().first() {
                        self.load_file(path.clone(), cx);
                        return true;
                    }
                }
                ClipboardEntry::String(_) => {}
            }
        }
        false
    }
}

impl Render for CoverPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let side = px(SIDE);
        let frame = div()
            .id("metadata-cover")
            .relative()
            .size(side)
            .flex_none()
            .overflow_hidden()
            .rounded_lg()
            .bg(c.layer(Neutral::N200))
            .border_2()
            .border_color(if self.hovered { c.primary } else { gpui_transparent() })
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, _, cx| this.choose_file(cx)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.load_file(path.clone(), cx);
                }
            }))
            .drag_over::<ExternalPaths>({
                let accent = c.primary;
                move |style, _, _, _| style.border_color(accent)
            });
        let frame = match self.shown.clone() {
            Some(image) => frame.child(img(ImageSource::from(image)).size_full().object_fit(ObjectFit::Cover)),
            None => frame.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_3()
                    .text_center()
                    .text_color(c.muted)
                    .child(Icon::new(GrooveIcon::DefaultCover).size(px(56.)).color(c.muted.opacity(0.7)))
                    .child(div().text_sm().child(t!("metadata.cover.hint"))),
            ),
        };
        let note = match (self.problem, &self.choice) {
            (Some(problem), _) => Some((problem.text(), c.danger)),
            (None, Choice::Set(_)) => Some((t!("metadata.cover.pending"), c.muted)),
            (None, Choice::Remove) => Some((t!("metadata.cover.removed"), c.muted)),
            (None, Choice::Keep) => None,
        };
        let changed = self.is_changed();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w(side)
            .flex_none()
            .child(frame)
            .when_some(note, |col, (text, color)| {
                col.child(
                    div()
                        .flex()
                        .items_start()
                        .gap_1()
                        .text_sm()
                        .text_color(color)
                        .when(self.problem.is_some(), |row| {
                            row.child(Icon::new(IconName::Warning).size(px(14.)).color(color))
                        })
                        .child(text),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("metadata-cover-choose", t!("metadata.cover.choose"))
                            .outlined()
                            .base()
                            .size(Size::Sm)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_file(cx))),
                    )
                    .child(if changed {
                        Button::new("metadata-cover-undo", t!("metadata.cover.undo"))
                            .text()
                            .base()
                            .size(Size::Sm)
                            .on_click(cx.listener(|this, _, _, cx| this.undo(cx)))
                    } else {
                        Button::new("metadata-cover-remove", t!("metadata.cover.remove"))
                            .text()
                            .danger()
                            .size(Size::Sm)
                            .disabled(!self.has_image())
                            .on_click(cx.listener(|this, _, _, cx| this.remove(cx)))
                    }),
            )
    }
}

fn gpui_transparent() -> materiant::gpui_kit::Hsla {
    materiant::gpui_kit::hsla(0., 0., 0., 0.)
}

#[cfg(test)]
mod tests {
    use groove_library::fixtures::TINY_PNG;

    use super::*;

    #[test]
    fn sniffs_the_five_accepted_formats() {
        assert_eq!(sniff_format(TINY_PNG), Some(ImageFormat::Png));
        assert_eq!(sniff_format(&[0xff, 0xd8, 0xff, 0xe0, 0, 0]), Some(ImageFormat::Jpeg));
        assert_eq!(sniff_format(b"GIF89a...."), Some(ImageFormat::Gif));
        assert_eq!(sniff_format(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some(ImageFormat::Webp));
        assert_eq!(sniff_format(b"BM\x00\x00"), Some(ImageFormat::Bmp));
    }

    #[test]
    fn rejects_text_and_other_riff() {
        assert_eq!(sniff_format(b"hello world, not an image"), None);
        assert_eq!(sniff_format(b"RIFF\x00\x00\x00\x00WAVEfmt "), None, "WAV cũng là RIFF");
        assert_eq!(sniff_format(&[]), None);
    }

    #[test]
    fn size_is_checked_before_format() {
        let mut huge = TINY_PNG.to_vec();
        huge.resize(MAX_COVER_BYTES + 1, 0);
        assert_eq!(check_cover_bytes(&huge), Err(CoverProblem::TooLarge));
        assert_eq!(check_cover_bytes(b"text"), Err(CoverProblem::Unsupported));
        assert_eq!(check_cover_bytes(TINY_PNG), Ok(ImageFormat::Png));
    }

    #[test]
    fn reads_a_file_and_reports_what_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("cover.png");
        std::fs::write(&good, TINY_PNG).unwrap();
        assert_eq!(read_cover_file(&good).unwrap(), TINY_PNG);
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "x").unwrap();
        assert_eq!(read_cover_file(&text), Err(CoverProblem::Unsupported));
        assert_eq!(read_cover_file(&dir.path().join("missing.png")), Err(CoverProblem::ReadFailed));
    }

    #[test]
    fn mime_maps_to_format() {
        assert_eq!(format_of_mime("image/JPEG"), Some(ImageFormat::Jpeg));
        assert_eq!(format_of_mime("image/svg+xml"), None);
    }
}
