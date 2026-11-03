//! groove-gd1_21: hộp đoạn timeline ở cột trái Now Playing (đặc tả 1.5, 1.6).
//!
//! Đầu hộp: bốn thanh nhảy [`PlayingBars`] và `<hiện tại> / <tổng>`. Dưới là từng dòng `<thời gian> <tên>` trong một
//! `VirtualList`; dòng đang phát tô màu nhấn và tự cuộn vào giữa khi đổi đoạn. Bấm dòng (hoặc Enter) là tua tới đầu đoạn
//! và phát. Bài không có timeline thì hộp trống, không chiếm chỗ.
//!
//! Hộp chỉ đọc `Player` (`timeline`, `position_sec`), không giữ bản sao của timeline: đoạn đã parse theo thời lượng thật.

use std::rc::Rc;

use groove_core::t;
use groove_core::timeline::{self, Segment};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use materiant::list::{Align, VirtualList, VirtualListHandle};
use materiant::scroll::TRACK_THICKNESS;
use materiant::theme::{ActiveTheme, Neutral};

use crate::player::{self, Player};
use crate::views::playing_bars::PlayingBars;

/// Cao tối thiểu một dòng đoạn (bản cũ `py-1.5` + chữ `text-sm`); tên đoạn dài xuống dòng thì dòng cao thêm.
pub const ROW_HEIGHT: f32 = 34.;
/// Đệm trên / dưới của dòng đoạn (= `py-1.5`).
const ROW_PADDING_Y: f32 = 6.;
const HEADER_HEIGHT: f32 = 36.;
/// Đệm trái của nội dung dòng; cộng `TRACK_THICKNESS` (9) của danh sách = 12, thẳng hàng với tiêu đề (`px_3`).
const ROW_INSET: f32 = 3.;

/// `<hiện tại> / <tổng>` theo `m:ss` hoặc `h:mm:ss`.
pub fn progress_label(position_sec: f64, duration_sec: f64) -> String {
    t!(
        "now_playing.timeline.current",
        position = timeline::format_time(position_sec),
        duration = timeline::format_time(duration_sec)
    )
}

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
    _observe: Subscription,
}

impl TimelineBox {
    pub fn new(cx: &mut Context<Self>) -> TimelineBox {
        let player = player::player(cx);
        let observe = cx.observe(&player, |this, player, cx| {
            let active = {
                let player = player.read(cx);
                active_segment(player.timeline(), player.position_sec())
            };
            if active != this.active {
                this.active = active;
                if let Some(ix) = active {
                    this.list.scroll_to(ix, Align::Center);
                }
            }
            cx.notify();
        });
        let active = {
            let player = player.read(cx);
            active_segment(player.timeline(), player.position_sec())
        };
        TimelineBox { player, list: VirtualListHandle::new(), active, lines_key: 0, _observe: observe }
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

impl Render for TimelineBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (lines, position, duration, playing) = {
            let player = self.player.read(cx);
            let segments = player.timeline();
            let lines: Vec<Line> = segments
                .iter()
                .map(|segment| Line {
                    time: timeline::format_time(segment.start).into(),
                    title: segment.title.clone().into(),
                })
                .collect();
            (lines, player.position_sec(), player.duration_sec(), player.is_playing())
        };
        if lines.is_empty() {
            return div().into_any_element();
        }
        let c = cx.theme().colors().clone();
        // Đổi bài / sửa timeline ⇒ chữ khác ⇒ đo lại cao dòng (`measured_rows`).
        let key = lines_key(&lines);
        if key != self.lines_key {
            self.lines_key = key;
            self.list.remeasure();
        }
        let lines = Rc::new(lines);
        let count = lines.len();

        let header = div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(c.border.opacity(0.4))
            .child(PlayingBars::new(playing).size(px(14.)))
            .child(div().text_sm().text_color(c.muted).child(progress_label(position, duration)));

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

        div()
            .id("timeline-box")
            // Now Playing nhúng `cached` trong khung `flex_1` + `mt_4` (cpu): gốc lấp đúng khung chủ đưa.
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_xl()
            .border_1()
            .border_color(c.border.opacity(0.4))
            .bg(c.layer(Neutral::N100).opacity(0.5))
            .child(header)
            .child(
                VirtualList::new("timeline-box-rows", count, px(ROW_HEIGHT), rows)
                    .measured_rows()
                    .selected(active)
                    .on_select(jump.clone())
                    .on_activate(jump)
                    .track(&self.list)
                    // D-16: khung cuộn chạm hai mép khung, đệm ngang nằm trong để thumb sát mép phải.
                    .padding_x(TRACK_THICKNESS)
                    .flex_1()
                    .min_h_0()
                    .py_1(),
            )
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
    fn progress_label_uses_clock_format() {
        groove_core::i18n::set_language(groove_core::i18n::Language::En);
        assert_eq!(progress_label(65., 3725.), "1:05 / 1:02:05");
        assert_eq!(progress_label(f64::NAN, -1.), "0:00 / 0:00");
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
