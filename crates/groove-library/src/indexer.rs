//! groove-gd1_08: import, rescan.
//!
//! Mỗi việc một luồng nền: quét, đọc thẻ và giải bìa ngoài transaction, rồi mới gửi từng lô `BATCH_ROWS` dòng cho
//! `LibraryActor` và chờ commit (N7). Gọi `Indexer` từ luồng UI chỉ tạo luồng, không chạm đĩa.
//!
//! Một dòng chỉ bị xoá khi hệ tệp trả lời chắc chắn "không có": đọc lỗi không phải là đã xoá (N1).

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::{fs, io, thread};

use futures::channel::oneshot;
use groove_core::paths;
use rusqlite::{Connection, params};

use crate::actor::{BATCH_ROWS, LibraryError, LibraryEvent, LibraryResult, Progress, ProgressKind, SyncReport};
use crate::covers::{CoverStore, cover_hash};
use crate::db::migrate::CoverRecord;
use crate::db::queries::{self, SongUpsert, UpsertOutcome};
use crate::db::schema;
use crate::scanner::{self, ScanOutcome};
use crate::store::Library;
use crate::tags::{self, CoverArt, FileStamp, ScannedTrack, TagError};

/// Mỗi luồng giữ một bìa đầy đủ trong lúc đọc thẻ nên không chạy theo số nhân.
const MAX_READ_THREADS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Gốc người dùng chọn không đọc được: không ghi gì, không xoá gì.
    #[error("{} is not readable: {message}", path.display())]
    RootUnavailable { path: PathBuf, message: String },
    #[error(transparent)]
    Library(#[from] LibraryError),
}

/// Kết quả của một việc. Bỏ đi không huỷ việc: việc chạy nốt rồi phát `Synced`.
pub struct IndexJob<T> {
    rx: oneshot::Receiver<Result<T, IndexError>>,
}

impl<T> IndexJob<T> {
    /// Chỉ cho luồng nền và test, không bao giờ trên luồng UI.
    pub fn wait(self) -> Result<T, IndexError> {
        futures::executor::block_on(self)
    }
}

impl<T> Future for IndexJob<T> {
    type Output = Result<T, IndexError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.rx).poll(cx).map(|reply| reply.unwrap_or(Err(LibraryError::Closed.into())))
    }
}

/// Đếm theo tệp đã đọc. `missing`, `failed` không làm mất dòng nào.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexTally {
    pub added: usize,
    pub updated: usize,
    /// Hệ tệp báo không có.
    pub missing: Vec<PathBuf>,
    /// Có tệp nhưng đọc không được: không phải âm thanh, mất quyền, tệp độc.
    pub failed: Vec<PathBuf>,
}

impl IndexTally {
    pub fn read(&self) -> usize {
        self.added + self.updated
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub tally: IndexTally,
    /// `(mtime, size)` khớp DB nên không đọc lại (S26).
    pub unchanged: usize,
    pub removed: Vec<i64>,
    /// Thư mục con không đọc được; dòng dưới đó được giữ, UI nên báo (N1).
    pub failed_dirs: Vec<PathBuf>,
    /// Gốc quét ra 0 tệp trong khi DB còn dòng dưới nó: coi là offline, không xoá gì (N1).
    pub root_offline: bool,
}

/// Một bài đã đọc xong, không còn byte bìa. Điểm vào chung của import, watcher, rescan, sửa thẻ (S7).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackRow {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_sec: f64,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub lyrics: Option<String>,
    pub timeline: Option<String>,
    pub cover_hash: Option<String>,
    /// `file_mtime` tính bằng ms, cùng đơn vị với `FileStamp`. Dòng từ DB cũ tính bằng giây cho tới khi đọc lại thẻ.
    pub mtime_ms: i64,
    pub size: u64,
}

impl TrackRow {
    /// `cover_hash` do caller có được từ `CoverStore`; `track.cover` bị bỏ tại đây.
    pub fn from_scanned(path: PathBuf, track: ScannedTrack, cover_hash: Option<String>) -> TrackRow {
        let FileStamp { size, mtime_ms } = track.stamp;
        let fields = track.fields;
        TrackRow {
            path,
            title: fields.title,
            artist: fields.artist,
            album: fields.album,
            album_artist: fields.album_artist,
            duration_sec: track.duration_ms as f64 / 1000.0,
            track_no: parse_number(fields.track_no.as_deref()),
            disc_no: parse_number(fields.disc_no.as_deref()),
            lyrics: fields.lyrics,
            timeline: fields.timeline,
            cover_hash,
            mtime_ms,
            size,
        }
    }
}

/// `3`, `03`, `3/12` thành 3. Rác và số 0 thành trống, để `songs` xếp bài đó cuối album.
fn parse_number(raw: Option<&str>) -> Option<u32> {
    raw?.split('/').next()?.trim().parse().ok().filter(|number| *number > 0)
}

/// Một bài vào DB: thư mục cha trực tiếp, album theo `group_key` (S1), gieo bìa nhóm nếu nhóm chưa có (S8). Gọi trong
/// `writer().transaction`. Dòng `covers` của `cover_hash` phải có trước, xem `queries::upsert_cover`.
pub fn apply_scanned_track(conn: &Connection, row: &TrackRow) -> queries::Result<UpsertOutcome> {
    apply_row(conn, row, &mut HashMap::new())
}

fn apply_row(conn: &Connection, row: &TrackRow, folders: &mut HashMap<PathBuf, i64>) -> queries::Result<UpsertOutcome> {
    let parent = row.path.parent().unwrap_or(Path::new(""));
    let folder_id = match folders.get(parent) {
        Some(id) => *id,
        None => {
            let name = parent.file_name().map_or_else(|| parent.to_string_lossy(), |name| name.to_string_lossy());
            let id = queries::get_or_create_folder(conn, &name, parent)?;
            folders.insert(parent.to_owned(), id);
            id
        }
    };
    let album_id = match row.album.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            let group_key = schema::album_group_key(row.album_artist.as_deref(), folder_id);
            let artist = row.album_artist.as_deref().or(row.artist.as_deref()).map(str::trim);
            Some(queries::get_or_create_album(conn, name, &group_key, artist.filter(|a| !a.is_empty()))?)
        }
        None => None,
    };
    let outcome = queries::upsert_song(
        conn,
        &SongUpsert {
            path: row.path.clone(),
            folder_id,
            album_id,
            title: row.title.clone(),
            artist: row.artist.clone(),
            album_artist: row.album_artist.clone(),
            duration_sec: row.duration_sec,
            track_no: row.track_no,
            disc_no: row.disc_no,
            lyrics: row.lyrics.clone(),
            timeline: row.timeline.clone(),
            cover_hash: row.cover_hash.clone(),
            file_mtime: Some(row.mtime_ms),
            file_size: i64::try_from(row.size).ok(),
            tags_read_at: Some(queries::now_ms()),
        },
    )?;
    if let Some(hash) = row.cover_hash.as_deref() {
        seed_group_covers(conn, folder_id, album_id, hash)?;
    }
    Ok(outcome)
}

fn seed_group_covers(conn: &Connection, folder_id: i64, album_id: Option<i64>, hash: &str) -> queries::Result<()> {
    let mut folder = conn.prepare_cached("SELECT cover_hash IS NULL FROM folders WHERE id = ?1")?;
    if folder.query_row([folder_id], |row| row.get(0))? {
        queries::set_folder_cover(conn, folder_id, Some(hash))?;
    }
    if let Some(album_id) = album_id {
        let mut album = conn.prepare_cached("SELECT cover_hash IS NULL FROM albums WHERE id = ?1")?;
        if album.query_row([album_id], |row| row.get(0))? {
            queries::set_album_cover(conn, album_id, Some(hash), false)?;
        }
    }
    Ok(())
}

/// Xoá thư mục không còn bài như `lib.rs:280`; chỉ import, vì chỉ import mới xoá dòng.
fn delete_empty_folders(conn: &Connection) -> queries::Result<usize> {
    let sql = "DELETE FROM folders WHERE NOT EXISTS (SELECT 1 FROM songs WHERE songs.folder_id = folders.id)";
    Ok(conn.prepare_cached(sql)?.execute([])?)
}

/// Album 12 bài có 12 bản bìa giống nhau: mỗi hash giải một lần. Khoá giữ suốt `save`, nên dù nhiều luồng đọc thẻ, mỗi lúc
/// chỉ một ảnh đang giải (đỉnh RAM một lần giải mã, xem `groove-gd1_07`).
struct CoverPipeline<'a> {
    store: &'a CoverStore,
    state: Mutex<CoverState>,
}

#[derive(Default)]
struct CoverState {
    known: HashSet<String>,
    broken: HashSet<String>,
}

impl<'a> CoverPipeline<'a> {
    fn load(store: &'a CoverStore, library: &Library) -> LibraryResult<CoverPipeline<'a>> {
        let known = library
            .read(|conn| {
                let mut stmt = conn.prepare_cached("SELECT hash FROM covers")?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                Ok(rows.collect::<rusqlite::Result<HashSet<String>>>()?)
            })
            .wait()?;
        Ok(CoverPipeline { store, state: Mutex::new(CoverState { known, broken: HashSet::new() }) })
    }

    /// `Err` của ảnh chỉ mất bìa, bài vẫn vào thư viện. Có `CoverRecord` khi vừa lưu, caller chèn dòng `covers`.
    fn prepare(&self, path: &Path, cover: &CoverArt) -> (Option<String>, Option<CoverRecord>) {
        let hash = cover_hash(&cover.data);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.broken.contains(&hash) {
            return (None, None);
        }
        // Cache có thể bị xoá trong khi DB còn dòng, nên vẫn soát tệp.
        if state.known.contains(&hash) && self.store.has_files(&hash) {
            return (Some(hash), None);
        }
        match self.store.save(&cover.data) {
            Ok(record) => {
                state.known.insert(hash);
                (Some(record.hash.clone()), Some(record))
            }
            Err(err) => {
                log::warn!("cover of {} skipped: {err}", path.display());
                state.broken.insert(hash);
                (None, None)
            }
        }
    }
}

enum ReadOutcome {
    Ready(Box<TrackRow>, Option<CoverRecord>),
    Missing,
    Failed,
}

fn read_one(path: &Path, covers: &CoverPipeline<'_>) -> ReadOutcome {
    match tags::read_track(path) {
        Ok(mut track) => {
            let (hash, record) = match track.cover.take() {
                Some(cover) => covers.prepare(path, &cover),
                None => (None, None),
            };
            ReadOutcome::Ready(Box::new(TrackRow::from_scanned(path.to_owned(), track, hash)), record)
        }
        Err(TagError::NotFound(_)) => ReadOutcome::Missing,
        Err(err) => {
            log::warn!("could not read {}: {err}", path.display());
            ReadOutcome::Failed
        }
    }
}

/// Giữ đúng thứ tự đầu vào: bài đầu tiên có bìa quyết định bìa nhóm. Tối đa một lô trong bộ nhớ.
fn read_chunk(chunk: &[PathBuf], covers: &CoverPipeline<'_>) -> Vec<ReadOutcome> {
    let threads = thread::available_parallelism().map_or(1, |count| count.get()).clamp(1, MAX_READ_THREADS);
    let next = AtomicUsize::new(0);
    let mut indexed: Vec<(usize, ReadOutcome)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.min(chunk.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = chunk.get(index) else {
                            return done;
                        };
                        done.push((index, read_one(path, covers)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
    });
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, outcome)| outcome).collect()
}

/// `song_file_states` không có `tags_read_at`, nên import tự lấy dấu vết: dòng chưa đọc thẻ luôn phải đọc, mtime của nó còn là giây.
struct KnownRow {
    id: i64,
    path: PathBuf,
    path_key: String,
    mtime: Option<i64>,
    size: Option<i64>,
    tags_read: bool,
}

fn known_rows(library: &Library) -> LibraryResult<Vec<KnownRow>> {
    library
        .read(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT id, file_path, path_key, file_mtime, file_size, tags_read_at NOT NULL FROM songs",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(KnownRow {
                    id: row.get(0)?,
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    path_key: row.get(2)?,
                    mtime: row.get(3)?,
                    size: row.get(4)?,
                    tags_read: row.get(5)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
        .wait()
}

/// Bài nằm trong gốc, không thấy lúc quét, chỗ chứa đã được xem hết, và hệ tệp nói chắc chắn là không có.
fn plan_removals(
    under_root: &[&KnownRow],
    scan: &ScanOutcome,
    scanned: &HashSet<String>,
    probe: impl Fn(&Path) -> io::Result<()>,
) -> Vec<i64> {
    under_root
        .iter()
        .filter(|row| !scanned.contains(&row.path_key) && !scan.may_still_exist(&row.path))
        .filter(|row| probe(&row.path).is_err_and(|err| err.kind() == io::ErrorKind::NotFound))
        .map(|row| row.id)
        .collect()
}

fn probe_path(path: &Path) -> io::Result<()> {
    fs::symlink_metadata(path).map(|_| ())
}

/// Sao chép rẻ. Mỗi lúc một việc chạy, việc sau chờ trên luồng của nó, để đỉnh RAM và thứ tự ghi không cộng dồn.
#[derive(Clone)]
pub struct Indexer {
    library: Library,
    covers: CoverStore,
    running: Arc<Mutex<()>>,
}

impl Indexer {
    pub fn new(library: Library, covers: CoverStore) -> Indexer {
        Indexer { library, covers, running: Arc::new(Mutex::new(())) }
    }

    /// Đường chạy thật: bìa ở `<cache>/covers`.
    pub fn standard(library: Library) -> io::Result<Indexer> {
        Ok(Indexer::new(library, CoverStore::default_location()?))
    }

    /// Đặc tả 1.11: quét đệ quy, xoá dòng đã mất thật, đọc thẻ tệp mới hoặc đổi, ghi gốc theo dõi, dọn album và thư mục rỗng.
    /// Tiến độ qua `LibraryEvent::Progress`, xong phát `Synced`.
    pub fn import(&self, root: impl Into<PathBuf>) -> IndexJob<ImportReport> {
        let root = root.into();
        self.spawn(move |indexer| indexer.run_import(&root))
    }

    /// Đọc lại mọi bài, đủ trường (S7). Tệp thiếu chỉ đếm, không xoá.
    pub fn rescan(&self) -> IndexJob<IndexTally> {
        self.spawn(|indexer| indexer.run_rescan())
    }

    /// Bài `tags_read_at IS NULL` sau khi chuyển DB cũ (đặc tả 2.3 bước 5): điền trường mới, gán lại album. Không có gì thì
    /// xong ngay, gọi được mỗi lần mở app. Bài đọc lỗi giữ `NULL`, lần mở sau thử lại.
    pub fn backfill_tags(&self) -> IndexJob<IndexTally> {
        self.spawn(|indexer| indexer.run_backfill())
    }

    /// Đọc đúng các tệp này, cho watcher. Không phát `Synced`, caller gộp với phần xoá và đổi tên của nó. Không xoá gì.
    pub fn index_paths(&self, paths: Vec<PathBuf>) -> IndexJob<IndexTally> {
        self.spawn(move |indexer| {
            let mut tally = IndexTally::default();
            let covers = CoverPipeline::load(&indexer.covers, &indexer.library)?;
            let mut steps = ProgressSteps::silent();
            indexer.index_files(&paths, &covers, &mut steps, &mut tally)?;
            indexer.library.writer().transaction(|conn| Ok(queries::delete_empty_albums(conn)?)).wait()?;
            Ok(tally)
        })
    }

    fn spawn<T, F>(&self, job: F) -> IndexJob<T>
    where
        T: Send + 'static,
        F: FnOnce(&Indexer) -> Result<T, IndexError> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let indexer = self.clone();
        let spawned = thread::Builder::new().name("groove-indexer".to_owned()).spawn(move || {
            let result = {
                let _running = indexer.running.lock().unwrap_or_else(PoisonError::into_inner);
                job(&indexer)
            };
            let _ = tx.send(result);
        });
        // Không tạo được luồng: đầu gửi đã bị thả, `IndexJob` ra `Closed`.
        if let Err(err) = spawned {
            log::error!("could not start the indexer: {err}");
        }
        IndexJob { rx }
    }

    fn run_import(&self, root: &Path) -> Result<ImportReport, IndexError> {
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
        let scan = scanner::scan(&root);
        if let Some(failure) = scan.root_failure(&root) {
            return Err(IndexError::RootUnavailable { path: root, message: failure.message.clone() });
        }
        let mut report = ImportReport {
            failed_dirs: scan.failed_dirs.iter().map(|failure| failure.path.clone()).collect(),
            ..ImportReport::default()
        };
        let result = self.import_steps(&root, &scan, &mut report);
        // Lô đã commit vẫn là thật dù lô sau lỗi: UI phải nạp lại.
        if result.is_ok() || report.tally.read() > 0 || !report.removed.is_empty() {
            self.emit_synced(&report.tally, report.removed.clone());
        }
        result?;
        Ok(report)
    }

    fn import_steps(&self, root: &Path, scan: &ScanOutcome, report: &mut ImportReport) -> LibraryResult<()> {
        let rows = known_rows(&self.library)?;
        let root_key = paths::normalize_path(root);
        let under_root: Vec<&KnownRow> = rows.iter().filter(|row| paths::is_under(&row.path_key, &root_key)).collect();

        report.root_offline = scan.files.is_empty() && !under_root.is_empty();
        let removals = if report.root_offline {
            Vec::new()
        } else {
            let scanned: HashSet<String> =
                scan.files.iter().map(|file| schema::path_key(&file.path.to_string_lossy())).collect();
            plan_removals(&under_root, scan, &scanned, probe_path)
        };

        let known: HashMap<&str, &KnownRow> = rows.iter().map(|row| (row.path_key.as_str(), row)).collect();
        let mut to_read = Vec::new();
        for file in &scan.files {
            let unchanged = known.get(schema::path_key(&file.path.to_string_lossy()).as_str()).is_some_and(|row| {
                row.tags_read
                    && row.mtime == Some(file.stamp.mtime_ms)
                    && row.size == i64::try_from(file.stamp.size).ok()
            });
            if unchanged {
                report.unchanged += 1;
            } else {
                to_read.push(file.path.clone());
            }
        }
        drop(known);
        drop(under_root);
        drop(rows);

        for chunk in removals.chunks(BATCH_ROWS) {
            let chunk = chunk.to_vec();
            let removed = self
                .library
                .writer()
                .transaction(move |conn| {
                    let mut removed = Vec::with_capacity(chunk.len());
                    for id in chunk {
                        if queries::delete_song(conn, id)? {
                            removed.push(id);
                        }
                    }
                    Ok(removed)
                })
                .wait()?;
            report.removed.extend(removed);
        }

        let covers = CoverPipeline::load(&self.covers, &self.library)?;
        let mut steps = ProgressSteps::new(ProgressKind::Import, to_read.len());
        self.index_files(&to_read, &covers, &mut steps, &mut report.tally)?;

        let root = root.to_owned();
        self.library
            .writer()
            .transaction(move |conn| {
                queries::add_watched_root(conn, &root)?;
                queries::delete_empty_albums(conn)?;
                delete_empty_folders(conn)?;
                Ok(())
            })
            .wait()
    }

    fn run_rescan(&self) -> Result<IndexTally, IndexError> {
        let paths: Vec<PathBuf> = self.library.song_file_states().wait()?.into_iter().map(|state| state.path).collect();
        let mut tally = IndexTally::default();
        let covers = CoverPipeline::load(&self.covers, &self.library)?;
        let mut steps = ProgressSteps::new(ProgressKind::Rescan, paths.len());
        let result = self.index_files(&paths, &covers, &mut steps, &mut tally).and_then(|()| self.tidy_albums());
        if result.is_ok() || tally.read() > 0 {
            self.emit_synced(&tally, Vec::new());
        }
        result?;
        Ok(tally)
    }

    fn run_backfill(&self) -> Result<IndexTally, IndexError> {
        let total: usize = self
            .library
            .read(|conn| {
                let sql = "SELECT COUNT(*) FROM songs WHERE tags_read_at IS NULL";
                Ok(conn.prepare_cached(sql)?.query_row([], |row| row.get::<_, i64>(0))? as usize)
            })
            .wait()?;
        let mut tally = IndexTally::default();
        if total == 0 {
            return Ok(tally);
        }
        let covers = CoverPipeline::load(&self.covers, &self.library)?;
        let mut steps = ProgressSteps::new(ProgressKind::TagBackfill, total);
        // Theo con trỏ `id`, không theo `pending_tag_reads`: bài đọc lỗi vẫn `NULL` sẽ bị lấy lại mãi.
        let mut after = 0;
        let result = loop {
            let page = self.pending_after(after);
            match page {
                Ok(page) if page.is_empty() => break self.tidy_albums(),
                Ok(page) => {
                    after = page.last().map_or(after, |(id, _)| *id);
                    let paths: Vec<PathBuf> = page.into_iter().map(|(_, path)| path).collect();
                    if let Err(err) = self.index_files(&paths, &covers, &mut steps, &mut tally) {
                        break Err(err);
                    }
                }
                Err(err) => break Err(err),
            }
        };
        if tally.read() > 0 {
            self.emit_synced(&tally, Vec::new());
        }
        result?;
        Ok(tally)
    }

    fn pending_after(&self, after: i64) -> LibraryResult<Vec<(i64, PathBuf)>> {
        self.library
            .read(move |conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT id, file_path FROM songs WHERE tags_read_at IS NULL AND id > ?1 ORDER BY id LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![after, BATCH_ROWS as i64], |row| {
                    Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?)))
                })?;
                Ok(rows.collect::<rusqlite::Result<_>>()?)
            })
            .wait()
    }

    fn tidy_albums(&self) -> LibraryResult<()> {
        self.library.writer().transaction(|conn| Ok(queries::delete_empty_albums(conn)?)).wait().map(drop)
    }

    /// `.wait()` từng lô là backpressure: channel ghi không giới hạn, không chờ thì cả thư viện nằm trong RAM.
    fn index_files(
        &self,
        files: &[PathBuf],
        covers: &CoverPipeline<'_>,
        steps: &mut ProgressSteps,
        tally: &mut IndexTally,
    ) -> LibraryResult<()> {
        for chunk in files.chunks(BATCH_ROWS) {
            let outcomes = read_chunk(chunk, covers);
            let mut rows = Vec::with_capacity(outcomes.len());
            let mut records = Vec::new();
            for (path, outcome) in chunk.iter().zip(outcomes) {
                match outcome {
                    ReadOutcome::Ready(row, record) => {
                        rows.push(*row);
                        records.extend(record);
                    }
                    ReadOutcome::Missing => tally.missing.push(path.clone()),
                    ReadOutcome::Failed => tally.failed.push(path.clone()),
                }
            }
            if !rows.is_empty() {
                let (added, updated) = self
                    .library
                    .writer()
                    .transaction(move |conn| {
                        for record in &records {
                            queries::upsert_cover(conn, record)?;
                        }
                        let mut folders = HashMap::new();
                        let (mut added, mut updated) = (0, 0);
                        for row in &rows {
                            if apply_row(conn, row, &mut folders)?.inserted {
                                added += 1;
                            } else {
                                updated += 1;
                            }
                        }
                        Ok((added, updated))
                    })
                    .wait()?;
                tally.added += added;
                tally.updated += updated;
            }
            if let Some(progress) = steps.advance(chunk.len()) {
                self.library.writer().emit(LibraryEvent::Progress(progress));
            }
        }
        Ok(())
    }

    fn emit_synced(&self, tally: &IndexTally, removed: Vec<i64>) {
        let report = SyncReport { added: tally.added, updated: tally.updated, removed };
        self.library.writer().emit(LibraryEvent::Synced(report));
    }
}

struct ProgressSteps {
    kind: Option<ProgressKind>,
    done: usize,
    total: usize,
}

impl ProgressSteps {
    fn new(kind: ProgressKind, total: usize) -> ProgressSteps {
        ProgressSteps { kind: Some(kind), done: 0, total }
    }

    fn silent() -> ProgressSteps {
        ProgressSteps { kind: None, done: 0, total: 0 }
    }

    fn advance(&mut self, count: usize) -> Option<Progress> {
        self.done += count;
        self.kind.map(|kind| Progress { kind, done: self.done, total: self.total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::ScanFailure;

    fn known(id: i64, path: &str) -> KnownRow {
        KnownRow {
            id,
            path: PathBuf::from(path),
            path_key: schema::path_key(path),
            mtime: None,
            size: None,
            tags_read: true,
        }
    }

    fn plan(rows: &[KnownRow], scan: &ScanOutcome, scanned: &[&str], kind: io::ErrorKind) -> Vec<i64> {
        let scanned = scanned.iter().map(|path| schema::path_key(path)).collect();
        let refs: Vec<&KnownRow> = rows.iter().collect();
        plan_removals(&refs, scan, &scanned, |_| Err(kind.into()))
    }

    #[test]
    fn only_a_confirmed_absence_removes_a_row() {
        let rows = [known(1, "/m/a.mp3"), known(2, "/m/b.mp3")];
        let scan = ScanOutcome::default();
        assert_eq!(plan(&rows, &scan, &["/m/a.mp3"], io::ErrorKind::NotFound), [2]);
        // N1: mất quyền, ổ mạng chập chờn, lỗi lạ đều không phải là đã xoá.
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::TimedOut, io::ErrorKind::Other] {
            assert!(plan(&rows, &scan, &["/m/a.mp3"], kind).is_empty(), "{kind:?}");
        }
        let exists = rows.iter().collect::<Vec<_>>();
        let found = plan_removals(&exists, &scan, &HashSet::new(), |_| Ok(()));
        assert!(found.is_empty());
    }

    #[test]
    fn rows_under_unseen_places_survive_even_if_absent() {
        let rows = [known(1, "/m/sub/a.mp3"), known(2, "/m/link/b.mp3"), known(3, "/m/c.mp3"), known(4, "/m/d.mp3")];
        let message = String::from("denied");
        let scan = ScanOutcome {
            failed_dirs: vec![ScanFailure { path: "/m/sub".into(), message: message.clone() }],
            failed_files: vec![ScanFailure { path: "/m/c.mp3".into(), message }],
            skipped_links: vec!["/m/link".into()],
            files: Vec::new(),
        };
        assert_eq!(plan(&rows, &scan, &[], io::ErrorKind::NotFound), [4]);
    }

    #[test]
    fn numbers_come_from_the_tag_text() {
        assert_eq!(parse_number(Some("3")), Some(3));
        assert_eq!(parse_number(Some("03/12")), Some(3));
        assert_eq!(parse_number(Some(" 7 ")), Some(7));
        for raw in ["", "0", "A1", "/12", "-1"] {
            assert_eq!(parse_number(Some(raw)), None, "{raw}");
        }
        assert_eq!(parse_number(None), None);
    }
}
