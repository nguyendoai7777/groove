//! groove-gd1_21: bảng hàng đợi ở cột phải Now Playing (đặc tả 1.4, 1.5; bản cũ `playlist-history.vue`).
//!
//! Hiển thị = lịch sử + bài hiện tại + hàng đợi (`Queue::visible`). Dòng đã phát mờ (25 %, rê chuột 50 %), dòng hiện tại có
//! [`PlayingBars`] và chữ màu nhấn, tự cuộn vào giữa khi đổi bài trừ khi người dùng vừa bấm ngay trong bảng. Bấm một dòng
//! là phát bài đó (`Player::play_track`, không kèm danh sách, bảng 1.4); bấm dòng hiện tại thì đảo phát / dừng như nút play
//! của thanh phát (`Player::toggle_play`, phát tiếp từ vị trí cũ). Phím mũi tên chỉ dời con trỏ, Enter mới phát / đảo.
//!
//! `Queue` chỉ giữ `TrackRef`, nên tên bài tra qua `Library::songs_for_tracks` ở luồng nền (UI không chạm đĩa, N7), nhớ
//! theo id: đổi thứ tự (shuffle) không phải tra lại. Chỉ tra phần chưa có.
//!
//! groove-gd2_03: chuột phải một dòng (hoặc Shift+F10) mở menu Lyrics / Timeline / Metadata / Save as new; panel chỉ phát
//! [`QueuePanelEvent`], `NowPlaying` mở đúng các hộp mà ba nút ở khung Lyrics đang mở.

use std::collections::HashMap;
use std::rc::Rc;

use groove_core::model::{Song, TrackRef};
use groove_core::t;
use groove_library::store::Library;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FontWeight, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Render, SharedString, Styled, Subscription, Task, WeakEntity, Window, div, px,
};
use materiant::list::{Align, VirtualList, VirtualListHandle};
use materiant::menu::{ContextMenuExt, Menu, MenuItem};
use materiant::scroll::TRACK_THICKNESS;
use materiant::theme::ActiveTheme;

use crate::player::{self, Player};
use crate::views::bars_overlay;
use crate::views::layout::SurfaceExt;
use crate::views::playing_bars::PlayingBars;
use crate::views::song_row::{display_artist, display_title};

/// Cao tối thiểu của một dòng (tên bài + nghệ sĩ một dòng); tên dài xuống dòng thì dòng cao thêm (`measured_rows`).
pub const ROW_HEIGHT: f32 = 52.;
/// Đệm trên / dưới chữ của dòng: dòng xuống nhiều hàng vẫn thoáng như dòng thường.
const ROW_PADDING_Y: f32 = 8.;
const INDEX_WIDTH: f32 = 28.;
/// Đệm trái của nội dung dòng; cộng `TRACK_THICKNESS` (9) của danh sách = 16, thẳng hàng với tiêu đề (`px_4`).
const ROW_INSET: f32 = 7.;

/// Chữ của một dòng.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub title: SharedString,
    pub artist: SharedString,
}

/// Dòng của một bài đã biết thông tin (tên rỗng thì lấy tên tệp, nghệ sĩ rỗng là `—`, như danh sách bài).
pub fn row_of(song: &Song) -> Row {
    Row { title: display_title(song).into(), artist: display_artist(song).into() }
}

/// Dòng cho từng bài hiển thị. Bài thư viện chưa tra xong là `None`; tệp ngoài biết ngay từ đường dẫn.
pub fn build_rows(tracks: &[TrackRef], cache: &HashMap<i64, Row>) -> Vec<Option<Row>> {
    tracks
        .iter()
        .map(|track| match track {
            TrackRef::Library(id) => cache.get(id).cloned(),
            TrackRef::External(path) => Some(row_of(&Song::external(path))),
        })
        .collect()
}

/// Bài thư viện chưa có trong `cache`, mỗi bài một lần.
pub fn missing_tracks(tracks: &[TrackRef], cache: &HashMap<i64, Row>) -> Vec<TrackRef> {
    let mut seen = std::collections::HashSet::new();
    tracks
        .iter()
        .filter(|track| matches!(track, TrackRef::Library(id) if !cache.contains_key(id) && seen.insert(*id)))
        .cloned()
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Played,
    Current,
    Upcoming,
}

/// Dòng `ix` so với bài hiện tại. Không có bài hiện tại thì mọi dòng còn chờ.
pub fn row_state(ix: usize, current: Option<usize>) -> RowState {
    match current {
        Some(current) if ix < current => RowState::Played,
        Some(current) if ix == current => RowState::Current,
        _ => RowState::Upcoming,
    }
}

/// `(<n> tracks)`.
pub fn count_label(count: usize) -> String {
    if count == 1 { t!("now_playing.queue.count_one").to_owned() } else { t!("now_playing.queue.count", count = count) }
}

/// Đổi bài thì cuộn vào giữa, trừ lần đổi do chính người dùng bấm trong bảng (cờ dùng một lần).
pub fn take_center(skip_next: &mut bool) -> bool {
    !std::mem::take(skip_next)
}

/// Mục của menu chuột phải trên một dòng hàng đợi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueMenuAction {
    Lyrics,
    Timeline,
    Metadata,
    SaveAs,
}

/// Người dùng chọn một mục menu cho bài `TrackRef`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuePanelEvent(pub QueueMenuAction, pub TrackRef);

pub struct QueuePanel {
    player: Entity<Player>,
    library: Option<Library>,
    tracks: Rc<Vec<TrackRef>>,
    cache: HashMap<i64, Row>,
    rows: Rc<Vec<Option<Row>>>,
    current: Option<usize>,
    /// `is_playing` ở lần đồng bộ trước: thanh nhảy của dòng hiện tại chạy / đứng theo nó.
    playing: bool,
    /// Con trỏ bàn phím; `None` thì tô dòng hiện tại.
    cursor: Option<usize>,
    /// Dòng vừa chuột phải: menu dùng bài của dòng này.
    menu_row: Option<usize>,
    skip_center: bool,
    /// Lần mở màn Now Playing (kể cả lần dựng đầu) chờ đưa bài đang phát lên đỉnh khung: chưa có bài thì đợi bài đầu.
    reveal_pending: bool,
    list: VirtualListHandle,
    load_task: Option<Task<()>>,
    _observe: Subscription,
}

impl EventEmitter<QueuePanelEvent> for QueuePanel {}

impl QueuePanel {
    pub fn new(library: Option<Library>, cx: &mut Context<Self>) -> QueuePanel {
        let player = player::player(cx);
        let observe = cx.observe(&player, |this, _, cx| this.sync(cx));
        let mut panel = QueuePanel {
            player,
            library,
            tracks: Rc::default(),
            cache: HashMap::new(),
            rows: Rc::default(),
            current: None,
            playing: false,
            cursor: None,
            menu_row: None,
            skip_center: false,
            reveal_pending: true,
            list: VirtualListHandle::new(),
            load_task: None,
            _observe: observe,
        };
        panel.sync(cx);
        panel
    }

    #[cfg(test)]
    /// Tay cầm danh sách (cho test: đọc / đặt vị trí cuộn).
    pub fn list(&self) -> &VirtualListHandle {
        &self.list
    }

    #[cfg(test)]
    /// Số bài đang hiển thị (cho test).
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    #[cfg(test)]
    /// Dòng đã dựng, `None` là chưa tra xong (cho test).
    pub fn rows(&self) -> &[Option<Row>] {
        &self.rows
    }

    #[cfg(test)]
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// Mở màn Now Playing: bài đang phát lên sát mép trên khung (queue-scroll-top, 03/11/2026). Chưa có bài thì để dành cho
    /// bài đầu tiên.
    pub fn reveal_current(&mut self, cx: &mut Context<Self>) {
        match self.current {
            Some(ix) => {
                self.reveal_pending = false;
                self.list.scroll_to(ix, Align::Start);
                cx.notify();
            }
            None => self.reveal_pending = true,
        }
    }

    /// Đồng bộ với hàng đợi của `Player` mỗi khi nó báo đổi (so sánh, không sao chép). Chỉ `notify` khi thứ panel vẽ đổi:
    /// panel nằm trong khung `cached` của Now Playing, `notify` thừa là dựng lại cả danh sách.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let (same, current, current_song, playing) = {
            let player = self.player.read(cx);
            let queue = player.queue();
            (
                queue.visible().eq(self.tracks.iter()),
                queue.current_index(),
                player.current_song().cloned(),
                player.is_playing(),
            )
        };
        let mut dirty = false;
        let mut changed = playing != self.playing;
        self.playing = playing;
        if !same {
            let tracks: Vec<TrackRef> = self.player.read(cx).queue().visible().cloned().collect();
            self.tracks = Rc::new(tracks);
            self.cursor = None;
            dirty = true;
        }
        // Sửa thẻ bài đang phát: `Player::refresh_current` đổi `current_song`, hàng đợi không có cách nào biết khác.
        if let Some(song) = current_song
            && let TrackRef::Library(id) = song.track
        {
            let row = row_of(&song);
            if self.cache.get(&id) != Some(&row) {
                self.cache.insert(id, row);
                dirty = true;
            }
        }
        if dirty {
            self.rows = Rc::new(build_rows(&self.tracks, &self.cache));
            // Chữ đổi ⇒ cao dòng đổi (tên dài xuống dòng).
            self.list.remeasure();
            self.load_missing(cx);
            changed = true;
        }
        if current != self.current {
            changed = true;
            self.current = current;
            self.cursor = None;
            if let Some(ix) = current {
                if std::mem::take(&mut self.reveal_pending) {
                    self.skip_center = false;
                    self.list.scroll_to(ix, Align::Start);
                } else if take_center(&mut self.skip_center) {
                    self.list.scroll_to(ix, Align::Center);
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn load_missing(&mut self, cx: &mut Context<Self>) {
        let missing = missing_tracks(&self.tracks, &self.cache);
        if missing.is_empty() {
            self.load_task = None;
            return;
        }
        let Some(library) = self.library.clone() else { return };
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let found = library.songs_for_tracks(missing.clone()).await;
            let _ = this.update(cx, |this, cx| {
                let Ok(songs) = found else { return };
                for song in &songs {
                    if let TrackRef::Library(id) = song.track {
                        this.cache.insert(id, row_of(song));
                    }
                }
                // Bài đã xoá khỏi thư viện: dòng "không rõ", không tra lại mãi.
                for track in &missing {
                    if let TrackRef::Library(id) = track {
                        this.cache.entry(*id).or_insert_with(|| Row {
                            title: t!("now_playing.queue.unknown").into(),
                            artist: "".into(),
                        });
                    }
                }
                this.rows = Rc::new(build_rows(&this.tracks, &this.cache));
                this.list.remeasure();
                cx.notify();
            });
        }));
    }

    /// Phát bài ở dòng `ix`; dòng hiện tại thì đảo phát / dừng (không phát lại từ đầu). `from_click`: bấm ngay trong bảng
    /// thì lần đổi bài kế không kéo bảng đi chỗ khác.
    fn play_at(&mut self, ix: usize, from_click: bool, cx: &mut Context<Self>) {
        let Some(track) = self.tracks.get(ix).cloned() else { return };
        if Some(ix) == self.current {
            self.player.update(cx, |player, cx| player.toggle_play(cx));
            return;
        }
        if from_click {
            self.skip_center = true;
        }
        self.player.update(cx, |player, cx| player.play_track(track, cx));
    }

    /// Chuột phải dòng `ix`: dời con trỏ tới đó, menu (dựng sau, ở khung bọc danh sách) đọc [`Self::menu_track`].
    fn right_click_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tracks.len() {
            self.menu_row = Some(ix);
            self.cursor = Some(ix);
            cx.notify();
        }
    }

    /// Bài của menu: dòng vừa chuột phải, không thì dòng con trỏ (Shift+F10), không thì bài hiện tại.
    pub fn menu_track(&self) -> Option<TrackRef> {
        let ix = self.menu_row.or(self.cursor).or(self.current)?;
        self.tracks.get(ix).cloned()
    }

    /// Bấm dòng `ix` bằng chuột.
    pub fn click_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.play_at(ix, true, cx);
    }
}

impl Render for QueuePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        bars_overlay::forget(cx.entity_id(), cx);
        let c = cx.theme().colors().clone();
        let playing = self.playing;
        let count = self.tracks.len();
        let current = self.current;
        let this = cx.entity();

        let header = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_baseline()
            .gap_2()
            .px_4()
            .pt_3()
            .pb_2()
            .child(div().text_lg().font_weight(FontWeight::NORMAL).child(t!("now_playing.queue.title")))
            .child(div().text_sm().text_color(c.muted).child(count_label(count)));

        let body: AnyElement = if count == 0 {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px_6()
                .text_center()
                .text_color(c.muted)
                .child(div().w_full().child(t!("now_playing.queue.empty")))
                .into_any_element()
        } else {
            let rows = self.rows.clone();
            let render_row = {
                let (this, c) = (this.clone(), c.clone());
                move |ix: usize, _: &mut Window, _: &mut App| -> AnyElement {
                    let state = row_state(ix, current);
                    let row = rows.get(ix).cloned().flatten();
                    let (title, artist) = match row {
                        Some(row) => (row.title, row.artist),
                        None => (t!("now_playing.queue.loading").into(), SharedString::default()),
                    };
                    let is_current = state == RowState::Current;
                    let on_click = {
                        let this = this.clone();
                        move |event: &materiant::gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                            // Nhấp đúp: lần nhấp đầu đã phát / đảo rồi, lần hai không đảo ngược lại.
                            if event.click_count() > 1 {
                                return;
                            }
                            this.update(cx, |panel, cx| panel.click_row(ix, cx));
                        }
                    };
                    let index_cell = div().w(px(INDEX_WIDTH)).flex_none().flex().items_center().justify_center().child(
                        if is_current {
                            if playing {
                                // Vẽ ở lớp phủ: panel nằm trong khung `cached` của Now Playing.
                                bars_overlay::anchor(px(14.)).into_any_element()
                            } else {
                                PlayingBars::new(false).size(px(14.)).into_any_element()
                            }
                        } else {
                            div().text_xs().text_color(c.muted).child((ix + 1).to_string()).into_any_element()
                        },
                    );
                    div()
                        .id(("np-queue-row", ix))
                        // Cao theo chữ (`measured_rows`), không `size_full`: dòng ngắn vẫn đủ `ROW_HEIGHT`, chữ giữa dòng.
                        .w_full()
                        .min_h(px(ROW_HEIGHT))
                        .px(px(ROW_INSET))
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(state == RowState::Played, |d| d.opacity(0.25).hover(|s| s.opacity(0.5)))
                        .on_click(on_click)
                        .debug_selector(move || format!("np-queue-row-{ix}"))
                        .on_mouse_down(MouseButton::Right, {
                            let this = this.clone();
                            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                                this.update(cx, |panel, cx| panel.right_click_row(ix, cx));
                            }
                        })
                        .child(index_cell)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .py(px(ROW_PADDING_Y))
                                .flex()
                                .flex_col()
                                // Không cắt "…": tên dài xuống dòng như Apple Music, dòng cao theo chữ.
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if is_current { c.primary_ink } else { c.text.opacity(0.9) })
                                        .child(title),
                                )
                                .child(div().text_xs().text_color(c.muted).child(artist)),
                        )
                        .into_any_element()
                }
            };
            let (select, activate) = (this.clone(), this.clone());
            let list = VirtualList::new("np-queue-rows", count, px(ROW_HEIGHT), render_row)
                .measured_rows()
                // Space là phím phát / dừng toàn cục: không để nó bật vòng focus quanh hàng đợi.
                .focus_ring(false)
                .selected(self.cursor.or(current))
                .on_select(move |ix: &usize, _, cx| {
                    let ix = *ix;
                    select.update(cx, |panel, cx| {
                        panel.cursor = Some(ix);
                        cx.notify();
                    });
                })
                // Enter phát / đảo; nhấp đúp thì `on_click` của dòng đã xử lý lần nhấp đầu.
                .on_activate(move |ix: &usize, window, cx| {
                    if window.last_input_was_keyboard() {
                        let ix = *ix;
                        activate.update(cx, |panel, cx| panel.play_at(ix, true, cx));
                    }
                })
                .track(&self.list)
                // D-16: khung cuộn chạm hai mép khung bảng, đệm ngang nằm trong (`padding_x`) để thumb sát mép phải.
                .padding_x(TRACK_THICKNESS)
                .flex_1()
                .min_h_0()
                .py_1();
            // Menu chuột phải: dòng đã ghi ở `on_mouse_down` (con chạy trước), hoặc Shift+F10 khi focus ở danh sách.
            let menu_panel = this.downgrade();
            div()
                .id("np-queue-host")
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(list)
                .context_menu(move |menu, _, cx| queue_menu(menu, &menu_panel, cx))
                .into_any_element()
        };

        div()
            .id("queue-panel")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .region_surface(cx)
            .child(header)
            .child(body)
    }
}

/// Menu của một dòng: bốn mục giống ba nút ở khung Lyrics + "Save as new". Chưa có bài thì rỗng.
fn queue_menu(menu: Menu, panel: &WeakEntity<QueuePanel>, cx: &App) -> Menu {
    let Some(track) = panel.upgrade().and_then(|p| p.read(cx).menu_track()) else { return menu };
    let item = |id: &'static str, label: &'static str, action: QueueMenuAction| {
        let (panel, track) = (panel.clone(), track.clone());
        MenuItem::new(id, label).on_select(move |_, cx| {
            // Hoãn: `on_select` chạy lúc cửa sổ đang bị mượn, `subscribe_in` của `NowPlaying` cần cửa sổ.
            let (panel, track) = (panel.clone(), track.clone());
            cx.defer(move |cx| {
                panel.update(cx, |_, cx| cx.emit(QueuePanelEvent(action, track))).ok();
            });
        })
    };
    menu.item(item("np-queue-lyrics", t!("actions.song.edit_lyrics"), QueueMenuAction::Lyrics))
        .item(item("np-queue-timeline", t!("actions.song.edit_timeline"), QueueMenuAction::Timeline))
        .item(item("np-queue-metadata", t!("actions.song.edit_metadata"), QueueMenuAction::Metadata))
        .separator()
        .item(item("np-queue-save-as", t!("save_as.menu"), QueueMenuAction::SaveAs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str) -> Row {
        Row { title: title.to_owned().into(), artist: "—".into() }
    }

    #[test]
    fn rows_resolve_from_cache_and_external_paths() {
        let cache = HashMap::from([(1, row("Một"))]);
        let tracks = [TrackRef::Library(1), TrackRef::Library(2), TrackRef::External("/nhac/Ngoai.mp3".into())];
        let rows = build_rows(&tracks, &cache);
        assert_eq!(rows[0], Some(row("Một")));
        assert_eq!(rows[1], None, "chưa tra xong");
        assert_eq!(rows[2].as_ref().unwrap().title, "Ngoai");
    }

    #[test]
    fn only_unknown_library_tracks_are_looked_up_once() {
        let cache = HashMap::from([(1, row("Một"))]);
        let tracks = [
            TrackRef::Library(1),
            TrackRef::Library(2),
            TrackRef::Library(2),
            TrackRef::External("a.mp3".into()),
            TrackRef::Library(3),
        ];
        assert_eq!(missing_tracks(&tracks, &cache), [TrackRef::Library(2), TrackRef::Library(3)]);
    }

    #[test]
    fn rows_before_the_current_one_are_played() {
        assert_eq!(row_state(0, Some(2)), RowState::Played);
        assert_eq!(row_state(2, Some(2)), RowState::Current);
        assert_eq!(row_state(3, Some(2)), RowState::Upcoming);
        assert_eq!(row_state(0, None), RowState::Upcoming);
    }

    #[test]
    fn click_suppresses_exactly_one_recenter() {
        let mut skip = true;
        assert!(!take_center(&mut skip));
        assert!(take_center(&mut skip), "lần đổi bài sau (tự sang bài) lại cuộn vào giữa");
    }

    #[test]
    fn count_label_handles_one_and_many() {
        groove_core::i18n::set_language(groove_core::i18n::Language::En);
        assert_eq!(count_label(1), "(1 track)");
        assert_eq!(count_label(0), "(0 tracks)");
        assert_eq!(count_label(12), "(12 tracks)");
    }
}
