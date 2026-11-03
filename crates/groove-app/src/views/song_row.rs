//! groove-gd1_16: dòng bài của danh sách (đặc tả 1.2, bản cũ `music-list.vue`).
//!
//! Bốn cột: `#`, tiêu đề, thời lượng, nghệ sĩ (ẩn khi cửa sổ hẹp). Cột `#`: bài khác hiện số, rê chuột hiện Play; bài
//! đang chọn hiện bars (chạy khi đang phát), rê chuột hiện Pause / Play. Dòng chỉ vẽ và báo chuột phải: nhấp, phím, hàng
//! đợi là việc của `song_list.rs`; khung dòng (cao, nền rê / chọn, nhấp) do `materiant::list::VirtualList` lo.

use std::rc::Rc;

use groove_core::model::Song;
use groove_core::t;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use materiant::icon::Icon;
use materiant::theme::{ActiveTheme, ThemeColors};

use crate::icons::GrooveIcon;
use crate::views::playing_bars::PlayingBars;

/// Cao tối thiểu một dòng (bản cũ `py-3` + chữ `text-sm`); chữ dài xuống dòng thì dòng cao thêm (`measured_rows`).
pub const ROW_HEIGHT: f32 = 44.;
/// Đệm trên / dưới của dòng (= `py-3`).
const ROW_PADDING_Y: f32 = 12.;
/// Cao một hàng chữ của dòng: ô `#` cao đúng chừng này để số / bars / Play nằm ngang hàng chữ đầu.
const LINE_HEIGHT: f32 = ROW_HEIGHT - 2. * ROW_PADDING_Y;
pub const INDEX_WIDTH: f32 = 48.;
pub const DURATION_WIDTH: f32 = 96.;
/// Cột nghệ sĩ ẩn dưới bề ngang cửa sổ này (bản cũ `hidden md:table-cell`, `md` = 768 px của Tailwind).
pub const ARTIST_MIN_WINDOW_WIDTH: f32 = 768.;
const CELL_PADDING: f32 = 16.;
const TEXT_SIZE: f32 = 14.;
const GROUP: &str = "song-row";

/// Cột nghệ sĩ có hiện không, theo bề ngang cửa sổ.
pub fn show_artist(window_width: f32) -> bool {
    window_width >= ARTIST_MIN_WINDOW_WIDTH
}

/// Tiêu đề rỗng thì dùng tên tệp bỏ đuôi (đặc tả 1.2).
pub fn display_title(song: &Song) -> String {
    match song.title.as_deref().map(str::trim) {
        Some(title) if !title.is_empty() => title.to_owned(),
        _ => strip_extension(&song.filename).to_owned(),
    }
}

/// Bỏ đuôi cuối (`a.b.mp3` → `a.b`); tên chỉ có dấu chấm đầu (`.mp3`) giữ nguyên, như `replace(/\.[^/.]+$/, '')` trừ
/// trường hợp tên rỗng sau khi bỏ.
fn strip_extension(filename: &str) -> &str {
    match filename.rfind('.') {
        Some(0) | None => filename,
        Some(dot) => &filename[..dot],
    }
}

/// Nghệ sĩ rỗng hiện `—`.
pub fn display_artist(song: &Song) -> String {
    match song.artist.as_deref().map(str::trim) {
        Some(artist) if !artist.is_empty() => artist.to_owned(),
        _ => "—".to_owned(),
    }
}

/// `m:ss`, từ một giờ thì `h:mm:ss`. Số âm, NaN, vô cực coi như 0; làm tròn xuống như `Math.floor` của bản cũ.
pub fn format_duration(sec: f64) -> String {
    let total = if sec.is_finite() && sec > 0. { sec as u64 } else { 0 };
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{}:{seconds:02}", total / 60) }
}

/// Dòng này đang ở đâu so với player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowPlay {
    /// Không phải bài đang chọn.
    Other,
    /// Bài đang chọn và đang phát.
    Playing,
    /// Bài đang chọn nhưng dừng.
    Paused,
}

/// Chuột phải trên dòng: vị trí con trỏ (toạ độ cửa sổ).
pub type ContextHandler = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SongRow {
    number: usize,
    title: SharedString,
    duration: SharedString,
    artist: SharedString,
    play: RowPlay,
    show_artist: bool,
    on_context: Option<ContextHandler>,
}

impl SongRow {
    /// `number` đếm từ 1.
    pub fn new(song: &Song, number: usize, play: RowPlay, show_artist: bool) -> SongRow {
        SongRow {
            number,
            title: display_title(song).into(),
            duration: format_duration(song.duration_sec).into(),
            artist: display_artist(song).into(),
            play,
            show_artist,
            on_context: None,
        }
    }

    pub fn on_context(mut self, handler: ContextHandler) -> SongRow {
        self.on_context = Some(handler);
        self
    }
}

/// Ô của một cột: lề cân hai bên. Chữ dài xuống dòng, không cắt "…" (chủ dự án 03/11/2026, như Apple Music).
fn cell() -> gpui::Div {
    div().px(px(CELL_PADDING)).min_w_0()
}

/// Dòng tiêu đề cột (`#`, tiêu đề, thời lượng, nghệ sĩ) cùng bố cục với [`SongRow`].
pub fn header_row(show_artist: bool, c: &ThemeColors) -> AnyElement {
    let label = |text: &'static str| cell().text_size(px(12.)).text_color(c.muted.opacity(0.8)).child(text);
    div()
        .flex_none()
        .w_full()
        .h(px(36.))
        .flex()
        .items_center()
        .child(label(t!("song_list.column.index")).w(px(INDEX_WIDTH)).flex_none().text_center())
        .child(label(t!("song_list.column.title")).flex_1())
        .child(label(t!("song_list.column.duration")).w(px(DURATION_WIDTH)).flex_none())
        .when(show_artist, |row| row.child(label(t!("song_list.column.artist")).flex_1()))
        .into_any_element()
}

impl SongRow {
    fn index_cell(&self, c: &ThemeColors) -> impl IntoElement + use<> {
        let accent = c.primary_ink;
        let hover_icon = || Icon::new(GrooveIcon::Play).size(px(12.)).color(accent);
        let stack =
            div().w(px(INDEX_WIDTH)).flex_none().h(px(LINE_HEIGHT)).relative().flex().items_center().justify_center();
        let rest = |child: AnyElement| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .group_hover(GROUP, |s| s.invisible())
                .child(child)
        };
        let on_hover = |child: AnyElement| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .invisible()
                .group_hover(GROUP, |s| s.visible())
                .child(child)
        };
        match self.play {
            RowPlay::Other => stack
                .child(rest(
                    div()
                        .text_size(px(12.))
                        .text_color(c.muted.opacity(0.8))
                        .child(self.number.to_string())
                        .into_any_element(),
                ))
                .child(on_hover(hover_icon().into_any_element())),
            RowPlay::Playing => stack
                .child(rest(PlayingBars::new(true).size(px(14.)).into_any_element()))
                .child(on_hover(Icon::new(GrooveIcon::Pause).size(px(12.)).color(accent).into_any_element())),
            RowPlay::Paused => stack
                .child(rest(PlayingBars::new(false).size(px(14.)).into_any_element()))
                .child(on_hover(hover_icon().into_any_element())),
        }
    }
}

impl RenderOnce for SongRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let current = self.play != RowPlay::Other;
        let title_color = if current { c.primary_ink } else { c.text.opacity(0.85) };
        let index = self.index_cell(&c);
        let on_context = self.on_context.clone();
        let number = self.number;
        div()
            .group(GROUP)
            .debug_selector(move || format!("song-row-{number}"))
            // Cao theo chữ: dòng ngắn đủ `ROW_HEIGHT`, chữ dài xuống dòng; các cột canh theo hàng chữ đầu.
            .w_full()
            .min_h(px(ROW_HEIGHT))
            .py(px(ROW_PADDING_Y))
            .flex()
            .items_start()
            .text_size(px(TEXT_SIZE))
            .when_some(on_context, |row, handler| {
                row.on_mouse_down(MouseButton::Right, move |event: &MouseDownEvent, window, cx| {
                    handler(event.position, window, cx);
                })
            })
            .child(index)
            .child(cell().flex_1().text_color(title_color).child(self.title))
            .child(cell().w(px(DURATION_WIDTH)).flex_none().text_color(c.muted).child(self.duration))
            .when(self.show_artist, |row| row.child(cell().flex_1().text_color(c.muted).child(self.artist)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: Option<&str>, artist: Option<&str>, filename: &str) -> Song {
        let mut song = Song::external(format!("/music/{filename}"));
        song.title = title.map(str::to_owned);
        song.artist = artist.map(str::to_owned);
        song.filename = filename.to_owned();
        song
    }

    #[test]
    fn empty_title_falls_back_to_filename_without_extension() {
        assert_eq!(display_title(&song(Some("Yesterday"), None, "x.mp3")), "Yesterday");
        assert_eq!(display_title(&song(None, None, "Let It Be.mp3")), "Let It Be");
        assert_eq!(display_title(&song(Some("   "), None, "a.b.flac")), "a.b");
        assert_eq!(display_title(&song(Some(""), None, "noext")), "noext");
        assert_eq!(display_title(&song(None, None, ".hidden")), ".hidden");
    }

    #[test]
    fn empty_artist_shows_a_dash() {
        assert_eq!(display_artist(&song(None, Some("Queen"), "a.mp3")), "Queen");
        assert_eq!(display_artist(&song(None, None, "a.mp3")), "—");
        assert_eq!(display_artist(&song(None, Some("  "), "a.mp3")), "—");
    }

    #[test]
    fn duration_formats_minutes_and_hours() {
        assert_eq!(format_duration(0.), "0:00");
        assert_eq!(format_duration(59.9), "0:59");
        assert_eq!(format_duration(61.), "1:01");
        assert_eq!(format_duration(600.), "10:00");
        assert_eq!(format_duration(3599.), "59:59");
        assert_eq!(format_duration(3600.), "1:00:00");
        assert_eq!(format_duration(3725.), "1:02:05");
        assert_eq!(format_duration(-3.), "0:00");
        assert_eq!(format_duration(f64::NAN), "0:00");
        assert_eq!(format_duration(f64::INFINITY), "0:00");
    }

    #[test]
    fn artist_column_hides_below_768() {
        assert!(!show_artist(560.));
        assert!(!show_artist(767.9));
        assert!(show_artist(768.));
    }
}
