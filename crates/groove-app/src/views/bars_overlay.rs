//! cpu (03/11/2026): thanh nhảy của bài đang phát nằm trong view `cached` (danh sách bài của trang thư viện, hàng đợi và dòng
//! thời gian của Now Playing) vẽ trên một lớp phủ cấp nội dung của `Shell`, không vẽ trong cây của view đó.
//!
//! Lý do: khung của hoạt ảnh `notify` view đang vẽ nó, GPUI đánh dấu bẩn cả mọi tổ tiên, và view `cached` bẩn thì dựng lại CẢ cây
//! con (`views::island`). Thanh nhảy 14 px ở một hàng kéo cả trang thư viện (lưới / danh sách ảo, hàng trăm phần tử) qua bố cục và
//! vẽ lại ~60 lần mỗi giây. Ở đây hàng chỉ đặt một [`anchor`] rỗng; lớp phủ [`BarsOverlay`] (con thường của `Shell`, nằm sau nội
//! dung) vẽ bốn thanh đúng chỗ đó mỗi khung; trang `cached` dùng lại nguyên.
//!
//! Đúng chỗ, đúng lúc (lý do phương án lớp phủ cấp Shell từng bị loại, `plan/confirm.md` mục `cpu`):
//!
//! - **Vị trí**: anchor ghi khung + vùng cắt (`content_mask`, để hàng cuộn khuất nửa thì thanh cũng khuất) lúc **paint**. View chứa
//!   nó được GPUI dùng lại từ cache chỉ khi bố cục y nguyên, nên vị trí đã ghi vẫn đúng; dựng lại thì anchor ghi lại.
//! - **Hết hiệu lực**: view chứa anchor gọi [`forget`] đầu `render` (hàng thôi phát, cuộn khuất, đổi bài thì lần vẽ sau không
//!   ghi lại); `Shell` gọi [`forget_all_except`] khi đổi view nội dung (trừ anchor của cột trái) (view cũ không vẽ nữa nên không tự xoá được).
//! - **Ẩn khi rê**: hàng ẩn thanh, hiện nút Pause khi rê (`invisible` + `group_hover`): GPUI bỏ `paint` của phần ẩn nên anchor
//!   không ghi; rê đổi trạng thái thì GPUI làm mới cả cửa sổ.
//! - **Lớp**: lớp phủ nằm trong vùng nội dung, trước thanh phát, bảng lệnh, Settings, hộp thoại, menu (`deferred`) — những thứ đó
//!   vẫn đè lên thanh.

use std::collections::HashMap;

use materiant::gpui_kit::{
    App, Bounds, ContentMask, Context, EntityId, Global, IntoElement, Pixels, Render, Styled, Window, canvas,
};
use materiant::theme::ActiveTheme;

use crate::views::playing_bars::{Phase, paint_bars};

#[derive(Clone, Copy, PartialEq)]
struct Anchor {
    bounds: Bounds<Pixels>,
    mask: ContentMask<Pixels>,
}

/// Anchor theo view đang vẽ nó (`window.current_view()` lúc paint).
#[derive(Default)]
struct Anchors {
    by_owner: HashMap<EntityId, Vec<Anchor>>,
}

impl Global for Anchors {}

/// Ô vuông cạnh `size` giữ chỗ cho bốn thanh đang chạy; lớp phủ vẽ lên đó. View chứa nó phải gọi [`forget`] đầu `render`.
pub fn anchor(size: Pixels) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |bounds, _, window, cx| {
            let anchor = Anchor { bounds, mask: window.content_mask() };
            let list = cx.default_global::<Anchors>().by_owner.entry(window.current_view()).or_default();
            if !list.contains(&anchor) {
                list.push(anchor);
            }
        },
    )
    .flex_none()
    .w(size)
    .h(size)
}

/// Bỏ anchor của view `owner` (gọi đầu `render` của nó: lần vẽ này ghi lại những anchor còn hiện).
pub fn forget(owner: EntityId, cx: &mut App) {
    if cx.has_global::<Anchors>() {
        cx.global_mut::<Anchors>().by_owner.remove(&owner);
    }
}

/// Bỏ mọi anchor (đổi view nội dung), trừ của view `keep` (cột trái của Shell: icon Now Playing, không thuộc nội dung).
pub fn forget_all_except(keep: EntityId, cx: &mut App) {
    if cx.has_global::<Anchors>() {
        cx.global_mut::<Anchors>().by_owner.retain(|owner, _| *owner == keep);
    }
}

/// Số anchor đang có (cho test).
#[cfg(test)]
pub fn count(cx: &App) -> usize {
    cx.try_global::<Anchors>().map_or(0, |anchors| anchors.by_owner.values().map(Vec::len).sum())
}

/// Lớp phủ phủ kín vùng nội dung, không nhận chuột. Đọc anchor lúc paint (sau khi nội dung đã vẽ trong cùng khung) và chỉ xin
/// khung kế khi có thanh đang chạy và cửa sổ active, như `PlayingBars`.
pub struct BarsOverlay;

impl Render for BarsOverlay {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            |_, _, window, cx| {
                let anchors: Vec<Anchor> = cx
                    .try_global::<Anchors>()
                    .map(|anchors| anchors.by_owner.values().flatten().copied().collect())
                    .unwrap_or_default();
                if anchors.is_empty() {
                    return;
                }
                let now = cx.background_executor().now();
                let running = !cx.reduce_motion() && window.is_window_active();
                let phase = cx.default_global::<Phase>();
                phase.set_active(running, now);
                let elapsed = phase.elapsed_ms(now);
                if running {
                    window.request_animation_frame();
                }
                let color = cx.theme().colors().primary;
                for anchor in anchors {
                    window.with_content_mask(Some(anchor.mask), |window| {
                        paint_bars(anchor.bounds, elapsed, color, window)
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}
