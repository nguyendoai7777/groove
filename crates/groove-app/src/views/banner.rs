//! banner-anim: đầu danh sách bài (bìa, tên, nút Play all) thu / nở mượt khi cuộn qua ngưỡng, như bản web
//! (`music-list.vue`: `transition-all duration-300 ease-in-out`).
//!
//! Không phải hoạt ảnh lặp: chỉ chạy [`DURATION`] sau mỗi lần đổi trạng thái rồi dừng hẳn. Vẫn giữ luật `views::island`:
//!
//! - Banner là entity riêng, khung của nó chỉ `notify` chính nó (`request_animation_frame` trong `render` của nó). GPUI dựng lại
//!   các view tổ tiên (danh sách bài, trang thư viện, `Shell`) — phần dựng cây rẻ; bảng bài nằm trong `Island` `cached` có khung
//!   cố định, khung hoạt ảnh dùng lại nguyên.
//! - Banner phủ lên đầu bảng (tuyệt đối), không đẩy bảng: cỡ banner đổi mỗi khung mà khung của bảng không đổi. Bảng chỉ dời một
//!   lần ở lúc đổi trạng thái (`song_list::SongList::sync_header`).
//! - Phần vẽ là hàm của view chủ ([`BannerFn`]) như `Island`: chủ `notify` (dữ liệu đổi) thì banner vẽ lại theo.

use std::time::{Duration, Instant};

use materiant::gpui_kit::base::motion::Easing;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{AnyElement, Context, Empty, Entity, IntoElement, Render, Subscription, WeakEntity, Window};

/// Thời lượng một lần thu / nở (web `duration-300`).
pub const DURATION: Duration = Duration::from_millis(300);

/// Hàm vẽ banner của view chủ `H` ở tiến độ `progress` (0 = mở, 1 = thu, đã qua easing).
pub type BannerFn<H> = fn(&mut H, f32, &mut Window, &mut Context<H>) -> AnyElement;

pub struct Banner<H: 'static> {
    host: WeakEntity<H>,
    part: BannerFn<H>,
    easing: Easing,
    /// Đích: 0 mở, 1 thu.
    target: f32,
    /// Tiến độ lúc bắt đầu lần chuyển đang chạy (đổi hướng giữa chừng thì chạy tiếp từ chỗ đang đứng).
    from: f32,
    /// Mốc bắt đầu lần chuyển đang chạy; `None` = đứng yên ở `target`.
    start: Option<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl<H: 'static> Banner<H> {
    /// Banner của view chủ đang dựng `cx`, đứng yên ở trạng thái mở. Vẽ lại khi chủ `notify`.
    pub fn new(part: BannerFn<H>, cx: &mut Context<H>) -> Entity<Banner<H>> {
        let host = cx.entity();
        cx.new(move |cx| Banner {
            host: host.downgrade(),
            part,
            // Tailwind `ease-in-out`.
            easing: Easing::cubic_bezier(0.4, 0., 0.2, 1.).unwrap_or(Easing::EaseInOut),
            target: 0.,
            from: 0.,
            start: None,
            _subscriptions: vec![cx.observe(&host, |_, _, cx| cx.notify())],
        })
    }

    /// Chuyển mượt tới trạng thái thu (`true`) / mở.
    pub fn animate_to(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        let target = if collapsed { 1. } else { 0. };
        if target == self.target {
            return;
        }
        let now = cx.background_executor().now();
        self.from = self.progress_at(now);
        self.target = target;
        self.start = Some(now);
        cx.notify();
    }

    /// Đặt thẳng trạng thái, không chuyển (đổi sang danh sách khác).
    pub fn jump_to(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.target = if collapsed { 1. } else { 0. };
        self.from = self.target;
        self.start = None;
        cx.notify();
    }

    /// Đang chạy lần chuyển nào không.
    #[cfg(test)]
    pub fn is_animating(&self) -> bool {
        self.start.is_some()
    }

    fn progress_at(&self, now: Instant) -> f32 {
        let Some(start) = self.start else { return self.target };
        let t = (now.saturating_duration_since(start).as_secs_f32() / DURATION.as_secs_f32()).clamp(0., 1.);
        self.from + (self.target - self.from) * self.easing.sample(t)
    }
}

impl<H: 'static> Render for Banner<H> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = cx.background_executor().now();
        if let Some(start) = self.start
            && (cx.reduce_motion() || now.saturating_duration_since(start) >= DURATION)
        {
            self.start = None;
        }
        let progress = self.progress_at(now);
        if self.start.is_some() {
            // Chỉ xin khung kế khi còn đang chuyển: xong là dừng hẳn, không khung nào nữa.
            window.request_animation_frame();
        }
        let part = self.part;
        self.host.update(cx, |host, cx| part(host, progress, window, cx)).unwrap_or_else(|_| Empty.into_any_element())
    }
}
