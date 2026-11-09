//! groove-gd1_17: LibraryCtl, nghiệp vụ của các nút trong thư viện (đặc tả 1.2, 1.11, 2.4; N9, S2, S3, S13).
//!
//! Một `Entity` cho cả app, sinh trong `library_page::install`: nghe `LibraryPageEvent`, giữ `Indexer`, `LibraryWatcher`,
//! báo trạng thái nhập / quét về view. UI không chạm đĩa: mọi việc đều chạy nền, kết quả báo bằng [`LibraryCtlEvent`].
//!
//! # Cho task sau
//!
//! - [`ctl`] lấy entity; `import_path`, `rescan`, `remove_category`, `play_category`, `set_category_cover`,
//!   `add_to_playlist`, `remove_from_playlist`, `forget_root`.
//! - Hộp metadata (task 19): `open_metadata`, `open_category_metadata`; `LibraryCtl` giữ entity, `LibraryPage` vẽ.
//! - Hộp timeline, lyrics (task 20): `open_text_dialog`.
//! - Hộp "Save as new" (groove-gd2_03): `open_save_as`; bài mới lưu xong vào thư viện qua `add_file`.
//! - Settings (task 24): danh sách gốc lấy từ `Library::watched_roots`, bỏ gốc bằng `forget_root`.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use futures::StreamExt;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use groove_core::model::{CategoryRef, TrackRef};
use groove_core::{paths, t};
use groove_library::actor::{LibraryError, LibraryEvent, OpenStatus, ProgressKind, SyncReport};
use groove_library::covers::CoverStore;
use groove_library::db::migrate::CoverRecord;
use groove_library::db::queries::{self, CategorySummary};
use groove_library::indexer::{ImportReport, IndexError, IndexTally, Indexer};
use groove_library::store::Library;
use groove_library::watcher::LibraryWatcher;
use materiant::gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, Global, PathPromptOptions, Subscription, Task, WeakEntity, Window,
};
use materiant::toast::{Toast, ToastKind, ToastSpec};

use crate::app;
use crate::player;
use crate::route::Route;
use crate::views::card_menu::ConfirmDelete;
use crate::views::library_page::{LibraryPage, LibraryPageEvent};
use crate::views::lyrics_dialog::TextDialog;
use crate::views::metadata::dialog::{GroupScope, MetadataDialog, MetadataDialogEvent};
use crate::views::playlist_dialog::PlaylistDialog;
use crate::views::save_as_dialog::SaveAsDialog;
use crate::views::timeline_dialog;

/// Hộp chọn của hệ điều hành cần gì.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    Folder,
    CoverImage,
}

/// Mở hộp chọn. Tách thành tham số để test không mở hộp thoại thật.
pub type Picker = Rc<dyn Fn(PickKind, &mut App) -> LocalBoxFuture<'static, Option<PathBuf>>>;

/// Hộp chọn thật của GPUI, không chặn UI. Không lọc được đuôi: tệp chọn nhầm báo lỗi ở `set_category_cover`.
pub fn native_picker() -> Picker {
    Rc::new(|kind, cx| {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: kind == PickKind::CoverImage,
            directories: kind == PickKind::Folder,
            multiple: false,
            prompt: None,
        });
        Box::pin(async move { picked.await.ok()?.ok()??.into_iter().next() })
    })
}

/// Loại hộp sửa của một bài.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongEditKind {
    Timeline,
    Lyrics,
}

#[derive(Clone, Debug)]
pub enum LibraryCtlEvent {
    /// Báo cho người dùng; `install` đổi thành toast.
    Notice { kind: ToastKind, message: String },
}

/// Kết quả của một lần bỏ album, thư mục (N9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Removed {
    pub songs: Vec<i64>,
    /// Danh sách gốc đã đổi: watcher phải đọc lại.
    pub root_removed: bool,
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    paths::is_under(&path.to_string_lossy(), &root.to_string_lossy())
}

/// Xoá dòng của mọi bài và thư mục dưới `root`, dọn album rỗng. Không đụng tệp.
fn delete_rows_under(conn: &rusqlite::Connection, root: &Path) -> queries::Result<Vec<i64>> {
    let songs: Vec<i64> = queries::song_file_states(conn)?
        .into_iter()
        .filter(|song| path_is_under(&song.path, root))
        .map(|song| song.id)
        .collect();
    // Đọc thư mục trước khi xoá bài: thư mục rỗng không còn trong `categories`.
    let folders = queries::categories(conn)?.folders;
    for song in &songs {
        queries::delete_song(conn, *song)?;
    }
    for folder in folders {
        if let (CategoryRef::Folder(id), Some(path)) = (folder.reference, folder.path.as_deref())
            && path_is_under(path, root)
        {
            queries::delete_folder(conn, id)?;
        }
    }
    queries::delete_empty_albums(conn)?;
    Ok(songs)
}

/// Bỏ một thư mục cùng mọi thư mục con khỏi thư viện. Là chính một gốc thì bỏ gốc, còn nằm dưới gốc khác thì ghi
/// `excluded_paths` để lần quét sau không đem về (N9).
fn remove_folder(conn: &rusqlite::Connection, id: i64) -> queries::Result<Removed> {
    let Some(path) = queries::category(conn, CategoryRef::Folder(id))?.and_then(|summary| summary.path) else {
        return Ok(Removed::default());
    };
    let songs = delete_rows_under(conn, &path)?;
    queries::delete_folder(conn, id)?;
    let root_removed = queries::remove_watched_root(conn, &path)?;
    let still_covered = queries::watched_roots(conn)?.iter().any(|root| path_is_under(&path, root));
    if !root_removed || still_covered {
        queries::add_excluded_path(conn, &path)?;
    }
    Ok(Removed { songs, root_removed })
}

/// Bỏ các bài của một album khỏi thư viện, không đụng tệp: ghi từng tệp vào `excluded_paths` (N9). Tính cả bài đang đánh dấu
/// mất, không thì tệp quay lại là bài cũng quay lại.
fn remove_album(conn: &rusqlite::Connection, id: i64) -> queries::Result<Removed> {
    let mut songs = queries::songs(conn, CategoryRef::Album(id))?;
    for missing in queries::missing_songs(conn)? {
        if let Some(song) = queries::song_by_id(conn, missing.id)?.filter(|song| song.album_id == Some(id)) {
            songs.push(song);
        }
    }
    let mut ids = Vec::with_capacity(songs.len());
    for song in &songs {
        queries::add_excluded_path(conn, &song.path)?;
        if let TrackRef::Library(song_id) = song.track {
            queries::delete_song(conn, song_id)?;
            ids.push(song_id);
        }
    }
    queries::delete_album(conn, id)?;
    Ok(Removed { songs: ids, root_removed: false })
}

/// Xoá theo `CategoryRef` trong một transaction, xong phát `Synced` với các bài đã bỏ để hàng đợi và view cập nhật.
pub async fn remove_category_from(library: &Library, reference: CategoryRef) -> Result<Removed, LibraryError> {
    let removed = match reference {
        CategoryRef::Playlist(id) => {
            library.delete_playlist(id).await?;
            return Ok(Removed::default());
        }
        CategoryRef::Folder(id) => library.writer().transaction(move |conn| Ok(remove_folder(conn, id)?)).await?,
        CategoryRef::Album(id) => library.writer().transaction(move |conn| Ok(remove_album(conn, id)?)).await?,
    };
    library.writer().emit(LibraryEvent::Synced(SyncReport { removed: removed.songs.clone(), ..SyncReport::default() }));
    Ok(removed)
}

/// Bỏ một gốc: bài dưới gốc ra khỏi DB, các thư mục đi theo.
pub async fn forget_root_in(library: &Library, root: PathBuf) -> Result<Vec<i64>, LibraryError> {
    let songs = library
        .writer()
        .transaction(move |conn| {
            let songs = delete_rows_under(conn, &root)?;
            queries::remove_watched_root(conn, &root)?;
            Ok(songs)
        })
        .await?;
    library.writer().emit(LibraryEvent::Synced(SyncReport { removed: songs.clone(), ..SyncReport::default() }));
    Ok(songs)
}

/// Gắn bìa người dùng chọn (S13). Playlist không có ảnh.
pub async fn store_category_cover(
    library: &Library,
    reference: CategoryRef,
    record: CoverRecord,
) -> Result<(), LibraryError> {
    library
        .writer()
        .transaction(move |conn| {
            queries::upsert_cover(conn, &record)?;
            match reference {
                CategoryRef::Album(id) => queries::set_album_cover(conn, id, Some(&record.hash), true)?,
                CategoryRef::Folder(id) => queries::set_folder_cover(conn, id, Some(&record.hash))?,
                CategoryRef::Playlist(_) => {}
            }
            Ok(())
        })
        .await?;
    library.writer().emit(LibraryEvent::Synced(SyncReport::default()));
    Ok(())
}

/// Dòng báo sau khi nhập thư mục. Gốc offline, thư mục con lỗi, tệp lỗi là cảnh báo.
pub fn import_notice(report: &ImportReport) -> (ToastKind, String) {
    if report.root_offline {
        return (ToastKind::Warning, t!("actions.import.offline").to_owned());
    }
    let added = report.tally.added;
    let unread = report.failed_dirs.len() + report.tally.failed.len();
    if unread > 0 {
        return (ToastKind::Warning, t!("actions.import.partial", added = added, failed = unread));
    }
    (ToastKind::Success, t!("actions.import.done", added = added, missing = report.marked_missing.len()))
}

/// Toast tóm tắt khi đĩa đổi ngoài app (đặc tả 1.11).
pub fn external_change_notice(report: &SyncReport) -> String {
    t!("actions.sync.changed", added = report.added, updated = report.updated, removed = report.removed.len())
}

pub fn rescan_notice(tally: &IndexTally) -> (ToastKind, String) {
    let kind =
        if tally.missing.is_empty() && tally.failed.is_empty() { ToastKind::Success } else { ToastKind::Warning };
    (kind, t!("actions.rescan.done", read = tally.read(), missing = tally.missing.len(), failed = tally.failed.len()))
}

/// Đang chạy = còn việc của ta chưa xong, hoặc `Progress` chưa tới đích.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Activity {
    jobs: usize,
    in_progress: bool,
}

impl Activity {
    fn running(self) -> bool {
        self.jobs > 0 || self.in_progress
    }

    fn job_finished(&mut self) {
        self.jobs -= 1;
        if self.jobs == 0 {
            self.in_progress = false;
        }
    }
}

pub struct LibraryCtl {
    library: Option<Library>,
    indexer: Option<Indexer>,
    covers: Option<CoverStore>,
    watcher: Option<LibraryWatcher>,
    picker: Picker,
    page: Option<WeakEntity<LibraryPage>>,
    playlists: Arc<Vec<CategorySummary>>,
    import: Activity,
    rescan: Activity,
    /// Hộp chọn đang mở: không mở cái thứ hai.
    picking: bool,
    started: bool,
    playlist_dialog: Option<Entity<PlaylistDialog>>,
    confirm_delete: Option<Entity<ConfirmDelete>>,
    /// Hộp lyrics, timeline: dựng ở lần mở đầu, trang vẽ.
    lyrics_dialog: Option<Entity<TextDialog>>,
    timeline_dialog: Option<Entity<TextDialog>>,
    /// Hộp "Save as new": dựng ở lần mở đầu, trang vẽ.
    save_as_dialog: Option<Entity<SaveAsDialog>>,
    /// Hộp metadata đang mở; `Closed` thì bỏ.
    metadata_dialog: Option<Entity<MetadataDialog>>,
    _metadata_events: Option<Subscription>,
    _events: Option<Task<()>>,
}

impl EventEmitter<LibraryCtlEvent> for LibraryCtl {}

impl LibraryCtl {
    pub fn new(
        library: Option<Library>,
        covers: Option<CoverStore>,
        picker: Picker,
        page: Option<WeakEntity<LibraryPage>>,
        cx: &mut Context<Self>,
    ) -> LibraryCtl {
        let indexer = library.clone().zip(covers.clone()).map(|(library, covers)| Indexer::new(library, covers));
        let mut ctl = LibraryCtl {
            library,
            indexer,
            covers,
            watcher: None,
            picker,
            page,
            playlists: Arc::default(),
            import: Activity::default(),
            rescan: Activity::default(),
            picking: false,
            started: false,
            playlist_dialog: None,
            confirm_delete: None,
            lyrics_dialog: None,
            timeline_dialog: None,
            save_as_dialog: None,
            metadata_dialog: None,
            _metadata_events: None,
            _events: None,
        };
        ctl.listen(cx);
        ctl.reload_playlists(cx);
        ctl
    }

    pub fn library(&self) -> Option<&Library> {
        self.library.as_ref()
    }

    /// Playlist cho menu con "Thêm vào danh sách phát": menu dựng đồng bộ nên giữ sẵn bản đọc gần nhất.
    pub fn playlists(&self) -> &Arc<Vec<CategorySummary>> {
        &self.playlists
    }

    pub fn is_importing(&self) -> bool {
        self.import.running()
    }

    pub fn is_rescanning(&self) -> bool {
        self.rescan.running()
    }

    #[cfg(test)]
    pub fn is_watching(&self) -> bool {
        self.watcher.is_some()
    }

    fn notice(&self, kind: ToastKind, message: impl Into<String>, cx: &mut Context<Self>) {
        cx.emit(LibraryCtlEvent::Notice { kind, message: message.into() });
    }

    fn listen(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &self.library else { return };
        // Nghe trước khi đọc trạng thái, để không lỡ `Opened`.
        let mut events = library.subscribe();
        self._events = Some(cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |this, cx| this.on_event(event, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn on_event(&mut self, event: LibraryEvent, cx: &mut Context<Self>) {
        match event {
            LibraryEvent::Opened(_) => {
                self.start_background(cx);
                self.reload_playlists(cx);
            }
            LibraryEvent::PlaylistsChanged => self.reload_playlists(cx),
            LibraryEvent::Synced(_) => {
                // Việc đọc thẻ đã xong: `Progress` nào chưa tới đích cũng không còn chạy.
                self.import.in_progress = false;
                self.rescan.in_progress = false;
                self.push_activity(cx);
            }
            LibraryEvent::Progress(progress) => {
                let running = progress.done < progress.total;
                match progress.kind {
                    ProgressKind::Import => self.import.in_progress = running,
                    ProgressKind::Rescan => self.rescan.in_progress = running,
                    ProgressKind::TagBackfill | ProgressKind::MetadataBulk => {}
                }
                self.push_activity(cx);
            }
            LibraryEvent::ExternalChange(report) => {
                self.notice(ToastKind::Info, external_change_notice(&report), cx);
            }
            LibraryEvent::OpenFailed(_) | LibraryEvent::Moved(_) => {}
        }
    }

    /// Đưa trạng thái đang chạy về view.
    fn push_activity(&self, cx: &mut Context<Self>) {
        let (importing, rescanning) = (self.is_importing(), self.is_rescanning());
        if let Some(page) = self.page.as_ref().and_then(WeakEntity::upgrade) {
            page.update(cx, |page, cx| {
                page.set_importing(importing, cx);
                page.set_rescanning(rescanning, cx);
            });
        }
    }

    fn reload_playlists(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| match library.playlists().await {
            Ok(playlists) => {
                this.update(cx, |this, cx| {
                    this.playlists = Arc::new(playlists);
                    cx.notify();
                })
                .ok();
            }
            Err(err) => log::debug!("library ctl: cannot read playlists: {err}"),
        })
        .detach();
    }

    /// Mở app: bổ sung thẻ còn thiếu (đặc tả 2.3), gắn watcher. `LibraryWatcher::start` tự so đĩa với DB nên không gọi
    /// `reconcile` riêng (`groove-gd1_09`). Thư viện chưa mở được thì chờ `Opened`.
    pub fn start_if_ready(&mut self, cx: &mut Context<Self>) {
        let ready = self.library.as_ref().is_some_and(|library| matches!(library.status(), OpenStatus::Ready(_)));
        if ready {
            self.start_background(cx);
        }
    }

    fn start_background(&mut self, cx: &mut Context<Self>) {
        let Some(indexer) = self.indexer.clone().filter(|_| !self.started) else { return };
        self.started = true;
        let backfill = indexer.backfill_tags();
        cx.spawn(async move |_, _| {
            if let Err(err) = backfill.await {
                log::warn!("library ctl: tag backfill failed: {err}");
            }
        })
        .detach();
        match LibraryWatcher::start(indexer) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(err) => log::warn!("library ctl: cannot start the watcher: {err}"),
        }
    }

    /// Thoát app: watcher dừng trước, rồi mới đóng thư viện. Chạy ở luồng riêng vì `stop` chờ luồng watcher.
    fn shutdown(&mut self) -> oneshot::Receiver<()> {
        let (done, finished) = oneshot::channel();
        let watcher = self.watcher.take();
        let library = self.library.clone();
        let spawned = thread::Builder::new().name("groove-shutdown".to_owned()).spawn(move || {
            if let Some(watcher) = watcher {
                watcher.stop();
            }
            if let Some(library) = library {
                futures::executor::block_on(library.shutdown());
            }
            let _ = done.send(());
        });
        if let Err(err) = spawned {
            log::error!("library ctl: cannot start the shutdown thread: {err}");
        }
        finished
    }

    fn refresh_roots(&self) {
        if let Some(watcher) = &self.watcher {
            watcher.refresh_roots();
        }
    }

    // ---- Nhập, quét lại ----

    /// Hộp chọn thư mục rồi nhập.
    pub fn pick_and_import(&mut self, cx: &mut Context<Self>) {
        self.pick(PickKind::Folder, cx, |this, path, cx| this.import_path(path, cx));
    }

    fn pick(
        &mut self,
        kind: PickKind,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static,
    ) {
        if self.picking {
            return;
        }
        self.picking = true;
        let picked = (self.picker)(kind, cx);
        cx.spawn(async move |this, cx| {
            let path = picked.await;
            this.update(cx, |this, cx| {
                this.picking = false;
                // Bấm huỷ thì không làm gì, không báo gì.
                if let Some(path) = path {
                    then(this, path, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn import_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(indexer) = self.indexer.clone() else {
            self.notice(ToastKind::Danger, t!("actions.unavailable"), cx);
            return;
        };
        self.import.jobs += 1;
        self.push_activity(cx);
        let job = indexer.import(path);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| {
                this.import.job_finished();
                this.push_activity(cx);
                match result {
                    Ok(report) => {
                        this.refresh_roots();
                        let (kind, message) = import_notice(&report);
                        this.notice(kind, message, cx);
                    }
                    Err(IndexError::RootUnavailable { .. }) => {
                        this.notice(ToastKind::Danger, t!("actions.import.unreadable"), cx);
                    }
                    Err(err) => {
                        log::warn!("library ctl: import failed: {err}");
                        this.notice(ToastKind::Danger, t!("actions.import.failed"), cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        let Some(indexer) = self.indexer.clone() else {
            self.notice(ToastKind::Danger, t!("actions.unavailable"), cx);
            return;
        };
        if self.rescan.running() {
            return;
        }
        self.rescan.jobs += 1;
        self.push_activity(cx);
        let job = indexer.rescan();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| {
                this.rescan.job_finished();
                this.push_activity(cx);
                match result {
                    Ok(tally) => {
                        let (kind, message) = rescan_notice(&tally);
                        this.notice(kind, message, cx);
                    }
                    Err(err) => {
                        log::warn!("library ctl: rescan failed: {err}");
                        this.notice(ToastKind::Danger, t!("actions.rescan.failed"), cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Bỏ một gốc (Settings, task 24): bài dưới đó ra khỏi thư viện, tệp vẫn còn.
    pub fn forget_root(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let result = forget_root_in(&library, root).await;
            this.update(cx, |this, cx| match result {
                Ok(_) => this.refresh_roots(),
                Err(err) => {
                    log::warn!("library ctl: cannot remove a root: {err}");
                    this.notice(ToastKind::Danger, t!("actions.remove.failed"), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- Phát ----

    /// Play all (S2, S3): đọc đúng `reference`, luôn thay hàng đợi; trùng bài đang phát thì `Player` giữ nguyên tiếng.
    pub fn play_category(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let songs = library.songs(reference).await;
            this.update(cx, |this, cx| {
                let tracks: Vec<_> = match songs {
                    Ok(songs) => songs.into_iter().map(|song| song.track).collect(),
                    Err(err) => {
                        log::warn!("library ctl: cannot read {reference:?}: {err}");
                        this.notice(ToastKind::Danger, t!("actions.play.failed"), cx);
                        return;
                    }
                };
                let Some(first) = tracks.first().cloned() else {
                    this.notice(ToastKind::Info, t!("actions.play.empty"), cx);
                    return;
                };
                player::player(cx).update(cx, |player, cx| player.play_list(first, tracks, cx));
            })
            .ok();
        })
        .detach();
    }

    // ---- Xoá ----

    /// Hộp xác nhận rồi mới xoá.
    pub fn request_delete(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        let (Some(library), Some(dialog)) = (self.library.clone(), self.confirm_delete.clone()) else { return };
        cx.spawn(async move |_, cx| {
            let name = library.category(reference).await.ok().flatten().map(|summary| summary.name);
            let Some(name) = name else { return };
            cx.update(|cx| {
                with_window(cx, move |window, cx| dialog.update(cx, |d, cx| d.open(reference, name, window, cx)));
            });
        })
        .detach();
    }

    /// Bỏ khỏi thư viện (N9); không đụng tệp.
    pub fn remove_category(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let result = remove_category_from(&library, reference).await;
            this.update(cx, |this, cx| match result {
                Ok(removed) => {
                    if removed.root_removed {
                        this.refresh_roots();
                    }
                }
                Err(err) => {
                    log::warn!("library ctl: cannot remove {reference:?}: {err}");
                    this.notice(ToastKind::Danger, t!("actions.remove.failed"), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- Playlist ----

    /// Hộp tạo playlist; `add_song` là bài thêm luôn sau khi tạo (đặc tả 1.2, tạo từ menu bài).
    pub fn open_create_playlist(&mut self, add_song: Option<i64>, cx: &mut Context<Self>) {
        let Some(dialog) = self.playlist_dialog.clone() else { return };
        with_window(cx, move |window, cx| dialog.update(cx, |d, cx| d.open(add_song, window, cx)));
    }

    pub fn add_to_playlist(&mut self, playlist: i64, song: i64, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let result = library.add_song_to_playlist(playlist, song).await;
            this.update(cx, |this, cx| match result {
                Ok(true) => this.notice(ToastKind::Success, t!("actions.playlist.added"), cx),
                Ok(false) => this.notice(ToastKind::Info, t!("actions.playlist.already_in"), cx),
                Err(err) => {
                    log::warn!("library ctl: cannot add to a playlist: {err}");
                    this.notice(ToastKind::Danger, t!("actions.playlist.add_failed"), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn remove_from_playlist(&mut self, playlist: i64, song: i64, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            if let Err(err) = library.remove_song_from_playlist(playlist, song).await {
                log::warn!("library ctl: cannot remove from a playlist: {err}");
                this.update(cx, |this, cx| this.notice(ToastKind::Danger, t!("actions.playlist.remove_failed"), cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Mở hộp lyrics hoặc timeline của `track`.
    pub fn open_text_dialog(&mut self, kind: SongEditKind, track: TrackRef, cx: &mut Context<Self>) {
        if self.library.is_none() {
            return;
        }
        let existing = match kind {
            SongEditKind::Lyrics => self.lyrics_dialog.clone(),
            SongEditKind::Timeline => self.timeline_dialog.clone(),
        };
        let Some(ops) = app::try_metadata_ops(cx) else {
            self.notice(ToastKind::Danger, t!("actions.unavailable"), cx);
            return;
        };
        let this = cx.entity();
        with_window(cx, move |window, cx| {
            let dialog = existing.unwrap_or_else(|| {
                let dialog = cx.new(|cx| match kind {
                    SongEditKind::Lyrics => TextDialog::lyrics(ops, window, cx),
                    SongEditKind::Timeline => timeline_dialog::timeline(ops, window, cx),
                });
                this.update(cx, |this, cx| this.adopt_text_dialog(kind, dialog.clone(), cx));
                dialog
            });
            dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
        });
    }

    fn adopt_text_dialog(&mut self, kind: SongEditKind, dialog: Entity<TextDialog>, cx: &mut Context<Self>) {
        match kind {
            SongEditKind::Lyrics => self.lyrics_dialog = Some(dialog.clone()),
            SongEditKind::Timeline => self.timeline_dialog = Some(dialog.clone()),
        }
        if let Some(page) = self.page.as_ref().and_then(WeakEntity::upgrade) {
            page.update(cx, |page, cx| page.add_overlay(dialog.into(), cx));
        }
    }

    /// Mở hộp "Save as new" cho `track` (groove-gd2_03).
    pub fn open_save_as(&mut self, track: TrackRef, cx: &mut Context<Self>) {
        let existing = self.save_as_dialog.clone();
        let library = self.library.clone();
        let this = cx.entity();
        with_window(cx, move |window, cx| {
            let dialog = existing.unwrap_or_else(|| {
                let dialog = cx.new(|cx| SaveAsDialog::new(library, window, cx));
                this.update(cx, |this, cx| {
                    this.save_as_dialog = Some(dialog.clone());
                    if let Some(page) = this.page.as_ref().and_then(WeakEntity::upgrade) {
                        page.update(cx, |page, cx| page.add_overlay(dialog.clone().into(), cx));
                    }
                });
                dialog
            });
            dialog.update(cx, |dialog, cx| dialog.open(track, window, cx));
        });
    }

    #[cfg(test)]
    pub fn save_as_dialog(&self) -> Option<&Entity<SaveAsDialog>> {
        self.save_as_dialog.as_ref()
    }

    /// Đưa một tệp vừa lưu vào thư viện (`Indexer::index_paths`, ở luồng nền), xong thì nạp lại danh sách và toast. `name` là
    /// tên tệp hiện trong toast.
    pub fn add_file(&mut self, path: PathBuf, name: String, cx: &mut Context<Self>) {
        let Some(indexer) = self.indexer.clone() else {
            self.notice(ToastKind::Success, t!("save_as.saved", name = name), cx);
            return;
        };
        let job = indexer.index_paths(vec![path]);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| match result {
                Ok(tally) if tally.read() > 0 => {
                    if let Some(page) = this.page.as_ref().and_then(WeakEntity::upgrade) {
                        page.update(cx, |page, cx| page.refresh_lists(cx));
                    }
                    this.notice(ToastKind::Success, t!("save_as.added", name = name), cx);
                }
                other => {
                    if let Err(err) = other {
                        log::warn!("library ctl: cannot add the saved file: {err}");
                    }
                    this.notice(ToastKind::Warning, t!("save_as.saved", name = name), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- Metadata (task 19) ----

    /// Mở từ dòng bài. Album, thư mục thì có ô áp cho cả nhóm; playlist không phải nhóm thẻ nên `None`.
    pub fn open_metadata(&mut self, track: TrackRef, from: CategoryRef, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let scope = match from {
                CategoryRef::Playlist(_) => None,
                _ => library
                    .category(from)
                    .await
                    .ok()
                    .flatten()
                    .map(|summary| GroupScope { category: from, songs: summary.song_count as usize }),
            };
            this.update(cx, |this, cx| this.show_metadata(track, scope, cx)).ok();
        })
        .detach();
    }

    /// Mở từ thẻ: sửa từ bài đầu, áp được cho cả nhóm. Danh sách rỗng thì toast.
    pub fn open_category_metadata(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        let Some(library) = self.library.clone() else { return };
        cx.spawn(async move |this, cx| {
            let songs = library.songs(reference).await.unwrap_or_default();
            this.update(cx, |this, cx| {
                let Some(first) = songs.first() else {
                    this.notice(ToastKind::Info, t!("actions.play.empty"), cx);
                    return;
                };
                let scope = GroupScope { category: reference, songs: songs.len() };
                this.show_metadata(first.track.clone(), Some(scope), cx);
            })
            .ok();
        })
        .detach();
    }

    fn show_metadata(&mut self, track: TrackRef, scope: Option<GroupScope>, cx: &mut Context<Self>) {
        if self.metadata_dialog.is_some() {
            return;
        }
        let Some(ops) = app::try_metadata_ops(cx) else {
            self.notice(ToastKind::Danger, t!("actions.unavailable"), cx);
            return;
        };
        let this = cx.entity();
        with_window(cx, move |window, cx| {
            // Hai lần mở sát nhau cùng chờ ở đây: lần sau không được thay mất hộp của lần trước.
            if this.read(cx).metadata_dialog.is_some() {
                return;
            }
            let dialog = MetadataDialog::open(ops, track, scope, window, cx);
            this.update(cx, |this, cx| this.adopt_metadata(dialog, cx));
        });
    }

    fn adopt_metadata(&mut self, dialog: Entity<MetadataDialog>, cx: &mut Context<Self>) {
        self._metadata_events = Some(cx.subscribe(&dialog, |this, _, event: &MetadataDialogEvent, cx| match event {
            MetadataDialogEvent::Saved => {
                if let Some(page) = this.page.as_ref().and_then(WeakEntity::upgrade) {
                    page.update(cx, |page, cx| page.refresh_lists(cx));
                }
            }
            MetadataDialogEvent::Closed => {
                this.metadata_dialog = None;
                this._metadata_events = None;
                this.push_metadata(cx);
            }
        }));
        self.metadata_dialog = Some(dialog);
        self.push_metadata(cx);
    }

    fn push_metadata(&self, cx: &mut Context<Self>) {
        let dialog = self.metadata_dialog.clone();
        if let Some(page) = self.page.as_ref().and_then(WeakEntity::upgrade) {
            page.update(cx, |page, cx| page.set_metadata_dialog(dialog, cx));
        }
    }

    // ---- Bìa ----

    pub fn pick_category_cover(&mut self, reference: CategoryRef, cx: &mut Context<Self>) {
        self.pick(PickKind::CoverImage, cx, move |this, path, cx| this.set_category_cover(reference, path, cx));
    }

    /// Đặt bìa từ tệp ảnh (đặc tả 2.4, S13). Đọc và giải mã ở nền; ảnh hỏng báo toast, bìa cũ giữ nguyên.
    pub fn set_category_cover(&mut self, reference: CategoryRef, path: PathBuf, cx: &mut Context<Self>) {
        let (Some(library), Some(covers)) = (self.library.clone(), self.covers.clone()) else { return };
        if matches!(reference, CategoryRef::Playlist(_)) {
            return;
        }
        let saved = cx.background_spawn(async move {
            let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
            covers.save(&bytes).map_err(|err| err.to_string())
        });
        cx.spawn(async move |this, cx| {
            let outcome = match saved.await {
                Ok(record) => store_category_cover(&library, reference, record).await.map_err(|err| err.to_string()),
                Err(err) => Err(err),
            };
            if let Err(err) = outcome {
                log::warn!("library ctl: cannot set the cover: {err}");
                this.update(cx, |this, cx| this.notice(ToastKind::Danger, t!("actions.cover.failed"), cx)).ok();
            }
        })
        .detach();
    }
}

/// Chạy với cửa sổ chính (app chỉ có một cửa sổ). Luôn hoãn: nơi gọi có thể đang trong `on_select` của menu (cửa sổ đang bị
/// mượn, `update` sẽ lỗi) hoặc trong `update` của chính `LibraryCtl`.
pub fn with_window(cx: &mut App, f: impl FnOnce(&mut Window, &mut App) + 'static) {
    cx.defer(move |cx| {
        if let Some(window) = cx.windows().first().copied()
            && let Err(err) = window.update(cx, |_, window, cx| f(window, cx))
        {
            log::warn!("library ctl: no window: {err}");
        }
    });
}

struct CtlHandle(Entity<LibraryCtl>);

impl Global for CtlHandle {}

/// Entity dùng chung; `None` khi chưa `install` (test của view khác).
pub fn ctl(cx: &App) -> Option<Entity<LibraryCtl>> {
    cx.try_global::<CtlHandle>().map(|handle| handle.0.clone())
}

fn handle_page_event(ctl: &Entity<LibraryCtl>, event: LibraryPageEvent, cx: &mut App) {
    ctl.update(cx, |ctl, cx| match event {
        LibraryPageEvent::Import => ctl.pick_and_import(cx),
        LibraryPageEvent::Rescan => ctl.rescan(cx),
        LibraryPageEvent::CreatePlaylist => ctl.open_create_playlist(None, cx),
        LibraryPageEvent::PlayAll(reference) => ctl.play_category(reference, cx),
        LibraryPageEvent::Delete(reference) => ctl.request_delete(reference, cx),
        LibraryPageEvent::Open(_) | LibraryPageEvent::CategoryGone(_) => {}
    });
}

/// Dựng `LibraryCtl`, nối với view, hộp thoại, toast và vòng đời app. `library_page::install_with` gọi một lần.
pub fn install(
    library: Option<Library>,
    covers: Option<CoverStore>,
    page: &Entity<LibraryPage>,
    cx: &mut App,
) -> Entity<LibraryCtl> {
    let weak_page = page.downgrade();
    let ctl = cx.new(|cx| LibraryCtl::new(library, covers, native_picker(), Some(weak_page), cx));
    connect(&ctl, page, cx);
    ctl
}

/// Phần nối của [`install`], tách ra để test đưa `Picker` giả vào.
pub fn connect(ctl: &Entity<LibraryCtl>, page: &Entity<LibraryPage>, cx: &mut App) {
    cx.set_global(CtlHandle(ctl.clone()));

    let page_ctl = ctl.clone();
    cx.subscribe(page, move |_, event: &LibraryPageEvent, cx| {
        handle_page_event(&page_ctl, *event, cx);
        if let LibraryPageEvent::CategoryGone(reference) = event
            && app::current_route(cx) == Route::Category(*reference)
        {
            app::go_back(cx);
        }
    })
    .detach();

    cx.subscribe(ctl, |_, event: &LibraryCtlEvent, cx| {
        let LibraryCtlEvent::Notice { kind, message } = event;
        let spec = ToastSpec::new(*kind, message.clone());
        with_window(cx, move |window, cx| {
            Toast::push(window, cx, spec);
        });
    })
    .detach();

    let weak_ctl = ctl.downgrade();
    let (owner, weak_page) = (ctl.clone(), page.downgrade());
    with_window(cx, move |window, cx| {
        let library = owner.read(cx).library().cloned();
        let playlist_dialog = cx.new(|cx| PlaylistDialog::new(library, weak_ctl.clone(), window, cx));
        let confirm_delete = cx.new(|cx| ConfirmDelete::new(weak_ctl, cx));
        owner.update(cx, |ctl, _| {
            ctl.playlist_dialog = Some(playlist_dialog.clone());
            ctl.confirm_delete = Some(confirm_delete.clone());
        });
        weak_page
            .update(cx, |page, cx| page.set_overlays(vec![playlist_dialog.into(), confirm_delete.into()], cx))
            .ok();
    });

    let quit_ctl = ctl.clone();
    cx.on_app_quit(move |cx| {
        let finished = quit_ctl.update(cx, |ctl, _| ctl.shutdown());
        async move {
            let _ = finished.await;
        }
    })
    .detach();

    ctl.update(cx, |ctl, cx| ctl.start_if_ready(cx));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::time::Duration;

    use gpui::{TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::test_dir::TestDir;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::indexer::IndexTally;
    use groove_library::metadata_ops::MetadataOps;
    use groove_library::store::LibraryConfig;
    use materiant::gpui_kit::base::Root;

    use super::*;
    use crate::player::PlayerDeps;
    use crate::route::Route;

    struct Env {
        dir: TestDir,
        library: Library,
        covers: CoverStore,
    }

    fn env() -> Env {
        let dir = TestDir::new();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let covers = CoverStore::new(dir.path().join("covers"));
        let library = Library::open(config, Box::new(CoverStore::new(dir.path().join("covers")))).unwrap();
        Env { dir, library, covers }
    }

    impl Env {
        fn indexer(&self) -> Indexer {
            Indexer::new(self.library.clone(), self.covers.clone())
        }

        /// Cây `music/{A/a1,a2 ; A/sub/s1 ; B/b1}` đã nhập; trả gốc.
        fn import_tree(&self) -> PathBuf {
            let root = self.dir.path().join("music");
            for (dir, stem) in [("A", "a1"), ("A", "a2"), ("A/sub", "s1"), ("B", "b1")] {
                let dir = root.join(dir);
                fs::create_dir_all(&dir).unwrap();
                fixtures::write_sample(&dir, stem, AudioKind::Wav).unwrap();
            }
            self.indexer().import(root.clone()).wait().unwrap();
            root
        }

        fn songs_count(&self) -> usize {
            let library = self.library.clone();
            library.read(|conn| Ok(queries::song_file_states(conn)?.len())).wait().unwrap()
        }

        fn folder(&self, name: &str) -> CategoryRef {
            let categories = self.library.categories().wait().unwrap();
            categories.folders.iter().find(|folder| folder.name == name).expect("có thư mục").reference
        }
    }

    fn block<T>(future: impl Future<Output = T>) -> T {
        futures::executor::block_on(future)
    }

    /// C1 (N9) — bỏ thư mục dưới gốc: bài, thư mục con ra khỏi DB, tệp còn, quét lại không về.
    #[test]
    fn removed_folder_stays_out_after_a_rescan_and_keeps_files() {
        let env = env();
        let root = env.import_tree();
        assert_eq!(env.songs_count(), 4);
        let removed = block(remove_category_from(&env.library, env.folder("A"))).unwrap();
        assert_eq!(removed.songs.len(), 3, "a1, a2 và thư mục con");
        assert!(!removed.root_removed);
        assert_eq!(env.songs_count(), 1);
        assert!(root.join("A/a1.wav").exists(), "không đụng tệp");

        env.indexer().import(root.clone()).wait().unwrap();
        assert_eq!(env.songs_count(), 1, "nhập lại gốc cha không đem về chỗ đã bỏ");
        let excluded = env.library.read(|conn| Ok(queries::excluded_paths(conn)?)).wait().unwrap();
        assert_eq!(excluded.len(), 1);
    }

    /// C2 (N9) — bỏ thư mục đúng là gốc: bỏ gốc, không ghi `excluded_paths`; chọn lại vẫn nhập được.
    #[test]
    fn removing_a_root_folder_forgets_the_root() {
        let env = env();
        let dir = env.dir.path().join("solo");
        fs::create_dir_all(&dir).unwrap();
        fixtures::write_sample(&dir, "only", AudioKind::Wav).unwrap();
        env.indexer().import(dir.clone()).wait().unwrap();
        let removed = block(remove_category_from(&env.library, env.folder("solo"))).unwrap();
        assert!(removed.root_removed);
        assert!(env.library.watched_roots().wait().unwrap().is_empty());
        assert!(env.library.read(|conn| Ok(queries::excluded_paths(conn)?)).wait().unwrap().is_empty());
        env.indexer().import(dir).wait().unwrap();
        assert_eq!(env.songs_count(), 1);
    }

    /// C3 (N9) — bỏ album: từng tệp vào `excluded_paths`, quét lại không về, tệp còn; bài của album khác giữ nguyên.
    #[test]
    fn removed_album_stays_out_after_import() {
        let env = env();
        let root = env.import_tree();
        let (album, songs) = env
            .library
            .writer()
            .transaction(|conn| {
                let album = queries::get_or_create_album(conn, "Pick", "Pick", None)?;
                let mut songs = Vec::new();
                for state in queries::song_file_states(conn)?.into_iter().take(2) {
                    let song = queries::song_by_id(conn, state.id)?.unwrap();
                    queries::upsert_song(
                        conn,
                        &queries::SongUpsert {
                            path: state.path.clone(),
                            folder_id: song.folder_id.unwrap(),
                            album_id: Some(album),
                            ..queries::SongUpsert::default()
                        },
                    )?;
                    songs.push(state.path);
                }
                Ok((album, songs))
            })
            .wait()
            .unwrap();
        block(remove_category_from(&env.library, CategoryRef::Album(album))).unwrap();
        assert_eq!(env.songs_count(), 2);
        assert!(songs.iter().all(|path| path.exists()));
        env.indexer().import(root).wait().unwrap();
        assert_eq!(env.songs_count(), 2, "nhập lại không đem bài của album đã bỏ về");
    }

    /// C4 (N9) — bỏ gốc từ Settings: không còn bài, không còn gốc.
    #[test]
    fn forgetting_a_root_removes_its_songs() {
        let env = env();
        let root = env.import_tree();
        let songs = block(forget_root_in(&env.library, root)).unwrap();
        assert_eq!(songs.len(), 4);
        assert_eq!(env.songs_count(), 0);
        assert!(env.library.watched_roots().wait().unwrap().is_empty());
    }

    #[test]
    fn notices_tell_what_happened() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let mut report = ImportReport { root_offline: true, ..ImportReport::default() };
        assert_eq!(import_notice(&report).0, ToastKind::Warning);
        report.root_offline = false;
        report.tally.added = 3;
        assert_eq!(import_notice(&report).0, ToastKind::Success);
        assert!(import_notice(&report).1.contains('3'));
        report.failed_dirs.push("x".into());
        assert_eq!(import_notice(&report).0, ToastKind::Warning);

        let mut tally = IndexTally { updated: 2, ..IndexTally::default() };
        assert_eq!(rescan_notice(&tally).0, ToastKind::Success);
        tally.missing.push("y".into());
        assert_eq!(rescan_notice(&tally).0, ToastKind::Warning);
    }

    #[test]
    fn activity_runs_until_job_and_progress_end() {
        let mut activity = Activity { jobs: 1, in_progress: true };
        assert!(activity.running());
        activity.job_finished();
        assert!(!activity.running());
        assert!(Activity { jobs: 0, in_progress: true }.running());
    }

    // ---- Trong App ảo: thư viện tạm, `FakeOutput`, `Picker` giả ----

    const TINY_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21,
        196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2, 135, 1, 128, 235, 71, 186,
        146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    type Notices = Rc<RefCell<Vec<(ToastKind, String)>>>;

    fn wait_until(cx: &mut TestAppContext, what: &str, mut done: impl FnMut(&mut TestAppContext) -> bool) {
        for _ in 0..800 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("quá hạn chờ: {what}");
    }

    fn fake_picker(answer: Option<PathBuf>, asked: Rc<RefCell<Vec<PickKind>>>) -> Picker {
        Rc::new(move |kind, _| {
            asked.borrow_mut().push(kind);
            let answer = answer.clone();
            Box::pin(async move { answer })
        })
    }

    fn ctl_with(cx: &mut TestAppContext, env: &Env, picker: Picker) -> (Entity<LibraryCtl>, Notices) {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.executor().allow_parking();
        cx.update(materiant::init);
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let ctl = cx.update(|cx| cx.new(|cx| LibraryCtl::new(Some(library), Some(covers), picker, None, cx)));
        let notices: Notices = Rc::default();
        let log = notices.clone();
        cx.update(|cx| {
            cx.subscribe(&ctl, move |_, event: &LibraryCtlEvent, _| {
                let LibraryCtlEvent::Notice { kind, message } = event;
                log.borrow_mut().push((*kind, message.clone()));
            })
            .detach();
        });
        (ctl, notices)
    }

    fn install_player(cx: &mut TestAppContext, env: &Env) -> Entity<player::Player> {
        let library = env.library.clone();
        cx.update(|cx| {
            player::install_for_test(
                PlayerDeps {
                    library: Some(library),
                    output: Box::new(FakeOutput::new(48_000, 2)),
                    options: EngineOptions::default(),
                    session_file: None,
                },
                cx,
            )
        })
    }

    /// C5 — chọn thư mục ⇒ nhập, xong toast đếm bài; huỷ thì không làm gì; đang chạy thì `is_importing`.
    #[gpui::test]
    fn import_goes_through_the_picker(cx: &mut TestAppContext) {
        let env = env();
        let dir = env.dir.path().join("in");
        fs::create_dir_all(&dir).unwrap();
        fixtures::write_sample(&dir, "one", AudioKind::Wav).unwrap();

        let asked: Rc<RefCell<Vec<PickKind>>> = Rc::default();
        let (cancel, notices) = ctl_with(cx, &env, fake_picker(None, asked.clone()));
        cancel.update(cx, |ctl, cx| ctl.pick_and_import(cx));
        cx.run_until_parked();
        assert_eq!(*asked.borrow(), [PickKind::Folder]);
        assert!(notices.borrow().is_empty() && !cancel.read_with(cx, |c, _| c.is_importing()));

        let (ctl, notices) = ctl_with(cx, &env, fake_picker(Some(dir), asked));
        ctl.update(cx, |ctl, cx| ctl.pick_and_import(cx));
        wait_until(cx, "nhập xong", |_| !notices.borrow().is_empty());
        assert_eq!(notices.borrow()[0].0, ToastKind::Success);
        assert!(notices.borrow()[0].1.contains('1'), "{:?}", notices.borrow());
        assert_eq!(env.songs_count(), 1);
        assert!(!ctl.read_with(cx, |c, _| c.is_importing()));
    }

    /// C6 — thư mục không đọc được báo bằng chữ, không kẹt trạng thái đang nhập.
    #[gpui::test]
    fn unreadable_import_reports_and_clears(cx: &mut TestAppContext) {
        let env = env();
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let gone = env.dir.path().join("does-not-exist");
        ctl.update(cx, |ctl, cx| ctl.import_path(gone, cx));
        assert!(ctl.read_with(cx, |c, _| c.is_importing()), "báo đang nhập ngay");
        wait_until(cx, "báo lỗi", |_| !notices.borrow().is_empty());
        assert_eq!(notices.borrow()[0].0, ToastKind::Danger);
        assert!(!ctl.read_with(cx, |c, _| c.is_importing()));
    }

    /// C7 — `Progress` bật cờ; `done == total` hoặc `Synced` tắt.
    #[gpui::test]
    fn progress_drives_the_flags(cx: &mut TestAppContext) {
        use groove_library::actor::Progress;

        let env = env();
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let emit = |kind, done, total| {
            env.library.writer().emit(LibraryEvent::Progress(Progress { kind, done, total }));
        };
        emit(ProgressKind::Import, 1, 3);
        wait_until(cx, "bật nhập", |cx| ctl.read_with(cx, |c, _| c.is_importing()));
        emit(ProgressKind::Import, 3, 3);
        wait_until(cx, "tắt nhập", |cx| !ctl.read_with(cx, |c, _| c.is_importing()));
        emit(ProgressKind::Rescan, 1, 3);
        wait_until(cx, "bật quét", |cx| ctl.read_with(cx, |c, _| c.is_rescanning()));
        env.library.writer().emit(LibraryEvent::Synced(SyncReport::default()));
        wait_until(cx, "tắt quét", |cx| !ctl.read_with(cx, |c, _| c.is_rescanning()));
    }

    /// Đặc tả 1.11: đĩa đổi ngoài app thì có toast tóm tắt; `Synced` của import, quét lại, thao tác trong app thì không.
    #[gpui::test]
    fn outside_changes_get_a_summary_toast(cx: &mut TestAppContext) {
        let env = env();
        let (_ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        env.library.writer().emit(LibraryEvent::Synced(SyncReport { added: 9, ..SyncReport::default() }));
        cx.run_until_parked();
        assert!(notices.borrow().is_empty(), "{:?}", notices.borrow());
        let report = SyncReport { added: 2, updated: 3, removed: vec![1] };
        env.library.writer().emit(LibraryEvent::ExternalChange(report));
        wait_until(cx, "toast", |_| !notices.borrow().is_empty());
        let (kind, message) = notices.borrow()[0].clone();
        assert_eq!(kind, ToastKind::Info);
        assert!(message.contains('2') && message.contains('3') && message.contains('1'), "{message}");
    }

    /// C8 — mở app: watcher chạy, một lần duy nhất; `shutdown` dừng watcher rồi đóng thư viện.
    #[gpui::test]
    fn starts_once_and_shuts_down_in_order(cx: &mut TestAppContext) {
        let env = env();
        env.library.playlists().wait().unwrap();
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        wait_until(cx, "thư viện mở", |_| matches!(env.library.status(), OpenStatus::Ready(_)));
        ctl.update(cx, |ctl, cx| ctl.start_if_ready(cx));
        assert!(ctl.read_with(cx, |c, _| c.is_watching()));
        ctl.update(cx, |ctl, cx| ctl.start_if_ready(cx));
        cx.run_until_parked();
        let finished = ctl.update(cx, |ctl, _| ctl.shutdown());
        block(finished).unwrap();
        assert!(!ctl.read_with(cx, |c, _| c.is_watching()));
        assert!(matches!(env.library.playlists().wait(), Err(LibraryError::Closed)));
    }

    /// C9 (S2) — đang phát bài đầu của album: Play all vẫn thay hàng đợi, không đổi bài.
    #[gpui::test]
    fn play_all_replaces_the_queue_when_the_first_song_is_playing(cx: &mut TestAppContext) {
        let env = env();
        env.import_tree();
        let player = install_player(cx, &env);
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let reference = env.folder("A");
        let tracks: Vec<TrackRef> =
            env.library.songs(reference).wait().unwrap().into_iter().map(|song| song.track).collect();
        assert!(tracks.len() >= 2);
        let first = tracks[0].clone();
        // Đang nghe bài đầu với hàng đợi chỉ có nó.
        player.update(cx, |p, cx| p.play_list(first.clone(), vec![first.clone()], cx));
        wait_until(cx, "đang phát", |cx| player.read_with(cx, |p, _| p.current_track() == Some(&first)));
        assert_eq!(player.read_with(cx, |p, _| p.queue().visible_len()), 1);

        ctl.update(cx, |ctl, cx| ctl.play_category(reference, cx));
        wait_until(cx, "thay hàng đợi", |cx| player.read_with(cx, |p, _| p.queue().visible_len() == tracks.len()));
        assert_eq!(player.read_with(cx, |p, _| p.current_track().cloned()), Some(first), "không đổi bài");
    }

    /// C10 (S3) — hai thư mục cùng tên: Play all theo `CategoryRef`, không theo tên.
    #[gpui::test]
    fn play_all_follows_the_reference_not_the_name(cx: &mut TestAppContext) {
        let env = env();
        let (one, two) = (env.dir.path().join("x/Album"), env.dir.path().join("y/Album"));
        for (dir, stem) in [(&one, "first"), (&two, "second")] {
            fs::create_dir_all(dir).unwrap();
            fixtures::write_sample(dir, stem, AudioKind::Wav).unwrap();
            env.indexer().import(dir.clone()).wait().unwrap();
        }
        let folders = env.library.categories().wait().unwrap().folders;
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, folders[1].name);
        let second = folders.iter().find(|folder| folder.path.as_deref() == Some(two.as_path())).unwrap().reference;
        let expected = env.library.songs(second).wait().unwrap()[0].track.clone();

        let player = install_player(cx, &env);
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        ctl.update(cx, |ctl, cx| ctl.play_category(second, cx));
        wait_until(cx, "phát", |cx| player.read_with(cx, |p, _| p.current_track() == Some(&expected)));
    }

    /// C11 — danh sách rỗng: toast, không đụng hàng đợi.
    #[gpui::test]
    fn play_all_on_an_empty_list_tells_the_user(cx: &mut TestAppContext) {
        let env = env();
        let id = env.library.create_playlist("Empty", "#06b6d4").wait().unwrap();
        let player = install_player(cx, &env);
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        ctl.update(cx, |ctl, cx| ctl.play_category(CategoryRef::Playlist(id), cx));
        wait_until(cx, "toast", |_| !notices.borrow().is_empty());
        assert_eq!(notices.borrow()[0], (ToastKind::Info, t!("actions.play.empty").to_owned()));
        assert_eq!(player.read_with(cx, |p, _| p.current_track().cloned()), None);
    }

    /// C12 (S13) — đặt bìa: ảnh thật ghi vào album; ảnh hỏng báo lỗi, bìa cũ giữ nguyên.
    #[gpui::test]
    fn cover_is_set_and_bad_images_are_reported(cx: &mut TestAppContext) {
        let env = env();
        let id = env
            .library
            .writer()
            .transaction(|conn| {
                let folder = queries::get_or_create_folder(conn, "F", Path::new("/f"))?;
                let album = queries::get_or_create_album(conn, "Al", "Al", None)?;
                queries::upsert_song(
                    conn,
                    &queries::SongUpsert {
                        path: "/f/a.mp3".into(),
                        folder_id: folder,
                        album_id: Some(album),
                        ..queries::SongUpsert::default()
                    },
                )?;
                Ok(album)
            })
            .wait()
            .unwrap();
        let reference = CategoryRef::Album(id);
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));

        let bad = env.dir.path().join("bad.png");
        fs::write(&bad, b"not an image").unwrap();
        ctl.update(cx, |ctl, cx| ctl.set_category_cover(reference, bad, cx));
        wait_until(cx, "ảnh hỏng", |_| !notices.borrow().is_empty());
        assert_eq!(notices.borrow()[0].0, ToastKind::Danger);
        assert!(env.library.category(reference).wait().unwrap().unwrap().cover_hash.is_none());

        let good = env.dir.path().join("good.png");
        fs::write(&good, TINY_PNG).unwrap();
        ctl.update(cx, |ctl, cx| ctl.set_category_cover(reference, good, cx));
        wait_until(cx, "bìa mới", |_| env.library.category(reference).wait().unwrap().unwrap().cover_hash.is_some());
    }

    /// C13 — thêm vào playlist; thêm lần hai báo đã có; menu đọc được playlist mới.
    #[gpui::test]
    fn add_to_playlist_reports_duplicates(cx: &mut TestAppContext) {
        let env = env();
        env.import_tree();
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let playlist = env.library.create_playlist("Gym", "#ff0000").wait().unwrap();
        wait_until(cx, "playlist mới", |cx| ctl.read_with(cx, |c, _| c.playlists().len() == 1));
        let TrackRef::Library(song) = env.library.songs(env.folder("A")).wait().unwrap()[0].track else {
            unreachable!()
        };
        ctl.update(cx, |ctl, cx| ctl.add_to_playlist(playlist, song, cx));
        wait_until(cx, "đã thêm", |_| notices.borrow().len() == 1);
        ctl.update(cx, |ctl, cx| ctl.add_to_playlist(playlist, song, cx));
        wait_until(cx, "đã có", |_| notices.borrow().len() == 2);
        assert_eq!(notices.borrow()[0].1, t!("actions.playlist.added"));
        assert_eq!(notices.borrow()[1].1, t!("actions.playlist.already_in"));
        ctl.update(cx, |ctl, cx| ctl.remove_from_playlist(playlist, song, cx));
        wait_until(cx, "đã xoá", |_| env.library.songs(CategoryRef::Playlist(playlist)).wait().unwrap().is_empty());
    }

    /// C14 — nối đủ: sự kiện view chạy nghiệp vụ; xác nhận xoá mới xoá; tạo playlist từ hộp, trùng tên báo dưới ô.
    #[gpui::test]
    fn page_events_reach_dialogs_and_business(cx: &mut TestAppContext) {
        use crate::views::library_page::LibraryPage;

        let env = env();
        env.import_tree();
        let _player = install_player(cx, &env);
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let window = cx.add_window(|window, cx| {
            let page = cx.new(|cx| LibraryPage::new(Some(library), Some(covers), Route::MyMusic, cx));
            Root::new(page, window, cx)
        });
        let page = window.update(cx, |root, _, _| root.view().clone().downcast::<LibraryPage>().unwrap()).unwrap();
        ctl.update(cx, |ctl, _| ctl.page = Some(page.downgrade()));
        cx.update(|cx| connect(&ctl, &page, cx));
        let cx_window = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx_window.run_until_parked();
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
        };
        draw(cx_window);
        assert!(
            ctl.read_with(cx_window, |c, _| c.is_watching() || !matches!(env.library.status(), OpenStatus::Ready(_)))
        );

        // Xoá: phải qua hộp xác nhận.
        let folder = env.folder("B");
        page.update(cx_window, |_, cx| cx.emit(LibraryPageEvent::Delete(folder)));
        let dialog = ctl.read_with(cx_window, |c, _| c.confirm_delete.clone()).unwrap();
        for _ in 0..500 {
            cx_window.run_until_parked();
            if dialog.read_with(cx_window, |d, _| d.is_open()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(dialog.read_with(cx_window, |d, _| d.is_open()));
        assert_eq!(env.songs_count(), 4, "chưa xác nhận thì chưa xoá");
        draw(cx_window);
        dialog.update_in(cx_window, |d, window, cx| d.confirm_for_test(window, cx));
        for _ in 0..500 {
            cx_window.run_until_parked();
            if env.songs_count() == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(env.songs_count(), 3);

        // Tạo playlist: trùng tên báo ngay dưới ô, hộp vẫn mở.
        env.library.create_playlist("Gym", "#ff0000").wait().unwrap();
        page.update(cx_window, |_, cx| cx.emit(LibraryPageEvent::CreatePlaylist));
        cx_window.run_until_parked();
        let create = ctl.read_with(cx_window, |c, _| c.playlist_dialog.clone()).unwrap();
        assert!(create.read_with(cx_window, |d, _| d.is_open()));
        draw(cx_window);
        create.update_in(cx_window, |d, window, cx| d.submit_for_test("Gym", window, cx));
        for _ in 0..500 {
            cx_window.run_until_parked();
            if !create.read_with(cx_window, |d, _| d.is_busy()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(create.read_with(cx_window, |d, _| d.is_open()), "trùng tên: hộp vẫn mở");
        create.update_in(cx_window, |d, window, cx| d.submit_for_test("Chill", window, cx));
        for _ in 0..500 {
            cx_window.run_until_parked();
            if !create.read_with(cx_window, |d, _| d.is_open()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let names: Vec<String> = env.library.playlists().wait().unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Chill", "Gym"]);
        assert!(notices.borrow().iter().all(|(kind, _)| *kind != ToastKind::Danger));
        let _ = dialog;
    }

    /// C16 — menu mở hộp metadata: chủ giữ entity, trang vẽ làm con; `Closed` bỏ entity; đang mở thì không mở chồng.
    #[gpui::test]
    fn metadata_dialog_is_owned_and_released(cx: &mut TestAppContext) {
        use crate::views::library_page::LibraryPage;

        let env = env();
        env.import_tree();
        let _player = install_player(cx, &env);
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let window = cx.add_window(|window, cx| {
            let page = cx.new(|cx| LibraryPage::new(Some(library), Some(covers), Route::MyMusic, cx));
            Root::new(page, window, cx)
        });
        let page = window.update(cx, |root, _, _| root.view().clone().downcast::<LibraryPage>().unwrap()).unwrap();
        ctl.update(cx, |ctl, _| ctl.page = Some(page.downgrade()));
        let folder = env.folder("A");
        let ops = MetadataOps::new(env.library.clone(), env.covers.clone()).unwrap();
        let library = env.library.clone();
        cx.update(|cx| {
            app::set_library(library, cx);
            app::set_metadata_ops(ops, cx);
        });
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();

        // Lyrics, timeline: dựng một lần, trang vẽ làm con.
        let first = env.library.songs(folder).wait().unwrap()[0].track.clone();
        ctl.update(cx, |ctl, cx| ctl.open_text_dialog(SongEditKind::Lyrics, first.clone(), cx));
        cx.run_until_parked();
        let lyrics = ctl.read_with(cx, |c, _| c.lyrics_dialog.clone()).expect("đã dựng");
        assert!(lyrics.read_with(cx, |d, _| d.is_open()));
        lyrics.update_in(cx, |d, window, cx| d.close(window, cx));
        ctl.update(cx, |ctl, cx| ctl.open_text_dialog(SongEditKind::Lyrics, first.clone(), cx));
        cx.run_until_parked();
        let again = ctl.read_with(cx, |c, _| c.lyrics_dialog.clone()).unwrap();
        assert_eq!(again.entity_id(), lyrics.entity_id());
        lyrics.update_in(cx, |d, window, cx| d.close(window, cx));
        ctl.update(cx, |ctl, cx| ctl.open_text_dialog(SongEditKind::Timeline, first, cx));
        cx.run_until_parked();
        let timeline = ctl.read_with(cx, |c, _| c.timeline_dialog.clone()).expect("đã dựng");
        assert!(timeline.read_with(cx, |d, _| d.is_open()));
        timeline.update_in(cx, |d, window, cx| d.close(window, cx));
        cx.run_until_parked();

        ctl.update(cx, |ctl, cx| ctl.open_category_metadata(folder, cx));
        for _ in 0..500 {
            cx.run_until_parked();
            if ctl.read_with(cx, |c, _| c.metadata_dialog.is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let dialog = ctl.read_with(cx, |c, _| c.metadata_dialog.clone()).expect("đã mở");
        assert!(page.read_with(cx, |p, _| p.has_metadata_dialog()), "trang vẽ hộp");
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        assert!(dialog.read_with(cx, |d, _| d.is_open()));

        let track = env.library.songs(folder).wait().unwrap()[0].track.clone();
        ctl.update(cx, |ctl, cx| ctl.show_metadata(track, None, cx));
        cx.run_until_parked();
        let same = ctl.read_with(cx, |c, _| c.metadata_dialog.clone()).unwrap();
        assert_eq!(same.entity_id(), dialog.entity_id());

        dialog.update(cx, |_, cx| cx.emit(MetadataDialogEvent::Closed));
        cx.run_until_parked();
        assert!(ctl.read_with(cx, |c, _| c.metadata_dialog.is_none()));
        assert!(!page.read_with(cx, |p, _| p.has_metadata_dialog()));
    }

    /// C15 — danh sách trong playlist: chuột phải và Shift+F10 đều mở menu của dòng; mục cuối xoá đúng bài khỏi playlist.
    #[gpui::test]
    fn song_menu_opens_by_mouse_and_keyboard(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton};

        use crate::views::library_page::LibraryPage;

        let env = env();
        env.import_tree();
        let mut songs = env.library.songs(env.folder("A")).wait().unwrap();
        songs.extend(env.library.songs(env.folder("B")).wait().unwrap());
        let playlist = env.library.create_playlist("Gym", "#ff0000").wait().unwrap();
        for song in &songs {
            let TrackRef::Library(id) = song.track else { unreachable!() };
            env.library.add_song_to_playlist(playlist, id).wait().unwrap();
        }
        let in_list = |env: &Env| env.library.songs(CategoryRef::Playlist(playlist)).wait().unwrap();
        let before = in_list(&env);
        assert_eq!(before.len(), 3);

        let _player = install_player(cx, &env);
        let (ctl, _) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let route = Route::Category(CategoryRef::Playlist(playlist));
        let window = cx.add_window(|window, cx| {
            let page = cx.new(|cx| LibraryPage::new(Some(library), Some(covers), route, cx));
            Root::new(page, window, cx)
        });
        cx.update(|cx| cx.set_global(CtlHandle(ctl.clone())));
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
        };
        let wait = |cx: &mut VisualTestContext, what: &str, done: &dyn Fn(&mut VisualTestContext) -> bool| {
            for _ in 0..500 {
                cx.run_until_parked();
                draw(cx);
                if done(cx) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("quá hạn chờ: {what}");
        };
        wait(cx, "dòng", &|cx| cx.debug_bounds("song-row-2").is_some());

        // Shift+F10: focus đã giao cho danh sách lúc vào category; chưa chọn dòng nên nhắm dòng đầu.
        cx.simulate_keystrokes("shift-f10");
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "xoá bằng phím", &|_| in_list(&env).len() == 2);
        assert_eq!(in_list(&env)[0].track, before[1].track, "dòng đầu");

        // Chuột phải đúng dòng: mục cuối của menu bỏ bài đó.
        wait(cx, "còn hai dòng", &|cx| cx.debug_bounds("song-row-3").is_none());
        let row = cx.debug_bounds("song-row-1").unwrap().center();
        cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::default());
        cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::default());
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "xoá bằng menu chuột", &|_| in_list(&env).len() == 1);

        // Đóng menu xong focus phải về danh sách: phím vẫn dùng được.
        wait(cx, "còn một dòng", &|cx| cx.debug_bounds("song-row-2").is_none());
        cx.simulate_keystrokes("shift-f10");
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "phím sau menu chuột", &|_| in_list(&env).is_empty());

        let TrackRef::Library(again) = before[0].track else { unreachable!() };
        env.library.add_song_to_playlist(playlist, again).wait().unwrap();
        wait(cx, "có lại dòng", &|cx| cx.debug_bounds("song-row-1").is_some());
        // "Tạo danh sách mới" từ menu con: chạy trong `on_select`, lúc cửa sổ đang bị mượn, hộp vẫn phải mở.
        let weak_ctl = ctl.downgrade();
        let create = cx.update(|window, cx| {
            let library = env.library.clone();
            cx.new(|cx| PlaylistDialog::new(Some(library), weak_ctl, window, cx))
        });
        ctl.update(cx, |ctl, _| ctl.playlist_dialog = Some(create.clone()));
        cx.simulate_keystrokes("shift-f10");
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("enter");
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "hộp tạo danh sách", &|cx| create.read_with(cx, |d, _| d.is_open()));
    }

    /// groove-gd2_03 C4, C5 — mục cuối của menu bài (thư mục) là "Save as new"; lưu đoạn ra tệp mới cạnh tệp gốc, tệp gốc
    /// không đổi, bài mới vào thư viện.
    #[gpui::test]
    fn save_as_new_from_the_song_menu_writes_and_adds_a_file(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton};

        use crate::views::library_page::LibraryPage;

        let env = env();
        let root = env.import_tree();
        let before = env.songs_count();
        let _player = install_player(cx, &env);
        let (ctl, notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let route = Route::Category(env.folder("B"));
        let window = cx.add_window(|window, cx| {
            let page = cx.new(|cx| LibraryPage::new(Some(library), Some(covers), route, cx));
            Root::new(page, window, cx)
        });
        cx.update(|cx| cx.set_global(CtlHandle(ctl.clone())));
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
        };
        // Đồng hồ của App ảo chỉ chạy khi đẩy: vòng đọc tiến độ của hộp chờ `timer(100 ms)`.
        let wait = |cx: &mut VisualTestContext, what: &str, done: &dyn Fn(&mut VisualTestContext) -> bool| {
            for _ in 0..500 {
                cx.executor().advance_clock(Duration::from_millis(100));
                cx.run_until_parked();
                draw(cx);
                if done(cx) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("quá hạn chờ: {what}");
        };
        wait(cx, "dòng", &|cx| cx.debug_bounds("song-row-1").is_some());
        let source = root.join("B").join("b1.wav");
        let original = fs::read(&source).unwrap();

        let row = cx.debug_bounds("song-row-1").unwrap().center();
        cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::default());
        cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::default());
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "hộp Save as new", &|cx| {
            ctl.read_with(cx, |c, cx| c.save_as_dialog().is_some_and(|d| d.read(cx).is_ready()))
        });
        let dialog = ctl.read_with(cx, |c, _| c.save_as_dialog().unwrap().clone());
        dialog.update_in(cx, |d, window, cx| {
            d.set_range_for_test(0.2, 0.7, groove_audio::export::ExportFormat::Current);
            d.save_for_test(window, cx);
        });
        wait(cx, "lưu xong, vào thư viện", &|cx| {
            !dialog.read_with(cx, |d, _| d.is_open()) && env.songs_count() == before + 1
        });
        let saved = root.join("B").join("b1 (cut).wav");
        assert!(saved.is_file(), "tệp mới cạnh tệp gốc");
        assert_eq!(fs::read(&source).unwrap(), original, "tệp gốc không đổi");
        wait(cx, "toast", &|_| notices.borrow().iter().any(|(_, m)| m.contains("b1 (cut).wav")));
    }

    /// Ô giờ của "Save as new": xoá bằng Backspace rồi gõ lại (ô rỗng, dạng dở `1:`) không làm sập app, giá trị mới vào đoạn.
    #[gpui::test]
    fn save_as_time_fields_survive_backspace_and_retyping(cx: &mut TestAppContext) {
        use gpui::{Modifiers, MouseButton};

        use crate::views::library_page::LibraryPage;

        let env = env();
        env.import_tree();
        let _player = install_player(cx, &env);
        let (ctl, _notices) = ctl_with(cx, &env, fake_picker(None, Rc::default()));
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let route = Route::Category(env.folder("B"));
        let window = cx.add_window(|window, cx| {
            let page = cx.new(|cx| LibraryPage::new(Some(library), Some(covers), route, cx));
            Root::new(page, window, cx)
        });
        cx.update(|cx| cx.set_global(CtlHandle(ctl.clone())));
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
        };
        let wait = |cx: &mut VisualTestContext, what: &str, done: &dyn Fn(&mut VisualTestContext) -> bool| {
            for _ in 0..500 {
                cx.executor().advance_clock(Duration::from_millis(100));
                cx.run_until_parked();
                draw(cx);
                if done(cx) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("quá hạn chờ: {what}");
        };
        wait(cx, "dòng", &|cx| cx.debug_bounds("song-row-1").is_some());
        let row = cx.debug_bounds("song-row-1").unwrap().center();
        cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::default());
        cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::default());
        draw(cx);
        draw(cx);
        cx.simulate_keystrokes("end enter");
        wait(cx, "hộp Save as new", &|cx| {
            ctl.read_with(cx, |c, cx| c.save_as_dialog().is_some_and(|d| d.read(cx).is_ready()))
        });
        let dialog = ctl.read_with(cx, |c, _| c.save_as_dialog().unwrap().clone());
        // Hộp nằm trong Shell lúc chạy thật; ở đây vẽ nó trong một cửa sổ riêng để phím tới ô nhập.
        let mut app = cx.cx.clone();
        let shown = dialog.clone();
        let window = app.add_window(|window, cx| Root::new(shown, window, cx));
        let cx = VisualTestContext::from_window(window.into(), &app).into_mut();
        cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(800.)));
        let focus = |end: bool, cx: &mut VisualTestContext| {
            dialog.update_in(cx, |d, window, cx| d.focus_time_for_test(end, window, cx));
            draw(cx);
        };
        // Bấm sát mép phải trong khung ô (chỗ trống sau chữ) cũng phải focus vào ô.
        draw(cx);
        for (end, id) in [(false, "save-as-start-box"), (true, "save-as-end-box")] {
            let bounds = cx.debug_bounds(id).expect(id);
            let at = gpui::point(bounds.right() - gpui::px(6.), bounds.top() + gpui::px(16.));
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
            draw(cx);
            assert!(dialog.update_in(cx, |d, window, cx| d.time_focused_for_test(end, window, cx)), "{id}");
        }
        for end in [false, true] {
            focus(end, cx);
            cx.simulate_keystrokes("end backspace backspace backspace backspace backspace backspace backspace");
            draw(cx);
            cx.simulate_input("0:00");
            cx.simulate_keystrokes("backspace backspace");
            draw(cx);
        }
        focus(false, cx);
        cx.simulate_keystrokes("end backspace backspace backspace backspace backspace backspace");
        cx.simulate_input("0.2");
        draw(cx);
        focus(true, cx);
        cx.simulate_keystrokes("end backspace backspace backspace backspace backspace backspace");
        cx.simulate_input("0.7");
        draw(cx);
        let (start, end) = dialog.read_with(cx, |d, _| d.range_for_test());
        assert!((start - 0.2).abs() < 1e-9 && (end - 0.7).abs() < 1e-9, "{start} {end}");
    }

    #[test]
    fn every_key_exists() {
        use groove_core::i18n::t;
        for key in [
            "actions.unavailable",
            "actions.import.done",
            "actions.import.partial",
            "actions.import.offline",
            "actions.import.unreadable",
            "actions.import.failed",
            "actions.rescan.done",
            "actions.rescan.failed",
            "actions.play.empty",
            "actions.play.failed",
            "actions.remove.failed",
            "actions.cover.failed",
            "actions.sync.changed",
            "actions.card.play_all",
            "actions.card.set_cover",
            "actions.card.edit_metadata",
            "actions.card.delete_playlist",
            "actions.card.remove_from_library",
            "actions.confirm.title",
            "actions.confirm.cancel",
            "actions.confirm.album",
            "actions.confirm.folder",
            "actions.confirm.playlist",
            "actions.playlist.title",
            "actions.playlist.name_label",
            "actions.playlist.name_placeholder",
            "actions.playlist.color_area",
            "actions.playlist.color_hue",
            "actions.playlist.color_hex",
            "actions.playlist.cancel",
            "actions.playlist.create",
            "actions.playlist.empty_name",
            "actions.playlist.duplicate",
            "actions.playlist.create_failed",
            "actions.playlist.added",
            "actions.playlist.already_in",
            "actions.playlist.add_failed",
            "actions.playlist.remove_failed",
            "actions.song.add_to_playlist",
            "actions.song.new_playlist",
            "actions.song.edit_timeline",
            "actions.song.edit_lyrics",
            "actions.song.edit_metadata",
            "actions.song.remove_from_playlist",
        ] {
            assert_ne!(t(key), key, "thiếu `{key}`");
        }
    }
}
