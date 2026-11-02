//! cpu: một phần của view chủ nhúng dạng view `cached` của GPUI, để khung hình của hoạt ảnh lặp (chữ chạy, thanh nhảy) không
//! dựng lại cả cửa sổ.
//!
//! GPUI vẽ lại mỗi khung mọi view "bẩn": view tự `notify` (hoạt ảnh xin khung là `notify` view đang vẽ nó) và **mọi view tổ
//! tiên** của nó. View con nhúng thường (`.child(entity)`) dựng lại theo cha; view nhúng `.cached(style)` mà không bẩn thì
//! dùng lại nguyên cây, bố cục, nét vẽ của khung trước. Đo ở desktop ẩn (03/11/2026): chữ chạy làm cả cửa sổ đi qua
//! `request_layout` + taffy + `prepaint` + `paint` mỗi nhịp màn hình, ~45 % một nhân; phần dựng cây (`render`) chỉ ~3 %.
//!
//! Luật dùng (groove/CLAUDE.md mục "Hoạt ảnh và view cached"):
//!
//! - Đường từ gốc tới view đang chạy hoạt ảnh nhúng **thường**; view `cached` mà bẩn thì GPUI dựng lại CẢ cây con của nó (kể
//!   cả các view `cached` bên trong). Mọi nhánh nặng bên cạnh đường đó nhúng `cached`.
//! - View `cached` chỉ dựng lại khi chính nó bẩn: thứ nó vẽ đổi thì phải có `notify` tới nó. [`Island`] lo việc đó bằng cách
//!   nghe (`observe`) view chủ: chủ `notify` là phần dựng lại, nên chủ phải nghe mọi entity mà phần đọc. Theme, ngôn ngữ, cỡ cửa sổ, rê chuột,
//!   focus thì GPUI làm mới cả cửa sổ (`refresh`), view `cached` tự dựng lại.
//! - Hoạt ảnh lặp là entity riêng (`Marquee`, `PlayingBars`): khung của nó chỉ `notify` chính nó, không `notify` view chủ (sẽ
//!   kéo mọi `Island` nghe chủ dựng lại mỗi khung).
//! - Hàm vẽ phần của chủ không được `notify` chủ (vòng lặp vẽ lại mãi).

use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, Context, Empty, Entity, IntoElement, Render, StyleRefinement, Subscription, ViewElement, WeakEntity,
    Window,
};

/// Hàm vẽ một phần của view chủ `H`, chạy trong lúc vẽ [`Island`] (chủ đã vẽ xong, không bị khoá).
pub type PartFn<H> = fn(&mut H, &mut Window, &mut Context<H>) -> AnyElement;

pub struct Island<H: 'static> {
    host: WeakEntity<H>,
    part: PartFn<H>,
    _subscriptions: Vec<Subscription>,
}

impl<H: 'static> Island<H> {
    /// Phần `part` của chủ đang dựng `cx`; dựng lại khi chủ `notify`. Phần đọc entity khác thì chủ phải nghe entity đó.
    pub fn new(part: PartFn<H>, cx: &mut Context<H>) -> Entity<Island<H>> {
        let host = cx.entity();
        cx.new(move |cx| {
            let subscriptions = vec![cx.observe(&host, |_, _, cx| cx.notify())];
            Island { host: host.downgrade(), part, _subscriptions: subscriptions }
        })
    }
}

impl<H: 'static> Render for Island<H> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = self.part;
        self.host.update(cx, |host, cx| part(host, window, cx)).unwrap_or_else(|_| Empty.into_any_element())
    }
}

/// Nhúng `island` dạng `cached`, bố cục theo `style` (cần cỡ xác định: GPUI không đo nội dung của view `cached`).
pub fn cached<H: 'static>(
    island: &Entity<Island<H>>,
    style: impl FnOnce(StyleRefinement) -> StyleRefinement,
) -> ViewElement<Entity<Island<H>>> {
    island.clone().cached(style(StyleRefinement::default()))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use ::gpui::{TestAppContext, VisualTestContext};
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::{div, px, size};

    use super::*;
    use crate::views::playing_bars::PlayingBars;

    /// Chủ có một phần `Island` đếm số lần dựng và một bộ thanh nhảy đang chạy (hoạt ảnh lặp).
    struct Host {
        value: u32,
        part_renders: Rc<Cell<usize>>,
        shown: Rc<Cell<u32>>,
        island: Option<Entity<Island<Host>>>,
    }

    impl Host {
        fn part(&mut self, _: &mut Window, _: &mut Context<Self>) -> AnyElement {
            self.part_renders.set(self.part_renders.get() + 1);
            self.shown.set(self.value);
            div().size_full().child(self.value.to_string()).into_any_element()
        }
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let island = self.island.get_or_insert_with(|| Island::new(Host::part, cx)).clone();
            div().size_full().child(cached(&island, |s| s.w(px(100.)).h(px(20.)))).child(PlayingBars::new(true))
        }
    }

    fn frame(cx: &mut VisualTestContext) {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    /// Khung của hoạt ảnh trong chủ không dựng lại phần `cached`; chủ `notify` thì phần dựng lại với dữ liệu mới.
    #[::gpui::test]
    fn part_survives_animation_frames_and_follows_host_notify(cx: &mut TestAppContext) {
        cx.update(materiant::init);
        let (part_renders, shown) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let host =
            cx.new(|_| Host { value: 1, part_renders: part_renders.clone(), shown: shown.clone(), island: None });
        let window = cx.add_window({
            let host = host.clone();
            move |window, cx| Root::new(host, window, cx)
        });
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(300.), px(100.)));
        cx.update(|window, _| window.activate_window());
        frame(cx);
        frame(cx);
        let before = part_renders.get();
        assert!(before >= 1 && shown.get() == 1);
        for _ in 0..20 {
            frame(cx);
        }
        assert_eq!(part_renders.get(), before, "hoạt ảnh lặp không dựng lại phần cached");

        host.update(cx, |h, cx| {
            h.value = 2;
            cx.notify();
        });
        frame(cx);
        assert_eq!(shown.get(), 2, "chủ notify thì phần vẽ dữ liệu mới");
        assert_eq!(part_renders.get(), before + 1);
    }
}
