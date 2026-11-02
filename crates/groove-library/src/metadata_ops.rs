//! groove-gd1_18: ghi thẻ an toàn (N2, N3, S5, S7, S8).
//!
//! Mọi việc chạy lần lượt trên một luồng riêng, gửi đi không chặn nên gọi được từ luồng UI (N7). Thứ tự một lần lưu:
//! kiểm tên và đích → ghi thẻ qua bản tạm (`tags::write_tags`) → đọc lại → đổi tên không đè → transaction DB. Lỗi trước
//! bước ghi thẻ thì chưa có gì đổi; lỗi ở DB sau khi đổi tên thì đổi tên lại, DB vẫn trỏ đúng tệp.
//!
//! Lưu xong phát `Progress { kind: MetadataBulk, done == total }`, không phát `Synced`: chính app sửa thì không có toast
//! "N bài cập nhật" (S5). Dấu tệp ghi cùng transaction nên watcher so ra bằng và bỏ qua (S6).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fs, io};

use groove_core::model::{CategoryRef, Song, TrackRef};
use groove_core::paths;
use rusqlite::{Connection, params};

use crate::actor::{LibraryError, LibraryEvent, LibraryResult, MovedSong, Progress, ProgressKind, Reply, Worker};
use crate::covers::{CoverStore, cover_hash};
use crate::db::migrate::CoverRecord;
use crate::db::queries::{self, QueryError, SongText};
use crate::indexer::{TrackRow, apply_scanned_track};
use crate::store::Library;
use crate::tags::{self, CoverArt, CoverEdit, FileStamp, MetadataEdit, RawFrame, TagError, TagFields, WriteMode};

/// Trần tên tệp của hệ tệp là 255 byte, chừa chỗ cho đuôi bản tạm để lần ghi sau không hỏng.
const MAX_NAME_BYTES: usize = 255 - tags::TEMP_SUFFIX.len();
/// Cấm ở mọi OS: thư viện có thể nằm trên ổ dùng chung với Windows. `:` còn là luồng phụ NTFS.
const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
const RESERVED_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Vì sao tên tệp mới không dùng được. UI tra theo biến thể qua i18n.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileNameError {
    #[error("file name is empty")]
    Empty,
    /// Chứa `/`, `\` hoặc là `.`, `..`: sẽ đi ra ngoài thư mục của bài.
    #[error("file name must not contain a path")]
    NotSingleSegment,
    #[error("file name contains the forbidden character {0:?}")]
    ForbiddenChar(char),
    #[error("file name is reserved by Windows")]
    Reserved,
    /// Windows tự cắt dấu chấm cuối, tệp thật sẽ khác tên đã kiểm.
    #[error("file name must not end with a dot")]
    TrailingDot,
    #[error("file name is too long")]
    TooLong,
    /// UI giữ đuôi ở chip riêng. Đổi đuôi không đổi định dạng, còn làm bài rơi khỏi thư viện.
    #[error("file extension must stay the same")]
    ExtensionChanged,
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error(transparent)]
    InvalidName(#[from] FileNameError),
    /// Đích đã có tệp, hoặc DB đã có bài ở đó. Không đụng gì cả (N3).
    #[error("{} already exists", .0.display())]
    TargetExists(PathBuf),
    #[error("song {0} does not exist")]
    SongNotFound(i64),
    /// Thẻ chưa ghi: tệp gốc nguyên vẹn.
    #[error(transparent)]
    Tag(#[from] TagError),
    /// Thẻ đã ghi, DB đã theo; tệp vẫn ở tên cũ.
    #[error("tags were saved but {} could not be renamed: {source}", from.display())]
    RenameFailed { from: PathBuf, to: PathBuf, source: io::Error },
    /// Thẻ đã ghi nhưng DB chưa theo; tệp vẫn ở tên cũ. Dấu tệp khác DB nên watcher, reconcile sẽ đọc lại.
    #[error("tags were saved but the library was not updated: {0}")]
    DatabaseAfterWrite(LibraryError),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl MetadataError {
    /// Thẻ trong tệp đã đổi hay chưa, để báo người dùng đúng phần đã xong.
    pub fn file_written(&self) -> bool {
        matches!(self, MetadataError::RenameFailed { .. } | MetadataError::DatabaseAfterWrite(_))
    }

    fn library_closed(&self) -> bool {
        matches!(
            self,
            MetadataError::Library(LibraryError::Closed) | MetadataError::DatabaseAfterWrite(LibraryError::Closed)
        )
    }
}

pub type MetadataResult<T> = Result<T, MetadataError>;

/// Như `actor::Reply`: `await` trên UI, `wait` chỉ ở luồng nền. Bỏ đi không huỷ việc.
pub struct OpsReply<T>(Reply<MetadataResult<T>>);

impl<T> OpsReply<T> {
    pub fn wait(self) -> MetadataResult<T> {
        futures::executor::block_on(self)
    }
}

impl<T> Future for OpsReply<T> {
    type Output = MetadataResult<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx).map(|reply| reply.map_err(MetadataError::from).and_then(|result| result))
    }
}

/// Đọc từ tệp, không từ DB (đặc tả 1.8). Riêng lyrics, timeline tệp không có thì lấy DB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataView {
    pub track: TrackRef,
    pub path: PathBuf,
    pub filename: String,
    pub fields: TagFields,
    pub cover: Option<CoverArt>,
    pub duration_ms: u64,
    pub raw_frames: Vec<RawFrame>,
    pub stamp: FileStamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavedTrack {
    pub track: TrackRef,
    pub path: PathBuf,
    /// `Some` khi đã đổi tên.
    pub previous_path: Option<PathBuf>,
    /// Đọc lại từ tệp sau khi ghi, không tin phần sửa.
    pub fields: TagFields,
    pub stamp: FileStamp,
    /// `None` với tệp ngoài thư viện.
    pub song: Option<Song>,
}

/// Các trường được áp cho cả nhóm (đặc tả 1.8). Không có tiêu đề, số bài: kiểu đã chặn ghi đè giá trị riêng từng bài.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupEdit {
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub compilation: Option<bool>,
    pub cover: CoverEdit,
}

#[derive(Debug)]
pub struct BulkFailure {
    pub id: i64,
    pub path: PathBuf,
    /// `error.file_written()`: thẻ đã ghi, chỉ DB chưa theo.
    pub error: MetadataError,
}

/// Mỗi bài thuộc đúng một danh sách. Bài trong `done` đã ghi tệp và đã commit DB.
#[derive(Debug, Default)]
pub struct BulkReport {
    pub done: Vec<i64>,
    pub failed: Vec<BulkFailure>,
    /// Thư viện đóng giữa chừng: các bài này chưa bị đụng.
    pub not_started: Vec<i64>,
    /// Đã ghi các bài nhưng không đổi được bìa của chính nhóm.
    pub group_cover_failed: bool,
}

/// Bìa đã qua `CoverStore`. `record = None`: bảng `covers` đã có dòng và đủ tệp nên khỏi giải mã lại.
struct CoverSlot {
    hash: String,
    record: Option<CoverRecord>,
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|text| !text.is_empty())
}

/// Bài cuối rời album (ví dụ Fix grouping đổi `group_key`) thì album bị dọn: bìa người dùng chọn đi theo sang album mới.
fn carry_custom_cover(conn: &Connection, old: Option<i64>, new: Option<i64>) -> LibraryResult<()> {
    let (Some(old), Some(new)) = (old, new) else {
        return Ok(());
    };
    conn.prepare_cached(
        "UPDATE albums SET cover_is_custom = 1,
             cover_hash = (SELECT cover_hash FROM albums WHERE id = ?1)
         WHERE id = ?2 AND ?1 <> ?2 AND cover_is_custom = 0
             AND EXISTS (SELECT 1 FROM albums WHERE id = ?1 AND cover_is_custom = 1)
             AND NOT EXISTS (SELECT 1 FROM songs WHERE album_id = ?1)",
    )?
    .execute(params![old, new])?;
    Ok(())
}

/// Kiểm tên mới cho tệp đang tên `current`, trả tên đã cắt khoảng trắng. Không chạm đĩa.
pub fn validate_file_name(raw: &str, current: &str) -> Result<String, FileNameError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(FileNameError::Empty);
    }
    if name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(FileNameError::NotSingleSegment);
    }
    if let Some(bad) = name.chars().find(|c| FORBIDDEN_CHARS.contains(c) || c.is_control()) {
        return Err(FileNameError::ForbiddenChar(bad));
    }
    if name.ends_with('.') {
        return Err(FileNameError::TrailingDot);
    }
    let lower_ext = |text: &str| Path::new(text).extension().map(|ext| ext.to_string_lossy().to_lowercase());
    // `.mp3`: `Path` coi cả cụm là tên, không có đuôi. Thực ra là tên rỗng cộng đuôi (N3).
    if let (Some(ext), Some(rest)) = (lower_ext(current), name.strip_prefix('.'))
        && rest.to_lowercase() == ext
    {
        return Err(FileNameError::Empty);
    }
    let stem = Path::new(name).file_stem().map(|stem| stem.to_string_lossy()).unwrap_or_default();
    if lower_ext(name) != lower_ext(current) {
        return Err(FileNameError::ExtensionChanged);
    }
    let base = stem.split('.').next().unwrap_or_default().trim_end().to_uppercase();
    if RESERVED_STEMS.contains(&base.as_str()) {
        return Err(FileNameError::Reserved);
    }
    if name.len() > MAX_NAME_BYTES {
        return Err(FileNameError::TooLong);
    }
    Ok(name.to_owned())
}

fn is_same_entry_by_key(a: &Path, b: &Path) -> bool {
    paths::normalize_path(a) == paths::normalize_path(b)
}

/// Có đúng tên này trong thư mục không, phân biệt hoa thường. `exists` trên hệ tệp gập chữ sẽ thấy chính tệp đang đổi tên.
fn has_exact_entry(path: &Path) -> io::Result<bool> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Ok(false);
    };
    for entry in fs::read_dir(parent)? {
        if entry?.file_name() == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_taken(from: &Path, to: &Path) -> io::Result<bool> {
    if is_same_entry_by_key(from, to) {
        return has_exact_entry(to);
    }
    match fs::symlink_metadata(to) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// `fs::rename` đè tệp đích (N3), nên giữ chỗ bằng `create_new` trước: có tệp ở đó thì dừng, kể cả tệp vừa xuất hiện sau
/// lúc kiểm. Chỉ đổi hoa thường trên hệ tệp gập chữ thì không giữ chỗ được, vì đích chính là tệp nguồn.
fn rename_no_clobber(from: &Path, to: &Path) -> io::Result<()> {
    if is_same_entry_by_key(from, to) {
        if has_exact_entry(to)? {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        return fs::rename(from, to);
    }
    drop(fs::File::options().write(true).create_new(true).open(to)?);
    fs::rename(from, to).inspect_err(|_| {
        if let Err(e) = fs::remove_file(to) {
            log::warn!("cannot remove rename placeholder {}: {e}", to.display());
        }
    })
}

/// N2. `Clean` không có lyrics trong phần sửa thì đọc lại: DB trước, không có thì tệp. Tệp thiếu timeline mà DB có thì đẩy
/// ngược vào tệp ở mọi chế độ.
fn preserve_text(fields: &mut TagFields, mode: WriteMode, db: &SongText, file: Option<&TagFields>) {
    if mode == WriteMode::Clean && fields.lyrics.is_none() {
        let from_file = file.and_then(|file| file.lyrics.clone());
        fields.lyrics = non_empty(&db.lyrics).map(str::to_owned).or(from_file);
    }
    if fields.timeline.is_none()
        && let Some(timeline) = non_empty(&db.timeline)
        && file.is_some_and(|file| non_empty(&file.timeline).is_none())
    {
        fields.timeline = Some(timeline.to_owned());
    }
}

fn is_clearing(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|text| text.trim().is_empty())
}

type RenameFn = fn(&Path, &Path) -> io::Result<()>;

struct OpsState {
    library: Library,
    covers: CoverStore,
}

impl OpsState {
    /// `None` là tệp ngoài thư viện thật.
    fn resolve(&self, track: &TrackRef) -> MetadataResult<Option<Song>> {
        let song = self.library.song(track.clone()).wait()?;
        match (song, track) {
            (None, TrackRef::Library(id)) => Err(MetadataError::SongNotFound(*id)),
            (song, _) => Ok(song),
        }
    }

    fn read(&self, track: TrackRef) -> MetadataResult<MetadataView> {
        let song = self.resolve(&track)?;
        let (track, path) = match &song {
            Some(song) => (song.track.clone(), song.path.clone()),
            None => match track {
                TrackRef::External(path) => (TrackRef::External(path.clone()), path),
                TrackRef::Library(id) => return Err(MetadataError::SongNotFound(id)),
            },
        };
        let scanned = tags::read_track(&path)?;
        let raw_frames = tags::read_raw_frames(&path).unwrap_or_else(|e| {
            log::warn!("cannot read raw frames of {}: {e}", path.display());
            Vec::new()
        });
        let mut fields = scanned.fields;
        if let TrackRef::Library(id) = track {
            let text = self.library.song_text(id).wait()?.unwrap_or_default();
            if non_empty(&fields.lyrics).is_none() {
                fields.lyrics = text.lyrics.filter(|value| !value.trim().is_empty());
            }
            if non_empty(&fields.timeline).is_none() {
                fields.timeline = text.timeline.filter(|value| !value.trim().is_empty());
            }
        }
        let filename = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(MetadataView {
            track,
            path,
            filename,
            fields,
            cover: scanned.cover,
            duration_ms: scanned.duration_ms,
            raw_frames,
            stamp: scanned.stamp,
        })
    }

    fn save(
        &self,
        track: TrackRef,
        mut edit: MetadataEdit,
        mode: WriteMode,
        new_filename: Option<String>,
        rename_file: RenameFn,
    ) -> MetadataResult<SavedTrack> {
        let song = self.resolve(&track)?;
        let path = match (&song, track) {
            (Some(song), _) => song.path.clone(),
            (None, TrackRef::External(path)) => path,
            (None, TrackRef::Library(id)) => return Err(MetadataError::SongNotFound(id)),
        };
        let target = match new_filename {
            Some(name) => self.check_rename(song.as_ref(), &path, &name)?,
            None => None,
        };
        let saved = self.write_and_commit(song.as_ref(), path, &mut edit, mode, target, rename_file)?;
        if let Some(song) = &saved.song {
            if let (TrackRef::Library(id), Some(from)) = (&song.track, &saved.previous_path) {
                let moved = MovedSong { id: *id, from: from.clone(), to: saved.path.clone() };
                self.library.writer().emit(LibraryEvent::Moved(vec![moved]));
            }
            self.emit_progress(1, 1);
        }
        Ok(saved)
    }

    /// `Some(đích)` khi thật sự phải đổi tên. Lỗi ở đây chưa đụng gì cả.
    fn check_rename(&self, song: Option<&Song>, path: &Path, raw: &str) -> MetadataResult<Option<PathBuf>> {
        let current = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        // Tên giữ nguyên thì khỏi kiểm: tệp có sẵn trên Linux được phép có ký tự mà luật mới cấm.
        if raw.trim() == current {
            return Ok(None);
        }
        let name = validate_file_name(raw, &current)?;
        if name == current {
            return Ok(None);
        }
        let target = path.with_file_name(name);
        let taken = is_taken(path, &target)
            .map_err(|source| MetadataError::Tag(TagError::Io { path: target.clone(), source }))?;
        if taken {
            return Err(MetadataError::TargetExists(target));
        }
        let occupant = self.library.track_for_path(target.clone()).wait()?;
        let own = song.map(|song| &song.track);
        if matches!(occupant, TrackRef::Library(_)) && Some(&occupant) != own {
            return Err(MetadataError::TargetExists(target));
        }
        Ok(Some(target))
    }

    fn write_and_commit(
        &self,
        song: Option<&Song>,
        path: PathBuf,
        edit: &mut MetadataEdit,
        mode: WriteMode,
        target: Option<PathBuf>,
        rename_file: RenameFn,
    ) -> MetadataResult<SavedTrack> {
        let id = song.and_then(|song| match song.track {
            TrackRef::Library(id) => Some(id),
            TrackRef::External(_) => None,
        });
        let db_text = match id {
            Some(id) => self.library.song_text(id).wait()?.unwrap_or_default(),
            None => SongText::default(),
        };
        let fields = &edit.fields;
        let needs_file = (mode == WriteMode::Clean && fields.lyrics.is_none())
            || (fields.timeline.is_none() && non_empty(&db_text.timeline).is_some());
        let file_fields = if needs_file { Some(tags::read_track(&path)?.fields) } else { None };
        preserve_text(&mut edit.fields, mode, &db_text, file_fields.as_ref());
        let clear = (is_clearing(&edit.fields.lyrics), is_clearing(&edit.fields.timeline));

        tags::write_tags(&path, edit, mode)?;
        // Từ đây thẻ đã nằm trong tệp. Đọc lại trước khi đổi tên: đọc lỗi thì tệp vẫn ở chỗ DB biết.
        let scanned = tags::read_track(&path).map_err(|e| {
            log::error!("tags written but read-back failed for {}: {e}", path.display());
            MetadataError::DatabaseAfterWrite(LibraryError::Other(e.into()))
        })?;
        let cover = self.cover_slot(scanned.cover.as_ref()).map_err(MetadataError::DatabaseAfterWrite)?;
        let fields = scanned.fields.clone();
        let read_stamp = scanned.stamp;
        let (hash, record) = cover.map_or((None, None), |slot| (Some(slot.hash), slot.record));
        let mut row = TrackRow::from_scanned(path.clone(), scanned, hash);

        let rename_error = match &target {
            Some(target) => rename_file(&path, target).err().map(|source| (target.clone(), source)),
            None => None,
        };
        let final_path = match (&target, &rename_error) {
            (Some(target), None) => target.clone(),
            _ => path.clone(),
        };
        // Đổi tên không đổi thời gian sửa, nhưng hệ tệp mạng có thể làm tròn.
        let stamp = FileStamp::of(&final_path).unwrap_or(read_stamp);
        (row.mtime_ms, row.size) = (stamp.mtime_ms, stamp.size);
        row.path = final_path.clone();

        let committed = match id {
            Some(id) => {
                let old_album = song.and_then(|song| song.album_id);
                Some(self.commit(id, old_album, row, record, clear))
            }
            None => None,
        };
        if let Some((to, source)) = rename_error {
            if let Some(Err(e)) = committed {
                log::error!("library not updated after failed rename of {}: {e}", path.display());
            }
            return Err(MetadataError::RenameFailed { from: path, to, source });
        }
        let song_after = match committed {
            Some(Ok(song)) => Some(song),
            Some(Err(e)) => {
                if target.is_some()
                    && let Err(back) = rename_no_clobber(&final_path, &path)
                {
                    // Tệp còn nguyên ở tên mới; watcher ghép đổi tên giữ `id` (N4).
                    log::error!("cannot rename {} back: {back}", final_path.display());
                }
                return Err(MetadataError::DatabaseAfterWrite(e));
            }
            None => None,
        };
        Ok(SavedTrack {
            track: song_after
                .as_ref()
                .map_or_else(|| TrackRef::External(final_path.clone()), |song| song.track.clone()),
            previous_path: target.map(|_| path),
            path: final_path,
            fields,
            stamp,
            song: song_after,
        })
    }

    fn cover_slot(&self, cover: Option<&CoverArt>) -> LibraryResult<Option<CoverSlot>> {
        let Some(art) = cover else {
            return Ok(None);
        };
        let hash = cover_hash(&art.data);
        let wanted = hash.clone();
        let known = self
            .library
            .read(move |conn| {
                let mut stmt = conn.prepare_cached("SELECT 1 FROM covers WHERE hash = ?1")?;
                Ok(stmt.exists([wanted])?)
            })
            .wait()?;
        if known && self.covers.has_files(&hash) {
            return Ok(Some(CoverSlot { hash, record: None }));
        }
        match self.covers.save(&art.data) {
            Ok(record) => Ok(Some(CoverSlot { hash: record.hash.clone(), record: Some(record) })),
            Err(e) => {
                // Mất bìa, bài vẫn đúng.
                log::warn!("cannot store cover: {e}");
                Ok(None)
            }
        }
    }

    /// Một bài một transaction: lỗi của bài này không kéo bài khác, báo cáo lô đúng từng bài.
    fn commit(
        &self,
        id: i64,
        old_album: Option<i64>,
        row: TrackRow,
        cover: Option<CoverRecord>,
        clear: (bool, bool),
    ) -> LibraryResult<Song> {
        self.library
            .writer()
            .transaction(move |conn| {
                queries::move_song(conn, id, &row.path, None)?;
                if let Some(record) = &cover {
                    queries::upsert_cover(conn, record)?;
                }
                apply_scanned_track(conn, &row)?;
                // `upsert_song` giữ DB khi tệp rỗng, nên xoá thật phải nói rõ.
                if clear.0 {
                    queries::set_song_lyrics(conn, id, None)?;
                }
                if clear.1 {
                    queries::set_song_timeline(conn, id, None)?;
                }
                let song = queries::song_by_id(conn, id)?.ok_or(QueryError::SongNotFound(id))?;
                carry_custom_cover(conn, old_album, song.album_id)?;
                queries::delete_empty_albums(conn)?;
                Ok(song)
            })
            .wait()
    }

    fn emit_progress(&self, done: usize, total: usize) {
        let progress = Progress { kind: ProgressKind::MetadataBulk, done, total };
        self.library.writer().emit(LibraryEvent::Progress(progress));
    }

    fn save_bulk(&self, category: CategoryRef, group: GroupEdit, mode: WriteMode) -> MetadataResult<BulkReport> {
        // Đặc tả 1.8: bài khác không bao giờ `Clean`.
        let mode = if mode == WriteMode::Clean { WriteMode::CleanKeepKnown } else { mode };
        let songs = self.library.songs(category).wait()?;
        let total = songs.len();
        let fields = TagFields {
            album: group.album,
            album_artist: group.album_artist,
            genre: group.genre,
            year: group.year,
            compilation: group.compilation,
            ..TagFields::default()
        };
        // Một bản `edit` dùng lại, không chép byte bìa cho từng bài.
        let mut edit = MetadataEdit { fields, cover: group.cover };
        let mut report = BulkReport::default();
        let mut songs = songs.into_iter();
        for (index, song) in songs.by_ref().enumerate() {
            let TrackRef::Library(id) = song.track else {
                continue;
            };
            edit.fields.timeline = None;
            let path = song.path.clone();
            let outcome = self.write_and_commit(Some(&song), path.clone(), &mut edit, mode, None, rename_no_clobber);
            let closed = outcome.as_ref().err().is_some_and(MetadataError::library_closed);
            match outcome {
                Ok(_) => report.done.push(id),
                Err(error) => report.failed.push(BulkFailure { id, path, error }),
            }
            if closed {
                break;
            }
            self.emit_progress(index + 1, total);
        }
        report.not_started = songs
            .filter_map(|song| match song.track {
                TrackRef::Library(id) => Some(id),
                TrackRef::External(_) => None,
            })
            .collect();
        if !report.done.is_empty()
            && let Err(e) = self.apply_group_cover(category, &edit.cover)
        {
            log::error!("cannot update group cover: {e}");
            report.group_cover_failed = true;
        }
        Ok(report)
    }

    /// S8: bìa nhóm chỉ đổi ở đây, khi người dùng chọn áp cho nhóm. Đó là lựa chọn của họ nên đánh dấu `custom`.
    fn apply_group_cover(&self, category: CategoryRef, cover: &CoverEdit) -> LibraryResult<()> {
        let slot = match cover {
            CoverEdit::Keep => return Ok(()),
            CoverEdit::Remove => None,
            CoverEdit::Set(data) => {
                let art = CoverArt { mime: String::new(), data: data.clone() };
                match self.cover_slot(Some(&art))? {
                    Some(slot) => Some(slot),
                    None => return Err(LibraryError::Other(anyhow::anyhow!("cover could not be stored"))),
                }
            }
        };
        self.library
            .writer()
            .transaction(move |conn| {
                if let Some(record) = slot.as_ref().and_then(|slot| slot.record.as_ref()) {
                    queries::upsert_cover(conn, record)?;
                }
                let hash = slot.as_ref().map(|slot| slot.hash.as_str());
                match category {
                    CategoryRef::Album(id) => queries::set_album_cover(conn, id, hash, hash.is_some())?,
                    CategoryRef::Folder(id) => queries::set_folder_cover(conn, id, hash)?,
                    // Playlist không có bìa riêng trong schema.
                    CategoryRef::Playlist(_) => {}
                }
                Ok(())
            })
            .wait()
    }
}

/// `MetadataOps::{read, save, save_lyrics, save_timeline, save_bulk}` (đặc tả 2.4). Sao chép rẻ. Giữ một luồng nền đang ngủ
/// tới khi thả hết bản sao. Các việc chạy tuần tự, nên một lô dài làm lần lưu sau phải chờ.
#[derive(Clone)]
pub struct MetadataOps {
    worker: Worker<OpsState>,
}

impl MetadataOps {
    pub fn new(library: Library, covers: CoverStore) -> io::Result<MetadataOps> {
        let worker = Worker::spawn("groove-metadata-ops", OpsState { library, covers })?;
        Ok(MetadataOps { worker })
    }

    fn submit<T, F>(&self, job: F) -> OpsReply<T>
    where
        T: Send + 'static,
        F: FnOnce(&OpsState) -> MetadataResult<T> + Send + 'static,
    {
        OpsReply(self.worker.submit(move |state: &mut OpsState| Ok(job(state))))
    }

    /// Mở dialog: đọc nền từ tệp.
    pub fn read(&self, track: TrackRef) -> OpsReply<MetadataView> {
        self.submit(move |state| state.read(track))
    }

    /// `new_filename` là tên đủ kèm đuôi, giữ nguyên đuôi cũ; `None` là không đổi tên. Lỗi nào cũng để tệp và DB khớp nhau,
    /// xem `MetadataError`. Chỉ gieo bìa cho nhóm chưa có bìa; đổi bìa nhóm thì dùng `save_bulk`.
    pub fn save(
        &self,
        track: TrackRef,
        edit: MetadataEdit,
        mode: WriteMode,
        new_filename: Option<String>,
    ) -> OpsReply<SavedTrack> {
        self.submit(move |state| state.save(track, edit, mode, new_filename, rename_no_clobber))
    }

    /// Chuỗi rỗng là xoá. Ghi `Merge`, không đụng trường khác (S5).
    pub fn save_lyrics(&self, track: TrackRef, lyrics: String) -> OpsReply<SavedTrack> {
        let fields = TagFields { lyrics: Some(lyrics), ..TagFields::default() };
        self.save(track, MetadataEdit { fields, ..MetadataEdit::default() }, WriteMode::Merge, None)
    }

    /// Lưu văn bản nguyên gốc. Chuỗi rỗng là xoá.
    pub fn save_timeline(&self, track: TrackRef, timeline: String) -> OpsReply<SavedTrack> {
        let fields = TagFields { timeline: Some(timeline), ..TagFields::default() };
        self.save(track, MetadataEdit { fields, ..MetadataEdit::default() }, WriteMode::Merge, None)
    }

    /// Lỗi một bài không dừng cả lô; đọc `BulkReport` để biết từng bài. `WriteMode::Clean` được hạ thành `CleanKeepKnown`.
    /// Tiến độ qua `LibraryEvent::Progress`; `Err` chỉ khi chưa bắt đầu được bài nào.
    pub fn save_bulk(&self, category: CategoryRef, edit: GroupEdit, mode: WriteMode) -> OpsReply<BulkReport> {
        self.submit(move |state| state.save_bulk(category, edit, mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{AudioKind, write_sample};
    use crate::store::LibraryConfig;

    #[test]
    fn rejects_every_name_that_could_leave_the_folder_or_break_the_file() {
        let bad = |raw: &str| validate_file_name(raw, "a.mp3").unwrap_err();
        assert_eq!(bad(""), FileNameError::Empty);
        assert_eq!(bad("   "), FileNameError::Empty);
        assert_eq!(bad(".mp3"), FileNameError::Empty);
        assert_eq!(bad(".."), FileNameError::NotSingleSegment);
        assert_eq!(bad("."), FileNameError::NotSingleSegment);
        assert_eq!(bad("../b.mp3"), FileNameError::NotSingleSegment);
        assert_eq!(bad("..\\b.mp3"), FileNameError::NotSingleSegment);
        assert_eq!(bad("/etc/b.mp3"), FileNameError::NotSingleSegment);
        assert_eq!(bad("C:\\b.mp3"), FileNameError::NotSingleSegment);
        assert_eq!(bad("C:b.mp3"), FileNameError::ForbiddenChar(':'));
        assert_eq!(bad("a.mp3:evil"), FileNameError::ForbiddenChar(':'));
        assert_eq!(bad("a\nb.mp3"), FileNameError::ForbiddenChar('\n'));
        assert_eq!(bad("b.mp3."), FileNameError::TrailingDot);
        assert_eq!(bad("CON.mp3"), FileNameError::Reserved);
        assert_eq!(bad("nul.tar.mp3"), FileNameError::Reserved);
        assert_eq!(bad("b.wav"), FileNameError::ExtensionChanged);
        assert_eq!(bad("b"), FileNameError::ExtensionChanged);
        assert_eq!(bad("b.mp3.groove-tmp"), FileNameError::ExtensionChanged);
        assert_eq!(bad(&format!("{}.mp3", "x".repeat(250))), FileNameError::TooLong);
    }

    #[test]
    fn accepts_ordinary_names_and_trims() {
        let good = |raw: &str| validate_file_name(raw, "a.mp3").unwrap();
        assert_eq!(good("  Đường về.mp3 "), "Đường về.mp3");
        assert_eq!(good("a..b.mp3"), "a..b.mp3");
        assert_eq!(good("B.MP3"), "B.MP3");
        assert_eq!(good("CONCERT.mp3"), "CONCERT.mp3");
    }

    #[test]
    fn rename_never_replaces_an_existing_file_and_cleans_its_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a.mp3"), dir.path().join("b.mp3"));
        fs::write(&from, b"a").unwrap();
        fs::write(&to, b"b").unwrap();
        assert_eq!(rename_no_clobber(&from, &to).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&to).unwrap(), b"b");
        assert_eq!(fs::read(&from).unwrap(), b"a");

        let missing = dir.path().join("missing.mp3");
        let free = dir.path().join("free.mp3");
        assert!(rename_no_clobber(&missing, &free).is_err());
        assert!(!free.exists());
    }

    fn fail_rename(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::ErrorKind::PermissionDenied.into())
    }

    /// Lần đổi tên hỏng sau khi thẻ đã ghi: giữ tên cũ, DB theo thẻ mới, dấu tệp khớp.
    #[test]
    fn failed_rename_after_write_leaves_file_and_database_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let config = LibraryConfig { target: dir.path().join("db").join("library.db"), legacy: None };
        let sink = Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("unused") });
        let library = Library::open(config, sink).unwrap();
        let path = write_sample(dir.path(), "a", AudioKind::Mp3).unwrap();
        let row = TrackRow::from_scanned(path.clone(), tags::read_track(&path).unwrap(), None);
        let id = library.writer().transaction(move |conn| Ok(apply_scanned_track(conn, &row)?.id)).wait().unwrap();
        let state = OpsState { library: library.clone(), covers: CoverStore::new(dir.path().join("covers")) };

        let fields = TagFields { title: Some("New".into()), ..TagFields::default() };
        let edit = MetadataEdit { fields, ..MetadataEdit::default() };
        let err =
            state.save(TrackRef::Library(id), edit, WriteMode::Merge, Some("b.mp3".into()), fail_rename).unwrap_err();
        assert!(matches!(err, MetadataError::RenameFailed { .. }));
        assert!(err.file_written());

        assert!(path.exists());
        assert!(!dir.path().join("b.mp3").exists());
        assert_eq!(tags::read_track(&path).unwrap().fields.title.as_deref(), Some("New"));
        let song = library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
        assert_eq!(song.path, path);
        assert_eq!(song.title.as_deref(), Some("New"));
        let stamp = FileStamp::of(&path).unwrap();
        let state_row = library.song_file_states().wait().unwrap().pop().unwrap();
        assert_eq!(state_row.mtime, Some(stamp.mtime_ms));
        assert_eq!(state_row.size, Some(stamp.size as i64));
    }
}
