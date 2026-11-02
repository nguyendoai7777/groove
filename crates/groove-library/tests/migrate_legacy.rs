//! groove-gd1_04: chuyển DB cũ. Fixture tự dựng theo schema ở `src-tauri/src/db.rs:124-228`, dữ liệu giả hoàn toàn.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::bail;
use groove_library::db::migrate::{
    CoverRecord, CoverSink, MigrateError, OpenOutcome, migrate_legacy, open_fresh, open_or_migrate,
};
use groove_library::db::schema;
use rusqlite::{Connection, params};
use tempfile::TempDir;

/// Ảnh giả: `data:` trỏ tới byte 1, 2, 3 và 4, 5, 6.
const COVER_A: &str = "data:image/png;base64,AQID";
const COVER_B: &str = "data:image/jpeg;base64,BAUG";
const OLD_ACCENT: &str = "#06b6d4";

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

struct Fixture {
    _dir: TempDir,
    legacy: PathBuf,
    target: PathBuf,
}

impl Fixture {
    fn empty() -> Self {
        let dir = TempDir::new().unwrap();
        let legacy = dir.path().join("com.mrdxd.groovex").join("groovex.db");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        let target = dir.path().join("Groove X").join("library.db");
        Self { _dir: dir, legacy, target }
    }

    fn with_schema(sql: &str) -> Self {
        let fixture = Self::empty();
        Connection::open(&fixture.legacy).unwrap().execute_batch(sql).unwrap();
        fixture
    }

    /// Thư viện đầy đủ, `id` cố ý có lỗ để thấy được việc giữ nguyên.
    fn typical() -> Self {
        let fixture = Self::with_schema(LEGACY_SCHEMA);
        let conn = Connection::open(&fixture.legacy).unwrap();
        conn.execute_batch(&format!(
            "INSERT INTO folders (id, name, path, thumbnail, accent_color) VALUES
                 (3, 'Pop', 'C:/Music/Pop', '{COVER_A}', '{OLD_ACCENT}'),
                 (8, 'Rock', 'C:/Music/Rock', NULL, NULL);
             INSERT INTO albums (id, name, artist, thumbnail, accent_color) VALUES
                 (5, 'Tình Ca', 'Sơn Tùng', '{COVER_A}', '{OLD_ACCENT}'),
                 (9, 'Greatest Hits', NULL, '{COVER_B}', '{OLD_ACCENT}'),
                 (12, 'Empty Album', NULL, NULL, NULL);
             INSERT INTO songs (id, title, artist, album_id, folder_id, file_path, filename, duration, lyrics, timeline, file_mtime) VALUES
                 (10, 'Đường Về', 'Sơn Tùng', 5, 3, 'C:/Music/Pop/z.mp3', 'z.mp3', 200, 'lời', '[]', 1700),
                 (11, NULL, NULL, 5, 8, 'C:/Music/Rock/a.flac', 'a.flac', 100, NULL, NULL, NULL),
                 (20, 'Hit', 'Band', 9, 8, 'C:/Music/Rock/m.mp3', 'm.mp3', 300, NULL, NULL, 1800),
                 (21, 'Solo', NULL, NULL, 8, 'C:/Music/Rock/b.mp3', 'b.mp3', 50, NULL, NULL, NULL);
             INSERT INTO watched_roots (id, path) VALUES (4, 'C:/Music');
             INSERT INTO playlists (id, name, color) VALUES (2, 'Yêu thích', '#ff0000'), (6, 'Trống', '#00ff00');
             INSERT INTO playlist_songs (playlist_id, song_id) VALUES (2, 10), (2, 11), (2, 20), (2, 21);"
        ))
        .unwrap();
        fixture
    }

    fn migrate(&self) -> (Connection, OpenOutcome) {
        open_or_migrate(&self.target, Some(&self.legacy), &mut fake_sink).unwrap()
    }
}

/// Bìa giả: `hash` là byte ảnh viết liền, `accent` phân biệt với hằng cũ.
fn fake_sink(image: &[u8]) -> anyhow::Result<CoverRecord> {
    let hash: String = image.iter().map(|b| b.to_string()).collect();
    Ok(CoverRecord { hash, accent_color: "#112233".into(), width: Some(16), height: Some(9) })
}

fn no_covers(_: &[u8]) -> anyhow::Result<CoverRecord> {
    bail!("no cover expected")
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn text(conn: &Connection, sql: &str) -> Option<String> {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1)").unwrap();
    stmt.query_map([table], |row| row.get(0)).unwrap().map(Result::unwrap).collect()
}

fn files_next_to(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// C1
#[test]
fn keeps_ids_and_every_old_column() {
    let fixture = Fixture::typical();
    let (conn, outcome) = fixture.migrate();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };

    assert_eq!((report.folders, report.albums, report.songs), (2, 3, 4));
    assert_eq!((report.playlists, report.playlist_entries), (2, 4));
    assert_eq!((report.merged_songs, report.orphan_rows), (0, 0));
    assert_eq!(schema::user_version(&conn).unwrap(), 1);

    let song: (String, String, i64, i64, i64, String, String, i64) = conn
        .query_row(
            "SELECT title, artist, album_id, folder_id, duration, lyrics, timeline, file_mtime FROM songs WHERE id = 10",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .unwrap();
    assert_eq!(song, ("Đường Về".into(), "Sơn Tùng".into(), 5, 3, 200, "lời".into(), "[]".into(), 1700));

    let ids = |sql| -> Vec<i64> {
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(ids("SELECT id FROM songs ORDER BY id"), [10, 11, 20, 21]);
    assert_eq!(ids("SELECT id FROM albums ORDER BY id"), [5, 9, 12]);
    assert_eq!(ids("SELECT id FROM folders ORDER BY id"), [3, 8]);
    assert_eq!(ids("SELECT id FROM playlists ORDER BY id"), [2, 6]);
    assert_eq!(text(&conn, "SELECT path FROM watched_roots WHERE id = 4").unwrap(), "C:/Music");
    assert_eq!(text(&conn, "SELECT color FROM playlists WHERE id = 2").unwrap(), "#ff0000");
}

// C2
#[test]
fn never_writes_to_the_legacy_file() {
    let fixture = Fixture::typical();
    let before = fs::read(&fixture.legacy).unwrap();
    let (conn, _) = fixture.migrate();
    drop(conn);
    assert_eq!(fs::read(&fixture.legacy).unwrap(), before);
    assert_eq!(files_next_to(&fixture.legacy), ["groovex.db"]);
    // Bản cũ mở lại vẫn thấy schema cũ.
    let legacy = Connection::open(&fixture.legacy).unwrap();
    assert!(columns(&legacy, "folders").contains(&"thumbnail".to_string()));
    assert_eq!(schema::user_version(&legacy).unwrap(), 0);
}

// C3
#[test]
fn moves_covers_out_of_the_database() {
    let fixture = Fixture::typical();
    let mut calls = Vec::new();
    let mut sink = |image: &[u8]| {
        calls.push(image.to_vec());
        fake_sink(image)
    };
    let (conn, outcome) = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut sink).unwrap();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };

    // Thư mục 3 và album 5 chung một ảnh, chỉ lưu một lần.
    assert_eq!(calls, [vec![1, 2, 3], vec![4, 5, 6]]);
    assert_eq!((report.covers_stored, report.covers_failed), (2, 0));

    assert_eq!(text(&conn, "SELECT cover_hash FROM folders WHERE id = 3").unwrap(), "123");
    assert_eq!(text(&conn, "SELECT cover_hash FROM folders WHERE id = 8"), None);
    assert_eq!(text(&conn, "SELECT cover_hash FROM albums WHERE id = 5").unwrap(), "123");
    assert_eq!(text(&conn, "SELECT cover_hash FROM albums WHERE id = 9").unwrap(), "456");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM albums WHERE cover_is_custom <> 0"), 0);

    // `accent_color` lấy từ bìa, không phải hằng cũ.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM covers"), 2);
    assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM covers WHERE accent_color = '{OLD_ACCENT}'")), 0);
    assert_eq!(count(&conn, "SELECT width FROM covers WHERE hash = '123'"), 16);

    for table in ["folders", "albums"] {
        let cols = columns(&conn, table);
        assert!(!cols.contains(&"thumbnail".to_string()) && !cols.contains(&"accent_color".to_string()), "{table}");
    }
}

// C3b
#[test]
fn a_broken_cover_only_loses_that_cover() {
    let fixture = Fixture::with_schema(LEGACY_SCHEMA);
    Connection::open(&fixture.legacy)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO folders (id, name, path, thumbnail) VALUES
                 (1, 'Bad base64', 'C:/a', 'data:image/png;base64,###'),
                 (2, 'Sink error', 'C:/b', '{COVER_A}'),
                 (3, 'Sink panic', 'C:/c', '{COVER_B}'),
                 (4, 'Blank', 'C:/d', '  ');"
        ))
        .unwrap();
    let mut sink = |image: &[u8]| -> anyhow::Result<CoverRecord> {
        if image == [1, 2, 3] { bail!("cache dir is read only") } else { panic!("poisoned image") }
    };
    let (conn, outcome) = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut sink).unwrap();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };

    assert_eq!((report.folders, report.covers_stored, report.covers_failed), (4, 0, 3));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM folders WHERE cover_hash IS NOT NULL"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM covers"), 0);
}

// C4
#[test]
fn every_song_waits_for_a_tag_read_with_search_key_ready() {
    let fixture = Fixture::typical();
    let (conn, _) = fixture.migrate();

    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs WHERE tags_read_at IS NULL"), 4);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM songs WHERE album_artist IS NULL AND track_no IS NULL AND disc_no IS NULL
             AND cover_hash IS NULL AND file_size IS NULL"
        ),
        4
    );
    assert_eq!(
        text(&conn, "SELECT search_key FROM songs WHERE id = 10").unwrap(),
        "duong ve\nson tung\ntinh ca\nz.mp3"
    );
    assert_eq!(text(&conn, "SELECT search_key FROM songs WHERE id = 21").unwrap(), "solo\nb.mp3");
    let query = schema::normalize_text("SƠN");
    let hits: i64 = conn
        .query_row("SELECT COUNT(*) FROM songs WHERE instr(search_key, ?1) > 0", [query], |row| row.get(0))
        .unwrap();
    assert_eq!(hits, 1);
}

// C5
#[test]
fn old_albums_get_a_folder_group_key() {
    let fixture = Fixture::typical();
    let (conn, _) = fixture.migrate();

    // Album 5 có bài ở thư mục 3 và 8: lấy thư mục nhỏ nhất.
    assert_eq!(text(&conn, "SELECT group_key FROM albums WHERE id = 5").unwrap(), "folder:3");
    assert_eq!(text(&conn, "SELECT group_key FROM albums WHERE id = 9").unwrap(), "folder:8");
    assert_eq!(text(&conn, "SELECT group_key FROM albums WHERE id = 12").unwrap(), "folder:0");
    assert_eq!(text(&conn, "SELECT artist FROM albums WHERE id = 5").unwrap(), "Sơn Tùng");

    // S1: cùng tên, khác nhóm thì vào được; trùng cả hai thì không.
    let insert = "INSERT INTO albums (name, group_key) VALUES ('Greatest Hits', ?1)";
    conn.execute(insert, ["son tung"]).unwrap();
    assert!(conn.execute(insert, ["folder:8"]).is_err());
}

// C6
#[test]
fn playlist_order_is_what_the_old_app_showed() {
    let fixture = Fixture::typical();
    let (conn, _) = fixture.migrate();

    let mut stmt = conn
        .prepare("SELECT song_id, position, added_at FROM playlist_songs WHERE playlist_id = 2 ORDER BY position")
        .unwrap();
    let rows: Vec<(i64, i64, i64)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();

    // a.flac, b.mp3, m.mp3, z.mp3
    assert_eq!(rows.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>(), [(11, 0), (21, 1), (20, 2), (10, 3)]);
    assert!(rows.iter().all(|r| r.2 > 1_600_000_000_000), "added_at is milliseconds");
}

// C7
#[test]
fn duplicate_paths_merge_into_the_lowest_id() {
    let fixture = Fixture::with_schema(LEGACY_SCHEMA);
    Connection::open(&fixture.legacy)
        .unwrap()
        .execute_batch(
            r"INSERT INTO folders (id, name, path) VALUES (1, 'M', 'C:/M');
              INSERT INTO songs (id, title, folder_id, file_path, filename, duration, lyrics, timeline) VALUES
                  (1, 'First', 1, 'C:/M/a.mp3', 'a.mp3', 10, NULL, 'tl1'),
                  (2, 'Second', 1, 'C:\M\a.mp3', 'a.mp3', 10, 'lời 2', 'tl2'),
                  (3, 'Other', 1, 'C:/M/b.mp3', 'b.mp3', 10, NULL, NULL);
              INSERT INTO playlists (id, name, color) VALUES (1, 'Both', '#000'), (2, 'Loser', '#000');
              INSERT INTO playlist_songs (playlist_id, song_id) VALUES (1, 1), (1, 2), (1, 3), (2, 2);",
        )
        .unwrap();
    let (conn, outcome) = fixture.migrate();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };

    assert_eq!((report.songs, report.merged_songs, report.playlist_entries), (2, 1, 3));
    assert_eq!(text(&conn, "SELECT title FROM songs WHERE id = 1").unwrap(), "First");
    // Dòng thắng giữ dữ liệu của nó, chỉ lấp chỗ trống từ dòng gộp, không mất lyrics.
    assert_eq!(text(&conn, "SELECT lyrics FROM songs WHERE id = 1").unwrap(), "lời 2");
    assert_eq!(text(&conn, "SELECT timeline FROM songs WHERE id = 1").unwrap(), "tl1");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs WHERE id = 2"), 0);

    let positions: Vec<(i64, i64, i64)> = conn
        .prepare("SELECT playlist_id, song_id, position FROM playlist_songs ORDER BY playlist_id, position")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(positions, [(1, 1, 0), (1, 3, 1), (2, 1, 0)]);

    // UNIQUE giờ nằm trên `path_key`: khác kiểu dấu ngăn không vào được nữa.
    let dup = conn.execute(
        "INSERT INTO songs (folder_id, file_path, path_key, filename, duration) VALUES (1, 'x', ?1, 'x', 1)",
        [schema::path_key(r"C:\M\b.mp3")],
    );
    assert!(dup.is_err());
}

// C8
#[test]
fn accepts_a_database_from_an_old_version_of_the_tauri_app() {
    // Trước các `ALTER` và các bảng thêm sau ở `db.rs:164-228`.
    let fixture = Fixture::with_schema(
        "CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT UNIQUE NOT NULL,
             thumbnail TEXT, accent_color TEXT);
         CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, artist TEXT,
             thumbnail TEXT, accent_color TEXT);
         CREATE TABLE songs (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, artist TEXT, album_id INTEGER,
             folder_id INTEGER NOT NULL, file_path TEXT UNIQUE NOT NULL, filename TEXT NOT NULL, duration INTEGER NOT NULL);
         INSERT INTO folders (id, name, path) VALUES (1, 'M', 'C:/M');
         INSERT INTO songs (id, title, folder_id, file_path, filename, duration) VALUES (7, 'Old', 1, 'C:/M/a.mp3', 'a.mp3', 9);",
    );
    let (conn, _) = fixture.migrate();
    assert_eq!(text(&conn, "SELECT title FROM songs WHERE id = 7").unwrap(), "Old");
    assert_eq!(text(&conn, "SELECT lyrics FROM songs WHERE id = 7"), None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM playlists"), 0);
}

// C9
#[test]
fn drops_rows_whose_parent_is_gone() {
    let fixture = Fixture::with_schema(LEGACY_SCHEMA);
    Connection::open(&fixture.legacy)
        .unwrap()
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO folders (id, name, path) VALUES (1, 'M', 'C:/M');
             INSERT INTO songs (id, title, album_id, folder_id, file_path, filename, duration) VALUES
                 (1, 'Lost album', 99, 1, 'C:/M/a.mp3', 'a.mp3', 1),
                 (2, 'Lost folder', NULL, 99, 'C:/X/b.mp3', 'b.mp3', 1);
             INSERT INTO playlists (id, name, color) VALUES (1, 'P', '#000');
             INSERT INTO playlist_songs (playlist_id, song_id) VALUES (1, 1), (1, 2), (1, 50), (77, 1);",
        )
        .unwrap();
    let (conn, outcome) = fixture.migrate();
    let OpenOutcome::Migrated(report) = outcome else { panic!("expected a migration") };

    assert_eq!((report.songs, report.playlist_entries, report.orphan_rows), (1, 1, 4));
    let album: Option<i64> = conn.query_row("SELECT album_id FROM songs WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(album, None);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
}

// C10
#[test]
fn refuses_a_library_from_a_newer_build() {
    let fixture = Fixture::typical();
    fs::create_dir_all(fixture.target.parent().unwrap()).unwrap();
    {
        let newer = Connection::open(&fixture.target).unwrap();
        newer.execute_batch("CREATE TABLE future (x); PRAGMA user_version = 2;").unwrap();
    }
    let before = fs::read(&fixture.target).unwrap();

    let err = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut fake_sink).unwrap_err();
    assert!(matches!(err, MigrateError::TooNew { found: 2, .. }), "{err}");
    let err = open_fresh(&fixture.target).unwrap_err();
    assert!(matches!(err, MigrateError::TooNew { found: 2, .. }), "{err}");
    assert_eq!(fs::read(&fixture.target).unwrap(), before);
}

// C10b
#[test]
fn refuses_a_file_that_is_not_ours() {
    let fixture = Fixture::empty();
    fs::create_dir_all(fixture.target.parent().unwrap()).unwrap();
    {
        Connection::open(&fixture.target).unwrap().execute_batch("CREATE TABLE someone_elses (x);").unwrap();
    }
    let err = open_or_migrate(&fixture.target, None, &mut no_covers).unwrap_err();
    assert!(matches!(err, MigrateError::NotLibrary(_)), "{err}");
}

// C11
#[test]
fn a_failed_migration_leaves_nothing_behind() {
    type MakeLegacy = fn(&Path);
    let cases: [(&str, MakeLegacy); 3] = [
        ("garbage", |path| fs::write(path, b"this is not a database, not even close to one").unwrap()),
        ("not legacy", |path| Connection::open(path).unwrap().execute_batch("CREATE TABLE other (x);").unwrap()),
        ("newer", |path| {
            Connection::open(path).unwrap().execute_batch("CREATE TABLE songs (x); PRAGMA user_version = 9;").unwrap()
        }),
    ];
    for (name, make) in cases {
        let fixture = Fixture::empty();
        make(&fixture.legacy);
        let before = fs::read(&fixture.legacy).unwrap();

        let err = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut no_covers).unwrap_err();
        assert!(matches!(err, MigrateError::LegacyFailed { .. }), "{name}: {err}");
        assert_eq!(files_next_to(&fixture.target), Vec::<String>::new(), "{name}");
        assert_eq!(fs::read(&fixture.legacy).unwrap(), before, "{name}");

        // Lối thoát cho người dùng: thư viện trống, tệp cũ vẫn nguyên.
        let conn = open_fresh(&fixture.target).unwrap();
        assert_eq!(schema::user_version(&conn).unwrap(), 1, "{name}");
    }
}

// C11b
#[test]
fn a_crash_leftover_is_replaced() {
    let fixture = Fixture::typical();
    fs::create_dir_all(fixture.target.parent().unwrap()).unwrap();
    fs::write(fixture.target.with_file_name("library.db.migrating"), b"half written").unwrap();
    let (conn, outcome) = fixture.migrate();
    assert!(matches!(outcome, OpenOutcome::Migrated(_)));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs"), 4);
    assert!(!fixture.target.with_file_name("library.db.migrating").exists());
}

// C12
#[test]
fn second_open_does_not_migrate_again() {
    let fixture = Fixture::typical();
    {
        let (conn, _) = fixture.migrate();
        conn.execute("DELETE FROM songs WHERE id = 21", []).unwrap();
    }
    let (conn, outcome) = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut no_covers).unwrap();
    assert_eq!(outcome, OpenOutcome::Existing);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs"), 3);
}

// C13
#[test]
fn creates_a_new_library_when_there_is_no_legacy_file() {
    let fixture = Fixture::empty();
    let (conn, outcome) = open_or_migrate(&fixture.target, Some(&fixture.legacy), &mut no_covers).unwrap();
    assert_eq!(outcome, OpenOutcome::Created);
    assert_new_schema(&conn);
    drop(conn);

    // Tệp rỗng cũng là chưa có gì.
    let empty = TempDir::new().unwrap();
    let path = empty.path().join("library.db");
    fs::write(&path, b"").unwrap();
    let (conn, outcome) = open_or_migrate(&path, None, &mut no_covers).unwrap();
    assert_eq!(outcome, OpenOutcome::Created);
    assert_new_schema(&conn);
}

// C14
#[test]
fn migrated_and_new_libraries_have_the_same_schema() {
    let fixture = Fixture::typical();
    let (migrated, _) = fixture.migrate();
    assert_new_schema(&migrated);

    let dump = |conn: &Connection| -> Vec<(String, Option<String>)> {
        let mut stmt =
            conn.prepare("SELECT name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    };
    let dir = TempDir::new().unwrap();
    let fresh = open_fresh(&dir.path().join("library.db")).unwrap();
    assert_eq!(dump(&migrated), dump(&fresh));
}

// C15
#[test]
fn foreign_keys_cascade_in_the_new_schema() {
    let fixture = Fixture::typical();
    let (conn, _) = fixture.migrate();
    conn.execute("DELETE FROM albums WHERE id = 9", []).unwrap();
    let album: Option<i64> = conn.query_row("SELECT album_id FROM songs WHERE id = 20", [], |r| r.get(0)).unwrap();
    assert_eq!(album, None);

    conn.execute("DELETE FROM folders WHERE id = 8", params![]).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM playlist_songs"), 1);
}

// C16
#[test]
fn covers_do_not_stay_in_the_file_size() {
    let fixture = Fixture::with_schema(LEGACY_SCHEMA);
    let big = format!("data:image/png;base64,{}", "AQID".repeat(500_000));
    Connection::open(&fixture.legacy)
        .unwrap()
        .execute("INSERT INTO folders (id, name, path, thumbnail) VALUES (1, 'M', 'C:/M', ?1)", [big])
        .unwrap();
    let mut sink = |_: &[u8]| -> anyhow::Result<CoverRecord> {
        Ok(CoverRecord { hash: "h".into(), accent_color: "#112233".into(), width: None, height: None })
    };
    let report = migrate_legacy(&fixture.legacy, &fixture.target, &mut sink as &mut dyn CoverSink);
    assert_eq!(report.unwrap().covers_stored, 1);
    assert!(fs::metadata(&fixture.target).unwrap().len() < 200_000, "base64 must not stay in library.db");
}

// C17: cỡ thư viện thật (1 002 bài, đặc tả 2.3), số liệu giả. Xem thời gian: `-- --nocapture`.
#[test]
fn migrates_a_library_of_real_size() {
    let fixture = Fixture::with_schema(LEGACY_SCHEMA);
    {
        let mut conn = Connection::open(&fixture.legacy).unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO folders (id, name, path) VALUES (1, 'M', 'C:/M')", []).unwrap();
        for id in 1..=1002 {
            tx.execute(
                "INSERT INTO songs (id, title, folder_id, file_path, filename, duration) VALUES (?1, ?2, 1, ?3, ?4, 1)",
                params![id, format!("Bài hát số {id}"), format!("C:/M/{id}.mp3"), format!("{id}.mp3")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let started = std::time::Instant::now();
    let (conn, _) = fixture.migrate();
    println!("1002 songs migrated in {:?}", started.elapsed());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM songs WHERE search_key LIKE 'bai hat so %'"), 1002);
}

fn assert_new_schema(conn: &Connection) {
    assert_eq!(schema::user_version(conn).unwrap(), schema::SCHEMA_VERSION);

    let pragma = |name: &str| -> String {
        conn.query_row(&format!("SELECT * FROM pragma_{name}"), [], |row| {
            row.get::<_, rusqlite::types::Value>(0).map(|value| match value {
                rusqlite::types::Value::Integer(n) => n.to_string(),
                rusqlite::types::Value::Text(s) => s,
                other => format!("{other:?}"),
            })
        })
        .unwrap()
    };
    assert_eq!(pragma("journal_mode"), "wal");
    assert_eq!(pragma("synchronous"), "1"); // NORMAL
    assert_eq!(pragma("foreign_keys"), "1");
    assert_eq!(pragma("busy_timeout"), "5000");

    for (table, column) in [
        ("folders", "cover_hash"),
        ("albums", "group_key"),
        ("albums", "cover_is_custom"),
        ("songs", "album_artist"),
        ("songs", "track_no"),
        ("songs", "disc_no"),
        ("songs", "cover_hash"),
        ("songs", "file_size"),
        ("songs", "search_key"),
        ("songs", "tags_read_at"),
        ("songs", "path_key"),
        ("playlist_songs", "position"),
        ("playlist_songs", "added_at"),
        ("covers", "accent_color"),
    ] {
        assert!(columns(conn, table).contains(&column.to_string()), "{table}.{column}");
    }

    let indexed = |table: &str, column: &str| -> bool {
        let sql = format!(
            "SELECT COUNT(*) FROM sqlite_master m, pragma_index_info(m.name) i
             WHERE m.type = 'index' AND m.tbl_name = '{table}' AND i.name = '{column}'"
        );
        count(conn, &sql) > 0
    };
    assert!(indexed("songs", "folder_id"));
    assert!(indexed("songs", "album_id"));
    assert!(indexed("songs", "cover_hash"));
    assert!(indexed("playlist_songs", "song_id"));
}
