//! groove-gd1_04: chép và chuyển DB cũ.
//!
//! Đặc tả 2.3. Mở tệp cũ chỉ đọc, chụp bản chép sang `<target>.migrating`, viết lại bản chép trong một transaction, xong
//! mới đổi tên thành `library.db`. Nên `library.db` hoặc chưa có, hoặc đã đủ; tệp cũ không bao giờ bị ghi.
//!
//! Đọc lại thẻ không làm ở đây: mọi bài ra khỏi migration với `tags_read_at IS NULL`, tác vụ nền của indexer làm tiếp.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use groove_core::paths;
use rusqlite::{Connection, OpenFlags, params};

use super::schema::{self, SCHEMA_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// DB do bản mới hơn tạo. Không mở, không ghi đè (đặc tả 2.3 bước 6).
    #[error("{} has schema version {found}, this build supports up to {SCHEMA_VERSION}", path.display())]
    TooNew { path: PathBuf, found: i32 },
    /// Tệp có bảng nhưng không có `user_version`: không phải thư viện của ta, không đụng vào.
    #[error("{} is not a Groove X library", .0.display())]
    NotLibrary(PathBuf),
    /// Tệp cũ đọc được nhưng thiếu bảng của bản Tauri.
    #[error("{} is not a legacy Groove X database", .0.display())]
    NotLegacy(PathBuf),
    /// Chuyển đổi hỏng, `library.db` chưa được tạo, tệp cũ nguyên vẹn. Gọi lại lần sau được, hoặc `open_fresh` để bỏ qua.
    #[error("could not migrate {}: {source}", path.display())]
    LegacyFailed {
        path: PathBuf,
        #[source]
        source: Box<MigrateError>,
    },
    #[error("migrated data failed the foreign key check on {0} rows")]
    Integrity(i64),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, MigrateError>;

/// Kết quả lưu một ảnh bìa, thành dòng của bảng `covers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRecord {
    pub hash: String,
    pub accent_color: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Nơi nhận byte ảnh đã giải base64: ghi tệp bìa, tính accent. Task covers hiện thực; hàm `FnMut` cũng dùng được.
/// Lỗi hoặc panic chỉ làm mất bìa của dòng đó, không dừng chuyển đổi: tác vụ đọc thẻ còn lấy lại được bìa từ tệp nhạc.
pub trait CoverSink {
    fn store(&mut self, image: &[u8]) -> anyhow::Result<CoverRecord>;
}

impl<F: FnMut(&[u8]) -> anyhow::Result<CoverRecord>> CoverSink for F {
    fn store(&mut self, image: &[u8]) -> anyhow::Result<CoverRecord> {
        self(image)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub folders: usize,
    pub albums: usize,
    pub songs: usize,
    pub playlists: usize,
    pub playlist_entries: usize,
    /// Dòng trùng `path_key` đã gộp vào dòng có `id` nhỏ nhất (M2).
    pub merged_songs: usize,
    /// Dòng trỏ tới cha không còn tồn tại, bỏ đi.
    pub orphan_rows: usize,
    pub covers_stored: usize,
    pub covers_failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenOutcome {
    Existing,
    Created,
    Migrated(MigrationReport),
}

/// Mở thư viện ở đường dẫn mặc định, lần đầu thì chuyển DB của bản Tauri nếu có.
pub fn open_default(sink: &mut dyn CoverSink) -> Result<(Connection, OpenOutcome)> {
    let target = paths::library_db()?;
    let legacy = paths::legacy_db()?;
    open_or_migrate(&target, Some(&legacy), sink)
}

/// `library.db` có rồi thì mở; chưa có mà có DB cũ thì chuyển; không có gì thì tạo mới.
pub fn open_or_migrate(
    target: &Path,
    legacy: Option<&Path>,
    sink: &mut dyn CoverSink,
) -> Result<(Connection, OpenOutcome)> {
    if target.exists() {
        return open_existing(target);
    }
    if let Some(legacy) = legacy.filter(|path| path.exists()) {
        let report = migrate_legacy(legacy, target, sink)
            .map_err(|source| MigrateError::LegacyFailed { path: legacy.to_owned(), source: Box::new(source) })?;
        let conn = Connection::open(target)?;
        schema::apply_pragmas(&conn)?;
        return Ok((conn, OpenOutcome::Migrated(report)));
    }
    open_fresh(target).map(|conn| (conn, OpenOutcome::Created))
}

/// Tạo thư viện trống, không nhìn DB cũ. Đã có `library.db` thì mở tệp đó, vẫn qua kiểm phiên bản.
pub fn open_fresh(target: &Path) -> Result<Connection> {
    if target.exists() {
        return open_existing(target).map(|(conn, _)| conn);
    }
    ensure_parent(target)?;
    let mut conn = Connection::open(target)?;
    let tx = conn.transaction()?;
    schema::create_schema(&tx)?;
    tx.commit()?;
    schema::apply_pragmas(&conn)?;
    Ok(conn)
}

fn open_existing(target: &Path) -> Result<(Connection, OpenOutcome)> {
    let mut conn = Connection::open(target)?;
    let found = schema::user_version(&conn)?;
    if found > SCHEMA_VERSION {
        return Err(MigrateError::TooNew { path: target.to_owned(), found });
    }
    let mut outcome = OpenOutcome::Existing;
    if found == 0 {
        let tables: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))?;
        if tables > 0 {
            return Err(MigrateError::NotLibrary(target.to_owned()));
        }
        // Tệp rỗng, ví dụ kết nối trước chết ngay sau khi tạo tệp.
        let tx = conn.transaction()?;
        schema::create_schema(&tx)?;
        tx.commit()?;
        outcome = OpenOutcome::Created;
    } else if found < SCHEMA_VERSION {
        let tx = conn.transaction()?;
        schema::upgrade_schema(&tx, found)?;
        tx.commit()?;
        log::info!("library schema upgraded from {found} to {SCHEMA_VERSION}");
    }
    schema::apply_pragmas(&conn)?;
    Ok((conn, outcome))
}

/// Chuyển `legacy` thành `target`. `target` phải chưa tồn tại. Lỗi thì không để lại gì, tệp cũ không đổi.
pub fn migrate_legacy(legacy: &Path, target: &Path, sink: &mut dyn CoverSink) -> Result<MigrationReport> {
    ensure_parent(target)?;
    let mut work = target.as_os_str().to_owned();
    work.push(".migrating");
    let work = PathBuf::from(work);
    remove_work_files(&work);

    let result = snapshot(legacy, &work).and_then(|()| {
        let mut conn = Connection::open(&work)?;
        let report = rewrite(&mut conn, sink)?;
        // Windows không đổi tên được tệp còn đang mở.
        conn.close().map_err(|(_, err)| err)?;
        fs::rename(&work, target)?;
        Ok(report)
    });
    remove_work_files(&work);
    if let Ok(report) = &result {
        log::info!("legacy library migrated: {report:?}");
    }
    result
}

fn ensure_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent),
        None => Ok(()),
    }
}

fn remove_work_files(work: &Path) {
    let _ = fs::remove_file(work);
    let mut journal = work.as_os_str().to_owned();
    journal.push("-journal");
    let _ = fs::remove_file(PathBuf::from(journal));
}

/// `VACUUM INTO` thay vì chép tệp: nhất quán kể cả khi bản Tauri đang mở DB, không ghi gì vào tệp nguồn.
fn snapshot(legacy: &Path, work: &Path) -> Result<()> {
    let source = Connection::open_with_flags(legacy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.pragma_update(None, "busy_timeout", 5000)?;
    let found = schema::user_version(&source)?;
    if found != 0 {
        return Err(MigrateError::TooNew { path: legacy.to_owned(), found });
    }
    if !has_table(&source, "songs")? || !has_table(&source, "folders")? {
        return Err(MigrateError::NotLegacy(legacy.to_owned()));
    }
    let work =
        work.to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "data dir path is not valid UTF-8"))?;
    source.execute("VACUUM INTO ?1", [work])?;
    Ok(())
}

fn has_table(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |row| {
        row.get(0)
    })
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2", [table, column], |row| row.get(0))
}

fn rewrite(conn: &mut Connection, sink: &mut dyn CoverSink) -> Result<MigrationReport> {
    // Hai pragma này không có tác dụng trong transaction. `legacy_alter_table` để đổi tên bảng không sửa khoá ngoại.
    conn.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = ON;")?;
    let tx = conn.transaction()?;

    fill_old_shape(&tx)?;
    for table in LEGACY_TABLES {
        tx.execute_batch(&format!("ALTER TABLE {table} RENAME TO legacy_{table};"))?;
    }
    schema::create_schema(&tx)?;

    let mut report = MigrationReport::default();
    let mut covers = CoverImporter::new(sink);
    copy_folders(&tx, &mut covers, &mut report)?;
    copy_albums(&tx, &mut covers, &mut report)?;
    let kept_songs = copy_songs(&tx, &mut report)?;
    copy_playlists(&tx, &kept_songs, now_ms(), &mut report)?;
    tx.execute_batch("INSERT INTO watched_roots (id, path) SELECT id, path FROM legacy_watched_roots;")?;
    report.covers_stored = covers.stored;
    report.covers_failed = covers.failed;

    for table in LEGACY_TABLES {
        tx.execute_batch(&format!("DROP TABLE legacy_{table};"))?;
    }
    let broken: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))?;
    if broken > 0 {
        return Err(MigrateError::Integrity(broken));
    }
    tx.commit()?;

    // Base64 của bìa đã đi hết; nhả trang trống để `library.db` không mang cỡ của DB cũ (S10).
    conn.execute_batch("VACUUM;")?;
    Ok(report)
}

const LEGACY_TABLES: [&str; 6] = ["playlist_songs", "playlists", "watched_roots", "songs", "albums", "folders"];

/// DB của bản Tauri cũ hơn thiếu cột, bảng thêm dần ở `db.rs:164-228` mà nó chưa từng chạy tới. Chỉ sửa bản chép.
fn fill_old_shape(tx: &Connection) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS albums (
             id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL,
             artist TEXT, thumbnail TEXT, accent_color TEXT);
         CREATE TABLE IF NOT EXISTS watched_roots (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL);
         CREATE TABLE IF NOT EXISTS playlists (
             id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, color TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS playlist_songs (
             playlist_id INTEGER NOT NULL, song_id INTEGER NOT NULL, PRIMARY KEY(playlist_id, song_id));",
    )?;
    for (column, kind) in [("lyrics", "TEXT"), ("timeline", "TEXT"), ("file_mtime", "INTEGER")] {
        if !has_column(tx, "songs", column)? {
            tx.execute_batch(&format!("ALTER TABLE songs ADD COLUMN {column} {kind};"))?;
        }
    }
    Ok(())
}

fn copy_folders(tx: &Connection, covers: &mut CoverImporter, report: &mut MigrationReport) -> rusqlite::Result<()> {
    let mut rows = tx.prepare("SELECT id, name, path, thumbnail FROM legacy_folders ORDER BY id")?;
    let mut insert = tx.prepare("INSERT INTO folders (id, name, path, cover_hash) VALUES (?1, ?2, ?3, ?4)")?;
    let mut rows = rows.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let path: String = row.get(2)?;
        let cover = covers.import(tx, row.get::<_, Option<String>>(3)?)?;
        insert.execute(params![id, name, path, cover])?;
        report.folders += 1;
    }
    Ok(())
}

fn copy_albums(tx: &Connection, covers: &mut CoverImporter, report: &mut MigrationReport) -> rusqlite::Result<()> {
    // `name` cũ UNIQUE nên `(name, group_key)` không thể trùng, kể cả album chung `folder:0`.
    let mut rows = tx.prepare(
        "SELECT a.id, a.name, a.artist, a.thumbnail,
                (SELECT MIN(s.folder_id) FROM legacy_songs s WHERE s.album_id = a.id)
         FROM legacy_albums a ORDER BY a.id",
    )?;
    let mut insert =
        tx.prepare("INSERT INTO albums (id, name, artist, group_key, cover_hash) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    let mut rows = rows.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let artist: Option<String> = row.get(2)?;
        let cover = covers.import(tx, row.get::<_, Option<String>>(3)?)?;
        let folder: Option<i64> = row.get(4)?;
        // Tạm, tác vụ đọc thẻ gán lại khi biết `album_artist` (đặc tả 2.3 bước 5).
        let group_key = schema::album_group_key(None, folder.unwrap_or(schema::NO_FOLDER_ID));
        insert.execute(params![id, name, artist, group_key, cover])?;
        report.albums += 1;
    }
    Ok(())
}

/// Trả ánh xạ `id` cũ sang `id` còn lại, để playlist theo được dòng đã gộp.
fn copy_songs(tx: &Connection, report: &mut MigrationReport) -> rusqlite::Result<HashMap<i64, i64>> {
    let mut rows = tx.prepare(
        "SELECT s.id, s.title, s.artist, a.id, a.name, f.id, s.file_path, s.filename, s.duration,
                s.lyrics, s.timeline, s.file_mtime
         FROM legacy_songs s
         LEFT JOIN legacy_albums a ON a.id = s.album_id
         LEFT JOIN legacy_folders f ON f.id = s.folder_id
         ORDER BY s.id",
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO songs (id, title, artist, album_id, folder_id, file_path, path_key, filename, duration,
                            lyrics, timeline, file_mtime, search_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;
    let mut merge = tx.prepare(
        "UPDATE songs SET lyrics = COALESCE(lyrics, ?2), timeline = COALESCE(timeline, ?3), file_mtime = COALESCE(file_mtime, ?4)
         WHERE id = ?1",
    )?;

    let mut by_key: HashMap<String, i64> = HashMap::new();
    let mut kept: HashMap<i64, i64> = HashMap::new();
    let mut rows = rows.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let Some(folder_id) = row.get::<_, Option<i64>>(5)? else {
            report.orphan_rows += 1;
            continue;
        };
        let file_path: String = row.get(6)?;
        let lyrics: Option<String> = row.get(9)?;
        let timeline: Option<String> = row.get(10)?;
        let mtime: Option<i64> = row.get(11)?;
        let key = schema::path_key(&file_path);
        if let Some(&winner) = by_key.get(&key) {
            merge.execute(params![winner, lyrics, timeline, mtime])?;
            kept.insert(id, winner);
            report.merged_songs += 1;
            continue;
        }
        let title: Option<String> = row.get(1)?;
        let artist: Option<String> = row.get(2)?;
        let album_id: Option<i64> = row.get(3)?;
        let album: Option<String> = row.get(4)?;
        let filename: String = row.get(7)?;
        let duration: i64 = row.get(8)?;
        let search = schema::search_key(title.as_deref(), artist.as_deref(), album.as_deref(), &filename);
        insert.execute(params![
            id, title, artist, album_id, folder_id, file_path, key, filename, duration, lyrics, timeline, mtime, search
        ])?;
        by_key.insert(key, id);
        kept.insert(id, id);
        report.songs += 1;
    }
    Ok(kept)
}

fn copy_playlists(
    tx: &Connection,
    kept_songs: &HashMap<i64, i64>,
    added_at: i64,
    report: &mut MigrationReport,
) -> rusqlite::Result<()> {
    report.playlists =
        tx.execute("INSERT INTO playlists (id, name, color) SELECT id, name, color FROM legacy_playlists", [])?;

    // Bản cũ hiện playlist theo tên tệp (`db.rs:536`), không lưu thứ tự: `position` giữ đúng thứ người dùng đang thấy.
    let mut rows = tx.prepare(
        "SELECT ps.playlist_id, ps.song_id, p.id
         FROM legacy_playlist_songs ps
         LEFT JOIN legacy_playlists p ON p.id = ps.playlist_id
         LEFT JOIN legacy_songs s ON s.id = ps.song_id
         ORDER BY ps.playlist_id, s.filename, ps.song_id",
    )?;
    let mut insert = tx.prepare(
        "INSERT OR IGNORE INTO playlist_songs (playlist_id, song_id, position, added_at) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut next_position: HashMap<i64, i64> = HashMap::new();
    let mut rows = rows.query([])?;
    while let Some(row) = rows.next()? {
        let playlist_id: i64 = row.get(0)?;
        let song_id: i64 = row.get(1)?;
        let playlist_exists = row.get::<_, Option<i64>>(2)?.is_some();
        let Some(&song) = kept_songs.get(&song_id).filter(|_| playlist_exists) else {
            report.orphan_rows += 1;
            continue;
        };
        let position = next_position.entry(playlist_id).or_insert(0);
        // Hai dòng trùng đã gộp có thể cùng nằm trong một playlist: dòng sau bị bỏ, `position` không nhảy.
        if insert.execute(params![playlist_id, song, *position, added_at])? > 0 {
            *position += 1;
            report.playlist_entries += 1;
        }
    }
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// Đưa `thumbnail` cũ qua `CoverSink`. Thư mục và album thường chung một ảnh nên nhớ theo chuỗi gốc.
struct CoverImporter<'a> {
    sink: &'a mut dyn CoverSink,
    seen: HashMap<String, Option<String>>,
    stored: usize,
    failed: usize,
}

impl<'a> CoverImporter<'a> {
    fn new(sink: &'a mut dyn CoverSink) -> Self {
        Self { sink, seen: HashMap::new(), stored: 0, failed: 0 }
    }

    fn import(&mut self, tx: &Connection, thumbnail: Option<String>) -> rusqlite::Result<Option<String>> {
        let Some(thumbnail) = thumbnail.filter(|t| !t.trim().is_empty()) else {
            return Ok(None);
        };
        if let Some(known) = self.seen.get(&thumbnail) {
            return Ok(known.clone());
        }
        let record = self.store(&thumbnail);
        if let Some(record) = &record {
            // `accent_color` cũ là hằng `#06b6d4`, không chép (đặc tả 2.3 bước 4).
            tx.execute(
                "INSERT OR IGNORE INTO covers (hash, accent_color, width, height) VALUES (?1, ?2, ?3, ?4)",
                params![record.hash, record.accent_color, record.width, record.height],
            )?;
        }
        let hash = record.map(|record| record.hash);
        self.seen.insert(thumbnail, hash.clone());
        Ok(hash)
    }

    fn store(&mut self, thumbnail: &str) -> Option<CoverRecord> {
        let stored = decode_thumbnail(thumbnail).and_then(|image| {
            let sink = &mut *self.sink;
            match catch_unwind(AssertUnwindSafe(|| sink.store(&image))) {
                Ok(Ok(record)) => Some(record),
                Ok(Err(err)) => {
                    log::warn!("legacy cover skipped: {err:#}");
                    None
                }
                Err(_) => {
                    log::warn!("legacy cover skipped: cover sink panicked");
                    None
                }
            }
        });
        match stored {
            Some(_) => self.stored += 1,
            None => self.failed += 1,
        }
        stored
    }
}

/// `data:<mime>;base64,<dữ liệu>` (`lib.rs:347`), hoặc base64 trần.
fn decode_thumbnail(thumbnail: &str) -> Option<Vec<u8>> {
    let thumbnail = thumbnail.trim();
    let encoded = match thumbnail.strip_prefix("data:") {
        Some(uri) => {
            let (header, data) = uri.split_once(',')?;
            if !header.ends_with(";base64") {
                return None;
            }
            data
        }
        None => thumbnail,
    };
    STANDARD.decode(encoded).ok().filter(|image| !image.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_data_uri_and_bare_base64() {
        assert_eq!(decode_thumbnail("data:image/png;base64,AQID").unwrap(), [1, 2, 3]);
        assert_eq!(decode_thumbnail(" AQID\n").unwrap(), [1, 2, 3]);
    }

    #[test]
    fn version_1_upgrades_and_keeps_every_row() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("library.db");
        {
            let conn = Connection::open(&path).unwrap();
            schema::create_schema_v1_for_tests(&conn).unwrap();
            conn.execute_batch(
                "INSERT INTO folders (id, name, path) VALUES (1, 'M', '/m');
                 INSERT INTO songs (id, folder_id, file_path, path_key, filename, duration)
                     VALUES (7, 1, '/m/a.mp3', '/m/a.mp3', 'a.mp3', 10);
                 INSERT INTO playlists (id, name, color) VALUES (1, 'P', '#fff');
                 INSERT INTO playlist_songs VALUES (1, 7, 0, 0);",
            )
            .unwrap();
        }
        let (conn, outcome) = open_existing(&path).unwrap();
        assert_eq!(outcome, OpenOutcome::Existing);
        assert_eq!(schema::user_version(&conn).unwrap(), SCHEMA_VERSION);
        let missing: Option<i64> = conn.query_row("SELECT missing_since FROM songs", [], |row| row.get(0)).unwrap();
        assert_eq!(missing, None);
        let members: i64 = conn.query_row("SELECT COUNT(*) FROM playlist_songs", [], |row| row.get(0)).unwrap();
        assert_eq!(members, 1);
        assert!(has_table(&conn, "excluded_paths").unwrap());
        drop(conn);
        // Mở lại không nâng lần hai.
        open_existing(&path).unwrap();
    }

    #[test]
    fn a_failed_upgrade_leaves_version_1_intact() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("library.db");
        {
            let conn = Connection::open(&path).unwrap();
            schema::create_schema_v1_for_tests(&conn).unwrap();
            // Bước sau của nâng cấp sẽ đụng bảng này.
            conn.execute_batch("CREATE TABLE excluded_paths (x);").unwrap();
        }
        assert!(open_existing(&path).is_err());
        let conn = Connection::open(&path).unwrap();
        assert_eq!(schema::user_version(&conn).unwrap(), 1);
        assert!(!has_column(&conn, "songs", "missing_since").unwrap());
    }

    #[test]
    fn upgraded_and_fresh_schemas_are_the_same() {
        let dump = |conn: &Connection| -> Vec<(String, String)> {
            let mut stmt = conn
                .prepare(
                    "SELECT name, COALESCE(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
                )
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(|row| row.unwrap()).collect()
        };
        let fresh = Connection::open_in_memory().unwrap();
        schema::create_schema(&fresh).unwrap();
        let upgraded = Connection::open_in_memory().unwrap();
        schema::create_schema_v1_for_tests(&upgraded).unwrap();
        schema::upgrade_schema(&upgraded, 1).unwrap();
        assert_eq!(dump(&fresh), dump(&upgraded));
    }

    #[test]
    fn rejects_what_is_not_base64_image_data() {
        assert!(decode_thumbnail("data:image/png,AQID").is_none());
        assert!(decode_thumbnail("data:image/png;base64").is_none());
        assert!(decode_thumbnail("data:image/png;base64,").is_none());
        assert!(decode_thumbnail("asset://localhost/a.png").is_none());
    }
}
