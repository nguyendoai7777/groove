//! groove-gd1_21: cột giữa Now Playing, lời bài hát (đặc tả 1.5, 1.7).
//!
//! Đầu cột: tiêu đề, ba nút viên thuốc Metadata, Timeline, Sửa / Thêm lời (chỉ hiện khi có bài). Thân: không có bài, có lời (các dòng
//! canh giữa, cuộn được, dòng trống cao [`BLANK_LINE_HEIGHT`]) hoặc chưa có lời. Panel không sửa gì: nút bấm phát
//! [`LyricsPanelEvent`] cho `NowPlaying` mở hộp thoại (task 19, 20). Đổi bài thì cuộn về đầu.

use groove_core::model::TrackRef;
use groove_core::t;
use materiant::button::{Button, ButtonColor, Size};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, Context, Entity, EventEmitter, FontWeight, IntoElement, ParentElement, Render, ScrollHandle,
    Styled, Subscription, Window, div, point, px,
};
use materiant::scroll::ScrollArea;
use materiant::theme::{ActiveTheme, Neutral};

use crate::icons::GrooveIcon;
use crate::player::{self, Player};
use crate::views::layout::SurfaceExt;

/// Cao dòng trống giữa hai đoạn lời (đặc tả 1.5).
pub const BLANK_LINE_HEIGHT: f32 = 16.;

/// Nút nào vừa được bấm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricsPanelEvent {
    Metadata,
    Timeline,
    Lyrics,
}

/// Trạng thái thân cột.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    NoSong,
    Empty,
    Lyrics(String),
}

/// Một dòng lời đã chia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricLine<'a> {
    Text(&'a str),
    Blank,
}

/// Chia lời thành dòng: dòng chỉ có khoảng trắng là [`LyricLine::Blank`]; dòng trống ở cuối bỏ đi (không để lại khoảng cuộn
/// thừa).
pub fn lyric_lines(text: &str) -> Vec<LyricLine<'_>> {
    let mut lines: Vec<LyricLine<'_>> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(|line| if line.trim().is_empty() { LyricLine::Blank } else { LyricLine::Text(line.trim_end()) })
        .collect();
    while lines.last() == Some(&LyricLine::Blank) {
        lines.pop();
    }
    lines
}

/// Có lời đáng hiện không (chuỗi toàn khoảng trắng coi như không có).
pub fn has_lyrics(lyrics: Option<&str>) -> bool {
    lyrics.is_some_and(|text| !text.trim().is_empty())
}

pub struct LyricsPanel {
    player: Entity<Player>,
    scroll: ScrollHandle,
    shown: Option<TrackRef>,
    /// Thân đã vẽ: `Player` báo đổi (âm lượng, phát / dừng…) mà thân không đổi thì không dựng lại khung `cached` của lời.
    body: Body,
    in_drawer: bool,
    _observe: Subscription,
}

impl EventEmitter<LyricsPanelEvent> for LyricsPanel {}

/// Viên thuốc như ba nút của `now-playing.vue` (chủ dự án: tonal đậm quá): nền `bg-placeholder/50`, viền `border-hover/30`,
/// chữ 12 px màu nhấn, icon trái 14 px. Dáng text màu chữ theme nên nền rê / nhấn là lớp trung tính, ripple và vòng focus của
/// `Button` giữ nguyên. Dùng chung với nút Lời bài của Now Playing hẹp.
pub fn pill_button(id: &'static str, label: &'static str, icon: GrooveIcon, cx: &App) -> Button {
    let c = cx.theme().colors();
    Button::new(id, label)
        .text()
        .button_color(ButtonColor::Custom(c.text))
        .ink(c.primary_ink)
        .hover_ink(c.primary_ink)
        .size(Size::Sm)
        .icon(icon)
        .rounded_full()
        .h(px(28.))
        .px(px(12.))
        .text_size(px(12.))
        .bg(c.layer(Neutral::N200).opacity(0.5))
        .border_color(c.border.opacity(0.3))
}

impl LyricsPanel {
    pub fn new(cx: &mut Context<Self>) -> LyricsPanel {
        let player = player::player(cx);
        let observe = cx.observe(&player, |this, player, cx| {
            let now = player.read(cx).current_track().cloned();
            if now != this.shown {
                this.shown = now;
                this.scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
            let body = this.body(cx);
            if body != this.body {
                this.body = body;
                cx.notify();
            }
        });
        let shown = player.read(cx).current_track().cloned();
        let mut panel = LyricsPanel {
            player,
            scroll: ScrollHandle::new(),
            shown,
            body: Body::NoSong,
            in_drawer: false,
            _observe: observe,
        };
        panel.body = panel.body(cx);
        panel
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: GrooveIcon,
        event: LyricsPanelEvent,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let this = cx.entity();
        pill_button(id, label, icon, cx)
            .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| this.update(cx, |_, cx| cx.emit(event)))
    }

    /// Đang vẽ trong drawer (Now Playing hẹp): bỏ khung thẻ, bề mặt drawer là khung.
    pub fn set_in_drawer(&mut self, in_drawer: bool, cx: &mut Context<Self>) {
        if self.in_drawer != in_drawer {
            self.in_drawer = in_drawer;
            cx.notify();
        }
    }

    fn body(&self, cx: &App) -> Body {
        let player = self.player.read(cx);
        if player.current_track().is_none() {
            return Body::NoSong;
        }
        match player.lyrics().filter(|text| has_lyrics(Some(text))) {
            Some(text) => Body::Lyrics(text.to_owned()),
            None => Body::Empty,
        }
    }
}

impl LyricsPanel {
    #[cfg(test)]
    /// Thân cột lúc này (cho test).
    pub fn state(&self, cx: &App) -> Body {
        self.body(cx)
    }
}

impl Render for LyricsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let body = self.body(cx);
        let has_song = !matches!(body, Body::NoSong);
        let has_text = matches!(body, Body::Lyrics(_));

        let header = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_4()
            .pt_2()
            .pb_2()
            .child(div().text_lg().font_weight(FontWeight::NORMAL).child(t!("now_playing.lyrics.title")))
            .when(has_song, |row| {
                let edit =
                    if has_text { t!("now_playing.actions.edit_lyrics") } else { t!("now_playing.actions.add_lyrics") };
                row.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(self.button(
                            "np-metadata",
                            t!("now_playing.actions.metadata"),
                            GrooveIcon::Album,
                            LyricsPanelEvent::Metadata,
                            cx,
                        ))
                        .child(self.button(
                            "np-timeline",
                            t!("now_playing.actions.timeline"),
                            GrooveIcon::Loop,
                            LyricsPanelEvent::Timeline,
                            cx,
                        ))
                        .child(self.button("np-lyrics", edit, GrooveIcon::Edit, LyricsPanelEvent::Lyrics, cx)),
                )
            });

        let message = |text: &'static str| {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px_6()
                .text_center()
                .text_color(c.muted)
                .child(div().w_full().child(text))
        };
        let content = match body {
            Body::NoSong => message(t!("now_playing.lyrics.no_song")).into_any_element(),
            Body::Empty => message(t!("now_playing.lyrics.empty")).into_any_element(),
            Body::Lyrics(text) => ScrollArea::new("np-lyrics-scroll")
                .vertical()
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0()
                .child(div().w_full().px_4().py_4().flex().flex_col().gap_1().children(
                    lyric_lines(&text).into_iter().map(|line| match line {
                        // Cỡ chữ tên bài của hàng chờ bên phải (`text_sm`), căn trái (chủ dự án 03/11/2026).
                        LyricLine::Text(line) => div().w_full().text_sm().child(line.to_owned()),
                        LyricLine::Blank => div().w_full().h(px(BLANK_LINE_HEIGHT)),
                    }),
                ))
                .into_any_element(),
        };

        div()
            .id("lyrics-panel")
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(!self.in_drawer, |panel| panel.region_surface(cx))
            .child(header)
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_lines_keep_their_place_but_the_tail_is_trimmed() {
        let lines = lyric_lines("Một\n\n  \nHai  \n\n\n");
        assert_eq!(lines, [LyricLine::Text("Một"), LyricLine::Blank, LyricLine::Blank, LyricLine::Text("Hai")]);
        assert!(lyric_lines("\n \n").is_empty());
        assert_eq!(lyric_lines("\u{feff}Mở đầu"), [LyricLine::Text("Mở đầu")]);
    }

    #[test]
    fn whitespace_only_lyrics_count_as_none() {
        assert!(has_lyrics(Some("a")));
        assert!(!has_lyrics(Some(" \n\t")));
        assert!(!has_lyrics(Some("")));
        assert!(!has_lyrics(None));
    }
}
