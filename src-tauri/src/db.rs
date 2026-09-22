use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn normalize_char(c: char) -> Option<char> {
    match c {
        'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' |
        'À' | 'Á' | 'Ạ' | 'Ả' | 'Ã' | 'Â' | 'Ầ' | 'Ấ' | 'Ậ' | 'Ẩ' | 'Ẫ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ặ' | 'Ẳ' | 'Ẵ' => Some('a'),
        
        'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' |
        'È' | 'É' | 'Ẹ' | 'Ẻ' | 'Ẽ' | 'Ê' | 'Ề' | 'Ế' | 'Ệ' | 'Ể' | 'Ễ' => Some('e'),
        
        'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ' | 'ợ' | 'ở' | 'ỡ' |
        'Ò' | 'Ó' | 'Ọ' | 'Ỏ' | 'Õ' | 'Ô' | 'Ồ' | 'Ố' | 'Ộ' | 'Ổ' | 'Ỗ' | 'Ơ' | 'Ờ' | 'Ớ' | 'Ợ' | 'Ở' | 'Ỡ' => Some('o'),
        
        'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' |
        'Ù' | 'Ú' | 'Ụ' | 'Ủ' | 'Ũ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ự' | 'Ử' | 'Ữ' => Some('u'),
        
        'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' |
        'Ì' | 'Í' | 'Ị' | 'Ỉ' | 'Ĩ' => Some('i'),
        
        'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' |
        'Ỳ' | 'Ý' | 'Ỵ' | 'Ỷ' | 'Ỹ' => Some('y'),
        
        'đ' | 'Đ' => Some('d'),
        
        _ => None,
    }
}

fn normalize_string(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    for c in input.chars() {
        if let Some(norm_c) = normalize_char(c) {
            normalized.push(norm_c);
        } else {
            let lower = c.to_lowercase();
            for lc in lower {
                if lc.is_alphanumeric() {
                    normalized.push(lc);
                }
            }
        }
    }
    normalized
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub thumbnail: Option<String>,
    pub accent_color: Option<String>,
    pub songs_count: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Album {
    pub id: i64,
    pub name: String,
    pub artist: Option<String>,
    pub thumbnail: Option<String>,
    pub accent_color: Option<String>,
    pub songs_count: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Song {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_id: Option<i64>,
    pub folder_id: i64,
    pub file_path: String,
    pub filename: String,
    pub duration: u32,
    pub lyrics: Option<String>,
    pub timeline: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SongSearchResult {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_id: Option<i64>,
    pub album_name: Option<String>,
    pub folder_id: i64,
    pub file_path: String,
    pub filename: String,
    pub duration: u32,
    pub lyrics: Option<String>,
    pub timeline: Option<String>,
}

pub fn init_db(db_path: &Path) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    
    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON;", [])?;

    // Register custom normalize_str function
    conn.create_scalar_function(
        "normalize_str",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8 | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let val = ctx.get::<Option<String>>(0)?;
            match val {
                Some(s) => Ok(Some(normalize_string(&s))),
                None => Ok(None),
            }
        },
    )?;

    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            path TEXT UNIQUE NOT NULL,
            thumbnail TEXT,
            accent_color TEXT
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            artist TEXT,
            thumbnail TEXT,
            accent_color TEXT
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS songs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT,
            artist TEXT,
            album_id INTEGER,
            folder_id INTEGER NOT NULL,
            file_path TEXT UNIQUE NOT NULL,
            filename TEXT NOT NULL,
            duration INTEGER NOT NULL,
            lyrics TEXT,
            timeline TEXT,
            FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE SET NULL,
            FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE
        );",
        [],
    )?;

    // Migration: Add lyrics column to songs table if it does not exist
    let has_lyrics_col: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='lyrics';",
        [],
        |row| row.get(0)
    ).unwrap_or(0) > 0;

    if !has_lyrics_col {
        conn.execute("ALTER TABLE songs ADD COLUMN lyrics TEXT;", []).ok();
    }

    // Migration: Add timeline column to songs table if it does not exist
    let has_timeline_col: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='timeline';",
        [],
        |row| row.get(0)
    ).unwrap_or(0) > 0;

    if !has_timeline_col {
        conn.execute("ALTER TABLE songs ADD COLUMN timeline TEXT;", []).ok();
    }

    // Migration: file modification time (ms since the epoch) as last indexed, so
    // the startup reconcile can tell a file edited while the app was closed apart
    // from one that is unchanged. NULL means "not recorded yet".
    let has_mtime_col: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='file_mtime';",
        [],
        |row| row.get(0)
    ).unwrap_or(0) > 0;

    if !has_mtime_col {
        conn.execute("ALTER TABLE songs ADD COLUMN file_mtime INTEGER;", []).ok();
    }

    // The folders the user actually picked in the import dialog. `folders` only
    // records the parent directory of each file, which is not enough for the
    // watcher: a brand new subdirectory would have no row to watch.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS watched_roots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            color TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS playlist_songs (
            playlist_id INTEGER NOT NULL,
            song_id INTEGER NOT NULL,
            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY(song_id) REFERENCES songs(id) ON DELETE CASCADE,
            PRIMARY KEY(playlist_id, song_id)
        );",
        [],
    )?;

    Ok(conn)
}

pub fn get_or_create_folder(conn: &Connection, name: &str, path: &str) -> Result<i64> {
    let mut stmt = conn.prepare("SELECT id FROM folders WHERE path = ?1")?;
    let id_opt: Option<i64> = stmt
        .query_row(params![path], |row| row.get(0))
        .ok();

    if let Some(id) = id_opt {
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO folders (name, path) VALUES (?1, ?2)",
            params![name, path],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn get_or_create_album(conn: &Connection, name: &str, artist: Option<&str>) -> Result<i64> {
    let mut stmt = conn.prepare("SELECT id FROM albums WHERE name = ?1")?;
    let id_opt: Option<i64> = stmt
        .query_row(params![name], |row| row.get(0))
        .ok();

    if let Some(id) = id_opt {
        // Backfill the artist when the album was first created from a file whose
        // album-artist frame was empty; otherwise the card stays "Unknown Artist"
        // even after the tags are fixed. An existing value is left alone, since
        // tracks within one album legitimately differ.
        if let Some(artist) = artist.map(str::trim).filter(|s| !s.is_empty()) {
            conn.execute(
                "UPDATE albums SET artist = ?1 WHERE id = ?2 AND (artist IS NULL OR TRIM(artist) = '')",
                params![artist, id],
            )?;
        }
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO albums (name, artist) VALUES (?1, ?2)",
            params![name, artist],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn insert_song(
    conn: &Connection,
    title: Option<&str>,
    artist: Option<&str>,
    album_id: Option<i64>,
    folder_id: i64,
    file_path: &str,
    filename: &str,
    duration: u32,
    lyrics: Option<&str>,
    timeline: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO songs (title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(file_path) DO UPDATE SET
            title = excluded.title,
            artist = excluded.artist,
            album_id = excluded.album_id,
            folder_id = excluded.folder_id,
            filename = excluded.filename,
            duration = excluded.duration,
            lyrics = excluded.lyrics,
            timeline = excluded.timeline",
        params![title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline],
    )?;
    Ok(())
}

pub fn update_folder_thumbnail(conn: &Connection, id: i64, thumbnail: &str, accent_color: &str) -> Result<()> {
    conn.execute(
        "UPDATE folders SET thumbnail = ?1, accent_color = ?2 WHERE id = ?3",
        params![thumbnail, accent_color, id],
    )?;
    Ok(())
}

pub fn update_album_thumbnail(conn: &Connection, id: i64, thumbnail: &str, accent_color: &str) -> Result<()> {
    conn.execute(
        "UPDATE albums SET thumbnail = ?1, accent_color = ?2 WHERE id = ?3",
        params![thumbnail, accent_color, id],
    )?;
    Ok(())
}

pub fn fetch_folders(conn: &Connection) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.name, f.path, f.thumbnail, f.accent_color, COUNT(s.id) as songs_count
         FROM folders f
         LEFT JOIN songs s ON s.folder_id = f.id
         GROUP BY f.id"
    )?;

    let folder_iter = stmt.query_map([], |row| {
        Ok(Folder {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            thumbnail: row.get(3)?,
            accent_color: row.get(4)?,
            songs_count: row.get(5)?,
        })
    })?;

    let mut folders = Vec::new();
    for folder in folder_iter {
        folders.push(folder?);
    }
    Ok(folders)
}

pub fn fetch_albums(conn: &Connection) -> Result<Vec<Album>> {
	let mut stmt = conn.prepare(
		"SELECT a.id, a.name, a.artist, a.thumbnail, a.accent_color, COUNT(s.id) as songs_count
         FROM albums a
         INNER JOIN songs s ON s.album_id = a.id
         GROUP BY a.id"
	)?;

    let album_iter = stmt.query_map([], |row| {
        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            artist: row.get(2)?,
            thumbnail: row.get(3)?,
            accent_color: row.get(4)?,
            songs_count: row.get(5)?,
        })
    })?;

    let mut albums = Vec::new();
    for album in album_iter {
        albums.push(album?);
    }
    Ok(albums)
}

pub fn fetch_songs_by_folder(conn: &Connection, folder_id: i64) -> Result<Vec<Song>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline
         FROM songs WHERE folder_id = ?1 ORDER BY filename ASC"
    )?;

    let song_iter = stmt.query_map(params![folder_id], |row| {
        Ok(Song {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album_id: row.get(3)?,
            folder_id: row.get(4)?,
            file_path: row.get(5)?,
            filename: row.get(6)?,
            duration: row.get(7)?,
            lyrics: row.get(8)?,
            timeline: row.get(9)?,
        })
    })?;

    let mut songs = Vec::new();
    for song in song_iter {
        songs.push(song?);
    }
    Ok(songs)
}

pub fn fetch_songs_by_album(conn: &Connection, album_id: i64) -> Result<Vec<Song>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline
         FROM songs WHERE album_id = ?1 ORDER BY filename ASC"
    )?;

    let song_iter = stmt.query_map(params![album_id], |row| {
        Ok(Song {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album_id: row.get(3)?,
            folder_id: row.get(4)?,
            file_path: row.get(5)?,
            filename: row.get(6)?,
            duration: row.get(7)?,
            lyrics: row.get(8)?,
            timeline: row.get(9)?,
        })
    })?;

    let mut songs = Vec::new();
    for song in song_iter {
        songs.push(song?);
    }
    Ok(songs)
}

pub fn search_songs(conn: &Connection, query: &str) -> Result<Vec<SongSearchResult>> {
    let clean_query = normalize_string(query);
    let sql_query = format!("%{}%", clean_query);
    let mut stmt = conn.prepare(
        "SELECT s.id, s.title, s.artist, s.album_id, a.name as album_name, s.folder_id, s.file_path, s.filename, s.duration, s.lyrics, s.timeline
         FROM songs s
         LEFT JOIN albums a ON s.album_id = a.id
         WHERE normalize_str(s.title) LIKE ?1 
            OR normalize_str(s.artist) LIKE ?1 
            OR normalize_str(s.filename) LIKE ?1 
            OR normalize_str(a.name) LIKE ?1
         ORDER BY CASE WHEN normalize_str(s.title) LIKE ?1 THEN 0 ELSE 1 END, s.title ASC, s.filename ASC
         LIMIT 50"
    )?;

    let song_iter = stmt.query_map(params![sql_query], |row| {
        Ok(SongSearchResult {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album_id: row.get(3)?,
            album_name: row.get(4)?,
            folder_id: row.get(5)?,
            file_path: row.get(6)?,
            filename: row.get(7)?,
            duration: row.get(8)?,
            lyrics: row.get(9)?,
            timeline: row.get(10)?,
        })
    })?;

    let mut songs = Vec::new();
    for song in song_iter {
        songs.push(song?);
    }
    Ok(songs)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub songs_count: i32,
}

pub fn fetch_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.color, COUNT(ps.song_id) as songs_count
         FROM playlists p
         LEFT JOIN playlist_songs ps ON ps.playlist_id = p.id
         GROUP BY p.id
         ORDER BY p.name ASC"
    )?;

    let playlist_iter = stmt.query_map([], |row| {
        Ok(Playlist {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            songs_count: row.get(3)?,
        })
    })?;

    let mut playlists = Vec::new();
    for playlist in playlist_iter {
        playlists.push(playlist?);
    }
    Ok(playlists)
}

pub fn create_playlist(conn: &Connection, name: &str, color: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO playlists (name, color) VALUES (?1, ?2)",
        params![name, color],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_song_to_playlist(conn: &Connection, playlist_id: i64, song_id: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO playlist_songs (playlist_id, song_id) VALUES (?1, ?2)",
        params![playlist_id, song_id],
    )?;
    Ok(())
}

pub fn remove_song_from_playlist(conn: &Connection, playlist_id: i64, song_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM playlist_songs WHERE playlist_id = ?1 AND song_id = ?2",
        params![playlist_id, song_id],
    )?;
    Ok(())
}

pub fn fetch_songs_by_playlist(conn: &Connection, playlist_id: i64) -> Result<Vec<Song>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.title, s.artist, s.album_id, s.folder_id, s.file_path, s.filename, s.duration, s.lyrics, s.timeline
         FROM songs s
         INNER JOIN playlist_songs ps ON ps.song_id = s.id
         WHERE ps.playlist_id = ?1
         ORDER BY s.filename ASC"
    )?;

    let song_iter = stmt.query_map(params![playlist_id], |row| {
        Ok(Song {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album_id: row.get(3)?,
            folder_id: row.get(4)?,
            file_path: row.get(5)?,
            filename: row.get(6)?,
            duration: row.get(7)?,
            lyrics: row.get(8)?,
            timeline: row.get(9)?,
        })
    })?;

    let mut songs = Vec::new();
    for song in song_iter {
        songs.push(song?);
    }
    Ok(songs)
}

// ---------------------------------------------------------------------------
// Watched roots
// ---------------------------------------------------------------------------

use crate::paths::is_under;

pub fn add_watched_root(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO watched_roots (path) VALUES (?1)",
        params![path],
    )?;
    // Picking a parent of something already watched makes the old entry redundant;
    // leaving both would register overlapping recursive watches for the same tree.
    let existing = fetch_watched_roots_raw(conn)?;
    for other in existing {
        if other != path && is_under(&other, path) {
            conn.execute("DELETE FROM watched_roots WHERE path = ?1", params![other])?;
        }
    }
    Ok(())
}

fn fetch_watched_roots_raw(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM watched_roots ORDER BY path")?;
    let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    iter.collect()
}

/// Roots to hand the watcher, with nested entries collapsed into their ancestor.
///
/// Seeds itself from `folders` the first time it runs, so a library imported
/// before the watcher existed still gets watched without a re-import.
pub fn fetch_watched_roots(conn: &Connection) -> Result<Vec<String>> {
    let mut roots = fetch_watched_roots_raw(conn)?;

    if roots.is_empty() {
        let mut stmt = conn.prepare("SELECT DISTINCT path FROM folders")?;
        let folders: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<_>>()?;
        for path in &folders {
            conn.execute(
                "INSERT OR IGNORE INTO watched_roots (path) VALUES (?1)",
                params![path],
            )?;
        }
        roots = folders;
    }

    // Drop any root contained in another so each tree is watched exactly once.
    let mut deduped: Vec<String> = Vec::new();
    for candidate in roots {
        if deduped.iter().any(|kept| is_under(&candidate, kept)) {
            continue;
        }
        deduped.retain(|kept| !is_under(kept, &candidate));
        deduped.push(candidate);
    }
    Ok(deduped)
}

/// Deletes a song row by its path, returning whether anything was removed.
pub fn delete_song_by_path(conn: &Connection, file_path: &str) -> Result<bool> {
    let affected = conn.execute("DELETE FROM songs WHERE file_path = ?1", params![file_path])?;
    Ok(affected > 0)
}

/// A file's modification time in milliseconds since the epoch, or `None` when it
/// cannot be read (missing file, or a filesystem that does not report one).
pub fn file_mtime(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(since_epoch.as_millis()).ok()
}

/// Stores the file's current mtime on its row, marking the row as in step with
/// the file. Call after every write GrooveX makes to a track's tags.
pub fn record_file_mtime(conn: &Connection, file_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE songs SET file_mtime = ?1 WHERE file_path = ?2",
        params![file_mtime(Path::new(file_path)), file_path],
    )?;
    Ok(())
}

/// Every song's path with the mtime recorded when it was last indexed.
pub fn fetch_song_mtimes(conn: &Connection) -> Result<Vec<(String, Option<i64>)>> {
    let mut stmt = conn.prepare("SELECT file_path, file_mtime FROM songs")?;
    let iter = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    iter.collect()
}

pub fn song_exists(conn: &Connection, file_path: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM songs WHERE file_path = ?1",
        params![file_path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE watched_roots (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL);",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT UNIQUE NOT NULL, thumbnail TEXT, accent_color TEXT);",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn recorded_mtime_tracks_the_file_on_disk() {
        let dir = std::env::temp_dir().join("groovex_mtime_test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let conn = init_db(&dir.join("test.db")).unwrap();

        let track = dir.join("track.mp3");
        std::fs::write(&track, b"ID3").unwrap();
        let track_str = track.to_string_lossy().to_string();
        let folder_id = get_or_create_folder(&conn, "dir", &dir.to_string_lossy()).unwrap();
        insert_song(&conn, None, None, None, folder_id, &track_str, "track.mp3", 0, None, None)
            .unwrap();

        // A freshly inserted row has no mtime until one is recorded.
        assert_eq!(fetch_song_mtimes(&conn).unwrap(), vec![(track_str.clone(), None)]);

        record_file_mtime(&conn, &track_str).unwrap();
        let recorded = fetch_song_mtimes(&conn).unwrap()[0].1;
        assert!(recorded.is_some());
        assert_eq!(recorded, file_mtime(&track));

        // An external edit moves the mtime away from what was recorded.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&track)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_ne!(file_mtime(&track), recorded);

        drop(conn);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adding_a_parent_root_absorbs_its_children() {
        let conn = memory_db();
        add_watched_root(&conn, r"C:\Music\Pop").unwrap();
        add_watched_root(&conn, r"C:\Music\K Pop").unwrap();
        assert_eq!(fetch_watched_roots(&conn).unwrap().len(), 2);

        // Importing the parent makes both child watches redundant.
        add_watched_root(&conn, r"C:\Music").unwrap();
        assert_eq!(fetch_watched_roots(&conn).unwrap(), vec![r"C:\Music".to_string()]);
    }

    #[test]
    fn adding_a_child_of_an_existing_root_is_collapsed_on_read() {
        let conn = memory_db();
        add_watched_root(&conn, r"C:\Music").unwrap();
        add_watched_root(&conn, r"C:\Music\Pop").unwrap();

        // Watching one tree twice would register overlapping recursive watches.
        assert_eq!(fetch_watched_roots(&conn).unwrap(), vec![r"C:\Music".to_string()]);
    }

    #[test]
    fn roots_seed_from_existing_folders_on_first_read() {
        let conn = memory_db();
        for path in [r"C:\Music\Pop", r"C:\Music\K Pop"] {
            conn.execute(
                "INSERT INTO folders (name, path) VALUES ('x', ?1)",
                params![path],
            )
            .unwrap();
        }

        // A library imported before the watcher existed must still get watched.
        let mut roots = fetch_watched_roots(&conn).unwrap();
        roots.sort();
        assert_eq!(roots, vec![r"C:\Music\K Pop".to_string(), r"C:\Music\Pop".to_string()]);

        // Seeding writes the rows rather than deriving them each time, so a later
        // read is driven by watched_roots and not by whatever folders now contains.
        conn.execute("DELETE FROM folders", []).unwrap();
        let mut again = fetch_watched_roots(&conn).unwrap();
        again.sort();
        assert_eq!(again, roots);
    }

    #[test]
    fn unrelated_roots_are_all_kept() {
        let conn = memory_db();
        add_watched_root(&conn, r"C:\Music").unwrap();
        add_watched_root(&conn, r"D:\Archive\Audio").unwrap();

        let mut roots = fetch_watched_roots(&conn).unwrap();
        roots.sort();
        assert_eq!(roots, vec![r"C:\Music".to_string(), r"D:\Archive\Audio".to_string()]);
    }
}
