//! groove-gd1_21: màn Now Playing (đặc tả 1.5; bản cũ `pages/now-playing/now-playing.vue`).
//!
//! Ba cột cao đầy: trái (25 %, 180-300 px) bìa + tên + nghệ sĩ + [`TimelineBox`]; giữa [`LyricsPanel`]; phải (32 %, 240-340 px)
//! [`QueuePanel`]. Phía sau là quầng màu theo accent của bìa (cao 450 px, độ đục 0,3). `backdrop-blur` của bản cũ không có ở
//! GPUI: các khung dùng nền bán trong suốt thay (bảng thay thế đặc tả mục 4); quầng đổi màu tức thì, không có chuyển 700 ms.
//!
//! Màn này không tự ghi gì: ba nút Metadata, Timeline, Sửa / Thêm lời của `LyricsPanel` mở [`MetadataDialog`] và hai
//! [`TextDialog`] (task 19, 20) cho bài đang phát. Hộp lyrics, timeline tự đóng khi đổi bài, lưu xong tự nạp lại bài.
//! Tiêu đề cửa sổ là `<bài> - <nghệ sĩ> | Groove X` khi đang ở màn này.
//!
//! Accent đọc từ cột `covers.accent_color` của DB ở luồng nền (UI không chạm đĩa, N7). Bài là tệp ngoài thì không có accent
//! (không có hàng trong `covers`), quầng bỏ trống.
//!
//! ```ignore
//! now_playing::install(window_handle, cx);   // trong `app::run`, sau `player::install` và `library_page::install`
//! ```

use std::sync::Arc;

use groove_core::model::Song;
use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use groove_library::metadata_ops::MetadataOps;
use groove_library::store::Library;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, Context, Entity, FontWeight, Hsla, Image, ImageFormat, ImageSource, IntoElement,
    ObjectFit, ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div, img, linear_color_stop,
    linear_gradient, px, relative,
};
use materiant::theme::ActiveTheme;
use materiant::virtual_grid::LruImageCache;

use crate::app::{self, ShellEvent, Slot};
use crate::player::{self, Player, PlayerEvent};
use crate::route::Route;
use crate::views::cover_image::{CoverImage, CoverKind, parse_color};
use crate::views::lyrics_dialog::TextDialog;
use crate::views::lyrics_panel::{LyricsPanel, LyricsPanelEvent};
use crate::views::marquee::Marquee;
use crate::views::metadata::dialog::{MetadataDialog, MetadataDialogEvent};
use crate::views::queue_panel::QueuePanel;
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
const COVER_CACHE: usize = 4;

/// `<bài> - <nghệ sĩ> | Groove X`; bài không có nghệ sĩ thì bỏ phần `- <nghệ sĩ>`. Không có bài thì `None` (về tên trang).
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
    external: Option<Arc<Image>>,
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
        let timeline_box = cx.new(TimelineBox::new);
        let lyrics_panel = cx.new(LyricsPanel::new);
        let queue_panel = cx.new({
            let library = library.clone();
            move |cx| QueuePanel::new(library, cx)
        });
        let lyrics_dialog = ops.clone().map(|ops| cx.new(|cx| TextDialog::lyrics(ops, window, cx)));
        let timeline_dialog = ops.clone().map(|ops| cx.new(|cx| timeline_dialog::timeline(ops, window, cx)));
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
            let external = player.external_cover().and_then(|cover| {
                Some(Arc::new(Image::from_bytes(ImageFormat::from_mime_type(&cover.mime)?, cover.data.clone())))
            });
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
        match event {
            LyricsPanelEvent::Lyrics => {
                if let Some(dialog) = &self.lyrics_dialog {
                    dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
                }
            }
            LyricsPanelEvent::Timeline => {
                if let Some(dialog) = &self.timeline_dialog {
                    dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
                }
            }
            LyricsPanelEvent::Metadata => {
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

    fn cover(&self, cx: &App) -> AnyElement {
        if let Some(image) = &self.external {
            return div()
                .size_full()
                .overflow_hidden()
                .rounded_2xl()
                .child(img(ImageSource::Image(image.clone())).size_full().object_fit(ObjectFit::Cover))
                .into_any_element();
        }
        let path = self.player.read(cx).current_song().and_then(|song| {
            let hash = song.cover_hash.as_deref()?;
            Some(self.covers.as_ref()?.path(hash, CoverSize::Large))
        });
        CoverImage::new(path, CoverKind::Album, self.accent, &self.cache).into_any_element()
    }
}

/// Cột có bề ngang theo tỉ lệ, kẹp giữa hai mức.
fn column(spec: (f32, f32, f32)) -> materiant::gpui_kit::Div {
    div().w(relative(spec.0)).min_w(px(spec.1)).max_w(px(spec.2)).flex_none().h_full().min_h_0()
}

impl Render for NowPlaying {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
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

        let left = column(LEFT_COLUMN)
            .flex()
            .flex_col()
            .child(div().w_full().aspect_square().flex_none().overflow_hidden().rounded_2xl().child(self.cover(cx)))
            .child(
                div()
                    .mt_4()
                    .flex_none()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(Marquee::new("now-playing-title", title)),
            )
            .child(div().flex_none().truncate().text_color(c.muted).child(artist))
            .child(self.timeline_box.clone());

        let right = column(RIGHT_COLUMN).child(self.queue_panel.clone());

        div()
            .id("now-playing")
            .relative()
            .size_full()
            .overflow_hidden()
            .children(glow)
            .child(
                div()
                    .size_full()
                    .flex()
                    .gap_4()
                    .p_4()
                    .child(left)
                    .child(div().flex_1().min_w_0().h_full().child(self.lyrics_panel.clone()))
                    .child(right),
            )
            .children(self.lyrics_dialog.clone())
            .children(self.timeline_dialog.clone())
            .children(self.metadata_dialog.clone())
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
            "Let It Be - The Beatles | Groove X"
        );
        assert_eq!(window_title(Some(&song(None, None))).unwrap(), "Bai Hat | Groove X", "tên tệp, không nghệ sĩ");
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
    use groove_library::covers::CoverStore;
    use groove_library::db::migrate::CoverRecord;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::indexer::{TrackRow, apply_scanned_track};
    use groove_library::metadata_ops::MetadataOps;
    use groove_library::store::{Library, LibraryConfig};
    use groove_library::tags;
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::{Entity, SharedString, px, size};
    use tempfile::TempDir;

    use crate::player::{self, Player, PlayerDeps};
    use crate::route::Route;
    use crate::views::now_playing::NowPlaying;

    thread_local! {
        pub static TITLES: RefCell<Vec<Option<SharedString>>> = const { RefCell::new(Vec::new()) };
    }

    pub struct Rig {
        pub dir: TempDir,
        pub library: Library,
        pub ops: MetadataOps,
        pub fake: FakeOutput,
        pub player: Entity<Player>,
    }

    pub fn rig(cx: &mut TestAppContext) -> Rig {
        cx.executor().allow_parking();
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let dir = TempDir::new().unwrap();
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
        cx.simulate_resize(size(px(1000.), px(700.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (view, cx)
    }
}

#[cfg(test)]
mod view_tests {
    use gpui::TestAppContext;

    use super::tests_support::*;
    use super::*;
    use crate::views::lyrics_panel::Body;

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
