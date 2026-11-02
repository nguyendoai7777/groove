//! groove-gd1_05: hành vi của luồng ghi, luồng đọc (N7) và các lối mở thư viện (đặc tả 2.3). Chỉ dùng thư mục tạm.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::executor::block_on;
use groove_core::model::{CategoryRef, TrackRef};
use groove_library::actor::{LibraryError, LibraryEvent, OpenFailure, OpenStatus};
use groove_library::db::migrate::{CoverRecord, CoverSink, OpenOutcome};
use groove_library::db::queries::{self, QueryError, SongUpsert};
use groove_library::store::{Library, LibraryConfig};
use rusqlite::Connection;
use tempfile::TempDir;

const LEGACY_SCHEMA: &str = "
CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT UNIQUE NOT NULL,
    thumbnail TEXT, accent_color TEXT);
CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, artist TEXT,
    thumbnail TEXT, accent_color TEXT);
CREATE TABLE songs (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, artist TEXT, album_id INTEGER,
    folder_id INTEGER NOT NULL, file_path TEXT UNIQUE NOT NULL, filename TEXT NOT NULL, duration INTEGER NOT NULL,
    lyrics TEXT, timeline TEXT, file_mtime INTEGER);
CREATE TABLE watched_roots (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL);
CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL, color TEXT NOT NULL);
CREATE TABLE playlist_songs (playlist_id INTEGER NOT NULL, song_id INTEGER NOT NULL,
    PRIMARY KEY(playlist_id, song_id));
INSERT INTO folders (id, name, path, thumbnail) VALUES (3, 'Pop', 'C:/Music/Pop', 'data:image/png;base64,AQID');
INSERT INTO songs (id, title, folder_id, file_path, filename, duration)
    VALUES (10, 'Đường Về', 3, 'C:/Music/Pop/z.mp3', 'z.mp3', 200);
";

/// Thao tác phải trả về nhanh hơn thế này dù luồng nền đang bận. Rộng để máy chậm không báo giả; luồng nền bị giữ tới khi
/// test thả nên thao tác có chặn sẽ treo, không chỉ chậm.
const NON_BLOCKING: Duration = Duration::from_secs(2);

struct Paths {
    _dir: TempDir,
    target: PathBuf,
    legacy: PathBuf,
}

fn paths() -> Paths {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("Groove X").join("library.db");
    let legacy = dir.path().join("old").join("groovex.db");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    Paths { _dir: dir, target, legacy }
}

fn write_legacy(path: &Path) {
    Connection::open(path).unwrap().execute_batch(LEGACY_SCHEMA).unwrap();
}

fn no_covers() -> Box<dyn CoverSink + Send> {
    Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no covers in this test") })
}

fn open(paths: &Paths) -> Library {
    let config = LibraryConfig { target: paths.target.clone(), legacy: Some(paths.legacy.clone()) };
    Library::open(config, no_covers()).unwrap()
}

fn ready(paths: &Paths) -> Library {
    let library = open(paths);
    library.categories().wait().unwrap();
    library
}

fn add_song(library: &Library, path: &'static str) -> i64 {
    library
        .writer()
        .transaction(move |conn| {
            let folder = queries::get_or_create_folder(conn, "M", Path::new("/m"))?;
            let song = SongUpsert { path: path.into(), folder_id: folder, ..SongUpsert::default() };
            Ok(queries::upsert_song(conn, &song)?.id)
        })
        .wait()
        .unwrap()
}

#[test]
fn creates_a_library_and_reads_what_was_written() {
    let paths = paths();
    let library = open(&paths);
    let id = add_song(&library, "/m/a.mp3");
    assert_eq!(library.status(), OpenStatus::Ready(OpenOutcome::Created));
    assert_eq!(library.track_for_path("/m/a.mp3").wait().unwrap(), TrackRef::Library(id));
    assert_eq!(library.track_for_path("/x/b.mp3").wait().unwrap(), TrackRef::from_path("/x/b.mp3"));
    let song = library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.filename, "a.mp3");
    block_on(library.shutdown());

    // Lần sau mở lại đúng dữ liệu.
    let library = open(&paths);
    assert_eq!(library.song_ids(CategoryRef::Folder(1)).wait().unwrap(), [id]);
    assert_eq!(library.status(), OpenStatus::Ready(OpenOutcome::Existing));
    block_on(library.shutdown());
}

#[test]
fn reader_connection_cannot_write() {
    let paths = paths();
    let library = ready(&paths);
    let result = library
        .read(|conn| {
            conn.execute("INSERT INTO watched_roots (path) VALUES ('/x')", [])?;
            Ok(())
        })
        .wait();
    assert!(matches!(result, Err(LibraryError::Sqlite(_))));
}

#[test]
fn reads_do_not_wait_for_an_open_write_transaction() {
    // N7: bản cũ giữ một Mutex suốt, watcher đang chạy thì mọi lệnh DB chờ.
    let paths = paths();
    let library = ready(&paths);
    let existing = add_song(&library, "/m/a.mp3");

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let write = library.writer().transaction(move |conn| {
        let folder = queries::get_or_create_folder(conn, "M", Path::new("/m"))?;
        let song = SongUpsert { path: "/m/b.mp3".into(), folder_id: folder, ..SongUpsert::default() };
        let id = queries::upsert_song(conn, &song)?.id;
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(id)
    });
    entered_rx.recv().unwrap();

    let started = Instant::now();
    // Thấy bản đã commit, chưa thấy dòng dở. Đọc mà chờ ghi thì test này treo ở đây, không phải qua giả.
    assert_eq!(library.song_ids(CategoryRef::Folder(1)).wait().unwrap(), [existing]);
    // Việc ghi kế tiếp xếp hàng, gửi không chặn.
    let queued = library.create_playlist("L", "#000");
    assert!(started.elapsed() < NON_BLOCKING, "took {:?}", started.elapsed());

    release_tx.send(()).unwrap();
    let new_id = write.wait().unwrap();
    queued.wait().unwrap();
    assert_eq!(library.song_ids(CategoryRef::Folder(1)).wait().unwrap(), [existing, new_id]);
}

#[test]
fn open_returns_at_once_and_queued_reads_wait_for_the_migration() {
    // N7: chuyển DB cũ chạy nền kể cả khi bìa chậm, và việc đọc đến sớm không báo lỗi giả.
    let paths = paths();
    write_legacy(&paths.legacy);
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let (entered_tx, entered_rx) = mpsc::channel();
    let sink = move |image: &[u8]| -> anyhow::Result<CoverRecord> {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(CoverRecord { hash: format!("h{}", image.len()), accent_color: "#111111".into(), width: None, height: None })
    };
    let config = LibraryConfig { target: paths.target.clone(), legacy: Some(paths.legacy.clone()) };
    let started = Instant::now();
    let library = Library::open(config, Box::new(sink)).unwrap();
    let mut events = library.subscribe();
    let categories = library.categories();
    entered_rx.recv().unwrap();
    assert!(started.elapsed() < NON_BLOCKING);
    assert_eq!(library.status(), OpenStatus::Opening);

    release_tx.send(()).unwrap();
    let categories = categories.wait().unwrap();
    assert_eq!(categories.folders[0].reference, CategoryRef::Folder(3));
    assert_eq!(categories.folders[0].cover_hash.as_deref(), Some("h3"));
    let OpenStatus::Ready(OpenOutcome::Migrated(report)) = library.status() else {
        panic!("expected a migration, got {:?}", library.status());
    };
    assert_eq!(report.songs, 1);
    assert!(matches!(block_on(events.next()), Some(LibraryEvent::Opened(OpenOutcome::Migrated(_)))));
    // Id giữ nguyên, tìm được không dấu, gốc lấy từ thư mục vì DB cũ chưa có gốc nào.
    let results = library.search("duong ve", 5).wait().unwrap();
    assert_eq!(results.songs[0].track, TrackRef::Library(10));
    assert_eq!(library.watched_roots().wait().unwrap(), [PathBuf::from("C:/Music/Pop")]);
}

#[test]
fn too_new_database_is_refused_and_left_untouched() {
    let paths = paths();
    fs::create_dir_all(paths.target.parent().unwrap()).unwrap();
    {
        let conn = Connection::open(&paths.target).unwrap();
        conn.execute_batch("CREATE TABLE future (x); PRAGMA user_version = 99;").unwrap();
    }
    let before = fs::read(&paths.target).unwrap();

    let library = open(&paths);
    let result = library.categories().wait();
    assert!(matches!(result, Err(LibraryError::Unavailable(OpenFailure::TooNew { found: 99, .. }))));
    assert!(matches!(library.status(), OpenStatus::Failed(OpenFailure::TooNew { .. })));
    let write = library.create_playlist("L", "#000").wait();
    assert!(matches!(write, Err(LibraryError::Unavailable(OpenFailure::TooNew { .. }))));
    // Hai lối ra đều không ghi đè được.
    assert!(library.retry_open().wait().is_err());
    assert!(library.start_fresh().wait().is_err());
    block_on(library.shutdown());
    assert_eq!(fs::read(&paths.target).unwrap(), before);
}

#[test]
fn failed_migration_creates_nothing_and_offers_both_ways_out() {
    let paths = paths();
    fs::write(&paths.legacy, b"this is not a sqlite file, and it stays as it is").unwrap();
    let legacy_before = fs::read(&paths.legacy).unwrap();

    let library = open(&paths);
    let result = library.categories().wait();
    assert!(matches!(result, Err(LibraryError::Unavailable(OpenFailure::LegacyFailed { .. }))));
    assert!(!paths.target.exists(), "must not create an empty library by itself");
    assert_eq!(fs::read(&paths.legacy).unwrap(), legacy_before);

    // Thử lại, vẫn hỏng, vẫn báo qua sự kiện.
    let mut events = library.subscribe();
    assert!(matches!(library.retry_open().wait(), Err(LibraryError::Unavailable(_))));
    assert!(matches!(block_on(events.next()), Some(LibraryEvent::OpenFailed(OpenFailure::LegacyFailed { .. }))));

    // Tệp được sửa thì lần thử sau chuyển được.
    fs::remove_file(&paths.legacy).unwrap();
    write_legacy(&paths.legacy);
    assert!(matches!(library.retry_open().wait().unwrap(), OpenOutcome::Migrated(_)));
    assert_eq!(library.song_ids(CategoryRef::Folder(3)).wait().unwrap(), [10]);
    // Đã mở thì các lối ra không làm gì.
    assert_eq!(library.start_fresh().wait().unwrap(), OpenOutcome::Existing);
}

#[test]
fn start_fresh_skips_a_broken_legacy_database() {
    let paths = paths();
    fs::write(&paths.legacy, b"broken").unwrap();
    let library = open(&paths);
    assert!(library.categories().wait().is_err());
    assert_eq!(library.start_fresh().wait().unwrap(), OpenOutcome::Created);
    assert!(library.categories().wait().unwrap().folders.is_empty());
    assert_eq!(library.status(), OpenStatus::Ready(OpenOutcome::Created));
    assert_eq!(fs::read(&paths.legacy).unwrap(), b"broken");
    add_song(&library, "/m/a.mp3");
}

#[test]
fn failed_or_panicking_transaction_rolls_back_and_the_writer_lives_on() {
    let paths = paths();
    let library = ready(&paths);
    let failed: Result<(), _> = library
        .writer()
        .transaction(|conn| {
            queries::create_playlist(conn, "Dở", "#000")?;
            queries::create_playlist(conn, "Dở", "#000")?;
            Ok(())
        })
        .wait();
    assert!(matches!(failed, Err(LibraryError::Query(QueryError::DuplicatePlaylist(_)))));

    let panicked: Result<(), _> = library
        .writer()
        .transaction(|conn| {
            queries::create_playlist(conn, "Độc", "#000")?;
            panic!("poison");
        })
        .wait();
    assert!(matches!(panicked, Err(LibraryError::Panicked(_))));

    assert!(library.playlists().wait().unwrap().is_empty());
    library.create_playlist("Sống", "#000").wait().unwrap();
    assert_eq!(library.playlists().wait().unwrap()[0].name, "Sống");
}

#[test]
fn stale_search_is_dropped_and_sequence_numbers_grow() {
    // S18: kết quả về muộn không được đè kết quả mới.
    let paths = paths();
    let library = ready(&paths);
    add_song(&library, "/m/mua.mp3");

    // Giữ luồng đọc bận để hai lượt nằm trong hàng cùng lúc.
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let busy = library.read(move |_| {
        release_rx.recv().unwrap();
        Ok(())
    });
    let older = library.search("mu", 5);
    let newer = library.search("mua", 5);
    release_tx.send(()).unwrap();
    busy.wait().unwrap();

    assert!(matches!(older.wait(), Err(LibraryError::Superseded)));
    let newer = newer.wait().unwrap();
    assert_eq!(newer.seq, 2);
    assert_eq!(newer.songs.len(), 1);
    assert_eq!(library.search("mua", 5).wait().unwrap().seq, 3);
}

#[test]
fn playlist_writes_notify_only_after_commit() {
    let paths = paths();
    let library = ready(&paths);
    let song = add_song(&library, "/m/a.mp3");
    let mut events = library.subscribe();

    let list = library.create_playlist("L", "#06b6d4").wait().unwrap();
    assert!(library.add_song_to_playlist(list, song).wait().unwrap());
    assert_eq!(events.try_next().unwrap(), Some(LibraryEvent::PlaylistsChanged));
    assert_eq!(events.try_next().unwrap(), Some(LibraryEvent::PlaylistsChanged));

    // Lỗi: không có gì đổi, không báo.
    assert!(library.create_playlist("L", "#000").wait().is_err());
    assert!(events.try_next().is_err());

    assert_eq!(library.songs(CategoryRef::Playlist(list)).wait().unwrap().len(), 1);
    assert!(library.remove_song_from_playlist(list, song).wait().unwrap());
    assert!(library.delete_playlist(list).wait().unwrap());
}

#[test]
fn shutdown_finishes_queued_work_then_reports_closed() {
    let paths = paths();
    let library = ready(&paths);
    let queued = library.create_playlist("L", "#000");
    block_on(library.shutdown());
    queued.wait().unwrap();
    assert_eq!(library.status(), OpenStatus::Closed);
    assert!(matches!(library.playlists().wait(), Err(LibraryError::Closed)));
    assert!(matches!(library.create_playlist("M", "#000").wait(), Err(LibraryError::Closed)));

    // Tệp đã rảnh: mở lại thấy việc đã xếp hàng.
    let library = open(&paths);
    assert_eq!(library.playlists().wait().unwrap()[0].name, "L");
}

#[test]
fn library_can_be_shared_across_threads() {
    let paths = paths();
    let library = ready(&paths);
    let clone = library.clone();
    let worker = std::thread::spawn(move || clone.create_playlist("Từ luồng khác", "#000").wait().unwrap());
    let id = worker.join().unwrap();
    assert_eq!(library.category(CategoryRef::Playlist(id)).wait().unwrap().unwrap().name, "Từ luồng khác");
}
