//! groove-gd1_09: không làm mất thành viên playlist khi tệp tạm vắng hay đổi tên ngoài app (N1, N4, S6), trên tệp thật trong
//! thư mục tạm.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedReceiver;
use groove_core::model::{CategoryRef, TrackRef};
use groove_library::actor::{LibraryEvent, MovedSong, SyncReport};
use groove_library::covers::CoverStore;
use groove_library::db::migrate::{CoverRecord, CoverSink};
use groove_library::db::queries::{self, SongUpsert};
use groove_library::fixtures::{AudioKind, write_sample};
use groove_library::indexer::Indexer;
use groove_library::reconcile::{self, MISSING_GRACE_MS, RootState};
use groove_library::store::{Library, LibraryConfig};
use groove_library::tags::{self, CoverEdit, FileStamp, MetadataEdit, TagFields, WriteMode};
use groove_library::watcher::{self, ChangeBatch, LibraryWatcher, WatcherOptions};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    library: Library,
    indexer: Indexer,
    music: PathBuf,
}

fn no_covers() -> Box<dyn CoverSink + Send> {
    Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("unused") })
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let music = dir.path().join("Music");
    fs::create_dir_all(&music).unwrap();
    let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
    let library = Library::open(config, no_covers()).unwrap();
    let indexer = Indexer::new(library.clone(), CoverStore::new(dir.path().join("covers")));
    Fixture { dir, library, indexer, music }
}

fn write(dir: &Path, stem: &str, title: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = write_sample(dir, stem, AudioKind::Mp3).unwrap();
    retag(&path, title, None);
    path
}

fn retag(path: &Path, title: &str, album: Option<&str>) {
    let fields = TagFields { title: Some(title.to_owned()), album: album.map(str::to_owned), ..TagFields::default() };
    tags::write_tags(path, &MetadataEdit { fields, cover: CoverEdit::Remove }, WriteMode::Clean).unwrap();
}

fn id_of(fx: &Fixture, path: &Path) -> i64 {
    match fx.library.track_for_path(path.to_owned()).wait().unwrap() {
        TrackRef::Library(id) => id,
        TrackRef::External(path) => panic!("{} is not in the library", path.display()),
    }
}

fn playlist_with(fx: &Fixture, ids: &[i64]) -> i64 {
    let playlist = fx.library.create_playlist("P", "#fff").wait().unwrap();
    for id in ids {
        fx.library.add_song_to_playlist(playlist, *id).wait().unwrap();
    }
    playlist
}

fn playlist_ids(fx: &Fixture, playlist: i64) -> Vec<i64> {
    fx.library.song_ids(CategoryRef::Playlist(playlist)).wait().unwrap()
}

fn members(fx: &Fixture) -> i64 {
    scalar(fx, "SELECT COUNT(*) FROM playlist_songs")
}

fn scalar(fx: &Fixture, sql: &'static str) -> i64 {
    fx.library.read(move |conn| Ok(conn.query_row(sql, [], |row| row.get(0))?)).wait().unwrap()
}

fn drain(events: &mut UnboundedReceiver<LibraryEvent>) -> Vec<LibraryEvent> {
    let mut found = Vec::new();
    while let Ok(event) = events.try_recv() {
        found.push(event);
    }
    found
}

fn reconcile_now(fx: &Fixture) -> reconcile::ReconcileReport {
    reconcile::reconcile(&fx.indexer).wait().unwrap()
}

fn tick() {
    // Đủ để mtime ms của tệp vừa ghi khác đi.
    std::thread::sleep(Duration::from_millis(20));
}

/// Chặn đọc một thư mục thật, trả lại quyền khi xong để thư mục tạm xoá được. Xem `tests/indexer_import.rs`.
struct Locked(PathBuf);

impl Locked {
    #[cfg(windows)]
    fn new(path: &Path) -> Option<Locked> {
        let status = std::process::Command::new("icacls").arg(path).args(["/deny", "*S-1-1-0:(OI)(CI)(RX)"]).output();
        status.is_ok_and(|out| out.status.success()).then(|| Locked(path.to_owned()))
    }

    #[cfg(unix)]
    fn new(path: &Path) -> Option<Locked> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).ok()?;
        fs::read_dir(path).is_err().then(|| Locked(path.to_owned()))
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        #[cfg(windows)]
        let _ = std::process::Command::new("icacls").arg(&self.0).args(["/remove:d", "*S-1-1-0"]).output();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------ N1

#[test]
fn a_file_that_goes_away_and_returns_keeps_its_playlist_place() {
    let fx = fixture();
    let keep = write(&fx.music, "keep", "Còn");
    let away = write(&fx.music, "away", "Vắng");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (keep_id, away_id) = (id_of(&fx, &keep), id_of(&fx, &away));
    let playlist = playlist_with(&fx, &[away_id, keep_id]);
    let hidden = fx.dir.path().join("away.bak");
    fs::rename(&away, &hidden).unwrap();

    let mut events = fx.library.subscribe();
    let report = reconcile_now(&fx);
    assert_eq!(report.sync.marked_missing, [away_id]);
    assert_eq!(playlist_ids(&fx, playlist), [keep_id]);
    assert_eq!(members(&fx), 2, "row and membership stay");
    let found = drain(&mut events);
    assert!(found.contains(&LibraryEvent::Synced(SyncReport { added: 0, updated: 0, removed: vec![away_id] })));

    // Lần mở sau không báo lại, mốc 30 ngày không bị đẩy lùi.
    let since = scalar(&fx, "SELECT missing_since FROM songs WHERE missing_since IS NOT NULL");
    tick();
    assert!(reconcile_now(&fx).sync.marked_missing.is_empty());
    assert_eq!(scalar(&fx, "SELECT missing_since FROM songs WHERE missing_since IS NOT NULL"), since);

    fs::rename(&hidden, &away).unwrap();
    let report = reconcile_now(&fx);
    assert_eq!(report.sync.restored, [away_id]);
    assert_eq!(report.sync.tally.read(), 0, "an unchanged file is not read again");
    assert_eq!(playlist_ids(&fx, playlist), [away_id, keep_id]);
}

#[test]
fn a_folder_that_cannot_be_read_is_not_marked_on_open() {
    let fx = fixture();
    let locked_dir = fx.music.join("Locked");
    let inside = write(&locked_dir, "inside", "Trong");
    let dead = write(&fx.music, "dead", "Chết");
    write(&fx.music, "alive", "Sống");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (inside_id, dead_id) = (id_of(&fx, &inside), id_of(&fx, &dead));
    let playlist = playlist_with(&fx, &[inside_id]);
    fs::remove_file(&dead).unwrap();

    let Some(guard) = Locked::new(&locked_dir) else {
        eprintln!("skipped: cannot deny access here");
        return;
    };
    let report = reconcile_now(&fx);
    drop(guard);

    assert_eq!(report.failed_dirs, [locked_dir]);
    assert_eq!(report.sync.marked_missing, [dead_id]);
    assert_eq!(playlist_ids(&fx, playlist), [inside_id]);
}

#[test]
fn an_offline_root_is_left_alone_even_long_past_the_grace_period() {
    let fx = fixture();
    let a = write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    let playlist = playlist_with(&fx, &[id]);
    fs::remove_file(&a).unwrap();

    // Điểm gắn chưa mount: thư mục rỗng vẫn tồn tại.
    let far = queries::now_ms() + 10 * MISSING_GRACE_MS;
    let report = reconcile::reconcile_at(&fx.indexer, far).wait().unwrap();
    assert_eq!(report.roots, [(fx.music.clone(), RootState::Offline)]);
    assert!(report.sync.marked_missing.is_empty() && report.sync.purged.is_empty());
    assert_eq!(playlist_ids(&fx, playlist), [id]);

    // Gốc biến mất hẳn.
    fs::remove_dir(&fx.music).unwrap();
    let report = reconcile::reconcile_at(&fx.indexer, far).wait().unwrap();
    assert!(matches!(report.roots[0].1, RootState::Unavailable(_)));
    assert!(report.sync.marked_missing.is_empty());
    assert_eq!(playlist_ids(&fx, playlist), [id]);
}

#[test]
fn only_a_long_confirmed_absence_is_purged() {
    let fx = fixture();
    write(&fx.music, "stay", "Ở lại");
    let gone = write(&fx.music.join("Old"), "gone", "Đi");
    retag(&gone, "Đi", Some("Album cũ"));
    let back = write(&fx.music, "back", "Về");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (gone_id, back_id) = (id_of(&fx, &gone), id_of(&fx, &back));
    let playlist = playlist_with(&fx, &[gone_id, back_id]);
    let saved = fs::read(&back).unwrap();
    fs::remove_file(&gone).unwrap();
    fs::remove_dir(gone.parent().unwrap()).unwrap();
    fs::remove_file(&back).unwrap();
    reconcile_now(&fx);

    let almost = queries::now_ms() + MISSING_GRACE_MS - 60_000;
    assert!(reconcile::reconcile_at(&fx.indexer, almost).wait().unwrap().sync.purged.is_empty());
    assert_eq!(members(&fx), 2);

    // Bài kịp quay lại ngay trong lượt dọn thì không bị dọn.
    fs::write(&back, saved).unwrap();
    let mut events = fx.library.subscribe();
    let late = queries::now_ms() + MISSING_GRACE_MS + 60_000;
    let report = reconcile::reconcile_at(&fx.indexer, late).wait().unwrap();
    assert_eq!(report.sync.purged, [gone_id]);
    assert_eq!(playlist_ids(&fx, playlist), [back_id]);
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM albums"), 0, "the empty album goes with it");
    assert!(
        drain(&mut events).iter().any(
            |event| matches!(event, LibraryEvent::Synced(SyncReport { removed, .. }) if removed.contains(&gone_id))
        )
    );
}

#[test]
fn the_user_can_forget_missing_songs_but_not_returned_ones() {
    let fx = fixture();
    write(&fx.music, "stay", "Ở lại");
    let a = write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    let stay = id_of(&fx, &fx.music.join("stay.mp3"));
    let playlist = playlist_with(&fx, &[id]);
    fs::remove_file(&a).unwrap();
    reconcile_now(&fx);
    let listed = reconcile::missing_songs(&fx.library).wait().unwrap();
    assert_eq!(listed.iter().map(|song| song.id).collect::<Vec<_>>(), [id]);

    let mut events = fx.library.subscribe();
    assert_eq!(reconcile::forget_missing(&fx.library, vec![id, stay]).wait().unwrap(), [id]);
    assert!(playlist_ids(&fx, playlist).is_empty());
    assert_eq!(members(&fx), 0);
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM songs"), 1);
    assert!(drain(&mut events).iter().any(|event| matches!(event, LibraryEvent::Synced(_))));
}

#[test]
fn import_restores_a_returned_file() {
    let fx = fixture();
    write(&fx.music, "stay", "Ở lại");
    let a = write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    let hidden = fx.dir.path().join("a.bak");
    fs::rename(&a, &hidden).unwrap();
    assert_eq!(fx.indexer.import(&fx.music).wait().unwrap().marked_missing, [id]);
    fs::rename(&hidden, &a).unwrap();
    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(report.restored, [id]);
    assert_eq!(id_of(&fx, &a), id);
}

#[test]
fn rescan_brings_back_a_returned_file() {
    let fx = fixture();
    write(&fx.music, "stay", "Ở lại");
    let a = write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    let hidden = fx.dir.path().join("a.bak");
    fs::rename(&a, &hidden).unwrap();
    reconcile_now(&fx);
    fs::rename(&hidden, &a).unwrap();
    fx.indexer.rescan().wait().unwrap();
    assert_eq!(id_of(&fx, &a), id);
}

// ------------------------------------------------------------------------------------------------------------------ N4

#[test]
fn renaming_a_file_while_closed_keeps_id_playlist_and_notes() {
    let fx = fixture();
    let old = write(&fx.music, "old", "Bài");
    write(&fx.music, "other", "Khác");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &old);
    let playlist = playlist_with(&fx, &[id]);
    fx.library
        .writer()
        .transaction(move |conn| Ok(queries::set_song_timeline(conn, id, Some("0:10 Đoạn"))?))
        .wait()
        .unwrap();

    let new = fx.music.join("new.mp3");
    fs::rename(&old, &new).unwrap();
    let mut events = fx.library.subscribe();
    let report = reconcile_now(&fx);

    assert_eq!(report.sync.moved, [MovedSong { id, from: old.clone(), to: new.clone() }]);
    assert!(report.sync.marked_missing.is_empty());
    assert_eq!(report.sync.tally.added, 0);
    assert_eq!(id_of(&fx, &new), id);
    assert_eq!(playlist_ids(&fx, playlist), [id]);
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM songs"), 2);
    let text = fx.library.song_text(id).wait().unwrap().unwrap();
    assert_eq!(text.timeline.as_deref(), Some("0:10 Đoạn"));
    assert!(drain(&mut events).iter().any(|event| matches!(event, LibraryEvent::Moved(moved) if moved[0].id == id)));
}

#[test]
fn renaming_a_folder_while_closed_moves_every_song_and_the_album_group() {
    let fx = fixture();
    let old_dir = fx.music.join("Old");
    // Hai bản sao giống hệt: chỉ tên tệp phân biệt được.
    let a = write(&old_dir, "a", "Giống");
    let b = write(&old_dir, "b", "Giống");
    retag(&a, "Giống", Some("Album"));
    retag(&b, "Giống", Some("Album"));
    write(&fx.music, "stay", "Ở lại");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (a_id, b_id) = (id_of(&fx, &a), id_of(&fx, &b));
    let playlist = playlist_with(&fx, &[b_id, a_id]);

    let new_dir = fx.music.join("New");
    fs::rename(&old_dir, &new_dir).unwrap();
    let report = reconcile_now(&fx);

    assert_eq!(report.sync.moved.len(), 2);
    assert_eq!(id_of(&fx, &new_dir.join("a.mp3")), a_id);
    assert_eq!(id_of(&fx, &new_dir.join("b.mp3")), b_id);
    assert_eq!(playlist_ids(&fx, playlist), [b_id, a_id]);
    let categories = fx.library.categories().wait().unwrap();
    let mut folders: Vec<String> = categories.folders.iter().map(|folder| folder.name.clone()).collect();
    folders.sort();
    assert_eq!(folders, ["Music", "New"], "the old folder row is gone");
    // Không có `album_artist`: `group_key` theo thư mục, album đi theo thư mục mới, không để lại album rỗng.
    assert_eq!(categories.albums.len(), 1);
    assert_eq!(categories.albums[0].song_count, 2);
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM albums"), 1);
}

#[test]
fn import_also_links_renames() {
    let fx = fixture();
    let old = write(&fx.music, "old", "Bài");
    write(&fx.music, "other", "Khác");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &old);
    let new = fx.music.join("new.mp3");
    fs::rename(&old, &new).unwrap();
    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(report.moved.len(), 1);
    assert!(report.marked_missing.is_empty());
    assert_eq!(report.tally.added, 0);
    assert_eq!(id_of(&fx, &new), id);
}

#[test]
fn a_file_returning_under_a_new_name_after_being_marked_still_links() {
    // Hai lô rời nhau: xoá trước, tạo sau.
    let fx = fixture();
    write(&fx.music, "stay", "Ở lại");
    let old = write(&fx.music, "old", "Bài");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &old);
    let playlist = playlist_with(&fx, &[id]);
    let hidden = fx.dir.path().join("old.bak");
    fs::rename(&old, &hidden).unwrap();
    assert_eq!(reconcile_now(&fx).sync.marked_missing, [id]);

    let new = fx.music.join("Sub").join("new.mp3");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::rename(&hidden, &new).unwrap();
    assert_eq!(reconcile_now(&fx).sync.moved.len(), 1);
    assert_eq!(playlist_ids(&fx, playlist), [id]);
    assert_eq!(id_of(&fx, &new), id);
}

#[test]
fn indistinguishable_copies_do_not_steal_a_playlist_place() {
    let fx = fixture();
    let a = write(&fx.music, "a", "Giống");
    let b = write(&fx.music, "b", "Giống");
    write(&fx.music, "stay", "Ở lại");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (a_id, b_id) = (id_of(&fx, &a), id_of(&fx, &b));
    fs::rename(&a, fx.music.join("x.mp3")).unwrap();
    fs::rename(&b, fx.music.join("y.mp3")).unwrap();

    let report = reconcile_now(&fx);
    assert!(report.sync.moved.is_empty());
    assert_eq!(report.sync.tally.added, 2);
    let mut marked = report.sync.marked_missing.clone();
    marked.sort_unstable();
    assert_eq!(marked, [a_id, b_id]);
}

#[test]
fn a_reported_rename_is_trusted_even_when_the_tags_changed() {
    let fx = fixture();
    let old = write(&fx.music, "old", "Bài");
    write(&fx.music, "other", "Khác");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &old);
    let new = fx.music.join("new.mp3");
    fs::rename(&old, &new).unwrap();
    retag(&new, "Tên mới dài hơn hẳn tên cũ", None);

    let batch = ChangeBatch { renames: vec![(old.clone(), new.clone())], ..ChangeBatch::default() };
    let outcome = watcher::apply_batch(&fx.indexer, batch).wait().unwrap();
    assert_eq!(outcome.moved, [MovedSong { id, from: old, to: new.clone() }]);
    let song = fx.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.title.as_deref(), Some("Tên mới dài hơn hẳn tên cũ"));
    assert_eq!(song.path, new);
}

#[test]
fn a_reported_folder_rename_moves_the_tree() {
    let fx = fixture();
    let old_dir = fx.music.join("Old").join("Deep");
    let a = write(&old_dir, "a", "A");
    write(&fx.music, "stay", "Ở lại");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    let from = fx.music.join("Old");
    let to = fx.music.join("New");
    fs::rename(&from, &to).unwrap();
    let batch = ChangeBatch { renames: vec![(from, to.clone())], ..ChangeBatch::default() };
    let outcome = watcher::apply_batch(&fx.indexer, batch).wait().unwrap();
    assert_eq!(outcome.moved.len(), 1);
    assert_eq!(id_of(&fx, &to.join("Deep").join("a.mp3")), id);
}

#[test]
fn a_rename_onto_an_existing_song_does_not_merge_rows() {
    let fx = fixture();
    let a = write(&fx.music, "a", "A");
    let b = write(&fx.music, "b", "B");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (a_id, b_id) = (id_of(&fx, &a), id_of(&fx, &b));
    tick();
    fs::remove_file(&b).unwrap();
    fs::rename(&a, &b).unwrap();
    let batch = ChangeBatch { renames: vec![(a.clone(), b.clone())], ..ChangeBatch::default() };
    let outcome = watcher::apply_batch(&fx.indexer, batch).wait().unwrap();
    assert!(outcome.moved.is_empty());
    assert_eq!(outcome.marked_missing, [a_id]);
    assert_eq!(id_of(&fx, &b), b_id);
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM songs"), 2);
}

#[test]
fn an_event_on_a_root_with_no_audio_left_marks_nothing() {
    // N1: cùng luật với lúc mở app, thêm tệp lạ trong gốc cũng không đổi kết luận.
    let fx = fixture();
    let a = write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    fs::remove_file(&a).unwrap();
    fs::write(fx.music.join("note.txt"), "x").unwrap();
    let batch = ChangeBatch { dirty: vec![fx.music.clone()], ..ChangeBatch::default() };
    assert!(watcher::apply_batch(&fx.indexer, batch).wait().unwrap().marked_missing.is_empty());
}

#[test]
fn a_deleted_folder_in_a_batch_marks_its_songs_unless_the_root_is_gone() {
    let fx = fixture();
    let sub = fx.music.join("Sub");
    let a = write(&sub, "a", "A");
    let b = write(&fx.music, "b", "B");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (a_id, b_id) = (id_of(&fx, &a), id_of(&fx, &b));

    fs::remove_dir_all(&sub).unwrap();
    let batch = ChangeBatch { dirty: vec![sub], ..ChangeBatch::default() };
    assert_eq!(watcher::apply_batch(&fx.indexer, batch).wait().unwrap().marked_missing, [a_id]);

    // Gốc rỗng (chưa mount) hoặc biến mất: không đánh dấu thêm gì.
    fs::remove_file(&b).unwrap();
    let batch = ChangeBatch { dirty: vec![b.clone()], ..ChangeBatch::default() };
    assert!(watcher::apply_batch(&fx.indexer, batch).wait().unwrap().marked_missing.is_empty());
    fs::remove_dir(&fx.music).unwrap();
    let batch = ChangeBatch { dirty: vec![fx.music.clone(), b], ..ChangeBatch::default() };
    assert!(watcher::apply_batch(&fx.indexer, batch).wait().unwrap().marked_missing.is_empty());
    assert_eq!(fx.library.song(TrackRef::Library(b_id)).wait().unwrap().unwrap().title.as_deref(), Some("B"));
}

// ------------------------------------------------------------------------------------------------------------------ S6

#[test]
fn a_write_whose_stamp_is_stored_is_ignored_in_every_batch() {
    // S6: bản cũ gỡ dấu "tự ghi" ở lô đầu nên lô thứ hai vẫn đọc lại và toast thừa.
    let fx = fixture();
    let a = write(&fx.music, "a", "Cũ");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    tick();
    retag(&a, "Mới do app ghi", None);
    let stamp = FileStamp::of(&a).unwrap();
    fx.library
        .writer()
        .transaction(move |conn| {
            Ok(queries::set_song_file_state(conn, id, Some(stamp.mtime_ms), Some(stamp.size as i64))?)
        })
        .wait()
        .unwrap();

    let mut events = fx.library.subscribe();
    for _ in 0..2 {
        let batch = ChangeBatch { dirty: vec![a.clone()], ..ChangeBatch::default() };
        let outcome = watcher::apply_batch(&fx.indexer, batch).wait().unwrap();
        assert!(!outcome.changed_anything());
        assert_eq!(outcome.unchanged, 1);
    }
    assert!(drain(&mut events).is_empty(), "no toast, no reload");
    // Bằng chứng không đọc lại: DB vẫn giữ tiêu đề cũ.
    let song = fx.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.title.as_deref(), Some("Cũ"));
}

#[test]
fn an_outside_edit_is_read_once() {
    let fx = fixture();
    let a = write(&fx.music, "a", "Cũ");
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = id_of(&fx, &a);
    tick();
    retag(&a, "Sửa ở app khác", None);
    let mut events = fx.library.subscribe();
    let batch = ChangeBatch { dirty: vec![a.clone()], ..ChangeBatch::default() };
    assert_eq!(watcher::apply_batch(&fx.indexer, batch.clone()).wait().unwrap().tally.updated, 1);
    assert_eq!(id_of(&fx, &a), id);
    assert!(drain(&mut events).contains(&LibraryEvent::Synced(SyncReport { added: 0, updated: 1, removed: vec![] })));
    assert_eq!(watcher::apply_batch(&fx.indexer, batch).wait().unwrap().tally.updated, 0);
}

#[test]
fn rows_from_the_old_database_are_not_reported_as_changed() {
    // Dòng `tags_read_at IS NULL` có mtime tính bằng giây: chưa có mốc, không phải đã đổi.
    let fx = fixture();
    let a = write(&fx.music, "a", "Thẻ");
    let path = a.clone();
    let music = fx.music.clone();
    let id = fx
        .library
        .writer()
        .transaction(move |conn| {
            let folder = queries::get_or_create_folder(conn, "Music", &music)?;
            queries::add_watched_root(conn, &music)?;
            let song = SongUpsert {
                path,
                folder_id: folder,
                title: Some("Tên DB cũ".into()),
                file_mtime: Some(1_700_000_000),
                ..SongUpsert::default()
            };
            Ok(queries::upsert_song(conn, &song)?.id)
        })
        .wait()
        .unwrap();
    let mut events = fx.library.subscribe();
    let report = reconcile_now(&fx);
    assert!(!report.sync.changed_anything());
    assert!(drain(&mut events).is_empty());
    assert_eq!(scalar(&fx, "SELECT file_mtime FROM songs"), 1_700_000_000);
    let song = fx.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.title.as_deref(), Some("Tên DB cũ"));
}

#[test]
fn a_clean_open_says_nothing() {
    let fx = fixture();
    write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let mut events = fx.library.subscribe();
    let report = reconcile_now(&fx);
    assert_eq!(report.sync.unchanged, 1);
    assert!(drain(&mut events).is_empty());
}

#[test]
fn temporary_copies_never_become_songs() {
    let fx = fixture();
    write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let temp = fx.music.join("b.mp3.groove-tmp");
    fs::copy(fx.music.join("a.mp3"), &temp).unwrap();
    let batch = ChangeBatch { dirty: vec![temp], ..ChangeBatch::default() };
    assert!(!watcher::apply_batch(&fx.indexer, batch).wait().unwrap().changed_anything());
    assert!(!reconcile_now(&fx).sync.changed_anything());
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM songs"), 1);
}

// ------------------------------------------------------------------------------------------------------------------ N9

#[test]
fn excluded_places_are_not_scanned_marked_or_watched() {
    let fx = fixture();
    let out_dir = fx.music.join("Out");
    let inside = write(&out_dir, "inside", "Trong");
    write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    let inside_id = id_of(&fx, &inside);
    let excluded = out_dir.clone();
    fx.library.writer().transaction(move |conn| Ok(queries::add_excluded_path(conn, &excluded)?)).wait().unwrap();

    // Tệp mới dưới chỗ đã bỏ không vào thư viện, dù qua watcher hay mở app.
    let extra = write(&out_dir, "extra", "Thêm");
    let batch = ChangeBatch { dirty: vec![extra.clone()], ..ChangeBatch::default() };
    assert!(!watcher::apply_batch(&fx.indexer, batch).wait().unwrap().changed_anything());
    assert!(!reconcile_now(&fx).sync.changed_anything());
    // Dòng cũ dưới đó cũng không bị đánh dấu: bỏ thư viện hay giữ dòng là việc của caller.
    fs::remove_file(&inside).unwrap();
    assert!(!reconcile_now(&fx).sync.changed_anything());
    assert_eq!(fx.library.song(TrackRef::Library(inside_id)).wait().unwrap().unwrap().title.as_deref(), Some("Trong"));

    // Import gốc cha vẫn tôn trọng.
    assert_eq!(fx.indexer.import(&fx.music).wait().unwrap().tally.added, 0);
    // Chọn đúng chỗ đó thì gỡ che.
    assert_eq!(fx.indexer.import(&out_dir).wait().unwrap().tally.added, 1);
    let remaining = fx.library.read(|conn| Ok(queries::excluded_paths(conn)?)).wait().unwrap();
    assert!(remaining.is_empty());
    assert!(matches!(fx.library.track_for_path(extra).wait().unwrap(), TrackRef::Library(_)));
}

// -------------------------------------------------------------------------------------------------------- watcher thật

fn options() -> WatcherOptions {
    WatcherOptions { debounce: Duration::from_millis(200), root_retry: Duration::from_millis(100) }
}

/// Sự kiện hệ tệp đến không giờ cố định.
fn wait_until(what: &str, mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(started.elapsed() < Duration::from_secs(30), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_live_watcher_follows_renames_edits_deletes_and_new_files() {
    let fx = fixture();
    let a = write(&fx.music, "a", "A");
    let dir = fx.music.join("Dir");
    let inside = write(&dir, "inside", "Trong");
    let b = write(&fx.music, "b", "B");
    fx.indexer.import(&fx.music).wait().unwrap();
    let (a_id, inside_id, b_id) = (id_of(&fx, &a), id_of(&fx, &inside), id_of(&fx, &b));
    let playlist = playlist_with(&fx, &[a_id, inside_id, b_id]);

    let watcher = LibraryWatcher::start_with(fx.indexer.clone(), options()).unwrap();
    // Reconcile đầu phiên không có gì đổi, đợi watcher chạy xong lượt đó.
    std::thread::sleep(Duration::from_millis(500));

    let renamed = fx.music.join("renamed.mp3");
    fs::rename(&a, &renamed).unwrap();
    wait_until(
        "the file rename",
        || matches!(fx.library.track_for_path(renamed.clone()).wait().unwrap(), TrackRef::Library(id) if id == a_id),
    );

    let moved_dir = fx.music.join("Moved");
    fs::rename(&dir, &moved_dir).unwrap();
    let moved_inside = moved_dir.join("inside.mp3");
    wait_until(
        "the folder rename",
        || matches!(fx.library.track_for_path(moved_inside.clone()).wait().unwrap(), TrackRef::Library(id) if id == inside_id),
    );

    retag(&renamed, "A đã sửa ở nơi khác", None);
    wait_until("the outside edit", || {
        fx.library
            .song(TrackRef::Library(a_id))
            .wait()
            .unwrap()
            .is_some_and(|song| song.title.as_deref() == Some("A đã sửa ở nơi khác"))
    });

    let fresh = write(&fx.music, "fresh", "Mới");
    wait_until("the new file", || {
        matches!(fx.library.track_for_path(fresh.clone()).wait().unwrap(), TrackRef::Library(_))
    });

    fs::remove_file(&b).unwrap();
    wait_until("the delete", || fx.library.song(TrackRef::Library(b_id)).wait().unwrap().is_none());
    assert_eq!(members(&fx), 3, "the deleted song keeps its playlist row");
    assert_eq!(playlist_ids(&fx, playlist), [a_id, inside_id]);
    watcher.stop();
}

#[test]
fn a_root_absent_at_start_is_picked_up_when_it_returns() {
    let fx = fixture();
    let root = fx.dir.path().join("Removable");
    let a = write(&root, "a", "A");
    fx.indexer.import(&root).wait().unwrap();
    let a_id = id_of(&fx, &a);
    let playlist = playlist_with(&fx, &[a_id]);
    let parked = fx.dir.path().join("Parked");
    fs::rename(&root, &parked).unwrap();

    let watcher = LibraryWatcher::start_with(fx.indexer.clone(), options()).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(playlist_ids(&fx, playlist), [a_id], "absent root touches nothing");

    fs::rename(&parked, &root).unwrap();
    // Gắn lại rồi tự so: tệp mới thêm lúc vắng phải vào, và watcher phải sống trên gốc mới.
    let later = write(&root, "later", "Sau");
    wait_until("the reconcile after return", || {
        matches!(fx.library.track_for_path(later.clone()).wait().unwrap(), TrackRef::Library(_))
    });
    let live = write(&root, "live", "Đang theo dõi");
    wait_until("events on the returned root", || {
        matches!(fx.library.track_for_path(live.clone()).wait().unwrap(), TrackRef::Library(_))
    });
    assert_eq!(playlist_ids(&fx, playlist), [a_id]);
    watcher.stop();
}

#[test]
fn refresh_roots_starts_watching_an_imported_folder() {
    let fx = fixture();
    let watcher = LibraryWatcher::start_with(fx.indexer.clone(), options()).unwrap();
    write(&fx.music, "a", "A");
    fx.indexer.import(&fx.music).wait().unwrap();
    watcher.refresh_roots_blocking();
    let b = write(&fx.music, "b", "B");
    wait_until("the new file", || matches!(fx.library.track_for_path(b.clone()).wait().unwrap(), TrackRef::Library(_)));
    drop(watcher);
}
