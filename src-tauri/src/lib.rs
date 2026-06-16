// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use base64::prelude::*;
use lofty::prelude::*;
use lofty::probe::Probe;
use rusqlite::Connection;
use tauri::{Manager, State};
use id3::TagLike;

mod db;

struct DbState {
    conn: Mutex<Connection>,
}

fn clean_metadata_string(s: &str) -> String {
    s.replace('\0', "")
     .replace('\r', "")
     .replace('\n', "")
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

#[tauri::command]
fn import_music_folder(state: State<'_, DbState>) -> Result<CategoriesResponse, String> {
    // Open native folder dialog
    let folder_path = rfd::FileDialog::new()
        .set_title("Select Music Folder")
        .pick_folder();

    let folder_path = match folder_path {
        Some(p) => p,
        None => return get_music_categories(state), // User cancelled
    };

    let conn = state.conn.lock().unwrap();
    let mut files = Vec::new();
    scan_directory(&folder_path, &mut files);

    for file in files {
        let file_path_str = file.to_string_lossy().to_string();
        let filename = file.file_name().unwrap_or_default().to_string_lossy().to_string();
        
        // Parent folder info
        let parent_path = file.parent().unwrap_or(&folder_path);
        let parent_path_str = parent_path.to_string_lossy().to_string();
        let parent_name = parent_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Folder".to_string());

        let folder_id = db::get_or_create_folder(&conn, &parent_name, &parent_path_str)
            .map_err(|e| e.to_string())?;

        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_lower = ext.to_lowercase();

        let mut title = None;
        let mut artist = None;
        let mut album_name = None;
        let mut duration = 0;
        let mut thumbnail = None;

        // Try extracting metadata using lofty
        if let Ok(tagged_file) = Probe::open(&file).and_then(|p| p.read()) {
            duration = tagged_file.properties().duration().as_secs() as u32;
            
            for tag in tagged_file.tags() {
                if title.is_none() {
                    title = tag.title().map(|s| clean_metadata_string(&s));
                }
                if artist.is_none() {
                    artist = tag.artist().map(|s| clean_metadata_string(&s));
                }
                if album_name.is_none() {
                    album_name = tag.album().map(|s| clean_metadata_string(&s));
                }
                if thumbnail.is_none() {
                    if let Some(pic) = tag.pictures().first() {
                        let b64 = BASE64_STANDARD.encode(pic.data());
                        let mime = pic.mime_type().map(|m| m.to_string()).unwrap_or_else(|| "image/png".to_string());
                        thumbnail = Some(format!("data:{};base64,{}", mime, b64));
                    }
                }
            }
        }

        // Fallback to the mature `id3` crate if this is an MP3 and any metadata is missing
        if ext_lower == "mp3" && (album_name.is_none() || title.is_none() || artist.is_none() || thumbnail.is_none()) {
            if let Ok(tag) = id3::Tag::read_from_path(&file) {
                if title.is_none() {
                    title = tag.title().map(|s| clean_metadata_string(&s));
                }
                if artist.is_none() {
                    artist = tag.artist().map(|s| clean_metadata_string(&s));
                }
                if album_name.is_none() {
                    album_name = tag.album().map(|s| clean_metadata_string(&s));
                }
                if thumbnail.is_none() {
                    if let Some(pic) = tag.pictures().next() {
                        let b64 = BASE64_STANDARD.encode(&pic.data);
                        let mime = pic.mime_type.to_string();
                        thumbnail = Some(format!("data:{};base64,{}", mime, b64));
                    }
                }
            }
        }

        // Handle Album creation
        let album_id = if let Some(ref name) = album_name {
            if !name.is_empty() {
                let id = db::get_or_create_album(&conn, name, artist.as_deref())
                    .map_err(|e| e.to_string())?;
                Some(id)
            } else {
                None
            }
        } else {
            None
        };

        // Insert Song
        db::insert_song(
            &conn,
            title.as_deref(),
            artist.as_deref(),
            album_id,
            folder_id,
            &file_path_str,
            &filename,
            duration,
        ).map_err(|e| e.to_string())?;

        // If the file has a thumbnail, set it as category thumbnail if they don't have one
        if let Some(ref thumb) = thumbnail {
            // Check if folder needs thumbnail
            let mut stmt = conn.prepare("SELECT thumbnail FROM folders WHERE id = ?1").unwrap();
            let current_thumb: Option<String> = stmt.query_row([folder_id], |row| row.get(0)).unwrap_or(None);
            if current_thumb.is_none() {
                db::update_folder_thumbnail(&conn, folder_id, thumb, "#06b6d4").ok();
            }

            // Check if album needs thumbnail
            if let Some(a_id) = album_id {
                let mut stmt = conn.prepare("SELECT thumbnail FROM albums WHERE id = ?1").unwrap();
                let current_thumb: Option<String> = stmt.query_row([a_id], |row| row.get(0)).unwrap_or(None);
                if current_thumb.is_none() {
                    db::update_album_thumbnail(&conn, a_id, thumb, "#06b6d4").ok();
                }
            }
        }
    }

    // Clean up empty categories to avoid displaying ghost items in the UI
    conn.execute("DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)", []).ok();
    conn.execute("DELETE FROM folders WHERE id NOT IN (SELECT DISTINCT folder_id FROM songs)", []).ok();

    // Return fresh list of folders and albums
    let folders = db::fetch_folders(&conn).map_err(|e| e.to_string())?;
    let albums = db::fetch_albums(&conn).map_err(|e| e.to_string())?;
    Ok(CategoriesResponse { folders, albums })
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

const CLEAR_DB_ON_START: bool = true;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
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

            // Apply window vibrancy and transparency (Aero Blur is smooth and lag-free on Windows 10/11 with transparent: true)
            let window = app.get_webview_window("main").unwrap();
            
            #[cfg(target_os = "windows")]
            {
                // Aero Blur provides high performance blur behind transparent windows
                window_vibrancy::apply_blur(&window, Some((18, 18, 18, 120))).ok();
            }

            #[cfg(target_os = "macos")]
            {
                window_vibrancy::apply_vibrancy(
                    &window,
                    window_vibrancy::NSVisualEffectMaterial::Sidebar,
                    None,
                    None,
                ).ok();
            }
            
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_music_categories,
            import_music_folder,
            get_category_songs,
            update_music_thumbnail,
            update_category_accent_color,
            search_songs,
            delete_category
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
