use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
}

pub fn init_db(db_path: &Path) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    
    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON;", [])?;

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
            FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE SET NULL,
            FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE
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
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO songs (title, artist, album_id, folder_id, file_path, filename, duration)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![title, artist, album_id, folder_id, file_path, filename, duration],
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
         LEFT JOIN songs s ON s.album_id = a.id
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
        "SELECT id, title, artist, album_id, folder_id, file_path, filename, duration
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
        "SELECT id, title, artist, album_id, folder_id, file_path, filename, duration
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
        })
    })?;

    let mut songs = Vec::new();
    for song in song_iter {
        songs.push(song?);
    }
    Ok(songs)
}
