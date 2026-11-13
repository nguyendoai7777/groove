//! groove-gd1_21: hộp đoạn timeline ở cột trái Now Playing (đặc tả 1.5, 1.6).
//!
//! Đầu hộp: bốn thanh nhảy [`PlayingBars`] và tên đoạn đang phát (chữ chạy `Marquee` khi dài, dừng khi nhạc dừng / cửa sổ nền).
//! np-timeline-header (13/11/2026, chủ dự án): bỏ nhãn `<hiện tại> / <tổng>` — thanh phát đã có. Dưới là từng dòng
//! `<thời gian> <tên>` trong một `VirtualList`; dòng đang phát tô màu nhấn và tự cuộn vào giữa khi đổi đoạn. Bấm dòng (hoặc
//! Enter) là tua tới đầu đoạn và phát. Bài không có timeline thì hộp trống, không chiếm chỗ.
//!
//! Bố cục `views::island`: hộp nhúng THƯỜNG trong Now Playing (đường tới chữ chạy, thanh nhảy ở đầu hộp), danh sách là
//! `Island<TimelineBox>` nhúng `cached` — khung của chữ chạy không dựng lại các dòng.
//!
//! Hộp chỉ đọc `Player` (`timeline`, `position_sec`), không giữ bản sao của timeline: đoạn đã parse theo thời lượng thật.

use std::rc::Rc;

use groove_core::timeline::{self, Segment};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use materiant::list::{Align, VirtualList, VirtualListHandle};
use materiant::theme::{ActiveTheme, Neutral};

use crate::player::{self, Player};
use crate::views::island::{self, Island};
use crate::views::marquee::Marquee;
use crate::views::playing_bars::PlayingBars;

/// Cao tối thiểu một dòng đoạn (bản cũ `py-1.5` + chữ `text-sm`); tên đoạn dài xuống dòng thì dòng cao thêm.
pub const ROW_HEIGHT: f32 = 34.;
/// Đệm trên / dưới của dòng đoạn (= `py-1.5`).
const ROW_PADDING_Y: f32 = 6.;
const HEADER_HEIGHT: f32 = 36.;
/// timeline-rows (13/11/2026, chủ dự án): dòng tràn hai mép hộp, nền rê / chọn vuông; chữ lùi vào bằng đệm của chính dòng —
/// 12 px thẳng hàng với tiêu đề (`px_3`), bên phải ≥ `TRACK_THICKNESS` (9) để thumb không đè chữ (D-16).
const ROW_INSET: f32 = 12.;

/// Đoạn đang phát lúc `position_sec`.
pub fn active_segment(segments: &[Segment], position_sec: f64) -> Option<usize> {
    timeline::active_index(segments, position_sec)
}

/// Giây cần tua tới khi bấm dòng `ix`: đầu đoạn. Dòng không còn thì `None`.
pub fn jump_target(segments: &[Segment], ix: usize) -> Option<f64> {
    segments.get(ix).map(|segment| segment.start)
}

pub struct TimelineBox {
    player: Entity<Player>,
    list: VirtualListHandle,
    active: Option<usize>,
    /// `lines_key` của lần vẽ trước.
    lines_key: u64,
    /// Danh sách đoạn (`rows_part`), nhúng `cached`.
    rows: Option<Entity<Island<TimelineBox>>>,
    _observe: [Subscription; 2],
}

impl TimelineBox {
    /// Nghe `Player` (bài, timeline, phát / dừng) và nhịp vị trí [`PlaybackClock`](crate::player::PlaybackClock): nhịp chỉ
    /// `notify` khi dòng đang phát đổi (danh sách `cached` nghe hộp nên dựng lại theo).
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> TimelineBox {
        let player = player::player(cx);
        let clock = player.read(cx).clock().clone();
        let observe = [
            cx.observe(&player, |this, _, cx| {
                this.follow(cx);
                cx.notify();
            }),
            cx.observe_in(&clock, window, |this, clock, window, cx| {
                if clock.read(cx).due(window.is_window_active()) && this.follow(cx) {
                    cx.notify();
                }
            }),
        ];
        let mut view = TimelineBox {
            player,
            list: VirtualListHandle::new(),
            active: None,
            lines_key: 0,
            rows: None,
            _observe: observe,
        };
        view.rows = Some(Island::new(TimelineBox::rows_part, cx));
        view.follow(cx);
        view
    }

    /// Theo vị trí: cuộn tới dòng đang phát khi đổi dòng. `true` khi dòng đang phát đổi.
    fn follow(&mut self, cx: &mut Context<Self>) -> bool {
        let active = {
            let player = self.player.read(cx);
            active_segment(player.timeline(), player.position_sec())
        };
        if active == self.active {
            return false;
        }
        self.active = active;
        if let Some(ix) = active {
            self.list.scroll_to(ix, Align::Center);
        }
        true
    }

    #[cfg(test)]
    /// Bài hiện tại có timeline không.
    pub fn has_segments(&self, cx: &App) -> bool {
        !self.player.read(cx).timeline().is_empty()
    }
}

/// Dấu vân của chữ các dòng: khác ⇒ cao dòng có thể khác.
fn lines_key(lines: &[Line]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for line in lines {
        line.time.hash(&mut hasher);
        line.title.hash(&mut hasher);
    }
    hasher.finish()
}

struct Line {
    time: SharedString,
    title: SharedString,
}

impl TimelineBox {
    /// Danh sách đoạn (`Island`, `cached`). Thanh nhảy ở đầu hộp vẽ ngoài phần này.
    fn rows_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lines: Vec<Line> = self
            .player
            .read(cx)
            .timeline()
            .iter()
            .map(|segment| Line {
                time: timeline::format_time(segment.start).into(),
                title: segment.title.clone().into(),
            })
            .collect();
        let c = cx.theme().colors().clone();
        // Đổi bài / sửa timeline ⇒ chữ khác ⇒ đo lại cao dòng (`measured_rows`).
        let key = lines_key(&lines);
        if key != self.lines_key {
            self.lines_key = key;
            self.list.remeasure();
        }
        let lines = Rc::new(lines);
        let count = lines.len();
        let active = self.active;
        let rows = {
            let (lines, primary, text, muted) = (lines.clone(), c.primary_ink, c.text, c.muted);
            move |ix: usize, _: &mut Window, _: &mut App| -> AnyElement {
                let line = &lines[ix];
                let current = active == Some(ix);
                div()
                    .w_full()
                    .min_h(px(ROW_HEIGHT))
                    .px(px(ROW_INSET))
                    .py(px(ROW_PADDING_Y))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(40.))
                            .text_color(if current { primary } else { muted })
                            .child(line.time.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(if current { primary } else { text.opacity(0.85) })
                            .child(line.title.clone()),
                    )
                    .into_any_element()
            }
        };

        let player = self.player.clone();
        let jump = move |ix: &usize, _: &mut Window, cx: &mut App| {
            let Some(start) = jump_target(player.read(cx).timeline(), *ix) else { return };
            player.update(cx, |player, cx| {
                player.seek(start, cx);
                player.play(cx);
            });
        };
        VirtualList::new("timeline-box-rows", count, px(ROW_HEIGHT), rows)
            .measured_rows()
            .selected(active)
            .on_select(jump.clone())
            .on_activate(jump)
            .track(&self.list)
            // D-16: khung cuộn chạm hai mép hộp; dòng cũng tràn hai mép, đệm ngang nằm trong dòng (`ROW_INSET`).
            .padding_x(px(0.))
            .row_radius(px(0.))
            .size_full()
            .py_1()
            .into_any_element()
    }
}

impl Render for TimelineBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, playing) = {
            let player = self.player.read(cx);
            if player.timeline().is_empty() {
                return div().into_any_element();
            }
            let title =
                self.active.and_then(|ix| player.timeline().get(ix)).map(|s| s.title.clone()).unwrap_or_default();
            (title, player.is_playing())
        };
        let c = cx.theme().colors().clone();
        let header = div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(c.border.opacity(0.4))
            // Hộp nhúng thường: thanh nhảy vẽ thẳng, dừng tại chỗ khi nhạc dừng.
            .child(PlayingBars::new(playing).size(px(14.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .text_color(c.primary_ink)
                    .child(Marquee::new("timeline-box-active", title).paused(!playing)),
            );
        let rows = self.rows.as_ref().map(|rows| island::cached(rows, |s| s.w_full().flex_1().min_h_0()));

        div()
            .id("timeline-box")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_xl()
            .border_1()
            .border_color(c.border.opacity(0.4))
            .bg(c.layer(Neutral::N100).opacity(0.5))
            .child(header)
            .children(rows)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, end: f64, title: &str) -> Segment {
        Segment { start, end, title: title.to_owned() }
    }

    #[test]
    fn active_segment_follows_the_position_and_jump_goes_to_its_start() {
        let segments = [seg(0., 65., "A"), seg(65., 200., "B")];
        assert_eq!(active_segment(&segments, 10.), Some(0));
        assert_eq!(active_segment(&segments, 65.), Some(1), "đầu đoạn tính vào đoạn đó");
        assert_eq!(active_segment(&[], 10.), None);
        assert_eq!(jump_target(&segments, 1), Some(65.));
        assert_eq!(jump_target(&segments, 2), None);
    }
}
