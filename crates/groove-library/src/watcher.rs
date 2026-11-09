//! groove-gd1_09: watcher (N4, S6).
//!
//! Debounce rồi đưa từng lô qua `reconcile::plan`, cùng đường với import. Chống vòng lặp không dùng cửa sổ thời gian: tệp có
//! `(mtime, size)` bằng giá trị trong DB thì bỏ qua (S6), nên ai ghi tệp chỉ cần lưu dấu tệp vào DB.
//!
//! Luồng riêng giữ debouncer, không chạm luồng UI. Không có gì đổi thì không phát sự kiện.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::{fs, io};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use groove_core::model::is_audio_path;
use groove_core::paths;
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, DebouncedEvent, new_debouncer};

use crate::db::queries;
use crate::db::schema;
use crate::indexer::{self, IndexJob, Indexer, KnownRow};
use crate::reconcile::{self, Changes, SyncOutcome, Untagged, is_excluded_key};
use crate::scanner::{self, ScannedFile};
use crate::tags::{FileStamp, is_temp_path};

/// Đặc tả 1.11.
pub const DEBOUNCE: Duration = Duration::from_secs(2);
/// Gốc đang vắng (ổ mạng, thẻ nhớ): thử gắn lại theo nhịp này.
pub const ROOT_RETRY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatcherOptions {
    pub debounce: Duration,
    pub root_retry: Duration,
}

impl Default for WatcherOptions {
    fn default() -> WatcherOptions {
        WatcherOptions { debounce: DEBOUNCE, root_retry: ROOT_RETRY }
    }
}

/// Một lô đã gộp. Tên đường dẫn, chưa biết là tệp hay thư mục.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeBatch {
    pub dirty: Vec<PathBuf>,
    /// Cặp `(cũ, mới)` do hệ điều hành báo, tệp hoặc thư mục.
    pub renames: Vec<(PathBuf, PathBuf)>,
    /// Sự kiện bị rớt: phải so lại cả thư viện.
    pub need_rescan: bool,
}

/// Bỏ bản tạm khi ghi thẻ (N3) và sự kiện chỉ đọc.
fn classify(events: &[DebouncedEvent]) -> ChangeBatch {
    let mut batch = ChangeBatch::default();
    for event in events {
        batch.need_rescan |= event.need_rescan();
        match event.kind {
            EventKind::Access(_) => {}
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 2 => {
                let (from, to) = (&event.paths[0], &event.paths[1]);
                if is_temp_path(to) {
                    // Bản tạm thay tệp gốc: tệp gốc đổi, không phải đổi tên.
                    batch.dirty.push(to.with_file_name(temp_owner_name(to)));
                } else if is_temp_path(from) {
                    batch.dirty.push(to.clone());
                } else {
                    batch.renames.push((from.clone(), to.clone()));
                }
            }
            _ => batch.dirty.extend(event.paths.iter().filter(|path| !is_temp_path(path)).cloned()),
        }
    }
    batch
}

fn temp_owner_name(temp: &Path) -> String {
    let name = temp.file_name().unwrap_or_default().to_string_lossy();
    name.strip_suffix(crate::tags::TEMP_SUFFIX).unwrap_or(&name).to_owned()
}

/// Gốc chứa toàn bộ thứ hiện có không thể rỗng: thư mục trống nghĩa là chưa mount, hoặc đọc không được (N1).
fn root_is_present(root: &Path) -> bool {
    fs::read_dir(root).is_ok_and(|mut entries| entries.next().is_some())
}

fn gather(
    batch: &ChangeBatch,
    rows: &[KnownRow],
    roots: &[PathBuf],
    excluded: &[String],
    probe: impl Fn(&Path) -> io::Result<()> + Copy,
) -> Changes {
    let root_keys: Vec<String> = roots.iter().map(|root| paths::normalize_path(root)).collect();
    let root_of = |key: &str| root_keys.iter().position(|root| paths::is_under(key, root));
    let mut changes = Changes::default();
    let mut visited = HashSet::new();
    let dirty = batch.dirty.iter().chain(batch.renames.iter().flat_map(|(from, to)| [from, to]));
    for path in dirty {
        let key = paths::normalize_path(path);
        // Sự kiện của chính `excluded`, ngoài mọi gốc, hoặc đã xem: bỏ.
        if root_of(&key).is_none() || is_excluded_key(&key, excluded) || !visited.insert(key.clone()) {
            continue;
        }
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => {
                let scan = scanner::scan(path);
                // Chính gốc quét ra 0 tệp: cùng luật offline với lúc mở app (N1).
                if scan.root_failure(path).is_some() || (scan.files.is_empty() && root_keys.contains(&key)) {
                    continue;
                }
                let under: Vec<&KnownRow> = rows.iter().filter(|row| paths::is_under(&row.path_key, &key)).collect();
                let scanned: HashSet<String> =
                    scan.files.iter().map(|file| schema::path_key(&file.path.to_string_lossy())).collect();
                changes.vanished.extend(indexer::plan_removals(&under, &scan, &scanned, probe));
                changes.seen.extend(scan.files);
            }
            Ok(_) => {
                if is_audio_path(path) && !is_temp_path(path) {
                    match FileStamp::of(path) {
                        Ok(stamp) => changes.seen.push(ScannedFile { path: path.clone(), stamp }),
                        Err(err) => log::warn!("could not stat {}: {err}", path.display()),
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let gone = rows
                    .iter()
                    .filter(|row| paths::is_under(&row.path_key, &key))
                    .filter(|row| probe(&row.path).is_err_and(|err| err.kind() == io::ErrorKind::NotFound));
                changes.vanished.extend(gone.map(|row| row.id));
            }
            Err(err) => log::warn!("could not read {}: {err}", path.display()),
        }
    }

    // macos-arch (09/11/2026): FSEvents không ghép được cặp đổi tên thư mục — debouncer chỉ báo `Create(Folder)` ở nơi mới,
    // không báo nơi cũ ⇒ tệp trong thư mục vừa đổi tên / dời tới trông như tệp mới, dòng cũ không ai đánh dấu vắng, `reconcile`
    // không có ứng viên để ghép rename. Tệp lạ trong lô ⇒ dòng cùng tên tệp đã mất trên đĩa vào `vanished`, `reconcile::plan`
    // ghép như đổi tên tệp (cỡ, thẻ). Chỉ stat dòng trùng tên, không quét cả gốc.
    let known: HashSet<&str> = rows.iter().map(|row| row.path_key.as_str()).collect();
    let fresh_names: HashSet<&std::ffi::OsStr> = changes
        .seen
        .iter()
        .filter(|file| !known.contains(schema::path_key(&file.path.to_string_lossy()).as_str()))
        .filter_map(|file| file.path.file_name())
        .collect();
    if !fresh_names.is_empty() {
        let orphans = rows
            .iter()
            .filter(|row| row.path.file_name().is_some_and(|name| fresh_names.contains(name)))
            .filter(|row| root_of(&row.path_key).is_some())
            .filter(|row| probe(&row.path).is_err_and(|err| err.kind() == io::ErrorKind::NotFound));
        changes.vanished.extend(orphans.map(|row| row.id));
    }

    // N1: cả gốc vắng thì mọi "không có" ở trên chỉ nói là ổ chưa gắn.
    let by_id: HashMap<i64, &KnownRow> = rows.iter().map(|row| (row.id, row)).collect();
    let present: Vec<bool> = roots.iter().map(|root| root_is_present(root)).collect();
    changes.vanished.retain(|id| {
        by_id.get(id).is_some_and(|row| {
            !is_excluded_key(&row.path_key, excluded) && root_of(&row.path_key).is_none_or(|root| present[root])
        })
    });

    for (from, to) in &batch.renames {
        let from_key = paths::normalize_path(from);
        let to_key = paths::normalize_path(to);
        for row in rows.iter().filter(|row| paths::is_under(&row.path_key, &from_key)) {
            let new_key = format!("{to_key}{}", &row.path_key[from_key.len()..]);
            changes.hints.insert(new_key, row.id);
        }
    }
    changes
}

/// Áp một lô, chung lịch với import và rescan của `indexer`. Có thay đổi mới phát `Moved`, `Synced`.
pub fn apply_batch(indexer: &Indexer, batch: ChangeBatch) -> IndexJob<SyncOutcome> {
    indexer.spawn(move |indexer| {
        let library = indexer.library();
        let roots = library.watched_roots().wait()?;
        let excluded = library.read(|conn| Ok(queries::excluded_keys(conn)?)).wait()?;
        let rows = indexer::known_rows(library)?;
        let changes = gather(&batch, &rows, &roots, &excluded, indexer::probe_path);
        let plan = reconcile::plan(&rows, &excluded, changes, Untagged::Skip, indexer::probe_path);
        drop(rows);
        let mut outcome = SyncOutcome::default();
        let applied = indexer.apply_plan(plan, None, &mut outcome);
        if outcome.changed_anything() {
            indexer.emit_outcome(&outcome);
            indexer.emit_external_change(&outcome);
        }
        applied?;
        Ok(outcome)
    })
}

enum Command {
    /// `Sender`: báo khi đã đọc lại và gắn xong.
    RefreshRoots(Option<Sender<()>>),
    Stop,
}

/// Luồng watcher đang chạy. Giữ giá trị này suốt phiên; thả hoặc `stop` thì dừng theo dõi.
pub struct LibraryWatcher {
    commands: Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}

impl LibraryWatcher {
    /// Trả về ngay. Trên luồng nền: gắn watcher lên mọi gốc, so đĩa với DB (`reconcile`), rồi theo dõi. Gốc đang vắng được thử
    /// lại định kỳ, gắn được thì so lại.
    pub fn start(indexer: Indexer) -> io::Result<LibraryWatcher> {
        LibraryWatcher::start_with(indexer, WatcherOptions::default())
    }

    pub fn start_with(indexer: Indexer, options: WatcherOptions) -> io::Result<LibraryWatcher> {
        let (commands, inbox) = unbounded();
        let thread =
            thread::Builder::new().name("groove-watcher".to_owned()).spawn(move || run(indexer, options, inbox))?;
        Ok(LibraryWatcher { commands, thread: Some(thread) })
    }

    /// Gốc vừa đổi (import, bỏ gốc): đọc lại danh sách. Import đã tự so nên ở đây không so lại.
    pub fn refresh_roots(&self) {
        let _ = self.commands.send(Command::RefreshRoots(None));
    }

    /// Như [`Self::refresh_roots`], chờ tới khi gắn xong. Để test biết chắc từ lúc này watcher đã nghe, không dùng ở UI thread.
    #[doc(hidden)]
    pub fn refresh_roots_blocking(&self) {
        let (done, finished) = unbounded();
        let _ = self.commands.send(Command::RefreshRoots(Some(done)));
        let _ = finished.recv();
    }

    /// Chờ luồng thoát. Khi thoát app, gọi trước `Library::shutdown`.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        self.shut_down();
    }
}

fn run(indexer: Indexer, options: WatcherOptions, inbox: Receiver<Command>) {
    let (events_tx, events) = unbounded::<DebounceEventResult>();
    let handler = move |result: DebounceEventResult| {
        let _ = events_tx.send(result);
    };
    // Luồng của debouncer ngủ theo nhịp này cả khi không có gì đổi; mặc định 1/4 thời gian chờ (500 ms) là đánh thức app 2 lần
    // mỗi giây suốt đời. Nửa thời gian chờ: báo trễ nhất 1,5 lần thời gian chờ (3 s), thức 1 lần mỗi giây.
    let tick = options.debounce / 2;
    let mut debouncer = match new_debouncer(options.debounce, Some(tick), handler) {
        Ok(debouncer) => debouncer,
        Err(err) => {
            // Không theo dõi được thì vẫn so một lần khi mở app.
            log::error!("could not start the file watcher: {err}");
            run_reconcile(&indexer);
            return;
        }
    };
    let mut attached: Vec<PathBuf> = Vec::new();
    let mut waiting: Vec<PathBuf> = Vec::new();
    match indexer.library().watched_roots().wait() {
        Ok(roots) => sync_roots(&mut debouncer, &mut attached, &mut waiting, roots),
        Err(err) => log::warn!("could not read library roots: {err}"),
    }
    // Gắn trước, so sau: thay đổi trong lúc so nằm trong lô kế.
    run_reconcile(&indexer);

    loop {
        // Chỉ hẹn giờ thử gắn lại khi có gốc đang vắng: không thì chờ lệnh / sự kiện, không tự thức.
        let retry =
            if waiting.is_empty() { crossbeam_channel::never() } else { crossbeam_channel::after(options.root_retry) };
        select! {
            recv(inbox) -> command => match command {
                Ok(Command::RefreshRoots(done)) => {
                    if let Ok(roots) = indexer.library().watched_roots().wait() {
                        sync_roots(&mut debouncer, &mut attached, &mut waiting, roots);
                    }
                    if let Some(done) = done {
                        let _ = done.send(());
                    }
                }
                Ok(Command::Stop) | Err(_) => return,
            },
            recv(events) -> result => {
                let Ok(result) = result else { return };
                match result {
                    Ok(found) => {
                        let batch = classify(&found);
                        if batch.need_rescan {
                            run_reconcile(&indexer);
                        } else if (!batch.dirty.is_empty() || !batch.renames.is_empty())
                            && let Err(err) = apply_batch(&indexer, batch).wait()
                        {
                            log::warn!("watcher batch failed: {err}");
                        }
                    }
                    Err(errors) => {
                        for err in &errors {
                            log::warn!("file watcher: {err}");
                        }
                        run_reconcile(&indexer);
                    }
                }
                // Gốc bị rút: handle cũ đã chết, phải gắn lại khi nó quay về.
                let (alive, gone): (Vec<PathBuf>, Vec<PathBuf>) = attached.drain(..).partition(|root| root.is_dir());
                attached = alive;
                for root in gone {
                    let _ = debouncer.unwatch(&root);
                    waiting.push(root);
                }
            },
            recv(retry) -> _ => {
                let before = attached.len();
                let mut all = attached.clone();
                all.append(&mut waiting);
                sync_roots(&mut debouncer, &mut attached, &mut waiting, all);
                if attached.len() > before {
                    run_reconcile(&indexer);
                }
            },
        }
    }
}

fn sync_roots<T: notify::Watcher, C: notify_debouncer_full::FileIdCache>(
    debouncer: &mut notify_debouncer_full::Debouncer<T, C>,
    attached: &mut Vec<PathBuf>,
    waiting: &mut Vec<PathBuf>,
    roots: Vec<PathBuf>,
) {
    let same = |a: &Path, b: &Path| paths::normalize_path(a) == paths::normalize_path(b);
    let keep = |root: &PathBuf| roots.iter().any(|wanted| same(root, wanted));
    let (kept, dropped): (Vec<PathBuf>, Vec<PathBuf>) = attached.drain(..).partition(|root| keep(root));
    *attached = kept;
    for root in dropped {
        let _ = debouncer.unwatch(&root);
    }
    waiting.clear();
    for root in roots {
        if attached.iter().any(|known| same(known, &root)) {
            continue;
        }
        if !root.is_dir() {
            waiting.push(root);
            continue;
        }
        match debouncer.watch(&root, RecursiveMode::Recursive) {
            Ok(()) => attached.push(root),
            Err(err) => {
                log::warn!("could not watch {}: {err}", root.display());
                waiting.push(root);
            }
        }
    }
}

fn run_reconcile(indexer: &Indexer) {
    if let Err(err) = reconcile::reconcile(indexer).wait() {
        log::warn!("library reconcile failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::Event;
    use std::time::Instant;

    fn event(kind: EventKind, paths: &[&str]) -> DebouncedEvent {
        let mut event = Event::new(kind);
        for path in paths {
            event = event.add_path(PathBuf::from(path));
        }
        DebouncedEvent::new(event, Instant::now())
    }

    #[test]
    fn a_rename_pair_stays_a_pair() {
        let batch =
            classify(&[event(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &["/m/a.mp3", "/m/b.mp3"])]);
        assert_eq!(batch.renames, [(PathBuf::from("/m/a.mp3"), PathBuf::from("/m/b.mp3"))]);
        assert!(batch.dirty.is_empty());
    }

    #[test]
    fn temp_files_never_count_as_songs_or_renames() {
        // N3: ghi thẻ qua `<tên>.groove-tmp` rồi đổi tên đè lên tệp gốc.
        let both = EventKind::Modify(ModifyKind::Name(RenameMode::Both));
        let batch = classify(&[
            event(EventKind::Create(notify::event::CreateKind::File), &["/m/a.mp3.groove-tmp"]),
            event(both, &["/m/a.mp3.groove-tmp", "/m/a.mp3"]),
        ]);
        assert!(batch.renames.is_empty());
        assert_eq!(batch.dirty, [PathBuf::from("/m/a.mp3")]);
    }

    #[test]
    fn reads_are_ignored_and_dropped_events_force_a_rescan() {
        let batch = classify(&[event(EventKind::Access(notify::event::AccessKind::Any), &["/m/a.mp3"])]);
        assert_eq!(batch, ChangeBatch::default());
        let mut lost = Event::new(EventKind::Other);
        lost = lost.set_flag(notify::event::Flag::Rescan);
        let batch = classify(&[DebouncedEvent::new(lost, Instant::now())]);
        assert!(batch.need_rescan);
    }
}
