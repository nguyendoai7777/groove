//! groove-gd1_18: N2, N3, S5, S7, S8 và báo cáo sửa hàng loạt, trên tệp và DB thật trong thư mục tạm.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use futures::executor::block_on;
use groove_core::model::{CategoryRef, TrackRef};
use groove_library::actor::{LibraryEvent, ProgressKind};
use groove_library::covers::{CoverStore, cover_hash};
use groove_library::db::migrate::CoverRecord;
use groove_library::db::queries;
use groove_library::fixtures::{AudioKind, TINY_PNG, write_sample};
use groove_library::indexer::{TrackRow, apply_scanned_track};
use groove_library::metadata_ops::{FileNameError, GroupEdit, MetadataError, MetadataOps};
use groove_library::store::{Library, LibraryConfig};
use groove_library::tags::{self, CoverEdit, FileStamp, MetadataEdit, TagError, TagFields, WriteMode};
use tempfile::TempDir;

const ALL_KINDS: [AudioKind; 6] =
    [AudioKind::Mp3, AudioKind::Flac, AudioKind::Wav, AudioKind::M4a, AudioKind::M4r, AudioKind::Ogg];

struct Env {
    dir: TempDir,
    library: Library,
    ops: MetadataOps,
}

fn env() -> Env {
    let dir = TempDir::new().unwrap();
    let config = LibraryConfig { target: dir.path().join("data").join("library.db"), legacy: None };
    let sink = Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no legacy covers") });
    let library = Library::open(config, sink).unwrap();
    let covers = CoverStore::new(dir.path().join("covers"));
    let ops = MetadataOps::new(library.clone(), covers).unwrap();
    Env { dir, library, ops }
}

impl Env {
    fn music(&self) -> PathBuf {
        let music = self.dir.path().join("music");
        fs::create_dir_all(&music).unwrap();
        music
    }

    fn sample(&self, stem: &str, kind: AudioKind) -> PathBuf {
        write_sample(&self.music(), stem, kind).unwrap()
    }

    /// Đưa tệp vào thư viện đúng đường của app.
    fn index(&self, path: &Path) -> i64 {
        let row = TrackRow::from_scanned(path.to_owned(), tags::read_track(path).unwrap(), None);
        self.library.writer().transaction(move |conn| Ok(apply_scanned_track(conn, &row)?.id)).wait().unwrap()
    }

    fn song(&self, stem: &str, kind: AudioKind) -> (i64, PathBuf) {
        let path = self.sample(stem, kind);
        (self.index(&path), path)
    }

    fn set_text(&self, id: i64, lyrics: Option<&'static str>, timeline: Option<&'static str>) {
        self.library
            .writer()
            .transaction(move |conn| {
                queries::set_song_lyrics(conn, id, lyrics)?;
                queries::set_song_timeline(conn, id, timeline)?;
                Ok(())
            })
            .wait()
            .unwrap();
    }

    fn text(&self, id: i64) -> queries::SongText {
        self.library.song_text(id).wait().unwrap().unwrap()
    }

    fn exec(&self, sql: &'static str) {
        self.library.writer().transaction(move |conn| Ok(conn.execute_batch(sql)?)).wait().unwrap();
    }

    fn album_cover(&self, id: i64) -> (Option<String>, bool) {
        self.library
            .read(move |conn| {
                Ok(conn.query_row("SELECT cover_hash, cover_is_custom FROM albums WHERE id = ?1", [id], |row| {
                    Ok((row.get(0)?, row.get::<_, i64>(1)? != 0))
                })?)
            })
            .wait()
            .unwrap()
    }

    fn folder_cover(&self, id: i64) -> Option<String> {
        self.library
            .read(move |conn| {
                Ok(conn.query_row("SELECT cover_hash FROM folders WHERE id = ?1", [id], |row| row.get(0))?)
            })
            .wait()
            .unwrap()
    }

    fn album_of(&self, id: i64) -> i64 {
        self.library.song(TrackRef::Library(id)).wait().unwrap().unwrap().album_id.unwrap()
    }

    fn folder_of(&self, id: i64) -> i64 {
        self.library.song(TrackRef::Library(id)).wait().unwrap().unwrap().folder_id.unwrap()
    }

    fn assert_stamp_matches(&self, id: i64, path: &Path) {
        let stamp = FileStamp::of(path).unwrap();
        let row = self.library.song_file_states().wait().unwrap().into_iter().find(|row| row.id == id).unwrap();
        assert_eq!((row.mtime, row.size), (Some(stamp.mtime_ms), Some(stamp.size as i64)));
    }
}

fn edit(fields: TagFields) -> MetadataEdit {
    MetadataEdit { fields, cover: CoverEdit::Keep }
}

fn title(value: &str) -> MetadataEdit {
    edit(TagFields { title: Some(value.into()), ..TagFields::default() })
}

fn folder_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> =
        fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(readonly);
    fs::set_permissions(path, permissions).unwrap();
}

// ------------------------------------------------------------------------------------------------------------- N3

#[test]
fn renaming_onto_another_file_loses_nothing() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let other = env.sample("b", AudioKind::Mp3);
    let (before_a, before_b) = (fs::read(&path).unwrap(), fs::read(&other).unwrap());

    let err =
        env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("b.mp3".into())).wait().unwrap_err();

    assert!(matches!(&err, MetadataError::TargetExists(target) if *target == other));
    assert!(!err.file_written());
    assert_eq!(fs::read(&path).unwrap(), before_a, "thẻ không được ghi trước khi đích qua kiểm");
    assert_eq!(fs::read(&other).unwrap(), before_b);
    assert_eq!(env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap().path, path);
}

#[test]
fn a_database_row_at_the_target_also_blocks_the_rename() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let (_, ghost) = env.song("ghost", AudioKind::Mp3);
    fs::remove_file(&ghost).unwrap();

    let err = env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("ghost.mp3".into())).wait();

    assert!(matches!(err.unwrap_err(), MetadataError::TargetExists(_)));
    assert!(path.exists());
    assert!(!ghost.exists());
}

#[test]
fn names_with_a_path_never_leave_the_folder() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let outside = env.dir.path().join("x.mp3");
    let before = fs::read(&path).unwrap();
    let absolute = outside.to_string_lossy().into_owned();

    for name in ["../x.mp3", "..\\x.mp3", "..", ".mp3", "", absolute.as_str()] {
        let err = env
            .ops
            .save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some(name.to_owned()))
            .wait()
            .unwrap_err();
        assert!(matches!(err, MetadataError::InvalidName(_)), "{name}: {err}");
    }

    assert!(!outside.exists());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn rename_keeps_the_id_and_playlists_and_announces_the_move() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let playlist = env.library.create_playlist("P", "#fff").wait().unwrap();
    env.library.add_song_to_playlist(playlist, id).wait().unwrap();
    let mut events = env.library.subscribe();

    let saved = env
        .ops
        .save(TrackRef::Library(id), title("New"), WriteMode::CleanKeepKnown, Some("  b.mp3 ".into()))
        .wait()
        .unwrap();

    let target = env.music().join("b.mp3");
    assert_eq!(saved.path, target);
    assert_eq!(saved.previous_path.as_deref(), Some(path.as_path()));
    assert_eq!(saved.track, TrackRef::Library(id));
    assert!(!path.exists());
    assert_eq!(tags::read_track(&target).unwrap().fields.title.as_deref(), Some("New"));

    let song = env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!((song.path.as_path(), song.filename.as_str()), (target.as_path(), "b.mp3"));
    assert_eq!(env.library.song_ids(CategoryRef::Playlist(playlist)).wait().unwrap(), [id]);
    env.assert_stamp_matches(id, &target);
    assert_eq!(folder_names(&env.music()), ["b.mp3"], "không sót bản tạm, chỗ giữ tên");

    let mut moved = false;
    let mut finished = false;
    while !(moved && finished) {
        match block_on(events.next()).unwrap() {
            LibraryEvent::Moved(list) => {
                assert_eq!((list[0].id, list[0].to.as_path()), (id, target.as_path()));
                moved = true;
            }
            LibraryEvent::Progress(p) => {
                assert_eq!((p.kind, p.done, p.total), (ProgressKind::MetadataBulk, 1, 1));
                finished = true;
            }
            LibraryEvent::Synced(_) => panic!("sửa trong app không được hiện toast của watcher (S5)"),
            _ => {}
        }
    }
}

#[test]
fn changing_only_the_case_of_the_name_works() {
    let env = env();
    let (id, _) = env.song("a", AudioKind::Mp3);
    env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("A.mp3".into())).wait().unwrap();
    assert_eq!(folder_names(&env.music()), ["A.mp3"]);
    assert_eq!(env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap().filename, "A.mp3");
}

/// N3: decoder đang giữ handle `File::open` vẫn đọc tiếp được sau khi tệp bị thay và đổi tên.
#[test]
fn a_file_being_played_can_be_saved_and_renamed() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let original = fs::read(&path).unwrap();
    let mut playing = fs::File::open(&path).unwrap();

    env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("b.mp3".into())).wait().unwrap();

    let mut still_read = Vec::new();
    playing.read_to_end(&mut still_read).unwrap();
    assert_eq!(still_read, original);
    assert_eq!(folder_names(&env.music()), ["b.mp3"]);
}

#[test]
fn same_name_is_not_a_rename() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let saved =
        env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some(" a.mp3".into())).wait().unwrap();
    assert_eq!(saved.previous_path, None);
    assert_eq!(saved.path, path);
}

#[test]
fn tag_failure_with_rename_changes_nothing() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let before = fs::read(&path).unwrap();
    set_readonly(&path, true);

    let result = env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("b.mp3".into())).wait();
    set_readonly(&path, false);

    assert!(matches!(result.unwrap_err(), MetadataError::Tag(TagError::ReadOnly(_))));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(folder_names(&env.music()), ["a.mp3"]);
    assert_eq!(env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap().path, path);
}

#[test]
fn database_failure_after_rename_puts_the_file_back() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let playlist = env.library.create_playlist("P", "#fff").wait().unwrap();
    env.library.add_song_to_playlist(playlist, id).wait().unwrap();
    env.exec(
        "CREATE TRIGGER boom BEFORE UPDATE ON songs WHEN NEW.filename = 'boom.mp3'
         BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    );

    let err = env
        .ops
        .save(TrackRef::Library(id), title("New"), WriteMode::Merge, Some("boom.mp3".into()))
        .wait()
        .unwrap_err();

    assert!(matches!(err, MetadataError::DatabaseAfterWrite(_)), "{err}");
    assert!(err.file_written());
    assert_eq!(folder_names(&env.music()), ["a.mp3"], "tệp về tên cũ, DB không lệch");
    let song = env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.path, path);
    assert_eq!(song.filename, "a.mp3");
    assert_eq!(env.library.song_ids(CategoryRef::Playlist(playlist)).wait().unwrap(), [id]);
    // Tệp có thẻ mới, DB chưa: dấu tệp khác nên watcher sẽ đọc lại.
    assert_eq!(tags::read_track(&path).unwrap().fields.title.as_deref(), Some("New"));
    let stamp = FileStamp::of(&path).unwrap();
    let row = env.library.song_file_states().wait().unwrap().pop().unwrap();
    assert_ne!(row.mtime.zip(row.size), Some((stamp.mtime_ms, stamp.size as i64)));
}

#[test]
fn external_file_can_be_saved_and_renamed_without_touching_the_library() {
    let env = env();
    let path = env.sample("ext", AudioKind::Flac);
    let saved = env
        .ops
        .save(TrackRef::from_path(&path), title("Out"), WriteMode::Merge, Some("out.flac".into()))
        .wait()
        .unwrap();
    let target = env.music().join("out.flac");
    assert_eq!(saved.path, target);
    assert_eq!(saved.song, None);
    assert_eq!(saved.track, TrackRef::from_path(&target));
    assert_eq!(tags::read_track(&target).unwrap().fields.title.as_deref(), Some("Out"));
    assert!(env.library.song_file_states().wait().unwrap().is_empty());

    let err = env
        .ops
        .save(TrackRef::from_path(env.music().join("none.mp3")), title("x"), WriteMode::Merge, None)
        .wait()
        .unwrap_err();
    assert!(matches!(err, MetadataError::Tag(TagError::NotFound(_))));
}

#[test]
fn invalid_extension_is_reported_by_kind() {
    let env = env();
    let (id, _) = env.song("a", AudioKind::Mp3);
    let err = env.ops.save(TrackRef::Library(id), title("x"), WriteMode::Merge, Some("a.wav".into())).wait();
    assert!(matches!(err.unwrap_err(), MetadataError::InvalidName(FileNameError::ExtensionChanged)));
}

// ------------------------------------------------------------------------------------------------------------- N2

#[test]
fn clean_keeps_lyrics_and_timeline_on_every_format() {
    for kind in ALL_KINDS {
        let env = env();
        let (id, path) = env.song("a", kind);
        env.set_text(id, Some("DB lyrics"), Some("0:10 Intro"));

        env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Clean, None).wait().unwrap();

        let file = tags::read_track(&path).unwrap().fields;
        assert_eq!(file.title.as_deref(), Some("New"), "{kind:?}");
        assert_eq!(file.lyrics.as_deref(), Some("DB lyrics"), "{kind:?}");
        assert_eq!(file.timeline.as_deref(), Some("0:10 Intro"), "{kind:?}: đẩy ngược vào tệp");
        let text = env.text(id);
        assert_eq!(text.lyrics.as_deref(), Some("DB lyrics"), "{kind:?}");
        assert_eq!(text.timeline.as_deref(), Some("0:10 Intro"), "{kind:?}");
        env.assert_stamp_matches(id, &path);
    }
}

#[test]
fn every_mode_pushes_the_database_timeline_into_the_file() {
    for mode in [WriteMode::CleanKeepKnown, WriteMode::Clean, WriteMode::Merge] {
        let env = env();
        let (id, path) = env.song("a", AudioKind::Mp3);
        env.set_text(id, None, Some("1:00 Solo"));
        env.ops.save(TrackRef::Library(id), title("New"), mode, None).wait().unwrap();
        assert_eq!(tags::read_track(&path).unwrap().fields.timeline.as_deref(), Some("1:00 Solo"), "{mode:?}");
        assert_eq!(env.text(id).timeline.as_deref(), Some("1:00 Solo"), "{mode:?}");
    }
}

#[test]
fn saving_does_not_replace_a_timeline_the_file_already_has() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    env.ops.save_timeline(TrackRef::Library(id), "0:01 File".into()).wait().unwrap();
    env.ops.save(TrackRef::Library(id), title("New"), WriteMode::Clean, None).wait().unwrap();
    assert_eq!(tags::read_track(&path).unwrap().fields.timeline.as_deref(), Some("0:01 File"));
}

#[test]
fn clean_on_an_external_file_keeps_its_own_lyrics() {
    let env = env();
    let path = env.sample("ext", AudioKind::Mp3);
    let lyrics = edit(TagFields { lyrics: Some("Own".into()), ..TagFields::default() });
    tags::write_tags(&path, &lyrics, WriteMode::Merge).unwrap();

    env.ops.save(TrackRef::from_path(&path), title("New"), WriteMode::Clean, None).wait().unwrap();

    assert_eq!(tags::read_track(&path).unwrap().fields.lyrics.as_deref(), Some("Own"));
}

#[test]
fn explicit_empty_text_clears_both_file_and_database() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let track = TrackRef::Library(id);
    env.ops.save_lyrics(track.clone(), "L".into()).wait().unwrap();
    env.ops.save_timeline(track.clone(), "0:01 T".into()).wait().unwrap();
    assert_eq!(env.text(id).lyrics.as_deref(), Some("L"));

    env.ops.save_lyrics(track.clone(), String::new()).wait().unwrap();
    env.ops.save_timeline(track, "  ".into()).wait().unwrap();

    let file = tags::read_track(&path).unwrap().fields;
    assert_eq!((file.lyrics, file.timeline), (None, None));
    assert_eq!(env.text(id), queries::SongText::default());
}

#[test]
fn read_comes_from_the_file_and_falls_back_to_the_database_for_text() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    env.set_text(id, Some("DB"), Some("0:01 DB"));
    // Sửa ngoài app: dialog phải thấy bản trong tệp.
    tags::write_tags(&path, &title("Outside"), WriteMode::Merge).unwrap();

    let view = env.ops.read(TrackRef::Library(id)).wait().unwrap();

    assert_eq!(view.fields.title.as_deref(), Some("Outside"));
    assert_eq!(view.fields.lyrics.as_deref(), Some("DB"));
    assert_eq!(view.fields.timeline.as_deref(), Some("0:01 DB"));
    assert_eq!((view.path, view.filename.as_str()), (path, "a.mp3"));
    assert!(view.duration_ms > 0);
    assert!(view.raw_frames.iter().any(|frame| frame.id == "TIT2"));
    assert!(matches!(env.ops.read(TrackRef::Library(999)).wait(), Err(MetadataError::SongNotFound(999))));
}

// ------------------------------------------------------------------------------------------------------------- S5

#[test]
fn lyrics_use_id3v23_and_record_the_stamp() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    let mut events = env.library.subscribe();
    env.ops.save_lyrics(TrackRef::Library(id), "Line".into()).wait().unwrap();

    assert_eq!(id3::Tag::read_from_path(&path).unwrap().version(), id3::Version::Id3v23);
    assert_eq!(env.text(id).lyrics.as_deref(), Some("Line"));
    env.assert_stamp_matches(id, &path);
    assert!(matches!(block_on(events.next()), Some(LibraryEvent::Progress(_))));
}

#[test]
fn lyrics_write_failure_is_returned_and_the_database_keeps_the_old_text() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    env.set_text(id, Some("Old"), None);
    set_readonly(&path, true);
    let result = env.ops.save_lyrics(TrackRef::Library(id), "New".into()).wait();
    set_readonly(&path, false);
    assert!(matches!(result.unwrap_err(), MetadataError::Tag(TagError::ReadOnly(_))));
    assert_eq!(env.text(id).lyrics.as_deref(), Some("Old"));

    fs::remove_file(&path).unwrap();
    let result = env.ops.save_timeline(TrackRef::Library(id), "0:01 X".into()).wait();
    assert!(matches!(result.unwrap_err(), MetadataError::Tag(TagError::NotFound(_))));
    assert_eq!(env.text(id).timeline, None);
}

// ------------------------------------------------------------------------------------------------------------- S7

#[test]
fn apply_scanned_track_writes_every_field_and_seeds_a_new_album_cover() {
    let env = env();
    let path = env.sample("a", AudioKind::Mp3);
    let fields = TagFields {
        album: Some("Album".into()),
        album_artist: Some("Artist".into()),
        lyrics: Some("From file".into()),
        track_no: Some("3".into()),
        disc_no: Some("2".into()),
        ..TagFields::default()
    };
    tags::write_tags(&path, &edit(fields), WriteMode::Merge).unwrap();
    let hash = cover_hash(TINY_PNG);
    let store = CoverStore::new(env.dir.path().join("covers"));
    let record = store.save(TINY_PNG).unwrap();
    let row = TrackRow::from_scanned(path.clone(), tags::read_track(&path).unwrap(), Some(hash.clone()));
    let scanned_duration = row.duration_sec;

    let id = env
        .library
        .writer()
        .transaction(move |conn| {
            queries::upsert_cover(conn, &record)?;
            Ok(apply_scanned_track(conn, &row)?.id)
        })
        .wait()
        .unwrap();

    let song = env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
    assert_eq!(song.album.as_deref(), Some("Album"));
    assert_eq!((song.track_no, song.disc_no), (Some(3), Some(2)));
    assert_eq!(song.duration_sec, scanned_duration);
    assert_eq!(song.cover_hash.as_deref(), Some(hash.as_str()));
    assert_eq!(env.text(id).lyrics.as_deref(), Some("From file"));
    assert_eq!(env.album_cover(song.album_id.unwrap()).0.as_deref(), Some(hash.as_str()));
    assert_eq!(env.folder_cover(song.folder_id.unwrap()).as_deref(), Some(hash.as_str()));
    env.assert_stamp_matches(id, &path);
}

#[test]
fn rescan_picks_up_lyrics_edited_outside_the_app() {
    let env = env();
    let (id, path) = env.song("a", AudioKind::Mp3);
    env.set_text(id, Some("Old"), None);
    let lyrics = edit(TagFields { lyrics: Some("Outside".into()), ..TagFields::default() });
    tags::write_tags(&path, &lyrics, WriteMode::Merge).unwrap();

    assert_eq!(env.index(&path), id);

    assert_eq!(env.text(id).lyrics.as_deref(), Some("Outside"));
}

// ------------------------------------------------------------------------------------------------------------- S8

fn with_cover(album: &str) -> MetadataEdit {
    MetadataEdit {
        fields: TagFields { album: Some(album.into()), album_artist: Some("Artist".into()), ..TagFields::default() },
        cover: CoverEdit::Set(TINY_PNG.to_vec()),
    }
}

#[test]
fn saving_one_song_does_not_change_a_group_cover_that_exists() {
    let env = env();
    let (id, _) = env.song("a", AudioKind::Mp3);
    let track = TrackRef::Library(id);
    env.ops.save(track.clone(), with_cover("Album"), WriteMode::Merge, None).wait().unwrap();
    let (album, folder) = (env.album_of(id), env.folder_of(id));
    let hash = cover_hash(TINY_PNG);
    assert_eq!(env.album_cover(album).0.as_deref(), Some(hash.as_str()), "nhóm chưa có bìa thì gieo");

    env.library
        .writer()
        .transaction(move |conn| {
            queries::set_album_cover(conn, album, Some("other"), false)?;
            queries::set_folder_cover(conn, folder, Some("other"))?;
            Ok(())
        })
        .wait()
        .unwrap();
    let year = edit(TagFields { year: Some("1999".into()), ..TagFields::default() });
    env.ops.save(track, year, WriteMode::Merge, None).wait().unwrap();

    assert_eq!(env.album_cover(album).0.as_deref(), Some("other"));
    assert_eq!(env.folder_cover(folder).as_deref(), Some("other"));
}

#[test]
fn custom_album_cover_is_never_seeded_over() {
    let env = env();
    let (id, _) = env.song("a", AudioKind::Mp3);
    let track = TrackRef::Library(id);
    let named =
        edit(TagFields { album: Some("Album".into()), album_artist: Some("Artist".into()), ..TagFields::default() });
    env.ops.save(track.clone(), named, WriteMode::Merge, None).wait().unwrap();
    let album = env.album_of(id);
    env.exec("UPDATE albums SET cover_hash = NULL, cover_is_custom = 1");

    env.ops.save(track, with_cover("Album"), WriteMode::Merge, None).wait().unwrap();

    assert_eq!(env.album_cover(album), (None, true));
}

#[test]
fn applying_a_cover_to_the_group_changes_it_even_when_custom() {
    let env = env();
    let (a, _) = env.song("a", AudioKind::Mp3);
    let (b, _) = env.song("b", AudioKind::Mp3);
    let album_edit =
        GroupEdit { album: Some("Album".into()), album_artist: Some("Artist".into()), ..GroupEdit::default() };
    let folder = CategoryRef::Folder(env.folder_of(a));
    env.ops.save_bulk(folder, album_edit, WriteMode::Merge).wait().unwrap();
    let album = env.album_of(a);
    env.library
        .writer()
        .transaction(move |conn| Ok(queries::set_album_cover(conn, album, Some("mine"), true)?))
        .wait()
        .unwrap();

    let cover = GroupEdit { cover: CoverEdit::Set(TINY_PNG.to_vec()), ..GroupEdit::default() };
    let report = env.ops.save_bulk(CategoryRef::Album(album), cover, WriteMode::Merge).wait().unwrap();

    assert!(!report.group_cover_failed);
    let hash = cover_hash(TINY_PNG);
    assert_eq!(env.album_cover(album), (Some(hash.clone()), true));
    assert_eq!(report.done.len(), 2);
    for id in [a, b] {
        let song = env.library.song(TrackRef::Library(id)).wait().unwrap().unwrap();
        assert_eq!(song.cover_hash.as_deref(), Some(hash.as_str()));
    }

    let none = GroupEdit { cover: CoverEdit::Remove, ..GroupEdit::default() };
    let folder_id = env.folder_of(a);
    env.ops.save_bulk(CategoryRef::Folder(folder_id), none, WriteMode::Merge).wait().unwrap();
    assert_eq!(env.folder_cover(folder_id), None);
    let song = env.library.song(TrackRef::Library(a)).wait().unwrap().unwrap();
    assert_eq!(song.cover_hash, None);
}

#[test]
fn custom_cover_follows_the_songs_when_fix_grouping_empties_the_album() {
    let env = env();
    let (a, _) = env.song("a", AudioKind::Mp3);
    let (_b, _) = env.song("b", AudioKind::Mp3);
    let folder = env.folder_of(a);
    let first = GroupEdit { album: Some("Album".into()), ..GroupEdit::default() };
    env.ops.save_bulk(CategoryRef::Folder(folder), first, WriteMode::Merge).wait().unwrap();
    let old = env.album_of(a);
    env.library
        .writer()
        .transaction(move |conn| Ok(queries::set_album_cover(conn, old, Some("mine"), true)?))
        .wait()
        .unwrap();

    let fix = GroupEdit { album_artist: Some("Album".into()), compilation: Some(true), ..GroupEdit::default() };
    let report = env.ops.save_bulk(CategoryRef::Album(old), fix, WriteMode::Merge).wait().unwrap();

    assert_eq!(report.done.len(), 2);
    let new = env.album_of(a);
    assert_ne!(new, old);
    assert_eq!(env.album_cover(new), (Some("mine".into()), true));
    assert_eq!(env.library.category(CategoryRef::Album(old)).wait().unwrap(), None, "album rỗng được dọn");
}

// ------------------------------------------------------------------------------------------------------- sửa hàng loạt

#[test]
fn bulk_reports_exactly_which_songs_finished_and_keeps_their_own_values() {
    let env = env();
    let (a, path_a) = env.song("a", AudioKind::Mp3);
    let (b, path_b) = env.song("b", AudioKind::Mp3);
    let (c, path_c) = env.song("c", AudioKind::Flac);
    env.set_text(a, None, Some("0:01 A"));
    tags::write_tags(&path_a, &title("Title A"), WriteMode::Merge).unwrap();
    tags::write_tags(&path_c, &title("Title C"), WriteMode::Merge).unwrap();
    let before_b = fs::read(&path_b).unwrap();
    let folder = env.folder_of(a);
    set_readonly(&path_b, true);
    let mut events = env.library.subscribe();

    let group = GroupEdit { genre: Some("Pop".into()), ..GroupEdit::default() };
    // `Clean` bị hạ xuống: tiêu đề riêng từng bài phải còn.
    let report = env.ops.save_bulk(CategoryRef::Folder(folder), group, WriteMode::Clean).wait().unwrap();
    set_readonly(&path_b, false);

    assert_eq!(report.done, [a, c]);
    assert!(report.not_started.is_empty());
    assert_eq!(report.failed.len(), 1);
    let failure = &report.failed[0];
    assert_eq!((failure.id, failure.path.as_path()), (b, path_b.as_path()));
    assert!(matches!(failure.error, MetadataError::Tag(TagError::ReadOnly(_))));
    assert!(!failure.error.file_written());

    assert_eq!(fs::read(&path_b).unwrap(), before_b);
    let file_a = tags::read_track(&path_a).unwrap().fields;
    let file_c = tags::read_track(&path_c).unwrap().fields;
    assert_eq!((file_a.genre.as_deref(), file_a.title.as_deref()), (Some("Pop"), Some("Title A")));
    assert_eq!((file_c.genre.as_deref(), file_c.title.as_deref()), (Some("Pop"), Some("Title C")));
    assert_eq!(file_a.timeline.as_deref(), Some("0:01 A"), "timeline không mất trên cả album");
    env.assert_stamp_matches(a, &path_a);
    env.assert_stamp_matches(c, &path_c);

    let mut seen = Vec::new();
    while seen.len() < 3 {
        if let LibraryEvent::Progress(p) = block_on(events.next()).unwrap() {
            assert_eq!((p.kind, p.total), (ProgressKind::MetadataBulk, 3));
            seen.push(p.done);
        }
    }
    assert_eq!(seen, [1, 2, 3]);
}

#[test]
fn bulk_separates_a_database_failure_from_a_file_failure() {
    let env = env();
    let (a, path_a) = env.song("a", AudioKind::Mp3);
    let (b, path_b) = env.song("b", AudioKind::Mp3);
    let (c, _) = env.song("c", AudioKind::Mp3);
    let folder = env.folder_of(a);
    env.exec(
        "CREATE TRIGGER boom BEFORE UPDATE ON songs WHEN NEW.filename = 'b.mp3' AND NEW.tags_read_at <> OLD.tags_read_at
         BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    );

    let group = GroupEdit { genre: Some("Rock".into()), ..GroupEdit::default() };
    let report = env.ops.save_bulk(CategoryRef::Folder(folder), group, WriteMode::Merge).wait().unwrap();

    assert_eq!(report.done, [a, c]);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].id, b);
    assert!(report.failed[0].error.file_written(), "thẻ đã ghi, chỉ DB chưa theo");
    assert_eq!(tags::read_track(&path_b).unwrap().fields.genre.as_deref(), Some("Rock"));
    env.assert_stamp_matches(a, &path_a);
}

#[test]
fn bulk_on_a_missing_file_continues_and_an_empty_group_reports_nothing() {
    let env = env();
    let (a, path_a) = env.song("a", AudioKind::Mp3);
    let (b, _) = env.song("b", AudioKind::Mp3);
    let folder = env.folder_of(a);
    fs::remove_file(&path_a).unwrap();

    let report = env.ops.save_bulk(CategoryRef::Folder(folder), GroupEdit::default(), WriteMode::Merge).wait().unwrap();
    assert_eq!(report.done, [b]);
    assert!(matches!(report.failed[0].error, MetadataError::Tag(TagError::NotFound(_))));

    let report = env.ops.save_bulk(CategoryRef::Playlist(999), GroupEdit::default(), WriteMode::Merge).wait().unwrap();
    assert!(report.done.is_empty() && report.failed.is_empty());
}

#[test]
fn every_song_is_accounted_for_when_the_library_closes_during_a_bulk() {
    let env = env();
    let mut ids = Vec::new();
    for index in 0..8 {
        ids.push(env.song(&format!("s{index}"), AudioKind::Mp3).0);
    }
    let folder = env.folder_of(ids[0]);
    let mut events = env.library.subscribe();
    let bulk = env.ops.save_bulk(CategoryRef::Folder(folder), GroupEdit::default(), WriteMode::Merge);
    while !matches!(block_on(events.next()), Some(LibraryEvent::Progress(_))) {}
    block_on(env.library.shutdown());

    let report = bulk.wait().unwrap();

    let mut all: Vec<i64> = report.done.iter().chain(&report.not_started).copied().collect();
    all.extend(report.failed.iter().map(|failure| failure.id));
    all.sort_unstable();
    assert_eq!(all, ids);
    assert!(!report.done.is_empty());
}

#[test]
fn calls_return_immediately_and_work_as_futures() {
    let env = env();
    let (id, _) = env.song("a", AudioKind::Mp3);
    let saved = block_on(env.ops.save_lyrics(TrackRef::Library(id), "F".into())).unwrap();
    assert_eq!(saved.fields.lyrics.as_deref(), Some("F"));
    assert_eq!(saved.song.unwrap().track, TrackRef::Library(id));
}
