//! groove-gd1_21: màn Now Playing (đặc tả 1.5; bản cũ `pages/now-playing/now-playing.vue`).
//!
//! Ba cột cao đầy: trái (25 %, 180-300 px) bìa + tên + nghệ sĩ + [`TimelineBox`]; giữa [`LyricsPanel`]; phải (32 %, 240-340 px)
//! [`QueuePanel`]. Phía sau là quầng màu theo accent của bìa (cao 450 px, độ đục 0,3). `backdrop-blur` của bản cũ không có ở
//! GPUI: các khung dùng nền bán trong suốt thay (bảng thay thế đặc tả mục 4); quầng đổi màu tức thì, không có chuyển 700 ms.
//!
//! Màn này không tự ghi gì: ba nút Metadata, Timeline, Sửa / Thêm lời của `LyricsPanel` mở [`MetadataDialog`] và hai
//! [`TextDialog`] (task 19, 20) cho bài đang phát; menu chuột phải của hàng đợi mở chính các hộp đó (và hộp "Save as new",
//! groove-gd2_03) cho bài của dòng. Hộp lyrics, timeline tự đóng khi đổi bài, lưu xong tự nạp lại bài.
//! Tiêu đề cửa sổ là `<bài> - <nghệ sĩ> | Groovex` khi đang ở màn này.
//!
//! Responsive (chủ dự án 03/11/2026; bản web không có): màn hẹp hơn [`NARROW_WIDTH`] thì ẩn cột lời bài, cột hàng đợi giãn ra;
//! nút Lời bài dưới tên nghệ sĩ mở chính [`LyricsPanel`] (cả ba nút Metadata, Timeline, Sửa lời) trong drawer phải
//! (`Dialog::drawer` của materiant: Esc, bấm nền, bẫy Tab). Bề ngang đo lúc `prepaint` (canvas phủ màn); đổi mức thì xin khung
//! mới. Rộng ra lại khi drawer đang mở thì drawer đóng, cột hiện lại.
//!
//! Accent đọc từ cột `covers.accent_color` của DB ở luồng nền (UI không chạm đĩa, N7). Bài là tệp ngoài thì không có accent
//! (không có hàng trong `covers`), quầng bỏ trống.
//!
//! ```ignore
//! now_playing::install(window_handle, cx);   // trong `app::run`, sau `player::install` và `library_page::install`
//! ```

use std::sync::Arc;

use groove_core::model::{Song, TrackRef};
use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use groove_library::metadata_ops::MetadataOps;
use groove_library::store::Library;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, Context, Entity, FontWeight, Hsla, IntoElement, ParentElement, Pixels, Render,
    RenderImage, SharedString, StyleRefinement, Styled, Subscription, Task, Window, canvas, div, linear_color_stop,
    linear_gradient, px, relative,
};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::theme::ActiveTheme;
use materiant::virtual_grid::LruImageCache;

use crate::app::{self, ShellEvent, Slot};
use crate::icons::GrooveIcon;
use crate::player::{self, Player, PlayerEvent};
use crate::route::Route;
use crate::views::cover_image::{CoverImage, CoverKind, PlaceholderLook, parse_color};
use crate::views::layout;
use crate::views::lyrics_dialog::TextDialog;
use crate::views::lyrics_panel::{self, LyricsPanel, LyricsPanelEvent};
use crate::views::marquee::Marquee;
use crate::views::metadata::dialog::{MetadataDialog, MetadataDialogEvent};
use crate::views::queue_panel::{QueueMenuAction, QueuePanel, QueuePanelEvent};
use crate::views::save_as_dialog::SaveAsDialog;
use crate::views::timeline_box::TimelineBox;
use crate::views::timeline_dialog;

/// Cao quầng màu phía trên (đặc tả 1.5).
pub const GLOW_HEIGHT: f32 = 450.;
/// Độ đục của quầng (đặc tả 1.5).
pub const GLOW_OPACITY: f32 = 0.3;
/// Cột trái: 25 % trong 150-300 px. Cột phải: 32 % trong 190-340 px. Đặc tả 1.5 ghi 180 / 240 px, nhưng ở cửa sổ 800 px mặc định
/// cột lời giữa chỉ còn ~86 px, ba nút Metadata / Timeline / Lời bị cắt (thấy khi chạy thật, groove-gd1_29).
pub const LEFT_COLUMN: (f32, f32, f32) = (0.25, 150., 300.);
pub const RIGHT_COLUMN: (f32, f32, f32) = (0.32, 190., 340.);
/// Màn hẹp hơn mức này (px, cả màn Now Playing) thì ẩn cột lời bài, mở nó bằng nút trong drawer: ở 1024 px cột lời còn ~376 px,
/// đủ tiêu đề + ba nút một hàng; hẹp hơn thì ba cột ép chật (ảnh chủ dự án, màn ~860 px).
pub const NARROW_WIDTH: f32 = 1024.;
/// Bề ngang drawer lời bài: đủ tiêu đề + ba nút một hàng và dòng lời dài.
const LYRICS_DRAWER_WIDTH: f32 = 480.;
const COVER_CACHE: usize = 4;

/// `<bài> - <nghệ sĩ> | Groovex`; bài không có nghệ sĩ thì bỏ phần `- <nghệ sĩ>`. Không có bài thì `None` (về tên trang).
pub fn window_title(song: Option<&Song>) -> Option<String> {
    let song = song?;
    let app = t!("common.app_name");
    Some(match song.display_artist() {
        Some(artist) => format!("{} - {artist} | {app}", song.display_title()),
        None => format!("{} | {app}", song.display_title()),
    })
}

type TitleSink = fn(Option<SharedString>, &mut App);

pub struct NowPlaying {
    player: Entity<Player>,
    library: Option<Library>,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    /// Bìa của tệp ngoài, dựng một lần mỗi lần đổi bài.
    external: Option<Arc<RenderImage>>,
    accent: Option<Hsla>,
    /// Bìa mà `accent` thuộc về (hoặc đang tra).
    accent_hash: Option<String>,
    accent_task: Option<Task<()>>,
    route: Route,
    set_title: TitleSink,
    ops: Option<MetadataOps>,
    timeline_box: Entity<TimelineBox>,
    lyrics_panel: Entity<LyricsPanel>,
    queue_panel: Entity<QueuePanel>,
    lyrics_dialog: Option<Entity<TextDialog>>,
    timeline_dialog: Option<Entity<TextDialog>>,
    metadata_dialog: Option<Entity<MetadataDialog>>,
    /// Hộp "Save as new" mở từ menu hàng đợi (groove-gd2_03).
    save_as_dialog: Entity<SaveAsDialog>,
    /// Màn hẹp: cột lời bài ẩn, lời bài mở trong [`Self::lyrics_drawer`].
    narrow: bool,
    lyrics_drawer: DialogHandle,
    _subscriptions: Vec<Subscription>,
}

impl NowPlaying {
    pub fn new(
        library: Option<Library>,
        ops: Option<MetadataOps>,
        covers: Option<CoverStore>,
        route: Route,
        set_title: TitleSink,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> NowPlaying {
        let player = player::player(cx);
        let timeline_box = cx.new(|cx| TimelineBox::new(window, cx));
        let lyrics_panel = cx.new(LyricsPanel::new);
        let queue_panel = cx.new({
            let library = library.clone();
            move |cx| QueuePanel::new(library, cx)
        });
        let lyrics_dialog = ops.clone().map(|ops| cx.new(|cx| TextDialog::lyrics(ops, window, cx)));
        let timeline_dialog = ops.clone().map(|ops| cx.new(|cx| timeline_dialog::timeline(ops, window, cx)));
        let save_as_library = library.clone();
        let save_as_dialog = cx.new(|cx| SaveAsDialog::new(save_as_library, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&player, window, |this, _, event: &PlayerEvent, window, cx| {
                if matches!(event, PlayerEvent::TrackChanged) {
                    this.close_stale_metadata(window, cx);
                    this.track_changed(cx);
                }
            }),
            cx.subscribe_in(&lyrics_panel, window, |this, _, event: &LyricsPanelEvent, window, cx| {
                this.panel_event(*event, window, cx);
            }),
            // Menu chuột phải của hàng đợi: cùng các hộp với ba nút ở khung Lyrics, cho bài của dòng đó (groove-gd2_03).
            cx.subscribe_in(&queue_panel, window, |this, _, QueuePanelEvent(action, track), window, cx| {
                this.open_editor(*action, track.clone(), window, cx);
            }),
        ];
        let mut view = NowPlaying {
            player,
            library,
            covers,
            cache: LruImageCache::new(COVER_CACHE, cx),
            external: None,
            accent: None,
            accent_hash: None,
            accent_task: None,
            route,
            set_title,
            ops,
            timeline_box,
            lyrics_panel,
            queue_panel,
            lyrics_dialog,
            timeline_dialog,
            metadata_dialog: None,
            save_as_dialog,
            narrow: false,
            lyrics_drawer: DialogHandle::new(cx),
            _subscriptions: subscriptions,
        };
        view.refresh_cover(cx);
        view
    }

    #[cfg(test)]
    pub fn timeline_box(&self) -> &Entity<TimelineBox> {
        &self.timeline_box
    }

    #[cfg(test)]
    pub fn lyrics_panel(&self) -> &Entity<LyricsPanel> {
        &self.lyrics_panel
    }

    #[cfg(test)]
    pub fn queue_panel(&self) -> &Entity<QueuePanel> {
        &self.queue_panel
    }

    #[cfg(test)]
    pub fn save_as_dialog(&self) -> &Entity<SaveAsDialog> {
        &self.save_as_dialog
    }

    #[cfg(test)]
    pub fn lyrics_dialog(&self) -> Option<&Entity<TextDialog>> {
        self.lyrics_dialog.as_ref()
    }

    #[cfg(test)]
    pub fn timeline_dialog(&self) -> Option<&Entity<TextDialog>> {
        self.timeline_dialog.as_ref()
    }

    #[cfg(test)]
    pub fn metadata_dialog(&self) -> Option<&Entity<MetadataDialog>> {
        self.metadata_dialog.as_ref()
    }

    /// Khung đổi màn: tiêu đề cửa sổ chỉ là của màn này khi đang ở đây (`Shell` đã xoá tiêu đề riêng khi rời màn).
    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        // Vào màn này từ màn khác: hàng đợi đưa bài đang phát lên đỉnh khung.
        if route == Route::NowPlaying && self.route != Route::NowPlaying {
            self.queue_panel.update(cx, |panel, cx| panel.reveal_current(cx));
        }
        self.route = route;
        self.apply_title(cx);
    }

    fn apply_title(&mut self, cx: &mut Context<Self>) {
        if self.route != Route::NowPlaying {
            return;
        }
        let title = window_title(self.player.read(cx).current_song()).map(SharedString::from);
        (self.set_title)(title, cx);
    }

    /// Đổi bài thì đóng hộp metadata của bài cũ (đặc tả 1.5). `TrackChanged` cũng đến khi vừa lưu bài đang mở: khi đó giữ nguyên.
    fn close_stale_metadata(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.player.read(cx).current_track().cloned();
        if let Some(dialog) = self.metadata_dialog.clone()
            && current.as_ref() != Some(dialog.read(cx).track())
        {
            dialog.update(cx, |dialog, cx| dialog.close(window, cx));
        }
    }

    fn track_changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_cover(cx);
        self.apply_title(cx);
        cx.notify();
    }

    /// Bìa tệp ngoài dựng một lần; accent bài thư viện tra nền khi đổi bìa.
    fn refresh_cover(&mut self, cx: &mut Context<Self>) {
        let (external, hash) = {
            let player = self.player.read(cx);
            let external = player.external_cover().cloned();
            (external, player.current_song().and_then(|song| song.cover_hash.clone()))
        };
        self.external = external;
        if hash == self.accent_hash {
            return;
        }
        self.accent_hash = hash.clone();
        self.accent = None;
        self.accent_task = None;
        let (Some(hash), Some(library)) = (hash, self.library.clone()) else { return };
        self.accent_task = Some(cx.spawn(async move |this, cx| {
            let wanted = hash.clone();
            let found = library
                .read(move |conn| {
                    Ok(conn
                        .query_row("SELECT accent_color FROM covers WHERE hash = ?1", [wanted], |row| {
                            row.get::<_, Option<String>>(0)
                        })
                        .ok()
                        .flatten())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                // Đổi bài trong lúc tra: kết quả của bìa cũ bỏ đi.
                if this.accent_hash.as_deref() == Some(hash.as_str()) {
                    this.accent = found.ok().flatten().and_then(|text| parse_color(&text));
                    cx.notify();
                }
            });
        }));
    }

    /// Nút ở `LyricsPanel` được bấm: mở hộp tương ứng cho bài đang phát.
    fn panel_event(&mut self, event: LyricsPanelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(track) = self.player.read(cx).current_track().cloned() else { return };
        let action = match event {
            LyricsPanelEvent::Lyrics => QueueMenuAction::Lyrics,
            LyricsPanelEvent::Timeline => QueueMenuAction::Timeline,
            LyricsPanelEvent::Metadata => QueueMenuAction::Metadata,
        };
        self.open_editor(action, track, window, cx);
    }

    /// Mở hộp sửa / cắt cho `track` (nút ở khung Lyrics: bài đang phát; menu hàng đợi: bài của dòng).
    fn open_editor(&mut self, action: QueueMenuAction, track: TrackRef, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            QueueMenuAction::SaveAs => {
                self.save_as_dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
            }
            QueueMenuAction::Lyrics => {
                if let Some(dialog) = &self.lyrics_dialog {
                    dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
                }
            }
            QueueMenuAction::Timeline => {
                if let Some(dialog) = &self.timeline_dialog {
                    dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
                }
            }
            QueueMenuAction::Metadata => {
                let Some(ops) = self.ops.clone() else { return };
                if self.metadata_dialog.is_some() {
                    return;
                }
                let dialog = MetadataDialog::open(ops, track, None, window, cx);
                cx.subscribe(&dialog, |this, _, event: &MetadataDialogEvent, cx| {
                    if matches!(event, MetadataDialogEvent::Closed) {
                        this.metadata_dialog = None;
                        cx.notify();
                    }
                })
                .detach();
                self.metadata_dialog = Some(dialog);
                cx.notify();
            }
        }
    }

    /// Đổi giữa ba cột và hai cột + drawer.
    fn set_narrow(&mut self, narrow: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.narrow == narrow {
            return;
        }
        self.narrow = narrow;
        if !narrow && self.lyrics_drawer.is_open() {
            self.lyrics_drawer.close(window, cx);
        }
        self.lyrics_panel.update(cx, |panel, cx| panel.set_in_drawer(narrow, cx));
        cx.notify();
    }

    #[cfg(test)]
    pub fn is_narrow(&self) -> bool {
        self.narrow
    }

    #[cfg(test)]
    pub fn lyrics_drawer(&self) -> &DialogHandle {
        &self.lyrics_drawer
    }

    fn cover(&self, cx: &App) -> AnyElement {
        let path = self.player.read(cx).current_song().and_then(|song| {
            let hash = song.cover_hash.as_deref()?;
            Some(self.covers.as_ref()?.path(hash, CoverSize::Large))
        });
        // Bìa thư viện trước đây bo 8 px trong khung 16 px: bo đúng góc của khung.
        CoverImage::new(path, CoverKind::Album, self.accent, &self.cache)
            .embedded(self.external.clone())
            .radius(COVER_RADIUS)
            .look(PlaceholderLook::Hero)
            .into_any_element()
    }
}

/// Bo góc bìa lớn (`rounded-2xl`).
const COVER_RADIUS: Pixels = px(16.);

/// Cột có bề ngang theo tỉ lệ, kẹp giữa hai mức.
fn column(spec: (f32, f32, f32)) -> materiant::gpui_kit::Div {
    div().w(relative(spec.0)).min_w(px(spec.1)).max_w(px(spec.2)).flex_none().h_full().min_h_0()
}

/// Khung `cached` lấp đầy chỗ chủ dành.
fn full() -> StyleRefinement {
    StyleRefinement::default().size_full()
}

/// cpu (03/11/2026): tên bài chạy vẽ lại view này mỗi nhịp màn hình (Shell nhúng nó thường), nên ba khung (dòng thời gian,
/// lời bài, hàng đợi) nhúng `cached`: chúng tự nghe `Player` và `notify`, không cần chủ dựng lại.
impl Render for NowPlaying {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let playing = self.player.read(cx).is_playing();
        let (title, artist) = match self.player.read(cx).current_song() {
            Some(song) => (
                song.display_title().to_owned(),
                song.display_artist().unwrap_or(t!("now_playing.song.unknown_artist")).to_owned(),
            ),
            None => (t!("now_playing.song.no_song").to_owned(), t!("now_playing.song.unknown_artist").to_owned()),
        };

        let glow = self.accent.map(|accent| {
            div().absolute().top_0().left_0().right_0().h(px(GLOW_HEIGHT)).bg(linear_gradient(
                180.,
                linear_color_stop(accent.opacity(GLOW_OPACITY), 0.),
                linear_color_stop(accent.opacity(0.), 1.),
            ))
        });

        // Kiểu thẻ: ba khung cách nhau và cách mép một khe. Kiểu lưới: sát nhau, đường kẻ ngăn cột, cột trái tự đệm
        // (bìa, tên bài không có thẻ bao) — `views::layout`.
        let narrow = self.narrow;
        let lyrics_button = narrow.then(|| {
            let drawer = self.lyrics_drawer.clone();
            // Hàng flex: nút giữ bề ngang nội dung như ba nút của khung lời, không giãn hết cột.
            div().flex_none().flex().mt_3().child(
                lyrics_panel::pill_button(
                    "np-open-lyrics",
                    t!("now_playing.lyrics.title"),
                    GrooveIcon::LyricsBubble,
                    cx,
                )
                .on_click(move |_, window, cx| drawer.open(window, cx)),
            )
        });
        let gap = px(layout::gap(cx));
        let left = column(LEFT_COLUMN)
            .flex()
            .flex_col()
            .when(layout::is_grid(cx), |d| d.p(px(layout::CARD_GAP)))
            .child(div().w_full().aspect_square().flex_none().rounded(COVER_RADIUS).child(self.cover(cx)))
            .child(
                div()
                    .mt_4()
                    .flex_none()
                    .text_xl()
                    .font_weight(FontWeight::NORMAL)
                    .child(Marquee::new("now-playing-title", title).paused(!playing)),
            )
            // Tên nghệ sĩ dài xuống dòng (không cắt "…").
            .child(div().flex_none().text_color(c.muted).child(artist))
            .children(lyrics_button)
            // np-timeline-header: hộp nhúng thường (chữ chạy, thanh nhảy ở đầu hộp), danh sách đoạn bên trong tự `cached`.
            .child(div().w_full().flex_1().min_h_0().mt_4().child(self.timeline_box.clone()));

        // Hẹp: hàng đợi lấp chỗ cột lời bài để lại.
        let right = if narrow { div().flex_1().min_w_0().h_full() } else { column(RIGHT_COLUMN) }
            .child(self.queue_panel.clone().cached(full()));
        let lyrics =
            (!narrow).then(|| div().flex_1().min_w_0().h_full().child(self.lyrics_panel.clone().cached(full())));
        let drawer = narrow.then(|| {
            Dialog::new("now-playing-lyrics", &self.lyrics_drawer)
                .drawer(true)
                .max_width(px(LYRICS_DRAWER_WIDTH))
                .flush_body(true)
                .child(div().flex_1().min_h_0().child(self.lyrics_panel.clone().cached(full())))
        });
        // Đo bề ngang của màn lúc prepaint; đổi mức thì đổi bố cục ở khung sau.
        let meter = {
            let this = cx.entity().downgrade();
            canvas(
                move |bounds, window, cx| {
                    let now = f32::from(bounds.size.width) < NARROW_WIDTH;
                    if now != narrow {
                        this.update(cx, |view, cx| view.set_narrow(now, window, cx)).ok();
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        div()
            .id("now-playing")
            .relative()
            .size_full()
            .overflow_hidden()
            .child(meter)
            .children(glow)
            .child(
                div()
                    .size_full()
                    .flex()
                    .gap(gap)
                    .p(gap)
                    .child(left)
                    .children(layout::vline(cx))
                    .children(lyrics)
                    .when(!narrow, |row| row.children(layout::vline(cx)))
                    .child(right),
            )
            .children(self.lyrics_dialog.clone())
            .children(self.timeline_dialog.clone())
            .children(drawer)
            .children(self.metadata_dialog.clone())
            .child(self.save_as_dialog.clone())
    }
}

/// Dựng màn vào `Slot::NowPlaying`. Gọi một lần trong `app::run`, sau `player::install` và `library_page::install`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let library = app::try_library(cx).cloned();
    let ops = app::try_metadata_ops(cx);
    let covers = CoverStore::default_location().map_err(|err| log::warn!("cannot locate covers: {err}")).ok();
    let route = app::current_route(cx);
    let result = window.update(cx, |_, window, cx| {
        let view = cx.new(|cx| NowPlaying::new(library, ops, covers, route, app::set_title_override, window, cx));
        let weak = view.downgrade();
        cx.subscribe(&app::shell(cx), move |_, event: &ShellEvent, cx| {
            if let ShellEvent::RouteChanged(route) = event {
                weak.update(cx, |view, cx| view.set_route(*route, cx)).ok();
            }
        })
        .detach();
        app::install_slot(Slot::NowPlaying, view.clone(), cx);
        view.update(cx, |view, cx| view.apply_title(cx));
    });
    if let Err(err) = result {
        log::error!("cannot install the now playing view: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: Option<&str>, artist: Option<&str>) -> Song {
        let mut song = Song::external("/nhac/Bai Hat.mp3");
        song.title = title.map(str::to_owned);
        song.artist = artist.map(str::to_owned);
        song
    }

    #[test]
    fn window_title_names_song_and_artist() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        assert_eq!(window_title(None), None);
        assert_eq!(
            window_title(Some(&song(Some("Let It Be"), Some("The Beatles")))).unwrap(),
            "Let It Be - The Beatles | Groovex"
        );
        assert_eq!(window_title(Some(&song(None, None))).unwrap(), "Bai Hat | Groovex", "tên tệp, không nghệ sĩ");
    }
}

#[cfg(test)]
mod tests_support {
    use std::cell::RefCell;
    use std::time::{Duration, Instant};

    use gpui::{AppContext, TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::model::TrackRef;
    use groove_core::test_dir::TestDir;
    use groove_library::covers::CoverStore;
    use groove_library::db::migrate::CoverRecord;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::indexer::{TrackRow, apply_scanned_track};
    use groove_library::metadata_ops::MetadataOps;
    use groove_library::store::{Library, LibraryConfig};
    use groove_library::tags;
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::{Entity, SharedString, px, size};

    use crate::player::{self, Player, PlayerDeps};
    use crate::route::Route;
    use crate::views::now_playing::NowPlaying;

    thread_local! {
        pub static TITLES: RefCell<Vec<Option<SharedString>>> = const { RefCell::new(Vec::new()) };
    }

    pub struct Rig {
        pub dir: TestDir,
        pub library: Library,
        pub ops: MetadataOps,
        pub fake: FakeOutput,
        pub player: Entity<Player>,
    }

    pub fn rig(cx: &mut TestAppContext) -> Rig {
        cx.executor().allow_parking();
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let dir = TestDir::new();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let sink = Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no covers in this test") });
        let library = Library::open(config, sink).unwrap();
        let ops = MetadataOps::new(library.clone(), CoverStore::new(dir.path().join("covers"))).unwrap();
        let fake = FakeOutput::new(48_000, 2);
        let deps = PlayerDeps {
            library: Some(library.clone()),
            output: Box::new(fake.clone()),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: Some(dir.path().join("session.json")),
        };
        let player = cx.update(|cx| {
            materiant::init(cx);
            player::install_for_test(deps, cx)
        });
        Rig { dir, library, ops, fake, player }
    }

    impl Rig {
        pub fn song(&self, stem: &str) -> (TrackRef, std::path::PathBuf) {
            let path = fixtures::write_sample(self.dir.path(), stem, AudioKind::Mp3).unwrap();
            let row = TrackRow::from_scanned(path.clone(), tags::read_track(&path).unwrap(), None);
            let id =
                self.library.writer().transaction(move |conn| Ok(apply_scanned_track(conn, &row)?.id)).wait().unwrap();
            (TrackRef::Library(id), path)
        }

        pub fn until(&self, cx: &mut VisualTestContext, what: &str, condition: impl Fn(&VisualTestContext) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                cx.run_until_parked();
                if condition(cx) {
                    return;
                }
                assert!(Instant::now() < deadline, "timed out: {what}");
                let _ = self.fake.pull(480);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn record_title(title: Option<SharedString>, _: &mut gpui::App) {
        TITLES.with(|t| t.borrow_mut().push(title));
    }

    pub fn open_view<'a>(rig: &Rig, cx: &'a mut TestAppContext) -> (Entity<NowPlaying>, &'a mut VisualTestContext) {
        let (library, ops) = (rig.library.clone(), rig.ops.clone());
        let window = cx.add_window(|window, cx| {
            let view = cx
                .new(|cx| NowPlaying::new(Some(library), Some(ops), None, Route::NowPlaying, record_title, window, cx));
            Root::new(view, window, cx)
        });
        let view = window.update(cx, |root, _, _| root.view().clone().downcast::<NowPlaying>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        // Rộng hơn `NARROW_WIDTH`: đủ ba cột.
        cx.simulate_resize(size(px(1200.), px(700.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (view, cx)
    }
}

#[cfg(test)]
mod view_tests {
    use gpui::{TestAppContext, VisualTestContext};

    use super::tests_support::*;
    use super::*;
    use crate::views::lyrics_panel::Body;

    /// queue-scroll-top: mở màn (lần đầu và mỗi lần quay lại từ màn khác) thì bài đang phát lên sát mép trên hàng đợi.
    #[gpui::test]
    fn opening_the_screen_brings_the_current_track_to_the_top(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let tracks: Vec<_> = (0..40).map(|i| rig.song(&format!("s{i:02}")).0).collect();
        let (view, cx) = open_view(&rig, cx);
        // Bài thứ 31 trong hàng đợi 40 bài: 30 bài đã phát (lịch sử) nằm trên nó.
        rig.player.update(cx, |p, cx| {
            p.play_list(tracks[0].clone(), tracks.clone(), cx);
            for _ in 0..30 {
                p.next(cx);
            }
        });
        let queue = view.read_with(cx, |v, _| v.queue_panel().clone());
        // Không qua `rig.until`: nó kéo âm thanh của engine giả, mẫu thử ngắn nên bài tự hết và nhảy sang bài sau.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !queue.read_with(cx, |q, _| q.len() == 40 && q.rows().iter().all(Option::is_some)) {
            assert!(std::time::Instant::now() < deadline, "timed out: queue ready");
            cx.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(queue.read_with(cx, |q, _| q.current()), Some(30));
        let top = |cx: &mut VisualTestContext| {
            for _ in 0..3 {
                cx.update(|window, cx| window.draw(cx).clear(cx));
            }
            queue.read_with(cx, |q, _| q.list().scroll_top())
        };
        let first = top(cx);
        assert!(first > 0., "lần đầu: đã cuộn tới bài 31 ({first})");

        queue.read_with(cx, |q, _| q.list().scroll_to(0, materiant::list::Align::Start));
        assert_eq!(top(cx), 0.);
        view.update(cx, |v, cx| v.set_route(Route::MyMusic, cx));
        view.update(cx, |v, cx| v.set_route(Route::NowPlaying, cx));
        assert!((top(cx) - first).abs() < 1., "quay lại màn: bài đang phát về đúng chỗ lần đầu");
    }

    /// Bấm bài hiện tại trong hàng đợi đảo phát / dừng như nút play, phát tiếp từ vị trí cũ; bấm bài khác thì phát bài đó.
    #[gpui::test]
    fn clicking_the_current_queue_row_toggles_playback(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (b, _) = rig.song("b");
        let (view, cx) = open_view(&rig, cx);
        rig.player.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b.clone()], cx));
        let queue = view.read_with(cx, |v, _| v.queue_panel().clone());
        rig.until(cx, "playing past the start", |cx| {
            queue.read_with(cx, |q, _| q.current() == Some(0))
                && rig.player.read_with(cx, |p, _| p.is_playing() && p.position_sec() > 0.1)
        });

        queue.update(cx, |q, cx| q.click_row(0, cx));
        rig.until(cx, "paused", |cx| !rig.player.read_with(cx, |p, _| p.is_playing()));
        let paused_at = rig.player.read_with(cx, |p, _| p.position_sec());
        assert!(paused_at > 0.1, "dừng tại chỗ, không về đầu ({paused_at})");
        assert_eq!(rig.player.read_with(cx, |p, _| p.current_track().cloned()), Some(a.clone()));

        queue.update(cx, |q, cx| q.click_row(0, cx));
        rig.until(cx, "resumed", |cx| rig.player.read_with(cx, |p, _| p.is_playing()));
        let resumed_at = rig.player.read_with(cx, |p, _| p.position_sec());
        assert!(resumed_at >= paused_at - 0.01, "phát tiếp từ {paused_at}, không từ 0 ({resumed_at})");
        assert_eq!(rig.player.read_with(cx, |p, _| p.current_track().cloned()), Some(a.clone()));

        queue.update(cx, |q, cx| q.click_row(1, cx));
        rig.until(cx, "other row plays", |cx| {
            rig.player.read_with(cx, |p, _| p.current_track() == Some(&b) && p.is_playing())
        });
    }

    /// C1 hàng đợi liệt kê đủ bài; C2 ba nút mở đúng hộp cho bài đang phát; C3 timeline hiện đoạn.
    #[gpui::test]
    fn buttons_open_dialogs_and_queue_lists_tracks(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (b, _) = rig.song("b");
        rig.ops.save_lyrics(a.clone(), "dòng một\n\ndòng hai".into()).wait().unwrap();
        rig.ops.save_timeline(a.clone(), "0:00 Mở đầu\n0:01 Đoạn hai".into()).wait().unwrap();
        let (view, cx) = open_view(&rig, cx);
        rig.player.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b.clone()], cx));
        rig.until(cx, "lyrics and queue ready", |cx| {
            let lyrics = view.read_with(cx, |v, cx| v.lyrics_panel().read(cx).state(cx));
            let rows_ready = view.read_with(cx, |v, cx| {
                let q = v.queue_panel().read(cx);
                q.len() == 2 && q.rows().iter().all(Option::is_some)
            });
            matches!(lyrics, Body::Lyrics(_)) && rows_ready
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(view.read_with(cx, |v, cx| v.queue_panel().read(cx).current()), Some(0));
        assert!(view.read_with(cx, |v, cx| v.timeline_box().read(cx).has_segments(cx)));

        let panel = view.read_with(cx, |v, _| v.lyrics_panel().clone());
        panel.update(cx, |_, cx| cx.emit(LyricsPanelEvent::Lyrics));
        let dialog = view.read_with(cx, |v, _| v.lyrics_dialog().unwrap().clone());
        assert!(dialog.read_with(cx, |d, _| d.is_open()));
        dialog.update_in(cx, |d, window, cx| d.close(window, cx));
        panel.update(cx, |_, cx| cx.emit(LyricsPanelEvent::Timeline));
        let dialog = view.read_with(cx, |v, _| v.timeline_dialog().unwrap().clone());
        assert!(dialog.read_with(cx, |d, _| d.is_open()));
        dialog.update_in(cx, |d, window, cx| d.close(window, cx));
        panel.update(cx, |_, cx| cx.emit(LyricsPanelEvent::Metadata));
        assert!(view.read_with(cx, |v, _| v.metadata_dialog().is_some()));

        // Đổi bài: hộp metadata của bài cũ đóng (đặc tả 1.5).
        rig.player.update(cx, |p, cx| p.next(cx));
        rig.until(cx, "metadata closes on track change", |cx| {
            view.read_with(cx, |v, cx| v.metadata_dialog().is_none_or(|d| !d.read(cx).is_open()))
        });
    }

    /// groove-gd2_03 C4 — chuột phải một dòng hàng đợi: menu bốn mục mở đúng các hộp của Now Playing cho bài của dòng đó
    /// (không phải bài đang phát).
    #[gpui::test]
    fn queue_row_menu_opens_editors_for_that_row(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton};

        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (b, _) = rig.song("b");
        let (view, cx) = open_view(&rig, cx);
        rig.player.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b.clone()], cx));
        rig.until(cx, "queue ready", |cx| {
            view.read_with(cx, |v, cx| {
                let q = v.queue_panel().read(cx);
                q.len() == 2 && q.rows().iter().all(Option::is_some)
            })
        });
        let draw = |cx: &mut VisualTestContext| {
            for _ in 0..2 {
                cx.update(|window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                });
            }
        };
        draw(cx);
        let queue = view.read_with(cx, |v, _| v.queue_panel().clone());
        let right_click = |cx: &mut VisualTestContext| {
            let row = cx.debug_bounds("np-queue-row-1").expect("dòng 2").center();
            cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::default());
            cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::default());
            draw(cx);
        };

        right_click(cx);
        assert_eq!(queue.read_with(cx, |q, _| q.menu_track()), Some(b.clone()));
        // Menu mở bằng chuột chưa tô mục nào: Home tới mục đầu.
        cx.simulate_keystrokes("home enter");
        cx.run_until_parked();
        draw(cx);
        let lyrics = view.read_with(cx, |v, _| v.lyrics_dialog().unwrap().clone());
        assert!(lyrics.read_with(cx, |d, _| d.is_open()), "mục đầu: lời bài");
        lyrics.update_in(cx, |d, window, cx| d.close(window, cx));

        right_click(cx);
        cx.simulate_keystrokes("end enter");
        cx.run_until_parked();
        draw(cx);
        let save_as = view.read_with(cx, |v, _| v.save_as_dialog().clone());
        assert!(save_as.read_with(cx, |d, _| d.is_open()), "mục cuối: Save as new");
        rig.until(cx, "save as reads the song", |cx| save_as.read_with(cx, |d, _| d.is_ready()));
    }

    /// Responsive: hẹp hơn `NARROW_WIDTH` thì ẩn cột lời bài, nút Lời bài mở drawer; rộng ra thì drawer đóng, cột hiện lại.
    #[gpui::test]
    fn narrow_screen_moves_the_lyrics_into_a_drawer(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (view, cx) = open_view(&rig, cx);
        assert!(!view.read_with(cx, |v, _| v.is_narrow()));
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
        };
        cx.simulate_resize(gpui::size(px(900.), px(700.)));
        draw(cx);
        assert!(view.read_with(cx, |v, _| v.is_narrow()));
        let drawer = view.read_with(cx, |v, _| v.lyrics_drawer().clone());
        cx.update(|window, cx| drawer.open(window, cx));
        draw(cx);
        assert!(view.read_with(cx, |v, _| v.lyrics_drawer().is_open()), "nút mở drawer");

        cx.simulate_resize(gpui::size(px(1200.), px(700.)));
        draw(cx);
        assert!(!view.read_with(cx, |v, _| v.is_narrow()));
        assert!(!view.read_with(cx, |v, _| v.lyrics_drawer().is_open()), "rộng ra thì drawer đóng");
    }

    /// C4 bấm một dòng hàng đợi phát bài đó.
    #[gpui::test]
    fn clicking_a_queue_row_plays_it(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (b, _) = rig.song("b");
        let (view, cx) = open_view(&rig, cx);
        rig.player.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b.clone()], cx));
        rig.until(cx, "queue ready", |cx| view.read_with(cx, |v, cx| v.queue_panel().read(cx).len() == 2));
        let queue = view.read_with(cx, |v, _| v.queue_panel().clone());
        queue.update(cx, |q, cx| q.click_row(1, cx));
        rig.until(cx, "second track is current", |cx| rig.player.read_with(cx, |p, _| p.current_track() == Some(&b)));
        rig.until(cx, "panel follows", |cx| queue.read_with(cx, |q, _| q.current() == Some(1)));
    }

    /// C5 tiêu đề cửa sổ theo bài, chỉ khi đang ở màn Now Playing.
    #[gpui::test]
    fn window_title_is_set_only_on_this_route(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (view, cx) = open_view(&rig, cx);
        TITLES.with(|t| t.borrow_mut().clear());
        rig.player.update(cx, |p, cx| p.play_track(a.clone(), cx));
        rig.until(cx, "title set", |_| TITLES.with(|t| t.borrow().iter().any(Option::is_some)));
        view.update(cx, |v, cx| v.set_route(Route::MyMusic, cx));
        let n = TITLES.with(|t| t.borrow().len());
        rig.player.update(cx, |p, cx| p.refresh_current(cx));
        rig.until(cx, "refresh settled", |_| true);
        assert_eq!(TITLES.with(|t| t.borrow().len()), n, "rời màn thì không đặt tiêu đề");
    }
}
