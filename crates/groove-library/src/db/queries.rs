//! groove-gd1_05: truy vấn.
//!
//! Hàm thuần trên `&Connection`, không biết luồng: đọc gọi từ `store`, ghi gọi từ `actor` trong transaction. Bài luôn tra
//! qua `path_key` (M2), `file_path` chỉ để mở tệp. Danh sách bài không kèm lyrics, timeline (S16): lấy riêng `song_text`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use groove_core::model::{CategoryRef, Song, TrackRef};
use groove_core::paths;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::migrate::CoverRecord;
use super::schema;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("playlist name is empty")]
    EmptyName,
    #[error("a playlist named {0:?} already exists")]
    DuplicatePlaylist(String),
    /// Đích đã là một bài khác, không ghi đè (N3).
    #[error("another song already uses {}", .0.display())]
    PathConflict(PathBuf),
    #[error("song {0} does not exist")]
    SongNotFound(i64),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, QueryError>;

/// Một thẻ của lưới. Chung một kiểu để `category_card` không phải rẽ nhánh theo loại; trường không áp cho loại đó là `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorySummary {
    pub reference: CategoryRef,
    pub name: String,
    /// Chỉ album.
    pub artist: Option<String>,
    /// Chỉ thư mục.
    pub path: Option<PathBuf>,
    /// Chỉ playlist.
    pub color: Option<String>,
    pub song_count: u32,
    pub cover_hash: Option<String>,
    pub accent_color: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Categories {
    pub albums: Vec<CategorySummary>,
    pub folders: Vec<CategorySummary>,
}

/// Chỉ nạp khi là bài đang phát hoặc đang sửa (S16).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SongText {
    pub lyrics: Option<String>,
    pub timeline: Option<String>,
}

/// Dấu vết tệp để reconcile so với đĩa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongFileState {
    pub id: i64,
    pub path: PathBuf,
    pub path_key: String,
    pub mtime: Option<i64>,
    pub size: Option<i64>,
}

/// Bài chưa đọc thẻ từ khi chuyển DB cũ (đặc tả 2.3 bước 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTagRead {
    pub id: i64,
    pub path: PathBuf,
}

/// Toàn bộ trường của một bài đọc được từ tệp. `filename`, `path_key`, `search_key` tự tính ở đây.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SongUpsert {
    pub path: PathBuf,
    pub folder_id: i64,
    pub album_id: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub duration_sec: f64,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub lyrics: Option<String>,
    pub timeline: Option<String>,
    pub cover_hash: Option<String>,
    pub file_mtime: Option<i64>,
    pub file_size: Option<i64>,
    pub tags_read_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpsertOutcome {
    pub id: i64,
    pub inserted: bool,
}

const SONG_COLUMNS: &str = "s.id, s.title, s.artist, a.name, s.album_artist, s.album_id, s.folder_id, s.file_path,
    s.filename, s.duration, s.track_no, s.disc_no, s.cover_hash
    FROM songs s LEFT JOIN albums a ON a.id = s.album_id";

fn song_from_row(row: &Row<'_>) -> rusqlite::Result<Song> {
    Ok(Song {
        track: TrackRef::Library(row.get(0)?),
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        album_artist: row.get(4)?,
        album_id: row.get(5)?,
        folder_id: row.get(6)?,
        path: PathBuf::from(row.get::<_, String>(7)?),
        filename: row.get(8)?,
        duration_sec: row.get(9)?,
        track_no: row.get(10)?,
        disc_no: row.get(11)?,
        cover_hash: row.get(12)?,
    })
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn filename_of(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn key_of(path: &Path) -> String {
    schema::path_key(&path.to_string_lossy())
}

/// Dấu thời gian của `added_at`, `tags_read_at`: ms từ epoch.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
}

// ---------------------------------------------------------------------------------------------------------------- đọc

/// Lưới theo tên không dấu (S19). Album không còn bài thì không hiện; thư mục rỗng vẫn hiện như bản cũ (`db.rs:324,350`).
pub fn categories(conn: &Connection) -> Result<Categories> {
    let mut albums = album_summaries(conn, None)?;
    let mut folders = folder_summaries(conn, None)?;
    sort_by_name(&mut albums);
    sort_by_name(&mut folders);
    Ok(Categories { albums, folders })
}

pub fn playlists(conn: &Connection) -> Result<Vec<CategorySummary>> {
    let mut rows = playlist_summaries(conn, None)?;
    sort_by_name(&mut rows);
    Ok(rows)
}

/// Một thẻ, cho đầu trang danh sách bài. Album rỗng vẫn trả về để trang đang mở không mất tiêu đề.
pub fn category(conn: &Connection, reference: CategoryRef) -> Result<Option<CategorySummary>> {
    let rows = match reference {
        CategoryRef::Album(id) => album_summaries(conn, Some(id))?,
        CategoryRef::Folder(id) => folder_summaries(conn, Some(id))?,
        CategoryRef::Playlist(id) => playlist_summaries(conn, Some(id))?,
    };
    Ok(rows.into_iter().next())
}

fn sort_by_name(rows: &mut [CategorySummary]) {
    rows.sort_by_cached_key(|row| schema::normalize_text(&row.name));
}

fn album_summaries(conn: &Connection, only: Option<i64>) -> Result<Vec<CategorySummary>> {
    let (join, filter) = if only.is_some() { ("LEFT", "a.id = ?1") } else { ("INNER", "?1 IS NULL") };
    let sql = format!(
        "SELECT a.id, a.name, a.artist, COUNT(s.id), a.cover_hash, c.accent_color
         FROM albums a {join} JOIN songs s ON s.album_id = a.id LEFT JOIN covers c ON c.hash = a.cover_hash
         WHERE {filter} GROUP BY a.id"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![only], |row| {
        Ok(CategorySummary {
            reference: CategoryRef::Album(row.get(0)?),
            name: row.get(1)?,
            artist: row.get(2)?,
            path: None,
            color: None,
            song_count: row.get(3)?,
            cover_hash: row.get(4)?,
            accent_color: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn folder_summaries(conn: &Connection, only: Option<i64>) -> Result<Vec<CategorySummary>> {
    let mut stmt = conn.prepare_cached(
        "SELECT f.id, f.name, f.path, COUNT(s.id), f.cover_hash, c.accent_color
         FROM folders f LEFT JOIN songs s ON s.folder_id = f.id LEFT JOIN covers c ON c.hash = f.cover_hash
         WHERE ?1 IS NULL OR f.id = ?1 GROUP BY f.id",
    )?;
    let rows = stmt.query_map(params![only], |row| {
        Ok(CategorySummary {
            reference: CategoryRef::Folder(row.get(0)?),
            name: row.get(1)?,
            artist: None,
            path: Some(PathBuf::from(row.get::<_, String>(2)?)),
            color: None,
            song_count: row.get(3)?,
            cover_hash: row.get(4)?,
            accent_color: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn playlist_summaries(conn: &Connection, only: Option<i64>) -> Result<Vec<CategorySummary>> {
    let mut stmt = conn.prepare_cached(
        "SELECT p.id, p.name, p.color, COUNT(ps.song_id)
         FROM playlists p LEFT JOIN playlist_songs ps ON ps.playlist_id = p.id
         WHERE ?1 IS NULL OR p.id = ?1 GROUP BY p.id",
    )?;
    let rows = stmt.query_map(params![only], |row| {
        Ok(CategorySummary {
            reference: CategoryRef::Playlist(row.get(0)?),
            name: row.get(1)?,
            artist: None,
            path: None,
            color: Some(row.get(2)?),
            song_count: row.get(3)?,
            cover_hash: None,
            accent_color: None,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Thứ tự theo S19: album `(disc_no, track_no, filename)`, thư mục theo filename, playlist theo `position`.
pub fn songs(conn: &Connection, reference: CategoryRef) -> Result<Vec<Song>> {
    let (sql, id) = songs_sql(reference);
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([id], song_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Đúng thứ tự của `songs`, cho thao tác hàng loạt (`save_bulk`).
pub fn song_ids(conn: &Connection, reference: CategoryRef) -> Result<Vec<i64>> {
    let (sql, id) = songs_sql(reference);
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn songs_sql(reference: CategoryRef) -> (String, i64) {
    match reference {
        CategoryRef::Album(id) => (
            format!(
                "SELECT {SONG_COLUMNS} WHERE s.album_id = ?1
                 ORDER BY COALESCE(s.disc_no, 1), s.track_no IS NULL, s.track_no, s.filename COLLATE NOCASE, s.id"
            ),
            id,
        ),
        CategoryRef::Folder(id) => {
            (format!("SELECT {SONG_COLUMNS} WHERE s.folder_id = ?1 ORDER BY s.filename COLLATE NOCASE, s.id"), id)
        }
        CategoryRef::Playlist(id) => (
            format!(
                "SELECT {SONG_COLUMNS} JOIN playlist_songs ps ON ps.song_id = s.id
                 WHERE ps.playlist_id = ?1 ORDER BY ps.position, s.id"
            ),
            id,
        ),
    }
}

/// `instr` trên `search_key` (S18). Truy vấn rỗng trả rỗng, không phải cả bảng. Khớp ở trường đầu (tiêu đề) xếp trước.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Song>> {
    let needle = schema::normalize_text(query);
    if needle.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {SONG_COLUMNS} WHERE instr(s.search_key, ?1) > 0
         ORDER BY instr(s.search_key, ?1) > instr(s.search_key || char(10), char(10)),
                  s.search_key, s.id
         LIMIT ?2"
    ))?;
    let rows = stmt.query_map(params![needle, limit as i64], song_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn song_by_id(conn: &Connection, id: i64) -> Result<Option<Song>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT {SONG_COLUMNS} WHERE s.id = ?1"))?;
    Ok(stmt.query_row([id], song_from_row).optional()?)
}

pub fn song_by_path(conn: &Connection, path: &Path) -> Result<Option<Song>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT {SONG_COLUMNS} WHERE s.path_key = ?1"))?;
    Ok(stmt.query_row([key_of(path)], song_from_row).optional()?)
}

/// Bài của một tham chiếu; tệp ngoài mà thực ra nằm trong thư viện cũng ra dòng thư viện. Tệp ngoài thật trả `None`.
pub fn song(conn: &Connection, track: &TrackRef) -> Result<Option<Song>> {
    match track {
        TrackRef::Library(id) => song_by_id(conn, *id),
        TrackRef::External(path) => song_by_path(conn, path),
    }
}

/// Đường dẫn thành tham chiếu (N6): có trong thư viện thì theo `id`, không thì `External`.
pub fn track_for_path(conn: &Connection, path: &Path) -> Result<TrackRef> {
    let mut stmt = conn.prepare_cached("SELECT id FROM songs WHERE path_key = ?1")?;
    let id: Option<i64> = stmt.query_row([key_of(path)], |row| row.get(0)).optional()?;
    Ok(id.map_or_else(|| TrackRef::External(path.to_owned()), TrackRef::Library))
}

/// Giữ thứ tự đầu vào, tham chiếu không còn thì bỏ. Tệp ngoài thật không qua đây: `Song::external` ở caller.
pub fn songs_for_tracks(conn: &Connection, tracks: &[TrackRef]) -> Result<Vec<Song>> {
    let mut found = Vec::with_capacity(tracks.len());
    for track in tracks {
        found.extend(song(conn, track)?);
    }
    Ok(found)
}

pub fn song_text(conn: &Connection, id: i64) -> Result<Option<SongText>> {
    let mut stmt = conn.prepare_cached("SELECT lyrics, timeline FROM songs WHERE id = ?1")?;
    let row = stmt.query_row([id], |row| Ok(SongText { lyrics: row.get(0)?, timeline: row.get(1)? }));
    Ok(row.optional()?)
}

pub fn song_file_states(conn: &Connection) -> Result<Vec<SongFileState>> {
    let mut stmt = conn.prepare_cached("SELECT id, file_path, path_key, file_mtime, file_size FROM songs")?;
    let rows = stmt.query_map([], |row| {
        Ok(SongFileState {
            id: row.get(0)?,
            path: PathBuf::from(row.get::<_, String>(1)?),
            path_key: row.get(2)?,
            mtime: row.get(3)?,
            size: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn pending_tag_reads(conn: &Connection, limit: usize) -> Result<Vec<PendingTagRead>> {
    let mut stmt =
        conn.prepare_cached("SELECT id, file_path FROM songs WHERE tags_read_at IS NULL ORDER BY id LIMIT ?1")?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(PendingTagRead { id: row.get(0)?, path: PathBuf::from(row.get::<_, String>(1)?) })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Mỗi cây một gốc: gốc nằm trong gốc khác thì gộp, tránh đăng ký watcher chồng nhau (`db.rs:593`).
pub fn watched_roots(conn: &Connection) -> Result<Vec<PathBuf>> {
    let mut kept: Vec<String> = Vec::new();
    for candidate in raw_watched_roots(conn)? {
        if kept.iter().any(|root| paths::is_under(&candidate, root)) {
            continue;
        }
        kept.retain(|root| !paths::is_under(root, &candidate));
        kept.push(candidate);
    }
    Ok(kept.into_iter().map(PathBuf::from).collect())
}

fn raw_watched_roots(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT path FROM watched_roots ORDER BY path")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------------------------------------------------- ghi

/// Chèn hoặc cập nhật theo `path_key`, giữ `id`, nên giữ cả playlist. Tệp không có lyrics, timeline mà DB có thì giữ DB (N2).
pub fn upsert_song(conn: &Connection, song: &SongUpsert) -> Result<UpsertOutcome> {
    let file_path = path_text(&song.path);
    let path_key = schema::path_key(&file_path);
    let filename = filename_of(&song.path);
    let album = album_name(conn, song.album_id)?;
    let search_key = schema::search_key(song.title.as_deref(), song.artist.as_deref(), album.as_deref(), &filename);
    let existed = song_id_by_key(conn, &path_key)?.is_some();

    let mut stmt = conn.prepare_cached(
        "INSERT INTO songs (title, artist, album_artist, album_id, folder_id, file_path, path_key, filename, duration,
             track_no, disc_no, lyrics, timeline, cover_hash, file_mtime, file_size, search_key, tags_read_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(path_key) DO UPDATE SET
             title = excluded.title,
             artist = excluded.artist,
             album_artist = excluded.album_artist,
             album_id = excluded.album_id,
             folder_id = excluded.folder_id,
             file_path = excluded.file_path,
             filename = excluded.filename,
             duration = excluded.duration,
             track_no = excluded.track_no,
             disc_no = excluded.disc_no,
             lyrics = COALESCE(NULLIF(excluded.lyrics, ''), songs.lyrics),
             timeline = COALESCE(NULLIF(excluded.timeline, ''), songs.timeline),
             cover_hash = excluded.cover_hash,
             file_mtime = excluded.file_mtime,
             file_size = excluded.file_size,
             search_key = excluded.search_key,
             tags_read_at = excluded.tags_read_at
         RETURNING id",
    )?;
    let id = stmt.query_row(
        params![
            song.title,
            song.artist,
            song.album_artist,
            song.album_id,
            song.folder_id,
            file_path,
            path_key,
            filename,
            song.duration_sec,
            song.track_no,
            song.disc_no,
            song.lyrics,
            song.timeline,
            song.cover_hash,
            song.file_mtime,
            song.file_size,
            search_key,
            song.tags_read_at,
        ],
        |row| row.get(0),
    )?;
    Ok(UpsertOutcome { id, inserted: !existed })
}

fn song_id_by_key(conn: &Connection, path_key: &str) -> Result<Option<i64>> {
    let mut stmt = conn.prepare_cached("SELECT id FROM songs WHERE path_key = ?1")?;
    Ok(stmt.query_row([path_key], |row| row.get(0)).optional()?)
}

fn album_name(conn: &Connection, album_id: Option<i64>) -> Result<Option<String>> {
    let Some(id) = album_id else {
        return Ok(None);
    };
    let mut stmt = conn.prepare_cached("SELECT name FROM albums WHERE id = ?1")?;
    Ok(stmt.query_row([id], |row| row.get(0)).optional()?)
}

/// Đổi đường dẫn, giữ `id` (N3, N4). Đích là bài khác thì báo lỗi, không đè; chỉ đổi hoa thường trên hệ tệp gập thì được.
pub fn move_song(conn: &Connection, id: i64, new_path: &Path, folder_id: Option<i64>) -> Result<()> {
    let file_path = path_text(new_path);
    let path_key = schema::path_key(&file_path);
    if song_id_by_key(conn, &path_key)?.is_some_and(|other| other != id) {
        return Err(QueryError::PathConflict(new_path.to_owned()));
    }
    let mut stmt = conn.prepare_cached(
        "SELECT s.title, s.artist, a.name FROM songs s LEFT JOIN albums a ON a.id = s.album_id WHERE s.id = ?1",
    )?;
    let fields: Option<(Option<String>, Option<String>, Option<String>)> =
        stmt.query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
    let (title, artist, album) = fields.ok_or(QueryError::SongNotFound(id))?;

    let filename = filename_of(new_path);
    let search_key = schema::search_key(title.as_deref(), artist.as_deref(), album.as_deref(), &filename);
    conn.prepare_cached(
        "UPDATE songs SET file_path = ?2, path_key = ?3, filename = ?4, search_key = ?5,
             folder_id = COALESCE(?6, folder_id)
         WHERE id = ?1",
    )?
    .execute(params![id, file_path, path_key, filename, search_key, folder_id])?;
    Ok(())
}

pub fn delete_song(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.prepare_cached("DELETE FROM songs WHERE id = ?1")?.execute([id])? > 0)
}

/// Trả `id` đã xoá, để hàng đợi bỏ đúng bài (N5).
pub fn delete_song_by_path(conn: &Connection, path: &Path) -> Result<Option<i64>> {
    let Some(id) = song_id_by_key(conn, &key_of(path))? else {
        return Ok(None);
    };
    delete_song(conn, id)?;
    Ok(Some(id))
}

pub fn set_song_lyrics(conn: &Connection, id: i64, lyrics: Option<&str>) -> Result<()> {
    update_one(conn, "UPDATE songs SET lyrics = ?2 WHERE id = ?1", id, lyrics)
}

pub fn set_song_timeline(conn: &Connection, id: i64, timeline: Option<&str>) -> Result<()> {
    update_one(conn, "UPDATE songs SET timeline = ?2 WHERE id = ?1", id, timeline)
}

fn update_one(conn: &Connection, sql: &str, id: i64, value: Option<&str>) -> Result<()> {
    match conn.prepare_cached(sql)?.execute(params![id, value])? {
        0 => Err(QueryError::SongNotFound(id)),
        _ => Ok(()),
    }
}

/// Ghi xong tệp hoặc đọc lại baseline: đồng bộ dấu vết để reconcile không coi là đổi.
pub fn set_song_file_state(conn: &Connection, id: i64, mtime: Option<i64>, size: Option<i64>) -> Result<()> {
    conn.prepare_cached("UPDATE songs SET file_mtime = ?2, file_size = ?3 WHERE id = ?1")?
        .execute(params![id, mtime, size])?;
    Ok(())
}

/// Thư mục so qua `paths::normalize` vì `folders.path` không có cột khoá (M2); số thư mục nhỏ nên quét được.
pub fn get_or_create_folder(conn: &Connection, name: &str, path: &Path) -> Result<i64> {
    let wanted = key_of(path);
    let mut stmt = conn.prepare_cached("SELECT id, path FROM folders")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, existing) = row?;
        if schema::path_key(&existing) == wanted {
            return Ok(id);
        }
    }
    conn.prepare_cached("INSERT INTO folders (name, path) VALUES (?1, ?2)")?.execute(params![name, path_text(path)])?;
    Ok(conn.last_insert_rowid())
}

/// `group_key` lấy từ `schema::album_group_key`. Đã có album thì lấp `artist` nếu đang trống, không đè.
pub fn get_or_create_album(conn: &Connection, name: &str, group_key: &str, artist: Option<&str>) -> Result<i64> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO albums (name, group_key, artist) VALUES (?1, ?2, ?3)
         ON CONFLICT(name, group_key) DO UPDATE SET artist = COALESCE(albums.artist, excluded.artist)
         RETURNING id",
    )?;
    Ok(stmt.query_row(params![name, group_key, artist], |row| row.get(0))?)
}

pub fn delete_folder(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.prepare_cached("DELETE FROM folders WHERE id = ?1")?.execute([id])? > 0)
}

pub fn delete_album(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.prepare_cached("DELETE FROM albums WHERE id = ?1")?.execute([id])? > 0)
}

/// Dọn album không còn bài sau khi gán lại; bìa tự chọn cũng đi theo dòng, người dùng đã xoá hết bài của nó.
pub fn delete_empty_albums(conn: &Connection) -> Result<usize> {
    Ok(conn
        .prepare_cached("DELETE FROM albums WHERE NOT EXISTS (SELECT 1 FROM songs WHERE songs.album_id = albums.id)")?
        .execute([])?)
}

pub fn upsert_cover(conn: &Connection, cover: &CoverRecord) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO covers (hash, accent_color, width, height) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(hash) DO UPDATE SET accent_color = excluded.accent_color,
             width = excluded.width, height = excluded.height",
    )?
    .execute(params![cover.hash, cover.accent_color, cover.width, cover.height])?;
    Ok(())
}

/// Bìa tự động không đè bìa người dùng chọn (S8, S13). `custom = true` luôn ghi và đánh dấu.
pub fn set_album_cover(conn: &Connection, id: i64, hash: Option<&str>, custom: bool) -> Result<()> {
    let sql = if custom {
        "UPDATE albums SET cover_hash = ?2, cover_is_custom = 1 WHERE id = ?1"
    } else {
        "UPDATE albums SET cover_hash = ?2 WHERE id = ?1 AND cover_is_custom = 0"
    };
    conn.prepare_cached(sql)?.execute(params![id, hash])?;
    Ok(())
}

pub fn set_folder_cover(conn: &Connection, id: i64, hash: Option<&str>) -> Result<()> {
    conn.prepare_cached("UPDATE folders SET cover_hash = ?2 WHERE id = ?1")?.execute(params![id, hash])?;
    Ok(())
}

/// Trả `false` khi gốc đã nằm trong một gốc có sẵn. Thêm gốc cha thì bỏ các gốc con (`db.rs:567`).
pub fn add_watched_root(conn: &Connection, path: &Path) -> Result<bool> {
    let text = path_text(path);
    let existing = raw_watched_roots(conn)?;
    if existing.iter().any(|root| paths::is_under(&text, root)) {
        return Ok(false);
    }
    for child in existing.iter().filter(|root| paths::is_under(root, &text)) {
        conn.prepare_cached("DELETE FROM watched_roots WHERE path = ?1")?.execute([child])?;
    }
    conn.prepare_cached("INSERT INTO watched_roots (path) VALUES (?1)")?.execute([text])?;
    Ok(true)
}

pub fn remove_watched_root(conn: &Connection, path: &Path) -> Result<bool> {
    let wanted = key_of(path);
    let mut removed = false;
    for root in raw_watched_roots(conn)? {
        if schema::path_key(&root) == wanted {
            conn.prepare_cached("DELETE FROM watched_roots WHERE path = ?1")?.execute([root])?;
            removed = true;
        }
    }
    Ok(removed)
}

/// DB cũ chưa từng ghi `watched_roots` thì lấy từ `folders` (`db.rs:596`). Chỉ gọi một lần, sau khi chuyển đổi: gọi ở lần
/// đọc thì người dùng đã bỏ hết gốc sẽ thấy gốc quay lại (N9).
pub fn seed_watched_roots_from_folders(conn: &Connection) -> Result<usize> {
    if !raw_watched_roots(conn)?.is_empty() {
        return Ok(0);
    }
    let folders: Vec<String> = {
        let mut stmt = conn.prepare_cached("SELECT path FROM folders ORDER BY length(path)")?;
        stmt.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    let mut added = 0;
    for folder in folders {
        added += usize::from(add_watched_root(conn, Path::new(&folder))?);
    }
    Ok(added)
}

/// Tên cắt khoảng trắng hai đầu. Trùng tên báo lỗi có kiểu, không phải lỗi SQL thô (đặc tả 1.2).
pub fn create_playlist(conn: &Connection, name: &str, color: &str) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(QueryError::EmptyName);
    }
    let mut stmt = conn.prepare_cached(
        "INSERT INTO playlists (name, color) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING RETURNING id",
    )?;
    stmt.query_row(params![name, color], |row| row.get(0))
        .optional()?
        .ok_or_else(|| QueryError::DuplicatePlaylist(name.to_owned()))
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.prepare_cached("DELETE FROM playlists WHERE id = ?1")?.execute([id])? > 0)
}

/// Thêm vào cuối. Bài đã có thì giữ chỗ cũ, trả `false`.
pub fn add_song_to_playlist(conn: &Connection, playlist_id: i64, song_id: i64) -> Result<bool> {
    let added = conn
        .prepare_cached(
            "INSERT INTO playlist_songs (playlist_id, song_id, position, added_at)
             VALUES (?1, ?2,
                 (SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_songs WHERE playlist_id = ?1), ?3)
             ON CONFLICT(playlist_id, song_id) DO NOTHING",
        )?
        .execute(params![playlist_id, song_id, now_ms()])?;
    Ok(added > 0)
}

/// `position` chỉ để xếp thứ tự, có lỗ sau khi xoá cũng đúng; số thứ tự hiển thị lấy từ chỉ số dòng.
pub fn remove_song_from_playlist(conn: &Connection, playlist_id: i64, song_id: i64) -> Result<bool> {
    let removed = conn
        .prepare_cached("DELETE FROM playlist_songs WHERE playlist_id = ?1 AND song_id = ?2")?
        .execute(params![playlist_id, song_id])?;
    Ok(removed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate::open_fresh;
    use tempfile::TempDir;

    fn db() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = open_fresh(&dir.path().join("library.db")).unwrap();
        (dir, conn)
    }

    fn folder(conn: &Connection) -> i64 {
        get_or_create_folder(conn, "Nhạc", Path::new("/m")).unwrap()
    }

    fn entry(path: &str, folder_id: i64) -> SongUpsert {
        SongUpsert { path: path.into(), folder_id, duration_sec: 10.0, ..SongUpsert::default() }
    }

    fn ids(songs: &[Song]) -> Vec<i64> {
        songs
            .iter()
            .map(|song| match song.track {
                TrackRef::Library(id) => id,
                TrackRef::External(_) => panic!("library row expected"),
            })
            .collect()
    }

    #[test]
    fn upsert_goes_through_path_key_and_keeps_the_id() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let first = upsert_song(&conn, &entry(r"C:\m\a.mp3", f)).unwrap();
        assert!(first.inserted);
        // Cùng tệp, cách viết khác: không thành dòng thứ hai (M2).
        let second = upsert_song(&conn, &SongUpsert { title: Some("Mới".into()), ..entry("C:/m/a.mp3", f) }).unwrap();
        assert_eq!(second, UpsertOutcome { id: first.id, inserted: false });
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM songs", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);

        let found = song_by_path(&conn, Path::new(r"C:\m\a.mp3")).unwrap().unwrap();
        assert_eq!(found.title.as_deref(), Some("Mới"));
        assert_eq!(found.path, PathBuf::from("C:/m/a.mp3"));
        assert_eq!(found.duration_sec, 10.0);
    }

    #[test]
    fn upsert_keeps_db_lyrics_and_timeline_when_the_file_has_none() {
        // N2: import lại không được xoá timeline nhập tay.
        let (_dir, conn) = db();
        let f = folder(&conn);
        let song = SongUpsert { lyrics: Some("lời".into()), timeline: Some("[1]".into()), ..entry("/m/a.mp3", f) };
        let id = upsert_song(&conn, &song).unwrap().id;
        upsert_song(&conn, &SongUpsert { lyrics: Some(String::new()), ..entry("/m/a.mp3", f) }).unwrap();
        let text = song_text(&conn, id).unwrap().unwrap();
        assert_eq!(text, SongText { lyrics: Some("lời".into()), timeline: Some("[1]".into()) });

        upsert_song(&conn, &SongUpsert { timeline: Some("[2]".into()), ..entry("/m/a.mp3", f) }).unwrap();
        assert_eq!(song_text(&conn, id).unwrap().unwrap().timeline.as_deref(), Some("[2]"));
        set_song_timeline(&conn, id, None).unwrap();
        assert_eq!(song_text(&conn, id).unwrap().unwrap().timeline, None);
        assert!(matches!(set_song_lyrics(&conn, 999, None), Err(QueryError::SongNotFound(999))));
    }

    #[test]
    fn song_list_rows_do_not_carry_lyrics_and_resolve_the_album_name() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let album = get_or_create_album(&conn, "Tình Ca", "folder:1", Some("A")).unwrap();
        upsert_song(&conn, &SongUpsert { album_id: Some(album), ..entry("/m/a.mp3", f) }).unwrap();
        let rows = songs(&conn, CategoryRef::Album(album)).unwrap();
        assert_eq!(rows[0].album.as_deref(), Some("Tình Ca"));
        assert_eq!(rows[0].filename, "a.mp3");
    }

    #[test]
    fn track_for_path_finds_library_rows_and_leaves_the_rest_external() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let id = upsert_song(&conn, &entry("/m/a.mp3", f)).unwrap().id;
        assert_eq!(track_for_path(&conn, Path::new("/m/a.mp3")).unwrap(), TrackRef::Library(id));
        assert_eq!(track_for_path(&conn, Path::new("/x/b.mp3")).unwrap(), TrackRef::from_path("/x/b.mp3"));
        // Tệp ngoài mà nằm trong thư viện vẫn ra đúng dòng; tệp ngoài thật và id mất bị bỏ, thứ tự giữ nguyên.
        let id2 = upsert_song(&conn, &entry("/m/b.mp3", f)).unwrap().id;
        let found = songs_for_tracks(
            &conn,
            &[
                TrackRef::Library(id2),
                TrackRef::Library(999),
                TrackRef::from_path("/x/none.mp3"),
                TrackRef::from_path("/m/a.mp3"),
            ],
        )
        .unwrap();
        assert_eq!(ids(&found), [id2, id]);
    }

    #[test]
    fn search_is_diacritic_free_and_never_matches_everything() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let hit = SongUpsert { title: Some("Đường Về".into()), ..entry("/m/x.mp3", f) };
        let id = upsert_song(&conn, &hit).unwrap().id;
        upsert_song(&conn, &entry("/m/other.mp3", f)).unwrap();

        assert_eq!(ids(&search(&conn, "DUONG ve", 10).unwrap()), [id]);
        assert_eq!(ids(&search(&conn, "đường", 10).unwrap()), [id]);
        // S18: `-` bản cũ khớp tất cả.
        assert!(search(&conn, "-", 10).unwrap().is_empty());
        assert!(search(&conn, "   ", 10).unwrap().is_empty());
        assert!(search(&conn, "duong", 0).unwrap().is_empty());
    }

    #[test]
    fn search_puts_title_matches_first_and_honours_the_limit() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let by_artist = SongUpsert { title: Some("Aaa".into()), artist: Some("Mưa".into()), ..entry("/m/1.mp3", f) };
        let by_title = SongUpsert { title: Some("Zzz mưa".into()), ..entry("/m/2.mp3", f) };
        let artist_id = upsert_song(&conn, &by_artist).unwrap().id;
        let title_id = upsert_song(&conn, &by_title).unwrap().id;
        assert_eq!(ids(&search(&conn, "mua", 10).unwrap()), [title_id, artist_id]);
        assert_eq!(ids(&search(&conn, "mua", 1).unwrap()), [title_id]);
    }

    #[test]
    fn upsert_and_move_refresh_the_search_key() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let id = upsert_song(&conn, &entry("/m/cu.mp3", f)).unwrap().id;
        assert_eq!(search(&conn, "cu.mp3", 5).unwrap().len(), 1);
        move_song(&conn, id, Path::new("/m/moi.mp3"), None).unwrap();
        assert!(search(&conn, "cu.mp3", 5).unwrap().is_empty());
        assert_eq!(search(&conn, "moi.mp3", 5).unwrap().len(), 1);
        let album = get_or_create_album(&conn, "Mùa Thu", "k", None).unwrap();
        upsert_song(&conn, &SongUpsert { album_id: Some(album), ..entry("/m/moi.mp3", f) }).unwrap();
        assert_eq!(ids(&search(&conn, "mua thu", 5).unwrap()), [id]);
    }

    #[test]
    fn move_keeps_id_and_playlist_and_refuses_to_overwrite() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let a = upsert_song(&conn, &entry("/m/a.mp3", f)).unwrap().id;
        let b = upsert_song(&conn, &entry("/m/b.mp3", f)).unwrap().id;
        let list = create_playlist(&conn, "L", "#fff").unwrap();
        add_song_to_playlist(&conn, list, a).unwrap();

        let g = get_or_create_folder(&conn, "Khác", Path::new("/n")).unwrap();
        move_song(&conn, a, Path::new("/n/c.mp3"), Some(g)).unwrap();
        let moved = song_by_path(&conn, Path::new("/n/c.mp3")).unwrap().unwrap();
        assert_eq!(moved.track, TrackRef::Library(a));
        assert_eq!(moved.folder_id, Some(g));
        assert_eq!(moved.filename, "c.mp3");
        assert_eq!(ids(&songs(&conn, CategoryRef::Playlist(list)).unwrap()), [a]);

        assert!(matches!(move_song(&conn, b, Path::new("/n/c.mp3"), None), Err(QueryError::PathConflict(_))));
        assert_eq!(song_by_id(&conn, b).unwrap().unwrap().filename, "b.mp3");
        assert!(matches!(move_song(&conn, 999, Path::new("/z.mp3"), None), Err(QueryError::SongNotFound(999))));
        // Chính nó: đổi cách viết đường dẫn không phải xung đột.
        move_song(&conn, b, Path::new("/m/b.mp3"), None).unwrap();
    }

    #[test]
    fn album_orders_by_disc_then_track_folder_by_filename() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let album = get_or_create_album(&conn, "A", "k", None).unwrap();
        let add = |path: &str, disc: Option<u32>, track: Option<u32>| {
            let song = SongUpsert { album_id: Some(album), disc_no: disc, track_no: track, ..entry(path, f) };
            upsert_song(&conn, &song).unwrap().id
        };
        let no_track = add("/m/a.mp3", None, None);
        let d2t1 = add("/m/b.mp3", Some(2), Some(1));
        let d1t2 = add("/m/c.mp3", Some(1), Some(2));
        let no_disc_t1 = add("/m/d.mp3", None, Some(1));

        let expected = [no_disc_t1, d1t2, no_track, d2t1];
        assert_eq!(ids(&songs(&conn, CategoryRef::Album(album)).unwrap()), expected);
        assert_eq!(song_ids(&conn, CategoryRef::Album(album)).unwrap(), expected);
        assert_eq!(ids(&songs(&conn, CategoryRef::Folder(f)).unwrap()), [no_track, d2t1, d1t2, no_disc_t1]);
        assert!(songs(&conn, CategoryRef::Folder(999)).unwrap().is_empty());
    }

    #[test]
    fn grids_sort_by_unaccented_name_and_hide_empty_albums() {
        let (_dir, conn) = db();
        let f = get_or_create_folder(&conn, "Zebra", Path::new("/z")).unwrap();
        get_or_create_folder(&conn, "Âm", Path::new("/a")).unwrap();
        let bai = get_or_create_album(&conn, "Đông", "k", Some("Ai")).unwrap();
        let cuoi = get_or_create_album(&conn, "An", "k", None).unwrap();
        let rong = get_or_create_album(&conn, "Rỗng", "k", None).unwrap();
        for (path, album) in [("/z/1.mp3", bai), ("/z/2.mp3", bai), ("/z/3.mp3", cuoi)] {
            upsert_song(&conn, &SongUpsert { album_id: Some(album), ..entry(path, f) }).unwrap();
        }
        let grid = categories(&conn).unwrap();
        let names: Vec<_> = grid.albums.iter().map(|album| album.name.as_str()).collect();
        assert_eq!(names, ["An", "Đông"]);
        assert_eq!(grid.albums[1].song_count, 2);
        assert_eq!(grid.albums[1].artist.as_deref(), Some("Ai"));
        let names: Vec<_> = grid.folders.iter().map(|folder| folder.name.as_str()).collect();
        assert_eq!(names, ["Âm", "Zebra"]);
        assert_eq!(grid.folders[0].song_count, 0);
        assert_eq!(grid.folders[1].path.as_deref(), Some(Path::new("/z")));

        // Trang đang mở vẫn lấy được tiêu đề của album rỗng; id lạ thì không.
        assert_eq!(category(&conn, CategoryRef::Album(rong)).unwrap().unwrap().song_count, 0);
        assert_eq!(category(&conn, CategoryRef::Folder(f)).unwrap().unwrap().name, "Zebra");
        assert!(category(&conn, CategoryRef::Playlist(1)).unwrap().is_none());
        assert_eq!(delete_empty_albums(&conn).unwrap(), 1);
        assert!(category(&conn, CategoryRef::Album(rong)).unwrap().is_none());
    }

    #[test]
    fn same_named_albums_stay_apart_by_group_key() {
        // S1, S3.
        let (_dir, conn) = db();
        let a = get_or_create_album(&conn, "Greatest Hits", "folder:1", None).unwrap();
        let b = get_or_create_album(&conn, "Greatest Hits", "folder:2", None).unwrap();
        assert_ne!(a, b);
        assert_eq!(get_or_create_album(&conn, "Greatest Hits", "folder:1", Some("X")).unwrap(), a);
        let artist = category(&conn, CategoryRef::Album(a)).unwrap().unwrap().artist;
        assert_eq!(artist.as_deref(), Some("X"));
        get_or_create_album(&conn, "Greatest Hits", "folder:1", Some("Y")).unwrap();
        let artist = category(&conn, CategoryRef::Album(a)).unwrap().unwrap().artist;
        assert_eq!(artist.as_deref(), Some("X"));
    }

    #[test]
    fn folder_lookup_ignores_spelling_of_the_path() {
        let (_dir, conn) = db();
        let id = get_or_create_folder(&conn, "M", Path::new(r"C:\m")).unwrap();
        assert_eq!(get_or_create_folder(&conn, "M", Path::new("C:/m/")).unwrap(), id);
        assert_ne!(get_or_create_folder(&conn, "M", Path::new("C:/m2")).unwrap(), id);
    }

    #[test]
    fn playlist_keeps_insertion_order_after_removal() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let list = create_playlist(&conn, " Yêu thích ", "#06b6d4").unwrap();
        let z = upsert_song(&conn, &entry("/m/z.mp3", f)).unwrap().id;
        let a = upsert_song(&conn, &entry("/m/a.mp3", f)).unwrap().id;
        let m = upsert_song(&conn, &entry("/m/m.mp3", f)).unwrap().id;
        for id in [z, a, m] {
            assert!(add_song_to_playlist(&conn, list, id).unwrap());
        }
        // Thêm lần hai giữ chỗ cũ.
        assert!(!add_song_to_playlist(&conn, list, z).unwrap());
        assert_eq!(ids(&songs(&conn, CategoryRef::Playlist(list)).unwrap()), [z, a, m]);

        let first: i64 = conn
            .query_row("SELECT MIN(position) FROM playlist_songs WHERE playlist_id = ?1", [list], |row| row.get(0))
            .unwrap();
        assert_eq!(first, 0);

        assert!(remove_song_from_playlist(&conn, list, a).unwrap());
        assert!(!remove_song_from_playlist(&conn, list, a).unwrap());
        assert!(add_song_to_playlist(&conn, list, a).unwrap());
        assert_eq!(ids(&songs(&conn, CategoryRef::Playlist(list)).unwrap()), [z, m, a]);

        let summary = &playlists(&conn).unwrap()[0];
        assert_eq!(summary.name, "Yêu thích");
        assert_eq!(summary.color.as_deref(), Some("#06b6d4"));
        assert_eq!(summary.song_count, 3);

        // FK cascade: xoá bài thì rời playlist.
        delete_song(&conn, z).unwrap();
        assert_eq!(ids(&songs(&conn, CategoryRef::Playlist(list)).unwrap()), [m, a]);
        assert!(delete_playlist(&conn, list).unwrap());
        assert!(playlists(&conn).unwrap().is_empty());
    }

    #[test]
    fn playlist_name_errors_are_typed() {
        let (_dir, conn) = db();
        assert!(matches!(create_playlist(&conn, "   ", "#000"), Err(QueryError::EmptyName)));
        create_playlist(&conn, "A", "#000").unwrap();
        assert!(
            matches!(create_playlist(&conn, " A", "#111"), Err(QueryError::DuplicatePlaylist(name)) if name == "A")
        );
    }

    #[test]
    fn roots_collapse_into_their_ancestor() {
        let (_dir, conn) = db();
        assert!(add_watched_root(&conn, Path::new("/m/pop")).unwrap());
        assert!(add_watched_root(&conn, Path::new("/m/rock")).unwrap());
        assert!(!add_watched_root(&conn, Path::new("/m/pop/2020")).unwrap());
        assert!(add_watched_root(&conn, Path::new("/m")).unwrap());
        assert!(add_watched_root(&conn, Path::new("/music")).unwrap());
        assert_eq!(watched_roots(&conn).unwrap(), [PathBuf::from("/m"), PathBuf::from("/music")]);
        assert!(remove_watched_root(&conn, Path::new("/m/")).unwrap());
        assert!(!remove_watched_root(&conn, Path::new("/m")).unwrap());
        assert_eq!(watched_roots(&conn).unwrap(), [PathBuf::from("/music")]);
    }

    #[test]
    fn roots_seed_from_folders_only_when_there_are_none() {
        let (_dir, conn) = db();
        get_or_create_folder(&conn, "A", Path::new("/m/a")).unwrap();
        get_or_create_folder(&conn, "M", Path::new("/m")).unwrap();
        assert_eq!(seed_watched_roots_from_folders(&conn).unwrap(), 1);
        assert_eq!(watched_roots(&conn).unwrap(), [PathBuf::from("/m")]);
        get_or_create_folder(&conn, "N", Path::new("/n")).unwrap();
        assert_eq!(seed_watched_roots_from_folders(&conn).unwrap(), 0);
    }

    #[test]
    fn automatic_cover_never_replaces_a_custom_one() {
        // S8, S13.
        let (_dir, conn) = db();
        let f = folder(&conn);
        let album = get_or_create_album(&conn, "A", "k", None).unwrap();
        upsert_song(&conn, &SongUpsert { album_id: Some(album), ..entry("/m/a.mp3", f) }).unwrap();
        let record = CoverRecord { hash: "h1".into(), accent_color: "#123456".into(), width: Some(1), height: None };
        upsert_cover(&conn, &record).unwrap();
        upsert_cover(&conn, &CoverRecord { accent_color: "#654321".into(), ..record }).unwrap();

        set_album_cover(&conn, album, Some("auto"), false).unwrap();
        set_album_cover(&conn, album, Some("h1"), true).unwrap();
        set_album_cover(&conn, album, Some("auto2"), false).unwrap();
        let summary = category(&conn, CategoryRef::Album(album)).unwrap().unwrap();
        assert_eq!(summary.cover_hash.as_deref(), Some("h1"));
        assert_eq!(summary.accent_color.as_deref(), Some("#654321"));

        set_folder_cover(&conn, f, Some("h1")).unwrap();
        let summary = category(&conn, CategoryRef::Folder(f)).unwrap().unwrap();
        assert_eq!(summary.accent_color.as_deref(), Some("#654321"));
    }

    #[test]
    fn reconcile_data_and_pending_tag_reads() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let read = SongUpsert { tags_read_at: Some(now_ms()), ..entry("/m/a.mp3", f) };
        let a = upsert_song(&conn, &read).unwrap().id;
        let b = upsert_song(&conn, &entry("/m/b.mp3", f)).unwrap().id;
        assert_eq!(pending_tag_reads(&conn, 10).unwrap(), [PendingTagRead { id: b, path: "/m/b.mp3".into() }]);
        assert!(pending_tag_reads(&conn, 0).unwrap().is_empty());

        set_song_file_state(&conn, a, Some(7), Some(9)).unwrap();
        let state = song_file_states(&conn).unwrap().into_iter().find(|state| state.id == a).unwrap();
        assert_eq!((state.mtime, state.size), (Some(7), Some(9)));
        assert_eq!(state.path_key, schema::path_key("/m/a.mp3"));

        assert_eq!(delete_song_by_path(&conn, Path::new("/m/a.mp3")).unwrap(), Some(a));
        assert_eq!(delete_song_by_path(&conn, Path::new("/m/a.mp3")).unwrap(), None);
    }

    #[test]
    fn deleting_a_folder_cascades_and_an_album_only_detaches() {
        let (_dir, conn) = db();
        let f = folder(&conn);
        let album = get_or_create_album(&conn, "A", "k", None).unwrap();
        let a = upsert_song(&conn, &SongUpsert { album_id: Some(album), ..entry("/m/a.mp3", f) }).unwrap().id;
        assert!(delete_album(&conn, album).unwrap());
        assert_eq!(song_by_id(&conn, a).unwrap().unwrap().album_id, None);
        assert!(delete_folder(&conn, f).unwrap());
        assert!(song_by_id(&conn, a).unwrap().is_none());
    }
}
