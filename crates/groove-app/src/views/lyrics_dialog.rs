//! groove-gd1_20: hộp thoại sửa lyrics và timeline của một bài (đặc tả 1.6, 1.7).
//!
//! Một lõi chung [`TextDialog`] cho cả hai, khác nhau ở [`TextKind`] (chữ, trường đọc / ghi, hướng dẫn); `timeline_dialog`
//! chỉ dựng bản timeline. Hộp rộng 500 px, ô nhiều dòng 12 dòng, hai nút Huỷ, Lưu.
//!
//! Luồng: `open(track)` đọc nền qua `MetadataOps::read` (lyrics, timeline lấy từ tệp, tệp không có thì DB), ô khoá tới khi
//! có chữ. Lưu gọi `save_lyrics` / `save_timeline`, xong thì `Player::refresh_current` rồi đóng. Lỗi (S5) ⇒ toast theo biến
//! thể, hộp vẫn mở và giữ nguyên chữ đang gõ (ghi tệp lỗi thì DB giữ bản cũ). Đổi bài đang phát sang bài khác thì đóng
//! (đặc tả 1.5); chuỗi rỗng là xoá cả tệp và DB.
//!
//! ```ignore
//! let dialog = cx.new(|cx| TextDialog::lyrics(ops.clone(), window, cx));  // view gốc giữ entity và vẽ nó
//! dialog.update(cx, |d, cx| d.open(track, window, cx));
//! ```

use groove_core::model::TrackRef;
use groove_core::t;
use groove_library::metadata_ops::{MetadataError, MetadataOps};
use materiant::button::Button;
use materiant::field::TextArea;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::toast::{Toast, ToastKind, ToastSpec};

use crate::player::{self, PlayerEvent};

/// Bề ngang hộp (đặc tả 1.6, 1.7).
const DIALOG_WIDTH: f32 = 500.;
/// Số dòng của ô (đặc tả 1.7).
const ROWS: usize = 12;

/// Hộp này sửa gì.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Lyrics,
    Timeline,
}

impl TextKind {
    fn title(self) -> &'static str {
        match self {
            TextKind::Lyrics => t!("song_dialogs.lyrics.title"),
            TextKind::Timeline => t!("song_dialogs.timeline.title"),
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            TextKind::Lyrics => t!("song_dialogs.lyrics.placeholder"),
            TextKind::Timeline => t!("song_dialogs.timeline.placeholder"),
        }
    }

    fn hint(self) -> Option<&'static str> {
        match self {
            TextKind::Lyrics => None,
            TextKind::Timeline => Some(t!("song_dialogs.timeline.hint")),
        }
    }

    fn save_failed(self) -> &'static str {
        match self {
            TextKind::Lyrics => t!("song_dialogs.lyrics.save_failed"),
            TextKind::Timeline => t!("song_dialogs.timeline.save_failed"),
        }
    }

    fn load_failed(self) -> &'static str {
        match self {
            TextKind::Lyrics => t!("song_dialogs.lyrics.load_failed"),
            TextKind::Timeline => t!("song_dialogs.timeline.load_failed"),
        }
    }
}

/// Câu cho từng biến thể lỗi, kèm cách gỡ (S5).
pub fn error_message(error: &MetadataError) -> &'static str {
    match error {
        MetadataError::SongNotFound(_) => t!("song_dialogs.error.not_found"),
        MetadataError::Tag(_) | MetadataError::InvalidName(_) | MetadataError::TargetExists(_) => {
            t!("song_dialogs.error.write_failed")
        }
        MetadataError::DatabaseAfterWrite(_) => t!("song_dialogs.error.database_after_write"),
        MetadataError::RenameFailed { .. } => t!("song_dialogs.error.rename_failed"),
        MetadataError::Library(_) => t!("song_dialogs.error.generic"),
    }
}

fn error_toast(title: &'static str, error: &MetadataError) -> ToastSpec {
    ToastSpec::new(ToastKind::Danger, error_message(error)).title(title)
}

pub struct TextDialog {
    kind: TextKind,
    ops: MetadataOps,
    handle: DialogHandle,
    area: Entity<TextArea>,
    track: Option<TrackRef>,
    /// Đang đọc (mở hộp) hoặc đang ghi: ô và nút Lưu khoá.
    busy: bool,
    /// Tăng mỗi lần mở / đóng: kết quả đọc, ghi của lần cũ không được đụng vào lần mới.
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl TextDialog {
    pub fn lyrics(ops: MetadataOps, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(TextKind::Lyrics, ops, window, cx)
    }

    pub fn new(kind: TextKind, ops: MetadataOps, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let handle = DialogHandle::new(cx);
        let area = cx.new(|cx| {
            let mut area = TextArea::new(("text-dialog-area", kind as usize), window, cx)
                .rows(ROWS)
                .placeholder(kind.placeholder());
            if let Some(hint) = kind.hint() {
                area = area.hint(hint);
            }
            area
        });
        let player = player::player(cx);
        let subscriptions = vec![cx.subscribe_in(&player, window, |this, _, event: &PlayerEvent, window, cx| {
            // Đổi bài thì đóng (đặc tả 1.5); `refresh_current` sau khi lưu cũng phát `TrackChanged` nhưng cùng bài.
            if matches!(event, PlayerEvent::TrackChanged) && this.track.is_some() {
                let now = player::player(cx).read(cx).current_track().cloned();
                if now != this.track {
                    this.close(window, cx);
                }
            }
        })];
        TextDialog { kind, ops, handle, area, track: None, busy: false, generation: 0, _subscriptions: subscriptions }
    }

    #[cfg(test)]
    pub fn kind(&self) -> TextKind {
        self.kind
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    /// Ô đang khoá vì đọc hoặc ghi chưa xong.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Chữ trong ô lúc này.
    pub fn text(&self, cx: &App) -> SharedString {
        self.area.read(cx).value(cx)
    }

    #[cfg(test)]
    /// Đặt chữ trong ô (cho test và cho view gốc muốn điền sẵn).
    pub fn set_text(&mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.into();
        self.area.update(cx, |area, cx| area.set_value(text, window, cx));
    }

    /// Mở cho `track`: ô trống và khoá tới khi đọc xong từ tệp.
    pub fn open(&mut self, track: TrackRef, window: &mut Window, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.track = Some(track.clone());
        self.busy = true;
        self.area.update(cx, |area, cx| {
            area.set_value("", window, cx);
            area.set_disabled(true, cx);
        });
        self.handle.open(window, cx);
        let kind = self.kind;
        let reply = self.ops.read(track);
        cx.spawn_in(window, async move |this, cx| {
            let result = reply.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.generation != generation || !this.handle.is_open() {
                    return;
                }
                match result {
                    Ok(view) => {
                        let text = match kind {
                            TextKind::Lyrics => view.fields.lyrics,
                            TextKind::Timeline => view.fields.timeline,
                        };
                        this.busy = false;
                        this.area.update(cx, |area, cx| {
                            area.set_value(text.unwrap_or_default(), window, cx);
                            area.set_disabled(false, cx);
                        });
                    }
                    Err(error) => {
                        log::warn!("cannot read {kind:?} for the dialog: {error}");
                        Toast::push(window, cx, error_toast(kind.load_failed(), &error));
                        this.close(window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        self.track = None;
        self.busy = false;
        self.generation += 1;
        cx.notify();
    }

    /// Ghi chữ trong ô. Xong: nạp lại bài đang phát rồi đóng. Lỗi: toast, mở nguyên, ô mở khoá lại.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(track) = self.track.clone() else { return };
        if self.busy {
            return;
        }
        self.busy = true;
        self.area.update(cx, |area, cx| area.set_disabled(true, cx));
        let text = self.text(cx).to_string();
        let kind = self.kind;
        let reply = match kind {
            TextKind::Lyrics => self.ops.save_lyrics(track, text),
            TextKind::Timeline => self.ops.save_timeline(track, text),
        };
        let generation = self.generation;
        cx.spawn_in(window, async move |this, cx| {
            let result = reply.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match &result {
                    Ok(_) => player::player(cx).update(cx, |player, cx| player.refresh_current(cx)),
                    Err(error) => {
                        log::warn!("cannot save {kind:?}: {error}");
                        Toast::push(window, cx, error_toast(kind.save_failed(), error));
                    }
                }
                // Hộp đã đóng, hay mở lại cho bài khác trong lúc chờ: kết quả chỉ còn là toast, không đụng trạng thái.
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                if result.is_ok() {
                    this.close(window, cx);
                } else {
                    this.area.update(cx, |area, cx| area.set_disabled(false, cx));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for TextDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = ("text-dialog", self.kind as usize);
        let cancel = cx.listener(|this, _, window, cx| this.close(window, cx));
        let save = cx.listener(|this, _, window, cx| this.save(window, cx));
        let actions: [AnyElement; 2] = [
            Button::new(("text-dialog-cancel", self.kind as usize), t!("common.cancel"))
                .text()
                .on_click(move |event, window, cx| cancel(event, window, cx))
                .into_any_element(),
            Button::new(("text-dialog-save", self.kind as usize), t!("common.save"))
                .primary()
                .disabled(self.busy)
                .loading(self.busy && self.track.is_some())
                .on_click(move |event, window, cx| save(event, window, cx))
                .into_any_element(),
        ];
        Dialog::new(id, &self.handle)
            .max_width(px(DIALOG_WIDTH))
            .title(self.kind.title())
            .close_button(t!("common.close"))
            .actions(actions)
            .child(div().w_full().child(self.area.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui::{TestAppContext, VisualTestContext, size};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_library::covers::CoverStore;
    use groove_library::db::migrate::CoverRecord;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::indexer::{TrackRow, apply_scanned_track};
    use groove_library::store::{Library, LibraryConfig};
    use groove_library::tags;
    use materiant::gpui_kit::base::Root;
    use tempfile::TempDir;

    use super::*;
    use crate::player::{Player, PlayerDeps};
    use crate::views::timeline_dialog;

    /// Thư mục tạm, DB tạm, `FakeOutput`: không phát tiếng, không chạm DB hay nhạc thật.
    struct Rig {
        dir: TempDir,
        library: Library,
        ops: MetadataOps,
        fake: FakeOutput,
        player: Entity<Player>,
    }

    fn rig(cx: &mut TestAppContext) -> Rig {
        // Engine, thư viện, `MetadataOps` chạy luồng thật, đánh thức foreground task từ luồng khác.
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
        fn song(&self, stem: &str) -> (TrackRef, std::path::PathBuf) {
            let path = fixtures::write_sample(self.dir.path(), stem, AudioKind::Mp3).unwrap();
            let row = TrackRow::from_scanned(path.clone(), tags::read_track(&path).unwrap(), None);
            let id =
                self.library.writer().transaction(move |conn| Ok(apply_scanned_track(conn, &row)?.id)).wait().unwrap();
            (TrackRef::Library(id), path)
        }

        fn until(&self, cx: &mut VisualTestContext, what: &str, condition: impl Fn(&VisualTestContext) -> bool) {
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

        fn player_lyrics(&self, cx: &VisualTestContext) -> Option<String> {
            self.player.read_with(cx, |p, _| p.lyrics().map(str::to_owned))
        }
    }

    fn open_dialog(
        cx: &mut TestAppContext,
        make: impl FnOnce(&mut Window, &mut Context<TextDialog>) -> TextDialog,
    ) -> (Entity<TextDialog>, &mut VisualTestContext) {
        let window = cx.add_window(|window, cx| {
            let dialog = cx.new(|cx| make(window, cx));
            Root::new(dialog, window, cx)
        });
        let dialog = window.update(cx, |root, _, _| root.view().clone().downcast::<TextDialog>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(1000.), px(700.)));
        (dialog, cx)
    }

    fn open(dialog: &Entity<TextDialog>, track: &TrackRef, cx: &mut VisualTestContext) {
        let track = track.clone();
        dialog.update_in(cx, |d, window, cx| d.open(track, window, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn ready(rig: &Rig, dialog: &Entity<TextDialog>, cx: &mut VisualTestContext) {
        let dialog = dialog.clone();
        rig.until(cx, "dialog loaded", move |cx| dialog.read_with(cx, |d, _| !d.is_busy()));
    }

    #[gpui::test]
    fn c1_lyrics_open_reads_the_file_and_save_writes_file_and_db_then_refreshes(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (track, _) = rig.song("a");
        let TrackRef::Library(id) = track.clone() else { unreachable!() };
        rig.ops.save_lyrics(track.clone(), "old line".into()).wait().unwrap();
        let (dialog, cx) = open_dialog(cx, |window, cx| TextDialog::lyrics(rig.ops.clone(), window, cx));
        rig.player.update(cx, |p, cx| p.play_track(track.clone(), cx));
        rig.until(cx, "player has the old lyrics", |cx| rig.player_lyrics(cx).as_deref() == Some("old line"));

        open(&dialog, &track, cx);
        assert!(dialog.read_with(cx, |d, _| d.is_open() && d.is_busy()), "ô khoá lúc đang đọc");
        ready(&rig, &dialog, cx);
        assert_eq!(dialog.read_with(cx, |d, cx| d.text(cx)).as_str(), "old line");

        dialog.update_in(cx, |d, window, cx| {
            d.set_text("new line 1\n\nnew line 2", window, cx);
            d.save(window, cx);
        });
        rig.until(cx, "dialog closed", |cx| dialog.read_with(cx, |d, _| !d.is_open()));
        rig.until(cx, "player refreshed", |cx| rig.player_lyrics(cx).as_deref() == Some("new line 1\n\nnew line 2"));

        let in_file = rig.ops.read(track).wait().unwrap().fields.lyrics;
        assert_eq!(in_file.as_deref(), Some("new line 1\n\nnew line 2"));
        let in_db = rig.library.song_text(id).wait().unwrap().unwrap().lyrics;
        assert_eq!(in_db.as_deref(), Some("new line 1\n\nnew line 2"));
    }

    #[gpui::test]
    fn c2_saving_empty_text_removes_the_lyrics(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (track, _) = rig.song("a");
        rig.ops.save_lyrics(track.clone(), "something".into()).wait().unwrap();
        let (dialog, cx) = open_dialog(cx, |window, cx| TextDialog::lyrics(rig.ops.clone(), window, cx));
        open(&dialog, &track, cx);
        ready(&rig, &dialog, cx);
        dialog.update_in(cx, |d, window, cx| {
            d.set_text("", window, cx);
            d.save(window, cx);
        });
        rig.until(cx, "dialog closed", |cx| dialog.read_with(cx, |d, _| !d.is_open()));
        let view = rig.ops.read(track).wait().unwrap();
        assert!(view.fields.lyrics.as_deref().unwrap_or("").is_empty());
    }

    #[gpui::test]
    fn c3_failed_save_keeps_the_dialog_open_with_the_typed_text(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (track, path) = rig.song("a");
        let (dialog, cx) = open_dialog(cx, |window, cx| TextDialog::lyrics(rig.ops.clone(), window, cx));
        open(&dialog, &track, cx);
        ready(&rig, &dialog, cx);
        std::fs::remove_file(&path).unwrap();
        dialog.update_in(cx, |d, window, cx| {
            d.set_text("typed text", window, cx);
            d.save(window, cx);
        });
        rig.until(cx, "save finished", |cx| dialog.read_with(cx, |d, _| !d.is_busy()));
        assert!(dialog.read_with(cx, |d, _| d.is_open()), "lỗi thì không đóng, để người dùng không mất chữ");
        assert_eq!(dialog.read_with(cx, |d, cx| d.text(cx)).as_str(), "typed text");
    }

    #[gpui::test]
    fn c4_changing_the_playing_track_closes_the_dialog(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (a, _) = rig.song("a");
        let (b, _) = rig.song("b");
        let (dialog, cx) = open_dialog(cx, |window, cx| TextDialog::lyrics(rig.ops.clone(), window, cx));
        rig.player.update(cx, |p, cx| p.play_track(a.clone(), cx));
        rig.until(cx, "a is current", |cx| rig.player.read_with(cx, |p, _| p.current_track() == Some(&a)));
        open(&dialog, &a, cx);
        ready(&rig, &dialog, cx);
        rig.player.update(cx, |p, cx| p.play_track(b.clone(), cx));
        rig.until(cx, "dialog closed by the track change", |cx| dialog.read_with(cx, |d, _| !d.is_open()));
    }

    #[gpui::test]
    fn c5_timeline_saves_the_raw_text_and_the_player_parses_it(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (track, _) = rig.song("a");
        let raw = "0:00 Intro\nnot a line\n0:01 Verse";
        let (dialog, cx) = open_dialog(cx, |window, cx| timeline_dialog::timeline(rig.ops.clone(), window, cx));
        assert_eq!(dialog.read_with(cx, |d, _| d.kind()), TextKind::Timeline);
        rig.player.update(cx, |p, cx| p.play_track(track.clone(), cx));
        rig.until(cx, "track loaded", |cx| rig.player.read_with(cx, |p, _| p.current_track() == Some(&track)));
        open(&dialog, &track, cx);
        ready(&rig, &dialog, cx);
        dialog.update_in(cx, |d, window, cx| {
            d.set_text(raw, window, cx);
            d.save(window, cx);
        });
        rig.until(cx, "dialog closed", |cx| dialog.read_with(cx, |d, _| !d.is_open()));
        rig.until(cx, "player parsed the timeline", |cx| rig.player.read_with(cx, |p, _| !p.timeline().is_empty()));
        assert_eq!(rig.ops.read(track).wait().unwrap().fields.timeline.as_deref(), Some(raw), "văn bản nguyên gốc");
    }

    #[test]
    fn c6_every_error_variant_has_its_own_catalog_text() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let errors = [
            MetadataError::SongNotFound(1),
            MetadataError::Library(groove_library::actor::LibraryError::Closed),
            MetadataError::RenameFailed { from: "a".into(), to: "b".into(), source: std::io::Error::other("x") },
            MetadataError::DatabaseAfterWrite(groove_library::actor::LibraryError::Closed),
            MetadataError::Tag(groove_library::tags::TagError::NotFound("a".into())),
        ];
        let texts: Vec<_> = errors.iter().map(error_message).collect();
        for text in &texts {
            assert!(!text.is_empty() && !text.contains("song_dialogs."), "thiếu khoá i18n: {text}");
        }
        let unique: std::collections::HashSet<_> = texts.iter().collect();
        assert_eq!(unique.len(), texts.len(), "mỗi biến thể một câu riêng: {texts:?}");
    }
}
