//! groove-gd1_08: import, quét lại, tác vụ nền trên tệp thật trong thư mục tạm (đặc tả 1.11, N1, N2, S1, S7, S8).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use futures::executor::block_on;
use groove_core::model::{CategoryRef, TrackRef};
use groove_library::actor::{LibraryEvent, Progress, ProgressKind, SyncReport};
use groove_library::covers::CoverStore;
use groove_library::db::migrate::{CoverRecord, CoverSink};
use groove_library::db::queries::{self, SongUpsert};
use groove_library::fixtures::{AudioKind, TINY_PNG, write_sample};
use groove_library::indexer::{IndexError, Indexer};
use groove_library::store::{Library, LibraryConfig};
use groove_library::tags::{self, CoverEdit, MetadataEdit, TagFields, WriteMode};
use id3::TagLike;
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    library: Library,
    indexer: Indexer,
    music: PathBuf,
    covers: PathBuf,
}

fn no_covers() -> Box<dyn CoverSink + Send> {
    Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("unused") })
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let music = dir.path().join("Music");
    let covers = dir.path().join("covers");
    fs::create_dir_all(&music).unwrap();
    let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
    let library = Library::open(config, no_covers()).unwrap();
    let indexer = Indexer::new(library.clone(), CoverStore::new(&covers));
    Fixture { dir, library, indexer, music, covers }
}

#[derive(Default, Clone)]
struct Tags {
    title: Option<&'static str>,
    artist: Option<&'static str>,
    album: Option<&'static str>,
    album_artist: Option<&'static str>,
    track: Option<&'static str>,
    lyrics: Option<&'static str>,
    cover: Option<Vec<u8>>,
}

fn write(dir: &Path, stem: &str, tags: Tags) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = write_sample(dir, stem, AudioKind::Mp3).unwrap();
    retag(&path, tags);
    path
}

fn retag(path: &Path, tags: Tags) {
    let fields = TagFields {
        title: tags.title.map(str::to_owned),
        artist: tags.artist.map(str::to_owned),
        album: tags.album.map(str::to_owned),
        album_artist: tags.album_artist.map(str::to_owned),
        track_no: tags.track.map(str::to_owned),
        lyrics: tags.lyrics.map(str::to_owned),
        ..TagFields::default()
    };
    let cover = tags.cover.map_or(CoverEdit::Remove, CoverEdit::Set);
    tags::write_tags(path, &MetadataEdit { fields, cover }, WriteMode::Clean).unwrap();
}

fn ids(fx: &Fixture, sql: &'static str) -> Vec<i64> {
    fx.library
        .read(move |conn| {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
        .wait()
        .unwrap()
}

fn text(fx: &Fixture, sql: &'static str) -> Vec<Option<String>> {
    fx.library
        .read(move |conn| {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
        .wait()
        .unwrap()
}

fn cover_files(fx: &Fixture) -> usize {
    fs::read_dir(&fx.covers).map_or(0, Iterator::count)
}

fn drain(events: &mut UnboundedReceiver<LibraryEvent>) -> Vec<LibraryEvent> {
    let mut found = Vec::new();
    while let Ok(event) = events.try_recv() {
        found.push(event);
    }
    found
}

fn solid_png(shade: u8) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(8, 8, image::Rgb([shade, 10, 10]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

// ------------------------------------------------------------------------------------------------------------- import

#[test]
fn import_indexes_files_under_their_direct_parent() {
    let fx = fixture();
    let sub = fx.music.join("Pop").join("2020");
    let a = write(
        &sub,
        "a",
        Tags {
            title: Some("Đường Về"),
            artist: Some("Ca Sĩ"),
            album: Some("X"),
            track: Some("3/12"),
            ..Tags::default()
        },
    );
    write(&fx.music, "b", Tags::default());
    fs::write(fx.music.join("note.txt"), "x").unwrap();

    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!((report.tally.added, report.tally.updated, report.marked_missing.len()), (2, 0, 0));

    let song = fx.library.track_for_path(a).wait().unwrap();
    let song = fx.library.song(song).wait().unwrap().unwrap();
    assert_eq!(song.title.as_deref(), Some("Đường Về"));
    assert_eq!(song.album.as_deref(), Some("X"));
    assert_eq!(song.track_no, Some(3));
    assert!(song.duration_sec > 0.0);
    let found = fx.library.search("duong ve", 5).wait().unwrap();
    assert_eq!(found.songs.len(), 1);

    let categories = fx.library.categories().wait().unwrap();
    let mut folders: Vec<_> = categories.folders.iter().map(|f| f.path.clone().unwrap()).collect();
    folders.sort();
    assert_eq!(folders, [fx.music.clone(), sub]);
    assert_eq!(categories.folders.iter().find(|f| f.name == "2020").unwrap().song_count, 1);
    assert_eq!(fx.library.watched_roots().wait().unwrap(), std::slice::from_ref(&fx.music));
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM songs WHERE tags_read_at IS NULL"), [0]);
}

/// Tệp qua hai trình sửa thẻ (hai thẻ ID3v2 nối tiếp, thẻ sau quá 1 KiB) và tệp khai thiếu kích thước thẻ: bản Tauri đọc được
/// thì bản này cũng phải index được, không thành "failed".
#[test]
fn files_with_two_id3_tags_or_a_short_tag_size_are_indexed() {
    let fx = fixture();
    let long = "c".repeat(3000);
    let double = groove_library::fixtures::write_mp3_double_id3(
        &fx.music,
        "double",
        &[("TIT2", "Full House - Bi Rain"), ("TALB", "K Pop")],
        &[("TIT2", "Why"), ("TPE1", "ATD Softs Production"), ("TCOP", long.as_str())],
    )
    .unwrap();
    let short = groove_library::fixtures::write_mp3_short_tag_size(
        &fx.music,
        "short",
        &[("TIT2", "Rise"), ("TPE1", "Katy Perry"), ("TCOM", long.as_str())],
        2500,
    )
    .unwrap();

    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!((report.tally.added, report.tally.failed.len()), (2, 0), "{:?}", report.tally.failed);
    for (path, title, artist) in
        [(double, "Full House - Bi Rain", "ATD Softs Production"), (short, "Rise", "Katy Perry")]
    {
        let track = fx.library.track_for_path(path).wait().unwrap();
        let song = fx.library.song(track).wait().unwrap().unwrap();
        assert_eq!(song.title.as_deref(), Some(title));
        assert_eq!(song.artist.as_deref(), Some(artist));
        assert!(song.duration_sec > 0.5, "{title}: {}", song.duration_sec);
    }
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM songs WHERE tags_read_at IS NULL"), [0]);

    // Đường "Read N songs again": đọc lại cả hai, không bài nào "failed".
    let tally = fx.indexer.rescan().wait().unwrap();
    assert_eq!((tally.read(), tally.failed.len()), (2, 0), "{:?}", tally.failed);
}

#[test]
fn a_parent_root_swallows_its_child() {
    let fx = fixture();
    let child = fx.music.join("Pop");
    write(&child, "a", Tags::default());
    fx.indexer.import(&child).wait().unwrap();
    fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(fx.library.watched_roots().wait().unwrap(), std::slice::from_ref(&fx.music));
    fx.indexer.import(&child).wait().unwrap();
    assert_eq!(fx.library.watched_roots().wait().unwrap(), std::slice::from_ref(&fx.music));
}

#[test]
fn albums_group_by_album_artist_or_folder() {
    // S1: bản cũ gộp hai "Greatest Hits" của hai nghệ sĩ làm một.
    let fx = fixture();
    let hits = |artist| Tags { album: Some("Greatest Hits"), album_artist: Some(artist), ..Tags::default() };
    write(&fx.music.join("one"), "a", hits("Ca Sĩ A"));
    write(&fx.music.join("two"), "b", hits("Ca Sĩ B"));
    // Đặc tả 2.2: có album_artist thì không xé theo thư mục; không có thì không xé theo nghệ sĩ từng bài.
    write(&fx.music.join("three"), "c", hits("ca si a"));
    let feat = |artist| Tags { album: Some("Mix"), artist: Some(artist), ..Tags::default() };
    write(&fx.music.join("mix"), "d", feat("Người 1"));
    write(&fx.music.join("mix"), "e", feat("Người 2"));
    write(&fx.music.join("mix2"), "f", feat("Người 3"));

    fx.indexer.import(&fx.music).wait().unwrap();
    let albums = fx.library.categories().wait().unwrap().albums;
    let mut seen: Vec<(String, u32)> = albums.iter().map(|a| (a.name.clone(), a.song_count)).collect();
    seen.sort();
    assert_eq!(
        seen,
        [
            ("Greatest Hits".to_owned(), 1),
            ("Greatest Hits".to_owned(), 2),
            ("Mix".to_owned(), 1),
            ("Mix".to_owned(), 2)
        ]
    );
    let hits_a = albums.iter().find(|a| a.name == "Greatest Hits" && a.song_count == 2).unwrap();
    // Nghệ sĩ giữ dạng đầu tiên, không đè.
    assert_eq!(hits_a.artist.as_deref(), Some("Ca Sĩ A"));
    let mix = albums.iter().find(|a| a.name == "Mix" && a.song_count == 2).unwrap();
    assert_eq!(mix.artist.as_deref(), Some("Người 1"));
}

#[test]
fn reimport_skips_unchanged_files_and_rereads_changed_ones() {
    let fx = fixture();
    let a = write(&fx.music, "a", Tags { title: Some("Một"), ..Tags::default() });
    write(&fx.music, "b", Tags { title: Some("Hai"), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = fx.library.track_for_path(a.clone()).wait().unwrap();

    let again = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!((again.tally.read(), again.unchanged), (0, 2));

    std::thread::sleep(Duration::from_millis(20));
    retag(&a, Tags { title: Some("Một mới, dài hơn hẳn"), ..Tags::default() });
    let changed = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!((changed.tally.added, changed.tally.updated, changed.unchanged), (0, 1, 1));
    // Giữ `id`, nên giữ thành viên playlist và hàng đợi.
    assert_eq!(fx.library.track_for_path(a).wait().unwrap(), id);
    let song = fx.library.song(id).wait().unwrap().unwrap();
    assert_eq!(song.title.as_deref(), Some("Một mới, dài hơn hẳn"));
}

#[test]
fn deleted_files_hide_with_their_groups_but_keep_their_playlist_place() {
    let fx = fixture();
    let keep = write(&fx.music, "keep", Tags { album: Some("Còn"), ..Tags::default() });
    let gone = write(&fx.music.join("Old"), "gone", Tags { album: Some("Hết"), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();
    let TrackRef::Library(gone_id) = fx.library.track_for_path(gone.clone()).wait().unwrap() else { panic!() };
    let playlist = fx.library.create_playlist("P", "#fff").wait().unwrap();
    fx.library.add_song_to_playlist(playlist, gone_id).wait().unwrap();

    fs::remove_file(&gone).unwrap();
    fs::remove_dir(gone.parent().unwrap()).unwrap();
    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(report.marked_missing, [gone_id]);
    assert!(!report.root_offline);

    let categories = fx.library.categories().wait().unwrap();
    assert_eq!(categories.albums.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Còn"]);
    assert_eq!(categories.folders.len(), 1);
    // N1: dòng, nhóm và chỗ trong playlist còn nguyên, chờ tệp quay lại.
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM albums"), [2]);
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM playlist_songs"), [1]);
    assert!(matches!(fx.library.track_for_path(keep).wait().unwrap(), TrackRef::Library(_)));
}

#[test]
fn an_empty_root_with_known_rows_counts_as_offline() {
    // N1: điểm gắn chưa mount là thư mục rỗng vẫn tồn tại; mất playlist không phục hồi được.
    let fx = fixture();
    let a = write(&fx.music, "a", Tags::default());
    fx.indexer.import(&fx.music).wait().unwrap();
    fs::remove_file(&a).unwrap();

    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert!(report.root_offline);
    assert!(report.marked_missing.is_empty());
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM songs"), [1]);
}

#[test]
fn an_unreadable_root_writes_nothing() {
    let fx = fixture();
    let gone = fx.dir.path().join("gone");
    let result = fx.indexer.import(&gone).wait();
    assert!(matches!(result, Err(IndexError::RootUnavailable { path, .. }) if path == gone));
    assert!(fx.library.watched_roots().wait().unwrap().is_empty());
}

/// Chặn đọc một thư mục thật, trả lại quyền khi xong để thư mục tạm xoá được.
struct Locked(PathBuf);

impl Locked {
    #[cfg(windows)]
    fn new(path: &Path) -> Option<Locked> {
        // Theo SID, tên "Everyone" đổi theo ngôn ngữ Windows.
        let status = std::process::Command::new("icacls").arg(path).args(["/deny", "*S-1-1-0:(OI)(CI)(RX)"]).output();
        status.is_ok_and(|out| out.status.success()).then(|| Locked(path.to_owned()))
    }

    #[cfg(unix)]
    fn new(path: &Path) -> Option<Locked> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).ok()?;
        // Root đọc được mọi thứ, khi đó không tái hiện được.
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

#[test]
fn a_folder_that_cannot_be_read_keeps_its_songs_and_playlists() {
    // N1: tái hiện bằng quyền thật. Bản cũ nuốt lỗi `read_dir` rồi xoá cả dải bài, cascade mất luôn playlist.
    let fx = fixture();
    let locked_dir = fx.music.join("Locked");
    let inside = write(&locked_dir, "inside", Tags::default());
    let outside = write(&fx.music, "outside", Tags::default());
    let dead = write(&fx.music, "dead", Tags::default());
    fx.indexer.import(&fx.music).wait().unwrap();
    let TrackRef::Library(inside_id) = fx.library.track_for_path(inside.clone()).wait().unwrap() else { panic!() };
    let TrackRef::Library(dead_id) = fx.library.track_for_path(dead.clone()).wait().unwrap() else { panic!() };
    let playlist = fx.library.create_playlist("P", "#fff").wait().unwrap();
    fx.library.add_song_to_playlist(playlist, inside_id).wait().unwrap();
    fs::remove_file(&dead).unwrap();

    let Some(guard) = Locked::new(&locked_dir) else {
        eprintln!("skipped: cannot deny access here");
        return;
    };
    let report = fx.indexer.import(&fx.music).wait().unwrap();
    drop(guard);

    assert_eq!(report.failed_dirs, [locked_dir]);
    // Tệp mất thật ở chỗ đọc được vẫn đi, chứng tỏ cơ chế xoá vẫn chạy.
    assert_eq!(report.marked_missing, [dead_id]);
    assert_eq!(fx.library.track_for_path(inside).wait().unwrap(), TrackRef::Library(inside_id));
    assert_eq!(fx.library.songs(CategoryRef::Playlist(playlist)).wait().unwrap().len(), 1);
    assert!(matches!(fx.library.track_for_path(outside).wait().unwrap(), TrackRef::Library(_)));
}

#[test]
fn a_file_that_will_not_parse_does_not_stop_the_rest_or_lose_its_row() {
    let fx = fixture();
    let good = write(&fx.music, "good", Tags { title: Some("Tốt"), ..Tags::default() });
    let flip = write(&fx.music, "flip", Tags { title: Some("Lành"), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();

    std::thread::sleep(Duration::from_millis(20));
    fs::write(&flip, b"this is not audio at all").unwrap();
    let bad = fx.music.join("bad.flac");
    fs::write(&bad, [0u8; 64]).unwrap();
    retag(&good, Tags { title: Some("Tốt hơn hẳn nhiều"), ..Tags::default() });

    let report = fx.indexer.import(&fx.music).wait().unwrap();
    let mut failed = report.tally.failed.clone();
    failed.sort();
    assert_eq!(failed, [bad, flip.clone()]);
    assert_eq!(report.tally.updated, 1);
    assert!(report.marked_missing.is_empty());
    // Dòng cũ giữ nguyên trường, không bị xoá trắng vì lần đọc lỗi.
    let track = fx.library.track_for_path(flip).wait().unwrap();
    assert_eq!(fx.library.song(track).wait().unwrap().unwrap().title.as_deref(), Some("Lành"));
}

// -------------------------------------------------------------------------------------------------------------- lyrics

#[test]
fn lyrics_and_timeline_in_the_db_survive_a_reimport() {
    // N2: bản cũ `timeline = excluded.timeline` nên import lại xoá nốt trong DB.
    let fx = fixture();
    let a = write(&fx.music, "a", Tags::default());
    fx.indexer.import(&fx.music).wait().unwrap();
    let TrackRef::Library(id) = fx.library.track_for_path(a.clone()).wait().unwrap() else { panic!() };
    fx.library
        .writer()
        .transaction(move |conn| {
            queries::set_song_lyrics(conn, id, Some("lời trong DB"))?;
            queries::set_song_timeline(conn, id, Some("0:10 Đoạn"))?;
            Ok(())
        })
        .wait()
        .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    retag(&a, Tags { title: Some("Đổi để đọc lại"), ..Tags::default() });
    assert_eq!(fx.indexer.import(&fx.music).wait().unwrap().tally.updated, 1);
    let kept = fx.library.song_text(id).wait().unwrap().unwrap();
    assert_eq!(kept.lyrics.as_deref(), Some("lời trong DB"));
    assert_eq!(kept.timeline.as_deref(), Some("0:10 Đoạn"));
}

// --------------------------------------------------------------------------------------------------------------- bìa

#[test]
fn covers_are_saved_once_and_seed_groups_from_the_first_song() {
    let fx = fixture();
    let first = solid_png(200);
    let second = solid_png(50);
    let tagged = |cover: &Vec<u8>| Tags {
        album: Some("A"),
        album_artist: Some("N"),
        cover: Some(cover.clone()),
        ..Tags::default()
    };
    write(&fx.music, "1", Tags { album: Some("A"), album_artist: Some("N"), ..Tags::default() });
    write(&fx.music, "2", tagged(&first));
    write(&fx.music, "3", tagged(&first));
    write(&fx.music, "4", tagged(&second));

    fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM covers"), [2]);
    // Mỗi ảnh hai cỡ.
    assert_eq!(cover_files(&fx), 4);
    let first_hash = groove_library::covers::cover_hash(&first);
    assert_eq!(text(&fx, "SELECT cover_hash FROM albums"), [Some(first_hash.clone())]);
    assert_eq!(text(&fx, "SELECT cover_hash FROM folders"), [Some(first_hash.clone())]);
    let songs = text(&fx, "SELECT cover_hash FROM songs ORDER BY filename");
    assert_eq!(songs[0], None);
    assert_eq!(songs[2], Some(first_hash));

    // Handoff 07: nhập lại không tăng số tệp, kể cả khi buộc đọc lại.
    fx.indexer.rescan().wait().unwrap();
    assert_eq!(cover_files(&fx), 4);
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM covers"), [2]);

    // Cache bị xoá trong khi DB còn dòng: tự có lại.
    fs::remove_dir_all(&fx.covers).unwrap();
    fx.indexer.rescan().wait().unwrap();
    assert_eq!(cover_files(&fx), 4);
}

#[test]
fn a_chosen_group_cover_is_never_replaced() {
    // S8: bản cũ đè bìa cả nhóm mỗi khi một bài có bìa.
    let fx = fixture();
    let a = write(&fx.music, "a", Tags { album: Some("A"), cover: Some(solid_png(100)), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();
    let album = ids(&fx, "SELECT id FROM albums")[0];
    fx.library
        .writer()
        .transaction(move |conn| {
            let record =
                CoverRecord { hash: "chosen".into(), accent_color: "#000000".into(), width: Some(1), height: Some(1) };
            queries::upsert_cover(conn, &record)?;
            Ok(queries::set_album_cover(conn, album, Some("chosen"), true)?)
        })
        .wait()
        .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    retag(&a, Tags { album: Some("A"), cover: Some(solid_png(10)), ..Tags::default() });
    write(&fx.music, "b", Tags { album: Some("A"), cover: Some(solid_png(20)), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(text(&fx, "SELECT cover_hash FROM albums"), [Some("chosen".to_owned())]);
    // Bìa tự động cũng không đổi khi nhóm đã có bìa.
    let folder_hash = groove_library::covers::cover_hash(&solid_png(100));
    assert_eq!(text(&fx, "SELECT cover_hash FROM folders"), [Some(folder_hash)]);
}

#[test]
fn a_broken_cover_only_loses_the_picture() {
    let fx = fixture();
    let path = fx.music.join("a.mp3");
    write(&fx.music, "a", Tags { title: Some("Bài"), ..Tags::default() });
    let mut tag = id3::Tag::read_from_path(&path).unwrap();
    tag.add_frame(id3::frame::Picture {
        mime_type: "image/png".to_owned(),
        picture_type: id3::frame::PictureType::CoverFront,
        description: String::new(),
        data: b"not an image, truly".to_vec(),
    });
    tag.write_to_path(&path, id3::Version::Id3v23).unwrap();

    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(report.tally.added, 1);
    assert_eq!(text(&fx, "SELECT cover_hash FROM songs"), [None]);
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM covers"), [0]);
}

#[test]
fn tiny_png_fixture_is_a_valid_cover() {
    let fx = fixture();
    write(&fx.music, "a", Tags { cover: Some(TINY_PNG.to_vec()), ..Tags::default() });
    fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM covers"), [1]);
}

// ------------------------------------------------------------------------------------------------------------- rescan

#[test]
fn rescan_refreshes_every_field_and_only_counts_missing_files() {
    // S7: bản cũ không cập nhật lyrics, không gieo bìa album mới.
    let fx = fixture();
    let a = write(&fx.music, "a", Tags { title: Some("Cũ"), album: Some("Cũ"), ..Tags::default() });
    let gone = write(&fx.music, "gone", Tags::default());
    fx.indexer.import(&fx.music).wait().unwrap();
    let id = fx.library.track_for_path(a.clone()).wait().unwrap();

    retag(
        &a,
        Tags {
            title: Some("Mới"),
            album: Some("Mới"),
            lyrics: Some("lời mới"),
            cover: Some(solid_png(77)),
            ..Tags::default()
        },
    );
    fs::remove_file(&gone).unwrap();
    let mut events = fx.library.subscribe();
    let tally = fx.indexer.rescan().wait().unwrap();

    assert_eq!((tally.read(), tally.missing.clone(), tally.failed.len()), (1, vec![gone.clone()], 0));
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM songs"), [2]);
    let song = fx.library.song(id.clone()).wait().unwrap().unwrap();
    assert_eq!((song.title.as_deref(), song.album.as_deref()), (Some("Mới"), Some("Mới")));
    let TrackRef::Library(raw_id) = id else { panic!() };
    assert_eq!(fx.library.song_text(raw_id).wait().unwrap().unwrap().lyrics.as_deref(), Some("lời mới"));
    // Album cũ hết bài thì đi, album mới có bìa.
    assert_eq!(text(&fx, "SELECT name FROM albums"), [Some("Mới".to_owned())]);
    assert_eq!(text(&fx, "SELECT cover_hash FROM albums"), [Some(groove_library::covers::cover_hash(&solid_png(77)))]);

    let synced = drain(&mut events);
    assert!(synced.contains(&LibraryEvent::Synced(SyncReport { added: 0, updated: 1, removed: Vec::new() })));
    assert!(synced.contains(&LibraryEvent::Progress(Progress { kind: ProgressKind::Rescan, done: 2, total: 2 })));
}

#[test]
fn index_paths_reads_only_what_it_is_given() {
    let fx = fixture();
    let a = write(&fx.music, "a", Tags { title: Some("A"), ..Tags::default() });
    write(&fx.music, "b", Tags { title: Some("B"), ..Tags::default() });
    let missing = fx.music.join("none.mp3");
    let tally = fx.indexer.index_paths(vec![a, missing.clone()]).wait().unwrap();
    assert_eq!((tally.added, tally.missing), (1, vec![missing]));
    assert_eq!(text(&fx, "SELECT title FROM songs"), [Some("A".to_owned())]);
}

// ----------------------------------------------------------------------------------------------- tác vụ nền DB cũ

#[test]
fn backfill_fills_old_rows_and_regroups_their_albums() {
    let fx = fixture();
    let a = write(
        &fx.music,
        "a",
        Tags {
            album: Some("Thật"),
            album_artist: Some("Người"),
            track: Some("2"),
            cover: Some(solid_png(9)),
            ..Tags::default()
        },
    );
    let lost = fx.music.join("lost.mp3");
    let bad = fx.music.join("bad.mp3");
    fs::write(&bad, b"nope").unwrap();

    // Như sau khi chuyển DB cũ: `group_key` tạm, chưa có trường mới, mtime tính bằng giây.
    let paths = [a.clone(), lost.clone(), bad.clone()];
    let folder_path = fx.music.clone();
    let song_ids = fx
        .library
        .writer()
        .transaction(move |conn| {
            let folder = queries::get_or_create_folder(conn, "Music", &folder_path)?;
            let album = queries::get_or_create_album(conn, "Cũ", &format!("folder:{folder}"), None)?;
            let mut made = Vec::new();
            for path in paths {
                let song = SongUpsert {
                    path,
                    folder_id: folder,
                    album_id: Some(album),
                    title: Some("Tên DB cũ".into()),
                    file_mtime: Some(1_700_000_000),
                    lyrics: Some("lời DB cũ".into()),
                    ..SongUpsert::default()
                };
                made.push(queries::upsert_song(conn, &song)?.id);
            }
            Ok(made)
        })
        .wait()
        .unwrap();
    let mut events = fx.library.subscribe();

    let tally = fx.indexer.backfill_tags().wait().unwrap();
    assert_eq!((tally.updated, tally.missing.clone(), tally.failed.clone()), (1, vec![lost], vec![bad]));

    let song = fx.library.song(TrackRef::Library(song_ids[0])).wait().unwrap().unwrap();
    assert_eq!(song.album_artist.as_deref(), Some("Người"));
    assert_eq!(song.track_no, Some(2));
    assert!(song.cover_hash.is_some());
    assert_eq!(song.album.as_deref(), Some("Thật"));
    let kept = fx.library.song_text(song_ids[0]).wait().unwrap().unwrap();
    assert_eq!(kept.lyrics.as_deref(), Some("lời DB cũ"));
    // Đọc xong là mtime theo ms.
    assert!(ids(&fx, "SELECT MAX(file_mtime) FROM songs")[0] > 1_000_000_000_000);

    // Không mất dòng, bài lỗi giữ NULL để lần mở sau thử lại, và lần này không lặp vô hạn.
    assert_eq!(ids(&fx, "SELECT COUNT(*) FROM songs"), [3]);
    assert_eq!(fx.library.pending_tag_reads(10).wait().unwrap().len(), 2);
    let names = text(&fx, "SELECT name FROM albums ORDER BY name");
    assert!(names.contains(&Some("Thật".to_owned())) && names.contains(&Some("Cũ".to_owned())));
    assert!(drain(&mut events).contains(&LibraryEvent::Progress(Progress {
        kind: ProgressKind::TagBackfill,
        done: 3,
        total: 3
    })));
    // Không còn gì để đọc thì xong ngay ở lần sau khi bài đã lành.
    let again = fx.indexer.backfill_tags().wait().unwrap();
    assert_eq!(again.read(), 0);
}

// ------------------------------------------------------------------------------------------------------- không chặn UI

#[test]
fn import_returns_at_once_and_reads_keep_flowing_while_the_writer_is_busy() {
    // N7: giữ luồng ghi tới khi test thả; lời gọi có chặn sẽ treo chứ không chỉ chậm.
    let fx = fixture();
    write(&fx.music, "a", Tags::default());
    fx.library.categories().wait().unwrap();

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let hold = fx.library.writer().transaction(move |_| {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(())
    });
    entered_rx.recv().unwrap();

    let started = Instant::now();
    let job = fx.indexer.import(&fx.music);
    let rescan = fx.indexer.rescan();
    fx.library.categories().wait().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));

    release_tx.send(()).unwrap();
    hold.wait().unwrap();
    assert_eq!(job.wait().unwrap().tally.added, 1);
    rescan.wait().unwrap();
}

#[test]
fn writes_go_in_batches_with_progress_and_end_with_synced() {
    let fx = fixture();
    let bytes = groove_library::fixtures::sample_bytes(AudioKind::Mp3);
    for index in 0..250 {
        fs::write(fx.music.join(format!("{index:03}.mp3")), &bytes).unwrap();
    }
    let mut events = fx.library.subscribe();
    let report = fx.indexer.import(&fx.music).wait().unwrap();
    assert_eq!(report.tally.added, 250);

    let found = drain(&mut events);
    let steps: Vec<(usize, usize)> = found
        .iter()
        .filter_map(|event| match event {
            LibraryEvent::Progress(Progress { kind: ProgressKind::Import, done, total }) => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(steps, [(100, 250), (200, 250), (250, 250)]);
    assert_eq!(found.last(), Some(&LibraryEvent::Synced(SyncReport { added: 250, updated: 0, removed: Vec::new() })));
}

#[test]
fn readers_see_a_batch_as_soon_as_it_commits() {
    // Người nghe nhận `Progress` là đọc được đúng số dòng đó: UI có thể hiện dần.
    let fx = fixture();
    let bytes = groove_library::fixtures::sample_bytes(AudioKind::Mp3);
    for index in 0..120 {
        fs::write(fx.music.join(format!("{index:03}.mp3")), &bytes).unwrap();
    }
    let mut events = fx.library.subscribe();
    let job = fx.indexer.import(&fx.music);
    let first = block_on(async {
        loop {
            match events.next().await {
                Some(LibraryEvent::Progress(progress)) => break progress.done,
                Some(_) => {}
                None => panic!("no progress"),
            }
        }
    });
    assert_eq!(first, 100);
    assert!(ids(&fx, "SELECT COUNT(*) FROM songs")[0] >= 100);
    job.wait().unwrap();
}

#[test]
fn a_closed_library_ends_the_job_with_an_error() {
    let fx = fixture();
    write(&fx.music, "a", Tags::default());
    fx.library.categories().wait().unwrap();
    block_on(fx.library.shutdown());
    assert!(matches!(fx.indexer.import(&fx.music).wait(), Err(IndexError::Library(_))));
}

#[test]
fn a_root_that_refuses_reading_is_unavailable_not_empty() {
    // N1: gốc mất quyền mà coi là rỗng thì lần import sau xoá hết.
    let fx = fixture();
    let root = fx.music.join("Locked");
    let inside = write(&root, "a", Tags::default());
    fx.indexer.import(&root).wait().unwrap();
    let Some(guard) = Locked::new(&root) else {
        eprintln!("skipped: cannot deny access here");
        return;
    };
    let result = fx.indexer.import(&root).wait();
    drop(guard);
    assert!(matches!(result, Err(IndexError::RootUnavailable { .. })));
    assert!(matches!(fx.library.track_for_path(inside).wait().unwrap(), TrackRef::Library(_)));
}
