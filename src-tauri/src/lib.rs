// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use base64::prelude::*;
use lofty::prelude::*;
use lofty::probe::Probe;
use rusqlite::Connection;
use tauri::{Manager, State, Emitter};
use id3::TagLike;

mod db;
mod paths;
mod tags;
mod watcher;

struct DbState {
    conn: Mutex<Connection>,
}
const CLEAR_DB_ON_START: bool = false;
fn clean_metadata_string(s: &str) -> String {
    s.replace('\0', "")
     .replace('\r', "")
     .replace('\n', "")
     .replace('\u{a0}', " ")
     .trim()
     .to_string()
}

fn clean_multiline_metadata_string(s: &str) -> String {
    s.replace('\0', "")
     .replace("\r\n", "\n")
     .replace('\r', "\n")
     .replace('\u{a0}', " ")
     .trim()
     .to_string()
}

#[derive(serde::Serialize)]
struct CategoriesResponse {
    folders: Vec<db::Folder>,
    albums: Vec<db::Album>,
}

// Scans recursively for audio files
fn scan_directory(path: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                scan_directory(&entry_path, files);
            } else if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "mp3" || ext_lower == "m4a" || ext_lower == "flac" || ext_lower == "wav" {
                    files.push(entry_path);
                }
            }
        }
    }
}

#[tauri::command]
fn get_music_categories(state: State<'_, DbState>) -> Result<CategoriesResponse, String> {
    let conn = state.conn.lock().unwrap();
    let folders = db::fetch_folders(&conn).map_err(|e| e.to_string())?;
    let albums = db::fetch_albums(&conn).map_err(|e| e.to_string())?;
    Ok(CategoriesResponse { folders, albums })
}

#[derive(serde::Serialize)]
struct ImportResponse {
    folders: Vec<db::Folder>,
    albums: Vec<db::Album>,
    added_count: usize,
    removed_count: usize,
}

use paths::is_under_path as is_subpath;

/// Reads one audio file's tags and writes the resulting row, folder and album.
///
/// Shared by the import scan and the folder watcher so a file picked up
/// automatically is indexed exactly like one found by an explicit import.
fn index_song_file(conn: &Connection, file: &Path) -> Result<(), String> {
    let file_path_str = file.to_string_lossy().to_string();
    let filename = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let parent_path = file.parent().ok_or_else(|| "File has no parent".to_string())?;
    let parent_name = parent_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Folder".to_string());
    let folder_id = db::get_or_create_folder(conn, &parent_name, &parent_path.to_string_lossy())
        .map_err(|e| e.to_string())?;

    // Read the tags through the same path the editor uses, so what the library
    // groups by is exactly what the metadata dialog shows.
    let meta = tags::read_metadata(file);
    let duration = Probe::open(file)
        .and_then(|p| p.read())
        .map(|t| t.properties().duration().as_secs() as u32)
        .unwrap_or(0);

    // TIMELINE is a GrooveX-only frame, so it is still read directly.
    let mut timeline = None;
    if let Ok(tagged_file) = Probe::open(file).and_then(|p| p.read()) {
        for tag in tagged_file.tags() {
            if let Some(item) = tag.get(&lofty::tag::ItemKey::Unknown("TIMELINE".to_string())) {
                timeline = Some(clean_multiline_metadata_string(item.value().text().unwrap_or("")));
                break;
            }
        }
    }
    if timeline.is_none() {
        if let Ok(tag) = id3::Tag::read_from_path(file) {
            for txxx in tag.extended_texts() {
                if txxx.description == "TIMELINE" {
                    timeline = Some(clean_multiline_metadata_string(&txxx.value));
                    break;
                }
            }
        }
    }

    // Group by album artist when present: it is the key iTunes, Plex and the
    // Windows shell use, and falling back to the per-track artist is what splits
    // one album into one entry per featured artist.
    let grouping_artist = meta
        .album_artist
        .as_deref()
        .or(meta.artist.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let album_id = match meta
        .album_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(name) => {
            Some(db::get_or_create_album(conn, name, grouping_artist).map_err(|e| e.to_string())?)
        }
        None => None,
    };

    db::insert_song(
        conn,
        meta.title.as_deref(),
        meta.artist.as_deref(),
        album_id,
        folder_id,
        &file_path_str,
        &filename,
        duration,
        meta.lyrics.as_deref(),
        timeline.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // Seed the folder and album covers from the first track that has one.
    if let Some(thumb) = meta.thumbnail.as_deref().filter(|t| !t.is_empty()) {
        let folder_thumb: Option<String> = conn
            .query_row(
                "SELECT thumbnail FROM folders WHERE id = ?1",
                [folder_id],
                |row| row.get(0),
            )
            .unwrap_or(None);
        if folder_thumb.is_none() {
            db::update_folder_thumbnail(conn, folder_id, thumb, "#06b6d4").ok();
        }

        if let Some(a_id) = album_id {
            let album_thumb: Option<String> = conn
                .query_row("SELECT thumbnail FROM albums WHERE id = ?1", [a_id], |row| {
                    row.get(0)
                })
                .unwrap_or(None);
            if album_thumb.is_none() {
                db::update_album_thumbnail(conn, a_id, thumb, "#06b6d4").ok();
            }
        }
    }

    Ok(())
}

#[tauri::command]
fn import_music_folder(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<ImportResponse, String> {
    // Open native folder dialog
    let folder_path = rfd::FileDialog::new()
        .set_title("Select Music Folder")
        .pick_folder();

    let folder_path = match folder_path {
        Some(p) => p,
        None => {
            let conn = state.conn.lock().unwrap();
            let folders = db::fetch_folders(&conn).map_err(|e| e.to_string())?;
            let albums = db::fetch_albums(&conn).map_err(|e| e.to_string())?;
            return Ok(ImportResponse {
                folders,
                albums,
                added_count: 0,
                removed_count: 0,
            });
        }
    };

    let conn = state.conn.lock().unwrap();

    // 1. Get existing songs belonging to this folder path
    let mut stmt = conn.prepare("SELECT id, file_path FROM songs").map_err(|e| e.to_string())?;
    let song_paths_iter = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| e.to_string())?;

    let mut existing_songs = Vec::new();
    for item in song_paths_iter {
        let (id, file_path) = item.map_err(|e| e.to_string())?;
        if is_subpath(Path::new(&file_path), &folder_path) {
            existing_songs.push((id, file_path));
        }
    }
    // Releases the borrow so the connection can be unlocked before the watcher runs.
    drop(stmt);

    let existing_paths_set: std::collections::HashSet<String> = existing_songs
        .iter()
        .map(|(_, path)| path.clone())
        .collect();

    // 2. Scan files on disk
    let mut files = Vec::new();
    scan_directory(&folder_path, &mut files);

    let scanned_paths_set: std::collections::HashSet<String> = files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    // 3. Track added & removed count
    let mut added_count = 0;
    for file in &files {
        let path_str = file.to_string_lossy().to_string();
        if !existing_paths_set.contains(&path_str) {
            added_count += 1;
        }
    }

    let mut removed_count = 0;
    for (id, file_path) in &existing_songs {
        if !scanned_paths_set.contains(file_path) {
            conn.execute("DELETE FROM songs WHERE id = ?1", [*id]).ok();
            removed_count += 1;
        }
    }

    // 4. Import / update scanned files
    for file in &files {
        index_song_file(&conn, file).map_err(|e| e.to_string())?;
    }

    // Remember the picked folder so the watcher keeps this tree in sync. Storing the
    // root rather than each file's parent is what lets a brand new subdirectory be
    // picked up without a re-import.
    db::add_watched_root(&conn, &folder_path.to_string_lossy()).map_err(|e| e.to_string())?;


    // Clean up empty categories to avoid displaying ghost items in the UI
    conn.execute("DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)", []).ok();
    conn.execute("DELETE FROM folders WHERE id NOT IN (SELECT DISTINCT folder_id FROM songs)", []).ok();

    // Return fresh list of folders and albums
    let folders = db::fetch_folders(&conn).map_err(|e| e.to_string())?;
    let albums = db::fetch_albums(&conn).map_err(|e| e.to_string())?;

    // The watcher takes the same database lock, so release it before re-arming.
    drop(conn);
    if let Err(e) = watcher::restart(&app) {
        eprintln!("Failed to restart library watcher: {e}");
    }

    Ok(ImportResponse { folders, albums, added_count, removed_count })
}

#[tauri::command]
fn get_category_songs(
    state: State<'_, DbState>,
    category_type: String,
    category_id: i64,
) -> Result<Vec<db::Song>, String> {
    let conn = state.conn.lock().unwrap();
    if category_type == "folder" {
        db::fetch_songs_by_folder(&conn, category_id).map_err(|e| e.to_string())
    } else {
        db::fetch_songs_by_album(&conn, category_id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn search_songs(
    state: State<'_, DbState>,
    query: String,
) -> Result<Vec<db::SongSearchResult>, String> {
    let conn = state.conn.lock().unwrap();
    db::search_songs(&conn, &query).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_music_thumbnail(
    state: State<'_, DbState>,
    category_type: String,
    category_id: i64,
) -> Result<String, String> {
    // Open native file dialog for image picking
    let image_path = rfd::FileDialog::new()
        .set_title("Select Cover Image")
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .pick_file();

    let image_path = match image_path {
        Some(p) => p,
        None => return Err("Cancelled".to_string()),
    };

    // Read file as base64
    let bytes = std::fs::read(&image_path).map_err(|e| e.to_string())?;
    let b64 = BASE64_STANDARD.encode(bytes);
    
    // Guess mime type
    let ext = image_path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    
    let thumbnail_data = format!("data:{};base64,{}", mime, b64);
    
    // Update SQLite database
    let conn = state.conn.lock().unwrap();
    if category_type == "folder" {
        db::update_folder_thumbnail(&conn, category_id, &thumbnail_data, "#06b6d4")
            .map_err(|e| e.to_string())?;
    } else {
        db::update_album_thumbnail(&conn, category_id, &thumbnail_data, "#06b6d4")
            .map_err(|e| e.to_string())?;
    }
    
    Ok(thumbnail_data)
}

#[tauri::command]
fn update_category_accent_color(
    state: State<'_, DbState>,
    category_type: String,
    category_id: i64,
    accent_color: String,
) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    if category_type == "folder" {
        conn.execute(
            "UPDATE folders SET accent_color = ?1 WHERE id = ?2",
            rusqlite::params![accent_color, category_id],
        ).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE albums SET accent_color = ?1 WHERE id = ?2",
            rusqlite::params![accent_color, category_id],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn update_song_lyrics(
    state: State<'_, DbState>,
    song_id: i64,
    lyrics: String,
) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();

    // 1. Get the file path of the song from the database
    let mut stmt = conn.prepare("SELECT file_path FROM songs WHERE id = ?1").map_err(|e| e.to_string())?;
    let file_path_str: String = stmt.query_row([song_id], |row| row.get(0)).map_err(|_| "Song not found in database".to_string())?;
    let path = Path::new(&file_path_str);

    // 2. Try to write lyrics to the file tag on disk
    if path.exists() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "mp3" {
            // Use ID3 crate for MP3 files since it is mature and supports USLT natively
            let mut tag = id3::Tag::read_from_path(path).unwrap_or_else(|_| id3::Tag::new());
            // Remove existing USLT frames to avoid duplicates
            tag.remove("USLT");
            let lyrics_frame = id3::frame::Lyrics {
                lang: "eng".to_string(),
                description: "".to_string(),
                text: lyrics.clone(),
            };
            tag.add_frame(id3::Frame::with_content("USLT", id3::Content::Lyrics(lyrics_frame)));
            if let Err(e) = tag.write_to_path(path, id3::Version::Id3v24) {
                eprintln!("Failed to write ID3 lyrics tag: {}", e);
            }
        } else {
            // For other formats (FLAC, M4A, WAV, etc.), use lofty
            if let Ok(mut tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                // Try primary tag first
                if let Some(tag) = tagged_file.primary_tag_mut() {
                    tag.insert_text(lofty::tag::ItemKey::Lyrics, lyrics.clone());
                } else if let Some(tag) = tagged_file.first_tag_mut() {
                    tag.insert_text(lofty::tag::ItemKey::Lyrics, lyrics.clone());
                } else {
                    // Create a new tag if none exists (using the primary tag type)
                    let tag_type = tagged_file.primary_tag_type();
                    let mut new_tag = lofty::tag::Tag::new(tag_type);
                    new_tag.insert_text(lofty::tag::ItemKey::Lyrics, lyrics.clone());
                    tagged_file.insert_tag(new_tag);
                }

                if let Err(e) = tagged_file.save_to_path(path, lofty::config::WriteOptions::default()) {
                    eprintln!("Failed to save lofty lyrics tag to path: {}", e);
                }
            }
        }
    }

    // 3. Update the lyrics in the database
    conn.execute(
        "UPDATE songs SET lyrics = ?1 WHERE id = ?2",
        rusqlite::params![lyrics, song_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn update_song_timeline(
    state: State<'_, DbState>,
    song_id: i64,
    timeline: String,
) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();

    // 1. Get the file path of the song from the database
    let mut stmt = conn.prepare("SELECT file_path FROM songs WHERE id = ?1").map_err(|e| e.to_string())?;
    let file_path_str: String = stmt.query_row([song_id], |row| row.get(0)).map_err(|_| "Song not found in database".to_string())?;
    let path = Path::new(&file_path_str);

    // 2. Try to write timeline to the file tag on disk
    if path.exists() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "mp3" {
            // Use ID3 custom text frame (TXXX) with description "TIMELINE"
            let mut tag = id3::Tag::read_from_path(path).unwrap_or_else(|_| id3::Tag::new());
            tag.remove_extended_text(Some("TIMELINE"), None);
            tag.add_frame(id3::Frame::with_content("TXXX", id3::Content::ExtendedText(id3::frame::ExtendedText {
                description: "TIMELINE".to_string(),
                value: timeline.clone(),
            })));
            if let Err(e) = tag.write_to_path(path, id3::Version::Id3v24) {
                eprintln!("Failed to write ID3 timeline tag: {}", e);
            }
        } else {
            // Use lofty custom field
            if let Ok(mut tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                let key = lofty::tag::ItemKey::Unknown("TIMELINE".to_string());
                if let Some(tag) = tagged_file.primary_tag_mut() {
                    tag.insert_text(key, timeline.clone());
                } else if let Some(tag) = tagged_file.first_tag_mut() {
                    tag.insert_text(key, timeline.clone());
                } else {
                    let tag_type = tagged_file.primary_tag_type();
                    let mut new_tag = lofty::tag::Tag::new(tag_type);
                    new_tag.insert_text(key, timeline.clone());
                    tagged_file.insert_tag(new_tag);
                }

                if let Err(e) = tagged_file.save_to_path(path, lofty::config::WriteOptions::default()) {
                    eprintln!("Failed to save lofty timeline tag to path: {}", e);
                }
            }
        }
    }

    // 3. Update the timeline in the database
    conn.execute(
        "UPDATE songs SET timeline = ?1 WHERE id = ?2",
        rusqlite::params![timeline, song_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
#[derive(serde::Serialize)]
struct FullMetadata {
    filename: String,
    file_path: String,
    #[serde(flatten)]
    fields: tags::MetadataPayload,
    /// App-only field, stored in the database rather than in the file.
    timeline: Option<String>,
    /// Every frame actually present in the file, junk included.
    raw_frames: Vec<tags::RawFrame>,
}

#[tauri::command]
fn get_song_metadata(state: State<'_, DbState>, song_id: i64) -> Result<FullMetadata, String> {
    let conn = state.conn.lock().unwrap();

    let mut stmt = conn
        .prepare("SELECT file_path, filename, timeline FROM songs WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let (file_path_str, filename, timeline) = stmt
        .query_row([song_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|_| "Song not found in database".to_string())?;

    let path = Path::new(&file_path_str);

    // The file on disk is the single source of truth. The database is only a cache
    // of it, and reading the cache here is what made the dialog show a stale album
    // after the tags had been edited in another program.
    let fields = tags::read_metadata(path);
    let raw_frames = tags::read_raw_frames(path);

    Ok(FullMetadata {
        filename,
        file_path: file_path_str,
        fields,
        timeline,
        raw_frames,
    })
}

/// Re-reads one song's tags from disk and updates its database row, creating or
/// reassigning its album as needed. Returns the resolved album id.
fn sync_song_row(
    conn: &Connection,
    song_id: i64,
    file_path: &str,
    filename: &str,
) -> Result<Option<i64>, String> {
    let meta = tags::read_metadata(Path::new(file_path));

    let album_name = meta
        .album_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // Group by album artist when the file has one; it is the key every other
    // player uses, so mirroring it keeps GrooveX's grouping consistent with them.
    let grouping_artist = meta
        .album_artist
        .as_deref()
        .or(meta.artist.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let album_id = match album_name {
        Some(name) => Some(
            db::get_or_create_album(conn, name, grouping_artist).map_err(|e| e.to_string())?,
        ),
        None => None,
    };

    conn.execute(
        "UPDATE songs SET title = ?1, artist = ?2, album_id = ?3, filename = ?4, file_path = ?5 WHERE id = ?6",
        rusqlite::params![
            meta.title,
            meta.artist,
            album_id,
            filename,
            file_path,
            song_id
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(album_id)
}

fn prune_empty_categories(conn: &Connection) {
    conn.execute(
        "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)",
        [],
    )
    .ok();
    conn.execute(
        "DELETE FROM folders WHERE id NOT IN (SELECT DISTINCT folder_id FROM songs)",
        [],
    )
    .ok();
}

#[derive(serde::Serialize)]
struct RescanResult {
    scanned: usize,
    missing: usize,
}

/// Re-reads tags from disk for the given songs, or for the whole library when
/// `song_ids` is omitted. This is the repair path for rows that went stale
/// because the files were retagged outside the app.
#[tauri::command]
fn rescan_songs(
    state: State<'_, DbState>,
    song_ids: Option<Vec<i64>>,
) -> Result<RescanResult, String> {
    let conn = state.conn.lock().unwrap();

    let rows: Vec<(i64, String, String)> = match &song_ids {
        Some(ids) if !ids.is_empty() => {
            let placeholders = vec!["?"; ids.len()].join(",");
            let sql = format!(
                "SELECT id, file_path, filename FROM songs WHERE id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let params = rusqlite::params_from_iter(ids.iter());
            let iter = stmt
                .query_map(params, |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(|e| e.to_string())?;
            iter.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        }
        _ => {
            let mut stmt = conn
                .prepare("SELECT id, file_path, filename FROM songs")
                .map_err(|e| e.to_string())?;
            let iter = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(|e| e.to_string())?;
            iter.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        }
    };

    let mut scanned = 0usize;
    let mut missing = 0usize;
    for (id, file_path, filename) in rows {
        if !Path::new(&file_path).exists() {
            missing += 1;
            continue;
        }
        if sync_song_row(&conn, id, &file_path, &filename).is_ok() {
            scanned += 1;
        }
    }

    prune_empty_categories(&conn);
    Ok(RescanResult { scanned, missing })
}

#[tauri::command]
fn update_song_metadata(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    song_id: i64,
    filename: Option<String>,
    metadata: tags::MetadataPayload,
    mode: Option<String>,
) -> Result<db::Song, String> {
    let conn = state.conn.lock().unwrap();
    let write_mode = tags::WriteMode::from_str(mode.as_deref().unwrap_or("clean_keep"));

    let mut stmt = conn
        .prepare("SELECT file_path, folder_id, filename, duration, lyrics, timeline FROM songs WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let (file_path_str, folder_id, original_filename, duration, db_lyrics, timeline) = stmt
        .query_row([song_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|_| "Song not found in database".to_string())?;
    drop(stmt);

    let path = Path::new(&file_path_str);
    let mut current_path = path.to_path_buf();
    let mut final_file_path_str = file_path_str.clone();
    let mut final_filename = original_filename.clone();

    if let Some(ref new_name) = filename {
        let trimmed_name = new_name.trim();
        if !trimmed_name.is_empty() && trimmed_name != original_filename {
            if let Some(parent) = path.parent() {
                let new_path = parent.join(trimmed_name);
                std::fs::rename(path, &new_path)
                    .map_err(|e| format!("Failed to rename file: {e}"))?;
                current_path = new_path;
                final_file_path_str = current_path.to_string_lossy().to_string();
                final_filename = trimmed_name.to_string();
            }
        }
    }

    // Claim both paths before writing so the watcher does not treat this edit
    // (or the rename that preceded it) as an external change.
    watcher::note_self_write(&app, path);
    watcher::note_self_write(&app, &current_path);
    tags::write_metadata(&current_path, &metadata, write_mode)?;

    // Read the file back rather than trusting the payload, so the database always
    // reflects what a different player would see.
    let album_id = sync_song_row(&conn, song_id, &final_file_path_str, &final_filename)?;
    let written = tags::read_metadata(&current_path);

    if let Some(thumb) = written.thumbnail.as_deref().filter(|t| !t.is_empty()) {
        db::update_folder_thumbnail(&conn, folder_id, thumb, "#06b6d4").ok();
        if let Some(a_id) = album_id {
            db::update_album_thumbnail(&conn, a_id, thumb, "#06b6d4").ok();
        }
    }

    prune_empty_categories(&conn);

    Ok(db::Song {
        id: song_id,
        title: written.title,
        artist: written.artist,
        album_id,
        folder_id,
        file_path: final_file_path_str,
        filename: final_filename,
        duration,
        lyrics: written.lyrics.or(db_lyrics),
        timeline,
    })
}

#[derive(serde::Serialize)]
struct BulkResult {
    updated: usize,
    failed: Vec<String>,
}

/// Applies the same fields to many songs at once.
///
/// This is what fixes an album that other players split apart: set `album_name`
/// and `album_artist` (and usually `compilation`) once, and every track ends up
/// with the identical grouping key. In the default `clean_keep` mode each file's
/// own title and track number survive while junk frames are dropped.
#[tauri::command]
fn apply_metadata_to_songs(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    song_ids: Vec<i64>,
    metadata: tags::MetadataPayload,
    mode: Option<String>,
) -> Result<BulkResult, String> {
    let conn = state.conn.lock().unwrap();
    let write_mode = tags::WriteMode::from_str(mode.as_deref().unwrap_or("clean_keep"));

    let placeholders = vec!["?"; song_ids.len()].join(",");
    if song_ids.is_empty() {
        return Ok(BulkResult {
            updated: 0,
            failed: Vec::new(),
        });
    }

    let sql = format!("SELECT id, file_path, filename FROM songs WHERE id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map(rusqlite::params_from_iter(song_ids.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut updated = 0usize;
    let mut failed = Vec::new();

    for (id, file_path, filename) in rows {
        let path = Path::new(&file_path);
        // A bulk pass rewrites every file in the album; without this the watcher
        // would re-index all of them and refresh the UI once per track.
        watcher::note_self_write(&app, path);
        match tags::write_metadata(path, &metadata, write_mode) {
            Ok(()) => {
                sync_song_row(&conn, id, &file_path, &filename).ok();
                updated += 1;
            }
            Err(e) => failed.push(format!("{filename}: {e}")),
        }
    }

    prune_empty_categories(&conn);
    Ok(BulkResult { updated, failed })
}

/// Song ids belonging to a folder or an album, so the UI can offer
/// "apply to every track in this album" without shipping the whole list around.
#[tauri::command]
fn get_category_song_ids(
    state: State<'_, DbState>,
    category_type: String,
    category_id: i64,
) -> Result<Vec<i64>, String> {
    let conn = state.conn.lock().unwrap();
    let sql = if category_type == "folder" {
        "SELECT id FROM songs WHERE folder_id = ?1 ORDER BY filename"
    } else {
        "SELECT id FROM songs WHERE album_id = ?1 ORDER BY filename"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let iter = stmt
        .query_map([category_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    iter.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_category(
    state: State<'_, DbState>,
    category_type: String,
    category_id: i64,
) -> Result<CategoriesResponse, String> {
    let conn = state.conn.lock().unwrap();
    if category_type == "folder" {
        conn.execute("DELETE FROM folders WHERE id = ?1", rusqlite::params![category_id])
            .map_err(|e| e.to_string())?;
    } else {
        conn.execute("DELETE FROM songs WHERE album_id = ?1", rusqlite::params![category_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM albums WHERE id = ?1", rusqlite::params![category_id])
            .map_err(|e| e.to_string())?;
    }

    conn.execute("DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)", []).ok();
    conn.execute("DELETE FROM folders WHERE id NOT IN (SELECT DISTINCT folder_id FROM songs)", []).ok();

    let folders = db::fetch_folders(&conn).map_err(|e| e.to_string())?;
    let albums = db::fetch_albums(&conn).map_err(|e| e.to_string())?;
    Ok(CategoriesResponse { folders, albums })
}

#[tauri::command]
fn get_playlists(state: State<'_, DbState>) -> Result<Vec<db::Playlist>, String> {
    let conn = state.conn.lock().unwrap();
    db::fetch_playlists(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_playlist(state: State<'_, DbState>, name: String, color: String) -> Result<i64, String> {
    let conn = state.conn.lock().unwrap();
    db::create_playlist(&conn, &name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_playlist(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::delete_playlist(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_song_to_playlist(state: State<'_, DbState>, playlist_id: i64, song_id: i64) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::add_song_to_playlist(&conn, playlist_id, song_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_song_from_playlist(state: State<'_, DbState>, playlist_id: i64, song_id: i64) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::remove_song_from_playlist(&conn, playlist_id, song_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_playlist_songs(state: State<'_, DbState>, playlist_id: i64) -> Result<Vec<db::Song>, String> {
    let conn = state.conn.lock().unwrap();
    db::fetch_songs_by_playlist(&conn, playlist_id).map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize)]
struct SingleInstancePayload {
    args: Vec<String>,
    cwd: String,
}

#[tauri::command]
fn get_song_by_path(state: State<'_, DbState>, file_path: String) -> Result<db::Song, String> {
    let conn = state.conn.lock().unwrap();
    
    // 1. Try to find the song in the database first
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline 
         FROM songs WHERE file_path = ?1"
    ).map_err(|e| e.to_string())?;
    
    let song_opt: Option<db::Song> = stmt.query_row([&file_path], |row| {
        Ok(db::Song {
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
    }).ok();
    
    if let Some(song) = song_opt {
        return Ok(song);
    }
    
    // 2. If not found in DB, parse metadata from the file on disk
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err("File does not exist".to_string());
    }
    
    let filename = path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("Unknown File")
        .to_string();
        
    let mut title = None;
    let mut artist = None;
    let mut duration = 0;
    let mut lyrics = None;
    let mut timeline = None;
    
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "mp3" {
        // Read via id3
        if let Ok(tag) = id3::Tag::read_from_path(path) {
            title = tag.title().map(|s| clean_metadata_string(&s));
            artist = tag.artist().map(|s| clean_metadata_string(&s));
            if let Some(lyric_frame) = tag.lyrics().next() {
                lyrics = Some(clean_multiline_metadata_string(&lyric_frame.text));
            }
            for txxx in tag.extended_texts() {
                if txxx.description == "TIMELINE" {
                    timeline = Some(clean_multiline_metadata_string(&txxx.value));
                    break;
                }
            }
        }
        
        // Duration from lofty
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            duration = tagged_file.properties().duration().as_secs() as u32;
        }
    } else {
        // Read via lofty
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            duration = tagged_file.properties().duration().as_secs() as u32;
            
            for tag in tagged_file.tags() {
                if title.is_none() {
                    title = tag.title().map(|s| clean_metadata_string(&s));
                }
                if artist.is_none() {
                    artist = tag.artist().map(|s| clean_metadata_string(&s));
                }
                if lyrics.is_none() {
                    if let Some(item) = tag.get(&lofty::tag::ItemKey::Lyrics) {
                        lyrics = Some(clean_multiline_metadata_string(item.value().text().unwrap_or("")));
                    }
                }
                if timeline.is_none() {
                    if let Some(item) = tag.get(&lofty::tag::ItemKey::Unknown("TIMELINE".to_string())) {
                        timeline = Some(clean_multiline_metadata_string(item.value().text().unwrap_or("")));
                    }
                }
            }
        }
    }
    
    // Construct a temporary Song object with id = -1 (not saved in DB)
    Ok(db::Song {
        id: -1,
        title,
        artist,
        album_id: None,
        folder_id: -1,
        file_path,
        filename,
        duration,
        lyrics,
        timeline,
    })
}

/// Tint painted behind the transparent window on Windows, as `(R, G, B, A)`.
///
/// The alpha is the only "how much of the desktop shows through" knob Aero Blur
/// exposes: 0 is fully clear, 255 fully opaque. Roughly 13 steps per 5%.
#[cfg(target_os = "windows")]
const WINDOWS_BLUR_TINT: (u8, u8, u8, u8) = (18, 18, 18, 107);

/// Material used for the macOS blur.
///
/// macOS has no alpha equivalent to [`WINDOWS_BLUR_TINT`] — the system owns the
/// translucency and only lets us name a material, so this is the knob for how
/// dark the window reads. `HudWindow` is the closest match to the dark, clearly
/// see-through tint used on Windows; `UnderWindowBackground` is more frosted and
/// `Sidebar` lighter still.
#[cfg(target_os = "macos")]
const MACOS_BLUR_MATERIAL: window_vibrancy::NSVisualEffectMaterial =
    window_vibrancy::NSVisualEffectMaterial::HudWindow;

/// Corner radius of the macOS blur view. The window is undecorated, so this view
/// is what draws the window's shape — left square, the blur spills past the
/// rounded corners macOS draws around it.
#[cfg(target_os = "macos")]
const MACOS_CORNER_RADIUS: f64 = 10.0;

/// Puts the desktop blur behind the transparent window.
///
/// Call this from the setup thread: `apply_vibrancy` asserts it is on the main
/// thread, and AppKit rejects the view anywhere else.
///
/// The macOS branch is architecture-independent — one source path covers Apple
/// Silicon and Intel, and `universal-apple-darwin` bundles both. What does differ
/// between them is the OS underneath: every arm64 Mac runs macOS 11 or newer,
/// while an Intel Mac may still be on 10.13, where the 10.14-era materials do not
/// exist. window-vibrancy handles that itself, substituting an older material
/// rather than failing, so old Intel hardware still gets a blur instead of a flat
/// window — which is why there is no version check here.
#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos")),
    allow(unused_variables)
)]
fn apply_window_effect(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        // Aero Blur provides high performance blur behind transparent windows
        if let Err(e) = window_vibrancy::apply_blur(window, Some(WINDOWS_BLUR_TINT)) {
            eprintln!("Failed to apply window blur: {e}");
        }
    }

    #[cfg(target_os = "macos")]
    {
        // `Active` rather than the default `FollowsWindowActiveState`: playback
        // carries on in the background, and letting the blur drop to flat grey
        // every time the window loses focus makes a playing app look dead.
        if let Err(e) = window_vibrancy::apply_vibrancy(
            window,
            MACOS_BLUR_MATERIAL,
            Some(window_vibrancy::NSVisualEffectState::Active),
            Some(MACOS_CORNER_RADIUS),
        ) {
            eprintln!("Failed to apply window vibrancy: {e}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Native-side boot timing. The web layer's own numbers start when the document
    // begins loading, so everything before that — process start, plugin init,
    // database open, WebView creation, window compositing — is invisible to it.
    // These marks cover that gap; run from a terminal to see them.
    let boot = std::time::Instant::now();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            let _ = app.emit("open-file", SingleInstancePayload {
                args: argv,
                cwd: cwd.to_string(),
            });
        }))
        .setup(move |app| {
            println!("[boot] setup entered {:?}", boot.elapsed());

            // Initialize database path in App Data directory
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            let db_path = app_data_dir.join("groovex.db");
            
            if CLEAR_DB_ON_START {
                if db_path.exists() {
                    std::fs::remove_file(&db_path).ok();
                    println!("Cleared database file on startup: {:?}", db_path);
                }
            }
            
            let conn = db::init_db(&db_path)
                .expect("Failed to initialize database");
                
            app.manage(DbState {
                conn: Mutex::new(conn),
            });
            app.manage(watcher::WatcherState::default());
            println!("[boot] database ready {:?}", boot.elapsed());

            // Start watching the imported folders so tracks added while the app is
            // running show up without a manual re-import.
            //
            // Off the setup thread on purpose: arming a recursive watch walks the
            // whole tree to build the file-id cache used for rename detection
            // (~200ms for this library, and far worse on a cold network drive).
            // Doing that here would hold up window creation for no benefit, since
            // nothing can change on disk before the user sees anything.
            let watcher_handle = app.handle().clone();
            std::thread::spawn(move || match watcher::restart(&watcher_handle) {
                Ok(roots) if !roots.is_empty() => {
                    println!("Watching {} music folder(s) for changes", roots.len());
                }
                Ok(_) => {}
                Err(e) => eprintln!("Failed to start library watcher: {e}"),
            });

            // Parse initial startup arguments (when launched via "Open with")
            let args: Vec<String> = std::env::args().collect();
            if args.len() > 1 {
                let file_path = args[1].clone();
                let path = Path::new(&file_path);
                if path.exists() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == "mp3" || ext == "flac" || ext == "wav" || ext == "m4a" || ext == "ogg" || ext == "m4r" {
                        let app_handle = app.handle().clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(800));
                            let _ = app_handle.emit("open-file", SingleInstancePayload {
                                args,
                                cwd: String::new(),
                            });
                        });
                    }
                }
            }

            // Apply window vibrancy and transparency (Aero Blur is smooth and lag-free on Windows 10/11 with transparent: true)
            let window = app.get_webview_window("main").unwrap();
            println!("[boot] window handle acquired {:?}", boot.elapsed());

            let effect_start = std::time::Instant::now();
            apply_window_effect(&window);

            println!(
                "[boot] window effect {:?} · setup done {:?}",
                effect_start.elapsed(),
                boot.elapsed()
            );

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_taskbar::init())
        .invoke_handler(tauri::generate_handler![
            get_music_categories,
            import_music_folder,
            get_category_songs,
            update_music_thumbnail,
            update_category_accent_color,
            search_songs,
            delete_category,
            update_song_lyrics,
            update_song_timeline,
            get_song_metadata,
            update_song_metadata,
            apply_metadata_to_songs,
            get_category_song_ids,
            rescan_songs,
            get_playlists,
            create_playlist,
            delete_playlist,
            add_song_to_playlist,
            remove_song_from_playlist,
            get_playlist_songs,
            get_song_by_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
