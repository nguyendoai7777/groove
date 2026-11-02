//! groove-gd1_07, C11: sink thật trong chuyển đổi DB cũ. DB giả dựng trong thư mục tạm, không đụng DB thật.

use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use groove_library::covers::{CoverSize, CoverStore, cover_hash};
use groove_library::db::migrate::{OpenOutcome, open_or_migrate};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use rusqlite::Connection;
use tempfile::TempDir;

const LEGACY_SCHEMA: &str = "
CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT UNIQUE NOT NULL,
    thumbnail TEXT, accent_color TEXT);
CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, artist TEXT,
    thumbnail TEXT, accent_color TEXT);
CREATE TABLE songs (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, artist TEXT, album_id INTEGER,
    folder_id INTEGER NOT NULL, file_path TEXT UNIQUE NOT NULL, filename TEXT NOT NULL, duration INTEGER NOT NULL,
    lyrics TEXT, timeline TEXT, file_mtime INTEGER,
    FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE SET NULL,
    FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE);
CREATE TABLE watched_roots (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL);
CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, color TEXT NOT NULL);
CREATE TABLE playlist_songs (playlist_id INTEGER NOT NULL, song_id INTEGER NOT NULL,
    FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY(song_id) REFERENCES songs(id) ON DELETE CASCADE,
    PRIMARY KEY(playlist_id, song_id));
";

#[test]
fn real_sink_turns_legacy_base64_into_files_and_a_true_accent() {
    let dir = TempDir::new().unwrap();
    let legacy = dir.path().join("groovex.db");
    let target = dir.path().join("Groove X").join("library.db");
    let mut store = CoverStore::new(dir.path().join("cache").join("covers"));

    let mut png = Vec::new();
    DynamicImage::ImageRgb8(RgbImage::from_pixel(600, 600, Rgb([40, 80, 120])))
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .unwrap();
    let uri = format!("data:image/png;base64,{}", STANDARD.encode(&png));
    let corrupt = format!("data:image/png;base64,{}", STANDARD.encode(b"not an image"));

    let old = Connection::open(&legacy).unwrap();
    old.execute_batch(LEGACY_SCHEMA).unwrap();
    old.execute(
        "INSERT INTO folders (id, name, path, thumbnail, accent_color) VALUES
             (1, 'Good', 'C:/good', ?1, '#06b6d4'), (2, 'Broken', 'C:/broken', ?2, '#06b6d4')",
        [&uri, &corrupt],
    )
    .unwrap();
    old.execute("INSERT INTO albums (id, name, thumbnail, accent_color) VALUES (1, 'Album', ?1, '#06b6d4')", [&uri])
        .unwrap();
    drop(old);

    let (conn, outcome) = open_or_migrate(&target, Some(&legacy), &mut store).unwrap();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };
    assert_eq!((report.covers_stored, report.covers_failed), (1, 1));

    let hash = cover_hash(&png);
    let cover_of = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, Option<String>>(0)).unwrap();
    assert_eq!(cover_of("SELECT cover_hash FROM folders WHERE id = 1").as_ref(), Some(&hash));
    assert_eq!(cover_of("SELECT cover_hash FROM albums WHERE id = 1").as_ref(), Some(&hash));
    assert_eq!(cover_of("SELECT cover_hash FROM folders WHERE id = 2"), None);

    let row: (String, u32, u32) = conn
        .query_row("SELECT accent_color, width, height FROM covers", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(row, ("#285078".to_owned(), 600, 600));
    assert!(store.path(&hash, CoverSize::Small).is_file());
    assert!(store.path(&hash, CoverSize::Large).is_file());
}
