//! groove-gd1_09: so đĩa với DB khi mở app (N1), kế hoạch chung cho import, reconcile, watcher (N4, S6).
//!
//! Ba nơi cùng đi một đường: gom thứ đã thấy trên đĩa thành `Changes`, `plan` so với DB thành `SyncPlan`,
//! `Indexer::apply_plan` ghi. Dòng chỉ bị đánh dấu `missing_since`; xoá thật chỉ ở `reconcile` sau `MISSING_GRACE_MS`, hoặc
//! `forget_missing` khi người dùng xác nhận.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use groove_core::paths;

use crate::actor::{LibraryEvent, LibraryResult, MovedSong, Reply, SyncReport};
use crate::db::queries;
use crate::db::schema;
use crate::indexer::{self, IndexError, IndexJob, IndexTally, Indexer, KnownRow, TrackRow, delete_empty_folders};
use crate::scanner::{self, ScanOutcome, ScannedFile};
use crate::store::Library;

/// Đặc tả N1: quá 30 ngày mà tệp vẫn không có thì mới dọn dòng.
pub const MISSING_GRACE_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Cùng một tệp đọc hai lần ra cùng thời lượng, chỉ chừa sai số làm tròn.
const SAME_DURATION_SEC: f64 = 0.01;
/// Dòng DB cũ lưu thời lượng theo giây, chưa có `file_size`.
const SAME_DURATION_LEGACY_SEC: f64 = 1.0;

/// Một lượt đồng bộ đã làm gì.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub tally: IndexTally,
    /// `(mtime, size)` khớp DB nên không đọc lại (S6, S26).
    pub unchanged: usize,
    /// Đổi tên, chuyển chỗ ngoài app, giữ `id` (N4).
    pub moved: Vec<MovedSong>,
    /// Đang đánh dấu mất mà tệp đã quay lại.
    pub restored: Vec<i64>,
    /// Vừa xác nhận mất.
    pub marked_missing: Vec<i64>,
    /// Mất quá hạn, đã xoá dòng.
    pub purged: Vec<i64>,
}

impl SyncOutcome {
    /// Không đổi gì thì không phát sự kiện: không toast, không nạp lại (S6).
    pub fn changed_anything(&self) -> bool {
        self.tally.read() > 0
            || !self.moved.is_empty()
            || !self.restored.is_empty()
            || !self.marked_missing.is_empty()
            || !self.purged.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootState {
    Scanned,
    /// Quét ra 0 tệp trong khi DB còn dòng dưới gốc: không đụng gì (N1).
    Offline,
    /// Không đọc được, hoặc không tồn tại.
    Unavailable(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub sync: SyncOutcome,
    pub roots: Vec<(PathBuf, RootState)>,
    /// Thư mục con không đọc được, dòng dưới đó được giữ (N1).
    pub failed_dirs: Vec<PathBuf>,
}

/// Thứ vừa thấy trên đĩa, chưa so với DB.
#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub(crate) seen: Vec<ScannedFile>,
    /// Dòng mà hệ tệp đã trả lời chắc chắn là không có.
    pub(crate) vanished: Vec<i64>,
    /// `path_key` đích của cặp đổi tên do watcher báo, chắc chắn hơn ghép theo thẻ.
    pub(crate) hints: HashMap<String, i64>,
}

/// Dòng chưa đọc thẻ từ khi chuyển DB cũ: mtime còn là giây, nên không phải "đã đổi".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Untagged {
    /// Import: người dùng yêu cầu, đọc luôn.
    Read,
    /// Mở app, watcher: để `backfill_tags` đọc, không báo cả nghìn bài cập nhật.
    Skip,
}

#[derive(Debug, Default)]
pub(crate) struct SyncPlan {
    pub(crate) fresh: Vec<ScannedFile>,
    pub(crate) changed: Vec<PathBuf>,
    pub(crate) restored: Vec<i64>,
    /// Đã đọc thẻ mà chưa có dấu tệp: lấy hiện tại làm mốc, không đọc lại.
    pub(crate) baselines: Vec<(i64, i64, i64)>,
    /// Cùng `path_key`, khác cách viết: đổi hoa thường trên hệ tệp gập.
    pub(crate) respelled: Vec<(i64, PathBuf)>,
    pub(crate) vanished: Vec<i64>,
    pub(crate) targets: Vec<RenameTarget>,
    pub(crate) hints: HashMap<String, i64>,
    pub(crate) unchanged: usize,
}

pub(crate) fn is_excluded_key(key: &str, excluded: &[String]) -> bool {
    excluded.iter().any(|root| paths::is_under(key, root))
}

/// Thêm kết quả quét một gốc vào `changes`. Trả `true` khi gốc bị coi là offline: khi đó không thêm gì.
pub(crate) fn collect_root(
    rows: &[KnownRow],
    excluded: &[String],
    root: &Path,
    scan: &ScanOutcome,
    changes: &mut Changes,
    probe: impl Fn(&Path) -> io::Result<()>,
) -> bool {
    let root_key = paths::normalize_path(root);
    let under_root: Vec<&KnownRow> = rows
        .iter()
        .filter(|row| paths::is_under(&row.path_key, &root_key) && !is_excluded_key(&row.path_key, excluded))
        .collect();
    let files: Vec<&ScannedFile> =
        scan.files.iter().filter(|file| !is_excluded_key(&file_key(&file.path), excluded)).collect();
    if files.is_empty() && !under_root.is_empty() {
        return true;
    }
    let scanned: HashSet<String> = files.iter().map(|file| file_key(&file.path)).collect();
    changes.vanished.extend(indexer::plan_removals(&under_root, scan, &scanned, probe));
    changes.seen.extend(files.into_iter().cloned());
    false
}

fn file_key(path: &Path) -> String {
    schema::path_key(&path.to_string_lossy())
}

/// So thứ đã thấy với DB. `probe` chỉ gọi khi có tệp mới cần ghép.
pub(crate) fn plan(
    rows: &[KnownRow],
    excluded: &[String],
    changes: Changes,
    untagged: Untagged,
    probe: impl Fn(&Path) -> io::Result<()>,
) -> SyncPlan {
    let known: HashMap<&str, &KnownRow> = rows.iter().map(|row| (row.path_key.as_str(), row)).collect();
    let mut plan = SyncPlan { hints: changes.hints, ..SyncPlan::default() };
    let mut seen_keys = HashSet::new();
    for file in changes.seen {
        let key = file_key(&file.path);
        if is_excluded_key(&key, excluded) || !seen_keys.insert(key.clone()) {
            continue;
        }
        let Some(row) = known.get(key.as_str()) else {
            plan.fresh.push(file);
            continue;
        };
        if row.path != file.path {
            plan.respelled.push((row.id, file.path.clone()));
        }
        let size = i64::try_from(file.stamp.size).ok();
        let has_stamp = row.mtime.is_some() && row.size.is_some();
        let same = row.mtime == Some(file.stamp.mtime_ms) && row.size == size;
        let read = match (row.tags_read, untagged) {
            (false, Untagged::Read) => true,
            (false, Untagged::Skip) => false,
            (true, Untagged::Read) => !same,
            (true, Untagged::Skip) => has_stamp && !same,
        };
        if read {
            plan.changed.push(file.path);
            continue;
        }
        if row.missing_since.is_some() {
            plan.restored.push(row.id);
        }
        if row.tags_read
            && !has_stamp
            && let Some(size) = size
        {
            plan.baselines.push((row.id, file.stamp.mtime_ms, size));
        }
        plan.unchanged += 1;
    }

    let by_id: HashMap<i64, &KnownRow> = rows.iter().map(|row| (row.id, row)).collect();
    let mut vanished_ids = HashSet::new();
    for id in changes.vanished {
        let Some(row) = by_id.get(&id) else { continue };
        if !seen_keys.contains(&row.path_key) && vanished_ids.insert(id) {
            plan.vanished.push(id);
        }
    }
    if !plan.fresh.is_empty() {
        let old_missing = rows.iter().filter(|row| {
            row.missing_since.is_some()
                && !vanished_ids.contains(&row.id)
                && !seen_keys.contains(&row.path_key)
                && !is_excluded_key(&row.path_key, excluded)
                && probe(&row.path).is_err_and(|err| err.kind() == io::ErrorKind::NotFound)
        });
        plan.targets = old_missing.map(RenameTarget::from_row).collect();
        plan.targets.extend(plan.vanished.iter().filter_map(|id| by_id.get(id)).map(|row| RenameTarget::from_row(row)));
    }
    plan
}

/// Dòng có thể là nguồn của một tệp mới.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenameTarget {
    pub(crate) id: i64,
    pub(crate) path: PathBuf,
    filename: String,
    size: Option<i64>,
    duration_sec: f64,
    title: String,
}

impl RenameTarget {
    fn from_row(row: &KnownRow) -> RenameTarget {
        RenameTarget {
            id: row.id,
            path: row.path.clone(),
            filename: filename_key(&row.path),
            size: row.size,
            duration_sec: row.duration_sec,
            title: schema::normalize_text(row.title.as_deref().unwrap_or_default()),
        }
    }

    fn accepts(&self, track: &NewTrack) -> bool {
        match self.size {
            Some(size) => {
                size == track.size
                    && self.title == track.title
                    && (self.duration_sec - track.duration_sec).abs() < SAME_DURATION_SEC
            }
            // DB cũ không có cỡ tệp: chỉ tin khi còn nguyên tên tệp và có tiêu đề để so.
            None => {
                self.filename == track.filename
                    && !self.title.is_empty()
                    && self.title == track.title
                    && (self.duration_sec - track.duration_sec).abs() < SAME_DURATION_LEGACY_SEC
            }
        }
    }
}

/// Tệp mới đã đọc thẻ, rút gọn để ghép.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NewTrack {
    path_key: String,
    filename: String,
    size: i64,
    duration_sec: f64,
    title: String,
}

impl NewTrack {
    pub(crate) fn from_row(row: &TrackRow) -> NewTrack {
        NewTrack {
            path_key: file_key(&row.path),
            filename: filename_key(&row.path),
            size: i64::try_from(row.size).unwrap_or(i64::MAX),
            duration_sec: row.duration_sec,
            title: schema::normalize_text(row.title.as_deref().unwrap_or_default()),
        }
    }
}

fn filename_key(path: &Path) -> String {
    file_key(Path::new(path.file_name().unwrap_or_default()))
}

/// Chỉ đọc trước những tệp có thể ghép được, để phần còn lại vẫn đi theo lô thường. `(candidates, rest)`.
pub(crate) fn split_rename_candidates(
    fresh: Vec<ScannedFile>,
    targets: &[RenameTarget],
    hints: &HashMap<String, i64>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let sizes: HashSet<i64> = targets.iter().filter_map(|target| target.size).collect();
    let legacy_names: HashSet<&str> =
        targets.iter().filter(|target| target.size.is_none()).map(|target| target.filename.as_str()).collect();
    let mut candidates = Vec::new();
    let mut rest = Vec::new();
    for file in fresh {
        let by_hint = hints.contains_key(&file_key(&file.path));
        let by_size = i64::try_from(file.stamp.size).is_ok_and(|size| sizes.contains(&size));
        let by_name = legacy_names.contains(filename_key(&file.path).as_str());
        if by_hint || by_size || by_name {
            candidates.push(file.path);
        } else {
            rest.push(file.path);
        }
    }
    (candidates, rest)
}

/// N4: chỉ số trong `tracks` thành `id` dòng cũ. Một dòng, một tệp. Lần lượt: cặp watcher báo, rồi cùng tên tệp (đổi tên thư
/// mục), rồi khác tên (đổi tên tệp). Không chắc thì không ghép: hai bản sao giống hệt mà không phân biệt được thì thành dòng
/// mới và dòng mất, không gán nhầm playlist.
pub(crate) fn match_renames(
    targets: &[RenameTarget],
    tracks: &[NewTrack],
    hints: &HashMap<String, i64>,
) -> HashMap<usize, i64> {
    let mut pairs = HashMap::new();
    let mut taken = HashSet::new();
    let known: HashSet<i64> = targets.iter().map(|target| target.id).collect();
    for (index, track) in tracks.iter().enumerate() {
        if let Some(id) = hints.get(&track.path_key).filter(|id| known.contains(id))
            && taken.insert(*id)
        {
            pairs.insert(index, *id);
        }
    }
    for same_name in [true, false] {
        let mut by_size: HashMap<Option<i64>, Vec<usize>> = HashMap::new();
        for (index, target) in targets.iter().enumerate() {
            if !taken.contains(&target.id) {
                by_size.entry(target.size).or_default().push(index);
            }
        }
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut track_degree: HashMap<usize, usize> = HashMap::new();
        let mut target_degree: HashMap<usize, usize> = HashMap::new();
        for (index, track) in tracks.iter().enumerate() {
            if pairs.contains_key(&index) {
                continue;
            }
            let exact = by_size.get(&Some(track.size)).into_iter().flatten();
            let legacy = by_size.get(&None).into_iter().flatten();
            for &target_index in exact.chain(legacy) {
                let target = &targets[target_index];
                if target.accepts(track) && (!same_name || target.filename == track.filename) {
                    edges.push((index, target_index));
                    *track_degree.entry(index).or_default() += 1;
                    *target_degree.entry(target_index).or_default() += 1;
                }
            }
        }
        for (index, target_index) in edges {
            if track_degree[&index] == 1 && target_degree[&target_index] == 1 {
                pairs.insert(index, targets[target_index].id);
                taken.insert(targets[target_index].id);
            }
        }
    }
    pairs
}

/// Đặc tả 1.11: so đĩa với DB. Gắn watcher trước rồi mới gọi, để thay đổi trong lúc quét không lọt. Gốc không đọc được hoặc
/// offline thì bỏ qua; tệp ngoài mọi gốc không đụng. Có thay đổi mới phát sự kiện.
pub fn reconcile(indexer: &Indexer) -> IndexJob<ReconcileReport> {
    indexer.spawn(|indexer| run_reconcile(indexer, queries::now_ms()))
}

/// `now_ms` riêng cho test thời gian chờ.
#[doc(hidden)]
pub fn reconcile_at(indexer: &Indexer, now_ms: i64) -> IndexJob<ReconcileReport> {
    indexer.spawn(move |indexer| run_reconcile(indexer, now_ms))
}

fn run_reconcile(indexer: &Indexer, now_ms: i64) -> Result<ReconcileReport, IndexError> {
    let library = indexer.library();
    let roots = library.watched_roots().wait()?;
    let excluded = library.read(|conn| Ok(queries::excluded_keys(conn)?)).wait()?;
    let rows = indexer::known_rows(library)?;
    let mut report = ReconcileReport::default();
    let mut changes = Changes::default();
    let mut expired: Vec<i64> = Vec::new();

    for root in roots {
        let scan = scanner::scan(&root);
        if let Some(failure) = scan.root_failure(&root) {
            log::info!("library root {} skipped: {}", root.display(), failure.message);
            report.roots.push((root, RootState::Unavailable(failure.message.clone())));
            continue;
        }
        report.failed_dirs.extend(scan.failed_dirs.iter().map(|failure| failure.path.clone()));
        if collect_root(&rows, &excluded, &root, &scan, &mut changes, indexer::probe_path) {
            log::info!("library root {} looks offline, nothing touched", root.display());
            report.roots.push((root, RootState::Offline));
            continue;
        }
        expired.extend(expired_rows(&rows, &excluded, &root, &scan, now_ms, indexer::probe_path));
        report.roots.push((root, RootState::Scanned));
    }

    let plan = plan(&rows, &excluded, changes, Untagged::Skip, indexer::probe_path);
    drop(rows);
    let applied = indexer.apply_plan(plan, None, &mut report.sync);
    let purged = applied.and_then(|()| purge(library, expired));
    match purged {
        Ok(ids) => report.sync.purged = ids,
        Err(err) => {
            if report.sync.changed_anything() {
                indexer.emit_outcome(&report.sync);

                indexer.emit_external_change(&report.sync);
            }
            return Err(err.into());
        }
    }
    if report.sync.changed_anything() {
        indexer.emit_outcome(&report.sync);

        indexer.emit_external_change(&report.sync);
    }
    Ok(report)
}

/// Quá hạn, nằm dưới gốc vừa xem được hết, và lúc này vẫn chắc chắn không có. Ổ tháo ra suốt 30 ngày thì gốc offline, không dọn.
fn expired_rows(
    rows: &[KnownRow],
    excluded: &[String],
    root: &Path,
    scan: &ScanOutcome,
    now_ms: i64,
    probe: impl Fn(&Path) -> io::Result<()>,
) -> Vec<i64> {
    let root_key = paths::normalize_path(root);
    let scanned: HashSet<String> = scan.files.iter().map(|file| file_key(&file.path)).collect();
    rows.iter()
        .filter(|row| row.missing_since.is_some_and(|since| now_ms.saturating_sub(since) >= MISSING_GRACE_MS))
        .filter(|row| paths::is_under(&row.path_key, &root_key) && !is_excluded_key(&row.path_key, excluded))
        .filter(|row| !scanned.contains(&row.path_key) && !scan.may_still_exist(&row.path))
        .filter(|row| probe(&row.path).is_err_and(|err| err.kind() == io::ErrorKind::NotFound))
        .map(|row| row.id)
        .collect()
}

fn purge(library: &Library, ids: Vec<i64>) -> LibraryResult<Vec<i64>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    library.writer().transaction(move |conn| purge_rows(conn, ids)).wait()
}

fn purge_rows(conn: &rusqlite::Connection, ids: Vec<i64>) -> LibraryResult<Vec<i64>> {
    let mut purged = Vec::with_capacity(ids.len());
    for id in ids {
        if queries::delete_missing_song(conn, id)? {
            purged.push(id);
        }
    }
    queries::delete_empty_albums(conn)?;
    delete_empty_folders(conn)?;
    Ok(purged)
}

/// Bài đang đánh dấu mất, cho hộp thoại xác nhận.
pub fn missing_songs(library: &Library) -> Reply<Vec<queries::MissingSong>> {
    library.read(|conn| Ok(queries::missing_songs(conn)?))
}

/// Người dùng xác nhận bỏ các bài này (đặc tả N1), không chờ hết hạn. Bài không còn đánh dấu mất thì bỏ qua: tệp vừa quay lại
/// không bị xoá nhầm. Trả `id` đã xoá, phát `Synced`.
pub fn forget_missing(library: &Library, ids: Vec<i64>) -> Reply<Vec<i64>> {
    let writer = library.writer().clone();
    library.writer().transaction(move |conn| {
        let purged = purge_rows(conn, ids)?;
        if !purged.is_empty() {
            writer.emit(LibraryEvent::Synced(SyncReport { added: 0, updated: 0, removed: purged.clone() }));
        }
        Ok(purged)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: i64, path: &str, size: Option<i64>, title: &str) -> RenameTarget {
        RenameTarget {
            id,
            path: path.into(),
            filename: filename_key(Path::new(path)),
            size,
            duration_sec: 10.0,
            title: title.to_owned(),
        }
    }

    fn track(path: &str, size: i64, title: &str) -> NewTrack {
        NewTrack {
            path_key: file_key(Path::new(path)),
            filename: filename_key(Path::new(path)),
            size,
            duration_sec: 10.0,
            title: title.to_owned(),
        }
    }

    fn matched(targets: &[RenameTarget], tracks: &[NewTrack]) -> Vec<(usize, i64)> {
        let mut pairs: Vec<_> = match_renames(targets, tracks, &HashMap::new()).into_iter().collect();
        pairs.sort_unstable();
        pairs
    }

    #[test]
    fn a_renamed_file_matches_on_size_duration_and_title() {
        let targets = [target(1, "/m/a.mp3", Some(100), "a"), target(2, "/m/b.mp3", Some(100), "b")];
        assert_eq!(matched(&targets, &[track("/m/x.mp3", 100, "b")]), [(0, 2)]);
        // Lệch một trong ba thì không phải nó.
        assert!(matched(&targets, &[track("/m/x.mp3", 101, "b")]).is_empty());
        assert!(matched(&targets, &[track("/m/x.mp3", 100, "c")]).is_empty());
        let longer = NewTrack { duration_sec: 11.0, ..track("/m/x.mp3", 100, "b") };
        assert!(matched(&targets, &[longer]).is_empty());
    }

    #[test]
    fn identical_copies_only_pair_when_the_filename_settles_it() {
        let targets = [target(1, "/old/a.mp3", Some(100), ""), target(2, "/old/b.mp3", Some(100), "")];
        // Đổi tên thư mục: cùng tên tệp phân biệt được.
        let folder = [track("/new/b.mp3", 100, ""), track("/new/a.mp3", 100, "")];
        assert_eq!(matched(&targets, &folder), [(0, 2), (1, 1)]);
        // Đổi cả tên: không biết bài nào là bài nào, không ghép.
        let renamed = [track("/new/x.mp3", 100, ""), track("/new/y.mp3", 100, "")];
        assert!(matched(&targets, &renamed).is_empty());
        // Một tệp mới cho hai dòng cũng không đoán.
        assert!(matched(&targets, &[track("/new/x.mp3", 100, "")]).is_empty());
        // Một dòng cho hai tệp cũng vậy.
        let one = [target(1, "/old/a.mp3", Some(100), "")];
        assert!(matched(&one, &renamed).is_empty());
    }

    #[test]
    fn a_hint_wins_even_when_the_tags_changed() {
        let targets = [target(1, "/m/a.mp3", Some(100), "a")];
        let tracks = [track("/m/b.mp3", 999, "khác hẳn")];
        let hints = HashMap::from([(file_key(Path::new("/m/b.mp3")), 1)]);
        assert_eq!(match_renames(&targets, &tracks, &hints), HashMap::from([(0, 1)]));
        // Gợi ý trỏ dòng không phải đích (vd dòng còn tệp) thì bỏ.
        let hints = HashMap::from([(file_key(Path::new("/m/b.mp3")), 9)]);
        assert!(match_renames(&targets, &tracks, &hints).is_empty());
    }

    #[test]
    fn legacy_rows_need_the_same_filename_and_a_title() {
        let targets = [target(1, "/old/a.mp3", None, "bài")];
        assert_eq!(matched(&targets, &[track("/new/a.mp3", 5, "bài")]), [(0, 1)]);
        assert!(matched(&targets, &[track("/new/b.mp3", 5, "bài")]).is_empty());
        let untitled = [target(1, "/old/a.mp3", None, "")];
        assert!(matched(&untitled, &[track("/new/a.mp3", 5, "")]).is_empty());
    }

    #[test]
    fn only_possible_matches_are_read_ahead() {
        let file =
            |path: &str, size| ScannedFile { path: path.into(), stamp: crate::tags::FileStamp { size, mtime_ms: 1 } };
        let targets = [target(1, "/old/a.mp3", Some(100), ""), target(2, "/old/legacy.mp3", None, "t")];
        let hints = HashMap::from([(file_key(Path::new("/n/hinted.mp3")), 1)]);
        let fresh =
            vec![file("/n/same.mp3", 100), file("/n/other.mp3", 7), file("/n/legacy.mp3", 7), file("/n/hinted.mp3", 7)];
        let (candidates, rest) = split_rename_candidates(fresh, &targets, &hints);
        let as_paths = |list: &[&str]| list.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(candidates, as_paths(&["/n/same.mp3", "/n/legacy.mp3", "/n/hinted.mp3"]));
        assert_eq!(rest, as_paths(&["/n/other.mp3"]));
    }
}
