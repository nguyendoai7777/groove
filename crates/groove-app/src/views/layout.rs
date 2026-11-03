//! layout-style: kiểu chia vùng của cửa sổ (Settings → Chung, `Settings::layout_style`) gom về một chỗ, view không tự rẽ
//! nhánh theo kiểu.
//!
//! - **Thẻ** (`Cards`, mặc định): mỗi khung (lời bài, hàng đợi) là thẻ bo 2xl, viền mờ, nền lớp; các khung cách nhau và cách
//!   mép bằng khe [`CARD_GAP`].
//! - **Lưới** (`Grid`, kiểu app Claude desktop): các vùng sát nhau, không bo, không khe, nền liền một khối; ngăn bằng đường kẻ
//!   1 px màu `border` ([`vline`], [`hline`]): giữa cột trái và nội dung, dưới thanh trên, giữa các cột Now Playing, dưới
//!   đầu trang thư viện, giữa danh sách và tab Settings. Thanh phát có viền trên ở cả hai kiểu.
//!
//! Thứ là nội dung chứ không phải vùng (bìa, thẻ album, hộp dòng thời gian, bảng EQ, nút) giữ bo góc ở cả hai kiểu
//! (`plan/confirm.md` mục `layout-style`).
//!
//! Đổi kiểu: `app::install_settings` gọi `refresh_windows`, view `cached` dựng lại như khi đổi ngôn ngữ.

use groove_core::settings::LayoutStyle;
use materiant::gpui_kit::{App, Div, ParentElement, Styled, div, px};
use materiant::theme::{ActiveTheme, Neutral};

use crate::app;

/// Khe giữa các thẻ và đệm quanh chúng ở kiểu thẻ (`gap-4`, `p-4`).
pub const CARD_GAP: f32 = 16.;

/// Kiểu đang chọn.
pub fn style(cx: &App) -> LayoutStyle {
    app::layout_style(cx)
}

pub fn is_grid(cx: &App) -> bool {
    style(cx) == LayoutStyle::Grid
}

/// Khe giữa hai vùng kề nhau: [`CARD_GAP`] ở kiểu thẻ, 0 ở kiểu lưới (vùng sát nhau, đường kẻ ngăn).
pub fn gap(cx: &App) -> f32 {
    if is_grid(cx) { 0. } else { CARD_GAP }
}

/// Đường kẻ dọc 1 px giữa hai cột; `None` ở kiểu thẻ.
pub fn vline(cx: &App) -> Option<Div> {
    is_grid(cx).then(|| line(cx).w(px(1.)).h_full())
}

/// Đường kẻ ngang 1 px giữa hai hàng; `None` ở kiểu thẻ.
pub fn hline(cx: &App) -> Option<Div> {
    is_grid(cx).then(|| line(cx).h(px(1.)).w_full())
}

/// `border` của theme tối có alpha: lót nền `surface` để đường kẻ đặt trên nền trong suốt của cửa sổ (giữa cột trái và nội
/// dung) cùng màu với đường trên nền nội dung.
fn line(cx: &App) -> Div {
    let c = cx.theme().colors();
    div().flex_none().bg(c.surface).child(div().size_full().bg(c.border))
}

/// Khung của một vùng: thẻ hay phẳng theo kiểu đang chọn.
pub trait SurfaceExt: Styled + Sized {
    /// Thẻ: bo 2xl, viền mờ, nền lớp. Lưới: không bo, không viền, nền trong suốt (liền với nền cửa sổ).
    fn region_surface(self, cx: &App) -> Self {
        if is_grid(cx) {
            return self;
        }
        let c = cx.theme().colors();
        self.rounded_2xl().border_1().border_color(c.border.opacity(0.5)).bg(c.layer(Neutral::N100).opacity(0.4))
    }
}

impl<E: Styled> SurfaceExt for E {}
