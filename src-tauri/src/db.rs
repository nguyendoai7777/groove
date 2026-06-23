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
