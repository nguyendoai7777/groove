//! Reading and writing audio tags.
//!
//! The goal of this module is that whatever GrooveX writes is what *every* other
//! player reads back. Three things break that in practice, and each is handled here:
//!
//! 1. **Album grouping keys off `TPE2` (Album Artist), not `TPE1`.** iTunes (and
//!    Plex, and Android players) group by `(album, album_artist)` and silently fall
//!    back to `TPE1` when `TPE2` is empty. A folder of 43 files sharing
//!    `TALB="K Pop"` therefore splits into one album per distinct `TPE1`. Writing a
//!    consistent `TPE2` — plus `TCMP=1` for iTunes' compilation path — collapses
//!    them back into one.
//! 2. **Windows reads ID3v2.3 far better than v2.4.** Explorer writes v2.3, so a
//!    file the app saved as v2.4 shows stale values in the shell. Everything is
//!    written as v2.3.
//! 3. **Legacy containers resurrect deleted fields.** An ID3v1 trailer or an APE
//!    tag left at the end of the file is what re-populates a field after F5. A
//!    clean write removes both, along with sort-order frames (`TSOA`/`TSO2`/`TSOT`)
//!    which make apps disagree about ordering even when `TALB` matches.

use base64::prelude::*;
use id3::TagLike;
use lofty::prelude::*;
use lofty::probe::Probe;
use std::path::Path;

/// Frames a clean write is allowed to emit. Anything else in the source file
/// (`TXXX`, `PRIV`, `TSSE`, `TENC`, `UFID`, the `TSO*` sort-order family, …) is
/// dropped, since those are exactly what make players disagree.
const TEXT_FRAMES: &[(&str, fn(&MetadataPayload) -> Option<String>)] = &[
    ("TIT2", |m| m.title.clone()),
    ("TPE1", |m| m.artist.clone()),
    ("TALB", |m| m.album_name.clone()),
    ("TPE2", |m| m.album_artist.clone()),
    ("TCON", |m| m.genre.clone()),
    ("TCOM", |m| m.composer.clone()),
    ("TPUB", |m| m.publisher.clone()),
    ("TCOP", |m| m.copyright.clone()),
    ("TBPM", |m| m.bpm.clone()),
    ("TIT1", |m| m.grouping.clone()),
    ("TYER", |m| m.year.clone()),
];

/// How a save should treat the frames already in the file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteMode {
    /// Only touch fields present in the payload; leave every other frame alone.
    Merge,
    /// Emit a brand new tag containing *only* the payload. Everything else dies.
    Clean,
    /// Read the file's known fields, overlay the payload, then emit a new tag.
    /// Junk frames die but per-file values (title, track number, …) survive —
    /// this is what a bulk "fix the whole album" pass wants.
    CleanKeepKnown,
}

impl WriteMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "merge" => WriteMode::Merge,
            "clean" => WriteMode::Clean,
            _ => WriteMode::CleanKeepKnown,
        }
    }

    fn strips(self) -> bool {
        self != WriteMode::Merge
    }
}

/// Every editable field, all optional.
///
/// `None` means "not supplied": left untouched in [`WriteMode::Merge`], inherited
/// from the file in [`WriteMode::CleanKeepKnown`], and absent in
/// [`WriteMode::Clean`]. `Some("")` always means "explicitly clear this field",
/// which is how the UI deletes a stubborn value such as `TCOP`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct MetadataPayload {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_name: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<String>,
    pub track_total: Option<String>,
    pub disc_number: Option<String>,
    pub disc_total: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub publisher: Option<String>,
    pub copyright: Option<String>,
    pub comment: Option<String>,
    pub bpm: Option<String>,
    pub grouping: Option<String>,
    pub lyrics: Option<String>,
    /// `TCMP`. Set on every track of a multi-artist album so iTunes keeps them together.
    pub compilation: Option<bool>,
    /// `APIC`, as a `data:` URI. `Some("")` removes the cover.
    pub thumbnail: Option<String>,
}

impl MetadataPayload {
    /// Overlay `other` on top of `self`; any field `other` supplies wins.
    fn overlay(&self, other: &MetadataPayload) -> MetadataPayload {
        macro_rules! pick {
            ($f:ident) => {
                other.$f.clone().or_else(|| self.$f.clone())
            };
        }
        MetadataPayload {
            title: pick!(title),
            artist: pick!(artist),
            album_name: pick!(album_name),
            album_artist: pick!(album_artist),
            track_number: pick!(track_number),
            track_total: pick!(track_total),
            disc_number: pick!(disc_number),
            disc_total: pick!(disc_total),
            year: pick!(year),
            genre: pick!(genre),
            composer: pick!(composer),
            publisher: pick!(publisher),
            copyright: pick!(copyright),
            comment: pick!(comment),
            bpm: pick!(bpm),
            grouping: pick!(grouping),
            lyrics: pick!(lyrics),
            compilation: other.compilation.or(self.compilation),
            thumbnail: pick!(thumbnail),
        }
    }
}

/// One frame exactly as it sits in the file, for the UI's inspector.
#[derive(serde::Serialize, Clone, Debug)]
pub struct RawFrame {
    pub id: String,
    pub name: String,
    pub value: String,
    /// False for frames a clean write would drop, so the UI can flag them.
    pub known: bool,
}

fn clean(s: &str) -> String {
    s.replace('\0', "")
        .replace('\r', "")
        .replace('\n', "")
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

fn clean_multiline(s: &str) -> String {
    s.replace('\0', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{a0}', " ")
        .trim()
        .to_string()
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// `"3"` / `"3/12"` depending on whether a total was supplied.
fn positional(number: &Option<String>, total: &Option<String>) -> Option<String> {
    let n = non_empty(number)?;
    match non_empty(total) {
        Some(t) => Some(format!("{n}/{t}")),
        None => Some(n.to_string()),
    }
}

/// Splits a `"3/12"` positional frame back into its parts.
fn split_positional(raw: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    match raw.split_once('/') {
        Some((n, t)) => (
            Some(n.trim().to_string()),
            Some(t.trim().to_string()).filter(|s| !s.is_empty()),
        ),
        None => (Some(raw.to_string()), None),
    }
}

fn decode_data_uri(uri: &str) -> Option<(String, Vec<u8>)> {
    let comma = uri.find(',')?;
    let mime = if uri.contains("image/jpeg") || uri.contains("image/jpg") {
        "image/jpeg"
    } else if uri.contains("image/webp") {
        "image/webp"
    } else {
        "image/png"
    };
    let bytes = BASE64_STANDARD.decode(&uri[comma + 1..]).ok()?;
    Some((mime.to_string(), bytes))
}

/// Human label for a frame id, used by the inspector.
fn frame_label(id: &str) -> &'static str {
    match id {
        "TIT2" => "Title",
        "TPE1" => "Artist",
        "TPE2" => "Album Artist",
        "TALB" => "Album",
        "TRCK" => "Track",
        "TPOS" => "Disc",
        "TYER" | "TDRC" => "Year",
        "TCON" => "Genre",
        "TCOM" => "Composer",
        "TPUB" => "Publisher",
        "TCOP" => "Copyright",
        "TBPM" => "BPM",
        "TIT1" => "Grouping",
        "TCMP" => "Compilation",
        "COMM" => "Comment",
        "USLT" => "Lyrics",
        "APIC" => "Cover art",
        "TSOA" => "Album sort order (junk)",
        "TSOT" => "Title sort order (junk)",
        "TSOP" => "Artist sort order (junk)",
        "TSO2" => "Album artist sort order (junk)",
        "TXXX" => "Custom text (junk)",
        "PRIV" => "Private data (junk)",
        "TSSE" => "Encoder settings (junk)",
        "TENC" => "Encoded by (junk)",
        "UFID" => "Unique file id (junk)",
        _ => "Other",
    }
}

fn is_known(id: &str) -> bool {
    matches!(
        id,
        "TIT2"
            | "TPE1"
            | "TPE2"
            | "TALB"
            | "TRCK"
            | "TPOS"
            | "TYER"
            | "TCON"
            | "TCOM"
            | "TPUB"
            | "TCOP"
            | "TBPM"
            | "TIT1"
            | "TCMP"
            | "COMM"
            | "USLT"
            | "APIC"
    )
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Reads every supported field. MP3 goes through `id3` (which handles the v2.3
/// frames Windows writes); everything else goes through `lofty`.
pub fn read_metadata(path: &Path) -> MetadataPayload {
    if !path.exists() {
        return MetadataPayload::default();
    }
    if is_mp3(path) {
        read_id3(path)
    } else {
        read_lofty(path)
    }
}

fn is_mp3(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}

fn read_id3(path: &Path) -> MetadataPayload {
    let Ok(tag) = id3::Tag::read_from_path(path) else {
        return MetadataPayload::default();
    };

    let text = |id: &str| tag.get(id).and_then(|f| f.content().text()).map(clean);
    let (track_number, track_total) = split_positional(tag.get("TRCK").and_then(|f| f.content().text()));
    let (disc_number, disc_total) = split_positional(tag.get("TPOS").and_then(|f| f.content().text()));

    let thumbnail = tag.pictures().next().map(|pic| {
        format!(
            "data:{};base64,{}",
            pic.mime_type,
            BASE64_STANDARD.encode(&pic.data)
        )
    });

    // Bound before the struct literal: the frame iterators borrow `tag`, and a
    // temporary iterator inside the literal would outlive it.
    let comment = tag.comments().next().map(|c| clean_multiline(&c.text));
    let lyrics = tag.lyrics().next().map(|l| clean_multiline(&l.text));

    MetadataPayload {
        title: text("TIT2"),
        artist: text("TPE1"),
        album_name: text("TALB"),
        album_artist: text("TPE2"),
        track_number,
        track_total,
        disc_number,
        disc_total,
        // Prefer v2.3's TYER, but accept the v2.4 TDRC a previous save may have left.
        year: text("TYER").or_else(|| text("TDRC").map(|d| d.chars().take(4).collect())),
        genre: text("TCON"),
        composer: text("TCOM"),
        publisher: text("TPUB"),
        copyright: text("TCOP"),
        comment,
        bpm: text("TBPM"),
        grouping: text("TIT1"),
        lyrics,
        compilation: text("TCMP").map(|v| v == "1"),
        thumbnail,
    }
}

fn read_lofty(path: &Path) -> MetadataPayload {
    let Ok(tagged) = Probe::open(path).and_then(|p| p.read()) else {
        return MetadataPayload::default();
    };
    // Prefer the format's primary tag; only fall back to a secondary one. Iterating
    // `tags()` and taking the first hit per field is what let a stale ID3v1 block
    // override the real ID3v2 album.
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return MetadataPayload::default();
    };

    let item = |key: ItemKey| tag.get_string(&key).map(clean);
    let (track_number, track_total) = (
        tag.track().map(|v| v.to_string()),
        tag.track_total().map(|v| v.to_string()),
    );

    let thumbnail = tag.pictures().first().map(|pic| {
        let mime = pic
            .mime_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "image/png".to_string());
        format!("data:{};base64,{}", mime, BASE64_STANDARD.encode(pic.data()))
    });

    MetadataPayload {
        title: item(ItemKey::TrackTitle),
        artist: item(ItemKey::TrackArtist),
        album_name: item(ItemKey::AlbumTitle),
        album_artist: item(ItemKey::AlbumArtist),
        track_number,
        track_total,
        disc_number: tag.disk().map(|v| v.to_string()),
        disc_total: tag.disk_total().map(|v| v.to_string()),
        year: tag.year().map(|y| y.to_string()),
        genre: item(ItemKey::Genre),
        composer: item(ItemKey::Composer),
        publisher: item(ItemKey::Publisher),
        copyright: item(ItemKey::CopyrightMessage),
        comment: item(ItemKey::Comment),
        bpm: item(ItemKey::Bpm),
        grouping: item(ItemKey::ContentGroup),
        lyrics: tag
            .get_string(&ItemKey::Lyrics)
            .map(clean_multiline),
        compilation: item(ItemKey::FlagCompilation).map(|v| v == "1"),
        thumbnail,
    }
}

/// Every frame in the file, in file order, for the UI inspector.
pub fn read_raw_frames(path: &Path) -> Vec<RawFrame> {
    if !path.exists() || !is_mp3(path) {
        return Vec::new();
    }
    let Ok(tag) = id3::Tag::read_from_path(path) else {
        return Vec::new();
    };

    tag.frames()
        .map(|frame| {
            let id = frame.id().to_string();
            let value = match frame.content() {
                id3::Content::Picture(p) => format!("<{} · {} bytes>", p.mime_type, p.data.len()),
                other => {
                    let s = clean_multiline(&other.to_string());
                    if s.chars().count() > 200 {
                        format!("{}…", s.chars().take(200).collect::<String>())
                    } else {
                        s
                    }
                }
            };
            RawFrame {
                name: frame_label(&id).to_string(),
                known: is_known(&id),
                id,
                value,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Writes `payload` to `path` according to `mode`.
///
/// MP3s are always emitted as ID3v2.3, and a stripping mode additionally removes
/// the ID3v1 trailer and any APE tag so nothing reappears on the next refresh.
pub fn write_metadata(path: &Path, payload: &MetadataPayload, mode: WriteMode) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    if is_mp3(path) {
        write_id3(path, payload, mode)
    } else {
        write_lofty(path, payload, mode)
    }
}

fn write_id3(path: &Path, payload: &MetadataPayload, mode: WriteMode) -> Result<(), String> {
    let effective = match mode {
        WriteMode::CleanKeepKnown => read_id3(path).overlay(payload),
        _ => payload.clone(),
    };

    let mut tag = match mode {
        // A fresh tag is the only reliable way to drop junk: `remove` would need an
        // exhaustive list of every frame id a tagger might have invented.
        WriteMode::Clean | WriteMode::CleanKeepKnown => id3::Tag::new(),
        WriteMode::Merge => id3::Tag::read_from_path(path).unwrap_or_else(|_| id3::Tag::new()),
    };

    for (id, get) in TEXT_FRAMES {
        let value = get(&effective);
        // In merge mode an absent field means "don't touch"; in a clean write the
        // tag starts empty, so skipping is equivalent to removing.
        if value.is_none() && mode == WriteMode::Merge {
            continue;
        }
        match non_empty(&value) {
            Some(v) => {
                tag.set_text(*id, v);
            }
            None => {
                tag.remove(*id);
            }
        }
    }

    write_positional(&mut tag, "TRCK", &effective.track_number, &effective.track_total, mode);
    write_positional(&mut tag, "TPOS", &effective.disc_number, &effective.disc_total, mode);

    // TCMP is the flag iTunes uses to keep a multi-artist album together.
    match effective.compilation {
        Some(true) => {
            tag.set_text("TCMP", "1");
        }
        Some(false) => {
            tag.remove("TCMP");
        }
        None => {
            if mode != WriteMode::Merge {
                tag.remove("TCMP");
            }
        }
    }

    if effective.comment.is_some() || mode.strips() {
        tag.remove_comment(None, None);
        if let Some(text) = non_empty(&effective.comment) {
            tag.add_frame(id3::frame::Comment {
                lang: "eng".to_string(),
                description: String::new(),
                text: text.to_string(),
            });
        }
    }

    if effective.lyrics.is_some() || mode.strips() {
        tag.remove_all_lyrics();
        if let Some(text) = non_empty(&effective.lyrics) {
            tag.add_frame(id3::frame::Lyrics {
                lang: "eng".to_string(),
                description: String::new(),
                text: text.to_string(),
            });
        }
    }

    if effective.thumbnail.is_some() || mode.strips() {
        tag.remove_all_pictures();
        if let Some((mime_type, data)) = effective.thumbnail.as_deref().and_then(decode_data_uri) {
            tag.add_frame(id3::Frame::with_content(
                "APIC",
                id3::Content::Picture(id3::frame::Picture {
                    mime_type,
                    picture_type: id3::frame::PictureType::CoverFront,
                    description: "Cover".to_string(),
                    data,
                }),
            ));
        }
    }

    // v2.3, not v2.4: the Windows shell and older players read it reliably.
    tag.write_to_path(path, id3::Version::Id3v23)
        .map_err(|e| format!("Failed to write ID3 tag: {e}"))?;

    if mode.strips() {
        strip_legacy_containers(path);
    }

    Ok(())
}

/// Removes ID3v1 and APE tags from the end of the file.
///
/// Leftovers in these containers are what resurrect a field after a refresh.
/// Taggers that append rather than replace can leave several stacked on top of
/// each other, and each removal only peels off the outermost one, so this repeats
/// until the file end is clean. The bound just stops a malformed file from
/// spinning forever.
fn strip_legacy_containers(path: &Path) {
    for _ in 0..8 {
        let removed_v1 = id3::v1::Tag::remove_from_path(path).unwrap_or(false);
        let removed_ape = lofty::tag::TagType::Ape.remove_from_path(path).is_ok();
        if !removed_v1 && !removed_ape {
            return;
        }
    }
}

fn write_positional(
    tag: &mut id3::Tag,
    id: &str,
    number: &Option<String>,
    total: &Option<String>,
    mode: WriteMode,
) {
    if number.is_none() && total.is_none() && mode == WriteMode::Merge {
        return;
    }
    match positional(number, total) {
        Some(v) => {
            tag.set_text(id, v);
        }
        None => {
            tag.remove(id);
        }
    }
}

fn write_lofty(path: &Path, payload: &MetadataPayload, mode: WriteMode) -> Result<(), String> {
    let effective = match mode {
        WriteMode::CleanKeepKnown => read_lofty(path).overlay(payload),
        _ => payload.clone(),
    };

    let mut tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let tag_type = tagged.primary_tag_type();

    if mode.strips() {
        // Same reasoning as the MP3 path: start from an empty tag of the format's
        // native type, and drop every other tag the file happens to carry.
        for existing in tagged.tags().iter().map(|t| t.tag_type()).collect::<Vec<_>>() {
            tagged.remove(existing);
        }
        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
    } else if tagged.primary_tag().is_none() && tagged.first_tag().is_none() {
        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
    }

    // Resolve the target type up front; borrowing `tagged` twice to fall back from
    // `primary_tag_mut` to `first_tag_mut` is rejected by the borrow checker.
    let target_type = match tagged.primary_tag() {
        Some(t) => t.tag_type(),
        None => tagged
            .first_tag()
            .map(|t| t.tag_type())
            .ok_or_else(|| "No writable tag in file".to_string())?,
    };
    let tag = tagged
        .tag_mut(target_type)
        .ok_or_else(|| "No writable tag in file".to_string())?;

    for (key, value) in [
        (ItemKey::TrackTitle, &effective.title),
        (ItemKey::TrackArtist, &effective.artist),
        (ItemKey::AlbumTitle, &effective.album_name),
        (ItemKey::AlbumArtist, &effective.album_artist),
        (ItemKey::TrackNumber, &effective.track_number),
        (ItemKey::TrackTotal, &effective.track_total),
        (ItemKey::DiscNumber, &effective.disc_number),
        (ItemKey::DiscTotal, &effective.disc_total),
        (ItemKey::Year, &effective.year),
        (ItemKey::RecordingDate, &effective.year),
        (ItemKey::Genre, &effective.genre),
        (ItemKey::Composer, &effective.composer),
        (ItemKey::Publisher, &effective.publisher),
        (ItemKey::CopyrightMessage, &effective.copyright),
        (ItemKey::Comment, &effective.comment),
        (ItemKey::Bpm, &effective.bpm),
        (ItemKey::ContentGroup, &effective.grouping),
        (ItemKey::Lyrics, &effective.lyrics),
    ] {
        match non_empty(value) {
            Some(v) => {
                tag.insert_text(key, v.to_string());
            }
            None => {
                if value.is_some() || mode.strips() {
                    tag.remove_key(&key);
                }
            }
        }
    }

    match effective.compilation {
        Some(true) => {
            tag.insert_text(ItemKey::FlagCompilation, "1".to_string());
        }
        Some(false) => {
            tag.remove_key(&ItemKey::FlagCompilation);
        }
        None => {
            if mode.strips() {
                tag.remove_key(&ItemKey::FlagCompilation);
            }
        }
    }

    if effective.thumbnail.is_some() || mode.strips() {
        while !tag.pictures().is_empty() {
            tag.remove_picture(0);
        }
        if let Some((mime, data)) = effective.thumbnail.as_deref().and_then(decode_data_uri) {
            tag.push_picture(lofty::picture::Picture::new_unchecked(
                lofty::picture::PictureType::CoverFront,
                Some(lofty::picture::MimeType::from_str(&mime)),
                None,
                data,
            ));
        }
    }

    tagged
        .save_to_path(path, lofty::config::WriteOptions::default())
        .map_err(|e| format!("Failed to write tag: {e}"))?;

    if mode.strips() {
        strip_legacy_containers(path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Any MP3 from the user's library, used purely as a valid audio container.
    ///
    /// Tests never assert on its original tags and never write to it: each test
    /// copies it and stamps its own starting state, so results do not depend on
    /// how the library happens to be tagged today.
    fn donor_mp3() -> Option<PathBuf> {
        for dir in [
            r"C:\Users\Mr.DxD\OneDrive\My Music\K Pop",
            r"C:\Users\Mr.DxD\OneDrive\My Music\Pop",
        ] {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_mp3 = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("mp3"))
                    .unwrap_or(false);
                if is_mp3 {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Copies the donor to a scratch file and gives it exactly `frames`, so each
    /// test controls its own precondition.
    fn fixture(name: &str, frames: &[(&str, &str)]) -> Option<PathBuf> {
        let donor = donor_mp3()?;
        let path = std::env::temp_dir().join(name);
        fs::copy(&donor, &path).ok()?;

        let mut tag = id3::Tag::new();
        for (id, value) in frames {
            tag.set_text(*id, *value);
        }
        tag.write_to_path(&path, id3::Version::Id3v23).ok()?;
        Some(path)
    }

    fn frame_ids(path: &Path) -> Vec<String> {
        read_raw_frames(path).into_iter().map(|f| f.id).collect()
    }

    fn id3_version(path: &Path) -> u8 {
        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[0..3], b"ID3", "file should start with an ID3v2 tag");
        bytes[3]
    }

    fn has_id3v1(path: &Path) -> bool {
        let bytes = fs::read(path).unwrap();
        bytes.len() >= 128 && &bytes[bytes.len() - 128..bytes.len() - 125] == b"TAG"
    }

    /// Appends a bare ID3v1 trailer, the leftover that makes a cleared field
    /// reappear in Explorer after a refresh.
    fn append_id3v1(path: &Path, album: &str) {
        let mut block = vec![0u8; 128];
        block[0..3].copy_from_slice(b"TAG");
        let bytes = album.as_bytes();
        block[63..63 + bytes.len()].copy_from_slice(bytes);
        let mut data = fs::read(path).unwrap();
        data.extend_from_slice(&block);
        fs::write(path, data).unwrap();
    }

    #[test]
    fn clean_write_drops_junk_frames_and_legacy_tags() {
        let Some(path) = fixture(
            "groovex_clean_write.mp3",
            &[
                ("TIT2", "Flower Road"),
                ("TALB", "Old Album"),
                ("TCOP", "(C) 2018 Some Label"),
                ("TSSE", "LAME 3.99"),
                ("TSOA", "Old Album"),
                ("TSO2", "Old Album"),
                ("TSOT", "Flower Road"),
                ("TENC", "Some Tagger"),
            ],
        ) else {
            return;
        };
        append_id3v1(&path, "Stale Album");

        assert!(frame_ids(&path).contains(&"TCOP".to_string()));
        assert!(has_id3v1(&path), "fixture should start with an ID3v1 trailer");

        write_metadata(
            &path,
            &MetadataPayload {
                title: Some("Flower Road".into()),
                album_name: Some("K Pop".into()),
                album_artist: Some("K Pop".into()),
                ..Default::default()
            },
            WriteMode::Clean,
        )
        .unwrap();

        let after = frame_ids(&path);
        for junk in ["TCOP", "TSSE", "TSOA", "TSO2", "TSOT", "TENC"] {
            assert!(!after.contains(&junk.to_string()), "{junk} survived: {after:?}");
        }
        assert_eq!(id3_version(&path), 3, "must be written as ID3v2.3");
        assert!(!has_id3v1(&path), "ID3v1 trailer must be stripped");

        let round = read_metadata(&path);
        assert_eq!(round.album_name.as_deref(), Some("K Pop"));
        assert_eq!(round.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(round.title.as_deref(), Some("Flower Road"));
        assert_eq!(round.copyright, None);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn clean_keep_known_preserves_title_while_forcing_grouping() {
        let Some(path) = fixture(
            "groovex_keep_known.mp3",
            &[
                ("TIT2", "Let Me Hear Your Voice"),
                ("TPE1", "BigBang"),
                ("TALB", "K Pop"),
                ("TRCK", "4/12"),
            ],
        ) else {
            return;
        };

        // A bulk album fix supplies only the grouping fields.
        write_metadata(
            &path,
            &MetadataPayload {
                album_name: Some("K Pop".into()),
                album_artist: Some("K Pop".into()),
                compilation: Some(true),
                ..Default::default()
            },
            WriteMode::CleanKeepKnown,
        )
        .unwrap();

        let after = read_metadata(&path);
        assert_eq!(after.title.as_deref(), Some("Let Me Hear Your Voice"));
        assert_eq!(after.artist.as_deref(), Some("BigBang"), "track artist kept");
        assert_eq!(after.track_number.as_deref(), Some("4"));
        assert_eq!(after.track_total.as_deref(), Some("12"));
        assert_eq!(after.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(after.compilation, Some(true));
        assert_eq!(id3_version(&path), 3);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn empty_string_clears_a_stubborn_field() {
        let Some(path) = fixture(
            "groovex_clear_field.mp3",
            &[
                ("TIT2", "Fxxk It"),
                ("TALB", "K Pop"),
                ("TCOP", "(C) 2018 YG Entertainment"),
            ],
        ) else {
            return;
        };
        assert_eq!(
            read_metadata(&path).copyright.as_deref(),
            Some("(C) 2018 YG Entertainment")
        );

        // Some("") is the UI's "delete this", distinct from None's "leave alone".
        write_metadata(
            &path,
            &MetadataPayload {
                copyright: Some(String::new()),
                ..Default::default()
            },
            WriteMode::CleanKeepKnown,
        )
        .unwrap();

        let after = read_metadata(&path);
        assert_eq!(after.copyright, None, "TCOP must be gone");
        assert_eq!(
            after.title.as_deref(),
            Some("Fxxk It"),
            "clearing one field must not disturb the others"
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn merge_mode_leaves_untouched_frames_alone() {
        let Some(path) = fixture(
            "groovex_merge.mp3",
            &[("TIT2", "Bad Boy"), ("TALB", "K Pop"), ("TSOA", "K Pop")],
        ) else {
            return;
        };

        write_metadata(
            &path,
            &MetadataPayload {
                album_artist: Some("K Pop".into()),
                ..Default::default()
            },
            WriteMode::Merge,
        )
        .unwrap();

        let after = read_metadata(&path);
        assert_eq!(after.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(after.album_name.as_deref(), Some("K Pop"), "TALB untouched");
        assert_eq!(after.title.as_deref(), Some("Bad Boy"), "TIT2 untouched");
        assert!(
            frame_ids(&path).contains(&"TSOA".to_string()),
            "merge must leave junk frames in place"
        );

        fs::remove_file(&path).ok();
    }

    /// The regression this work exists for: tracks sharing an album but differing
    /// in artist, with no album artist, are rendered as separate albums by iTunes.
    #[test]
    fn bulk_album_pass_gives_every_track_one_grouping_key() {
        // Mirrors the real K Pop folder: same TALB, several different TPE1 values,
        // TPE2 empty everywhere.
        let starting_artists = ["", "BigBang", "BoA", "ATD Softs Production"];
        let mut copies = Vec::new();
        for (i, artist) in starting_artists.iter().enumerate() {
            let mut frames = vec![("TIT2", "Track"), ("TALB", "K Pop")];
            if !artist.is_empty() {
                frames.push(("TPE1", *artist));
            }
            let Some(path) = fixture(&format!("groovex_bulk_{i}.mp3"), &frames) else {
                return;
            };
            copies.push(path);
        }

        let keys_before: std::collections::HashSet<_> = copies
            .iter()
            .map(|p| {
                let m = read_metadata(p);
                (
                    m.album_name.unwrap_or_default(),
                    m.album_artist.or(m.artist).unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            keys_before.len(),
            starting_artists.len(),
            "fixture should start split across grouping keys, got {keys_before:?}"
        );

        let album_payload = MetadataPayload {
            album_name: Some("K Pop".into()),
            album_artist: Some("K Pop".into()),
            compilation: Some(true),
            ..Default::default()
        };
        for path in &copies {
            write_metadata(path, &album_payload, WriteMode::CleanKeepKnown).unwrap();
        }

        let keys_after: std::collections::HashSet<_> = copies
            .iter()
            .map(|p| {
                let m = read_metadata(p);
                (
                    m.album_name.unwrap_or_default(),
                    m.album_artist.unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            keys_after.len(),
            1,
            "every track must share one grouping key, got {keys_after:?}"
        );
        assert_eq!(
            keys_after.into_iter().next().unwrap(),
            ("K Pop".to_string(), "K Pop".to_string())
        );

        for path in &copies {
            let m = read_metadata(path);
            assert_eq!(m.title.as_deref(), Some("Track"), "per-track title survives");
            assert_eq!(m.compilation, Some(true));
            assert_eq!(id3_version(path), 3, "all files normalised to ID3v2.3");
            assert!(!has_id3v1(path), "ID3v1 trailer stripped from {path:?}");
            fs::remove_file(path).ok();
        }
    }

    #[test]
    fn positional_frames_round_trip() {
        assert_eq!(
            positional(&Some("3".into()), &Some("12".into())).as_deref(),
            Some("3/12")
        );
        assert_eq!(positional(&Some("3".into()), &None).as_deref(), Some("3"));
        assert_eq!(positional(&None, &Some("12".into())), None);
        assert_eq!(
            split_positional(Some("3/12")),
            (Some("3".into()), Some("12".into()))
        );
        assert_eq!(split_positional(Some("3")), (Some("3".into()), None));
        assert_eq!(split_positional(None), (None, None));
    }
}
