//! groove-gd1_20: hộp thoại sửa timeline (đặc tả 1.6). Lõi dùng chung ở `lyrics_dialog::TextDialog`.
//!
//! Văn bản lưu nguyên gốc (`MetadataOps::save_timeline`), `timeline::parse` chỉ chạy lúc phát. Có hướng dẫn khuôn dạng dưới ô.

use groove_library::metadata_ops::MetadataOps;
use materiant::gpui_kit::{Context, Window};

pub use crate::views::lyrics_dialog::{TextDialog, TextKind};

/// Dựng hộp timeline.
pub fn timeline(ops: MetadataOps, window: &mut Window, cx: &mut Context<TextDialog>) -> TextDialog {
    TextDialog::new(TextKind::Timeline, ops, window, cx)
}
