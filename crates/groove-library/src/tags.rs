//! groove-gd1_06: đọc ghi thẻ.
//!
//! Mục tiêu: thứ Groove X ghi thì mọi player khác đọc đúng, và ghi thẻ không bao giờ làm mất dữ liệu.
//!
//! 1. Nhóm album theo `TPE2`, nên giữ `TPE2` nhất quán và `TCMP` cho iTunes.
//! 2. Windows đọc ID3v2.3 tốt hơn v2.4, nên luôn ghi v2.3.
//! 3. `ID3v1`, `APE` còn sót làm trường đã xoá hiện lại, nên chế độ clean bỏ cả hai cùng khung sắp xếp `TSO*`.
//! 4. [`APP_FRAMES`] là dữ liệu của app, mọi chế độ ghi đều giữ (N2). Khung chỉnh âm thanh lúc phát (ReplayGain, Sound
//!    Check, độ trễ gapless, `RVA2`…) cũng vậy: làm sạch thẻ không được làm bài nghe khác đi.
//! 6. Số 0 ở số thứ tự, tổng, năm, BPM là "không có": đọc thành trống, ghi thì xoá khung. Trình sửa thẻ khác tự điền 0 vào
//!    ô trống, làm các player sắp xếp theo đó thay vì theo tên.
//! 5. Ghi trên bản tạm, đọc lại kiểm tra rồi mới thay tệp gốc (N3).
//!
//! MP3 và WAV đi qua `id3` (WAV vì `lofty` không cho đặt `TXXX` qua thẻ chung); FLAC, OGG, M4A, M4R đi qua `lofty`.
//! Panic từ tệp độc được bắt thành [`TagError::Panicked`] (S25).

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use id3::TagLike;
use lofty::config::{GlobalOptions, ParseOptions, WriteOptions, apply_global_options};
use lofty::file::FileType;
use lofty::flac::FlacFile;
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst, Mp4File};
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OggPictureStorage, OpusFile, VorbisFile};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{Tag, TagType};

/// Khung của app: mọi chế độ ghi đều giữ, inspector không gắn nhãn rác (N2).
pub const APP_FRAMES: &[&str] = &[TIMELINE];

/// Đuôi nối vào tên tệp cho bản tạm khi ghi thẻ; watcher bỏ qua (N3).
pub const TEMP_SUFFIX: &str = ".groove-tmp";

const TIMELINE: &str = "TIMELINE";

/// Khung ID3 chỉnh âm thanh lúc phát: âm lượng tương đối, EQ, vang.
const SOUND_FRAME_IDS: &[&str] = &["RVA2", "RVAD", "EQU2", "EQUA", "RVRB"];
const MP4_FREEFORM_MEAN: &str = "com.apple.iTunes";

/// Trần khi đọc tệp bất kỳ (S25).
const MAX_FIELD_CHARS: usize = 4 * 1024;
const MAX_MULTILINE_CHARS: usize = 1024 * 1024;
const MAX_COVER_BYTES: usize = 32 * 1024 * 1024;
/// Mặc định của lofty là 16 MiB: bìa lớn hơn làm cả tệp đọc lỗi.
const LOFTY_ALLOCATION_LIMIT: usize = 2 * MAX_COVER_BYTES;
/// Thời lượng lệch quá mức này sau khi ghi tức là đã phá phần âm thanh.
const DURATION_TOLERANCE_MS: u64 = 1_000;

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("file not found: {0}")]
    NotFound(PathBuf),
    #[error("file is read-only: {0}")]
    ReadOnly(PathBuf),
    #[error("unrecognized audio format: {0}")]
    UnsupportedFormat(PathBuf),
    #[error("cover is not a JPEG, PNG, WebP, GIF or BMP image")]
    UnsupportedCover,
    #[error("cover is larger than the limit")]
    CoverTooLarge,
    #[error("cannot read {path}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("cannot write tags to {path}: {message}")]
    Write { path: PathBuf, message: String },
    #[error("read-back after writing found a wrong {field}: {path}")]
    Verify { path: PathBuf, field: &'static str },
    /// Windows: thường do app khác đang mở tệp mà không cho xoá. Tệp gốc còn nguyên.
    #[error("cannot replace {path}, another app may have it open: {source}")]
    Replace { path: PathBuf, source: io::Error },
    #[error("tag library panicked on: {0}")]
    Panicked(PathBuf),
    #[error("i/o error on {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

/// Cách đối xử với thẻ sẵn có trong tệp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// Đọc trường đã biết, phủ phần sửa, ghi thẻ mới: bỏ rác, giữ giá trị riêng từng tệp.
    #[default]
    CleanKeepKnown,
    /// Thẻ mới chỉ gồm phần sửa, [`APP_FRAMES`], khung âm thanh và bìa nếu [`CoverEdit::Keep`]. Lyrics không truyền là mất.
    /// Hộp sửa thẻ luôn ghi chế độ này.
    Clean,
    /// Chỉ đụng trường có trong phần sửa, giữ mọi thứ khác.
    Merge,
}

impl WriteMode {
    fn strips(self) -> bool {
        self != WriteMode::Merge
    }
}

/// Mọi trường sửa được. Khi sửa: `None` là không đụng, `Some("")` là xoá. Khi đọc: `None` là không có.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagFields {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<String>,
    pub track_total: Option<String>,
    pub disc_no: Option<String>,
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
    pub timeline: Option<String>,
    pub compilation: Option<bool>,
}

impl TagFields {
    /// Phủ `edit` lên `self`: trường nào `edit` có thì thắng.
    fn overlay(&self, edit: &TagFields) -> TagFields {
        macro_rules! pick {
            ($($f:ident),+ $(,)?) => {
                TagFields { $($f: edit.$f.clone().or_else(|| self.$f.clone()),)+ compilation: edit.compilation.or(self.compilation) }
            };
        }
        pick!(
            title,
            artist,
            album,
            album_artist,
            track_no,
            track_total,
            disc_no,
            disc_total,
            year,
            genre,
            composer,
            publisher,
            copyright,
            comment,
            bpm,
            grouping,
            lyrics,
            timeline,
        )
    }

    /// Ô số: số thứ tự, tổng của bài và đĩa, năm, BPM.
    fn numbers_mut(&mut self) -> [&mut Option<String>; 6] {
        [
            &mut self.track_no,
            &mut self.track_total,
            &mut self.disc_no,
            &mut self.disc_total,
            &mut self.year,
            &mut self.bpm,
        ]
    }

    /// Làm sạch phần sửa, giữ nghĩa `None` và `Some("")`. Số 0 ở ô số là xoá.
    fn normalized(&self) -> TagFields {
        let line = |v: &Option<String>| v.as_deref().map(clean);
        let multi = |v: &Option<String>| v.as_deref().map(clean_multiline);
        let mut fields = TagFields {
            title: line(&self.title),
            artist: line(&self.artist),
            album: line(&self.album),
            album_artist: line(&self.album_artist),
            track_no: line(&self.track_no),
            track_total: line(&self.track_total),
            disc_no: line(&self.disc_no),
            disc_total: line(&self.disc_total),
            year: line(&self.year),
            genre: line(&self.genre),
            composer: line(&self.composer),
            publisher: line(&self.publisher),
            copyright: line(&self.copyright),
            comment: multi(&self.comment),
            bpm: line(&self.bpm),
            grouping: line(&self.grouping),
            lyrics: multi(&self.lyrics),
            timeline: multi(&self.timeline),
            compilation: self.compilation,
        };
        for value in fields.numbers_mut() {
            if value.as_deref().is_some_and(is_zero) {
                *value = Some(String::new());
            }
        }
        fields
    }
}

/// Sửa bìa (đặc tả 5.8). `Keep` giữ bìa ở mọi chế độ, kể cả [`WriteMode::Clean`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CoverEdit {
    #[default]
    Keep,
    Remove,
    Set(Vec<u8>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetadataEdit {
    pub fields: TagFields,
    pub cover: CoverEdit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverArt {
    pub mime: String,
    pub data: Vec<u8>,
}

/// Dấu tệp để so "tệp có đổi không" (S6). So `(size, mtime_ms)` đúng từng giá trị.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_ms: i64,
}

impl FileStamp {
    pub fn of(path: &Path) -> io::Result<FileStamp> {
        Ok(FileStamp::from_metadata(&fs::metadata(path)?))
    }

    fn from_metadata(meta: &fs::Metadata) -> FileStamp {
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        FileStamp { size: meta.len(), mtime_ms }
    }
}

/// Mọi thứ đọc được từ một tệp trong một lần mở (S14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedTrack {
    pub fields: TagFields,
    pub cover: Option<CoverArt>,
    pub duration_ms: u64,
    pub stamp: FileStamp,
}

/// Có phải bản tạm khi ghi thẻ không; watcher dùng để bỏ qua.
pub fn is_temp_path(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(TEMP_SUFFIX))
}

// ---------------------------------------------------------------------------
// Làm sạch
// ---------------------------------------------------------------------------

/// `\0` là dấu ngăn nhiều giá trị của ID3v2.4, nên thành `; ` thay vì dính liền (S17).
fn clean(s: &str) -> String {
    s.split('\0')
        .map(|part| part.replace(['\r', '\n'], "").replace('\u{a0}', " ").trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn clean_multiline(s: &str) -> String {
    s.replace('\0', "").replace("\r\n", "\n").replace('\r', "\n").replace('\u{a0}', " ").trim().to_string()
}

fn truncated(s: String, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s;
    }
    s.chars().take(max_chars).collect()
}

/// Giá trị đọc từ tệp: sạch, có trần, rỗng thành `None`.
fn read_line(raw: &str) -> Option<String> {
    Some(truncated(clean(raw), MAX_FIELD_CHARS)).filter(|s| !s.is_empty())
}

fn read_multiline(raw: &str) -> Option<String> {
    Some(truncated(clean_multiline(raw), MAX_MULTILINE_CHARS)).filter(|s| !s.is_empty())
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// `"3"` hoặc `"3/12"`.
fn positional(number: &Option<String>, total: &Option<String>) -> Option<String> {
    let n = non_empty(number)?;
    Some(match non_empty(total) {
        Some(t) => format!("{n}/{t}"),
        None => n.to_string(),
    })
}

fn split_positional(raw: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    match raw.split_once('/') {
        Some((n, t)) => (read_line(n), read_line(t)),
        None => (read_line(raw), None),
    }
}

fn year_of(raw: &str) -> Option<String> {
    read_line(raw).map(|d| d.chars().take(4).collect())
}

fn is_app_frame(description: &str) -> bool {
    APP_FRAMES.iter().any(|name| description.eq_ignore_ascii_case(name))
}

/// Khoá tự do (`TXXX`, `COMM`, Vorbis comment, freeform MP4) mang âm lượng hay gapless: ReplayGain, MP3Gain (kể cả dữ liệu
/// hoàn tác), R128 của Opus, Sound Check (`iTunNORM`) và độ trễ bộ mã hoá (`iTunSMPB`) của iTunes.
fn is_sound_key(name: &str) -> bool {
    let upper = name.trim().to_ascii_uppercase();
    ["REPLAYGAIN_", "MP3GAIN_", "R128_"].iter().any(|prefix| upper.starts_with(prefix))
        || upper == "ITUNNORM"
        || upper == "ITUNSMPB"
}

fn is_sound_frame(frame: &id3::Frame) -> bool {
    match frame.content() {
        id3::Content::ExtendedText(t) => is_sound_key(&t.description),
        id3::Content::Comment(c) => is_sound_key(&c.description),
        _ => SOUND_FRAME_IDS.contains(&frame.id()),
    }
}

/// `0`, `00`, `0000`: ô số trình khác tự điền khi để trống.
fn is_zero(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().all(|c| c == '0')
}

/// Mime theo byte đầu của ảnh, thay cho dò chuỗi trong `data:` URI của bản cũ.
fn sniff_image_mime(data: &[u8]) -> Option<&'static str> {
    match data {
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [b'B', b'M', ..] => Some("image/bmp"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("image/webp"),
        _ => None,
    }
}

fn capped_cover(mime: String, data: &[u8], path: &Path) -> Option<CoverArt> {
    if data.len() > MAX_COVER_BYTES {
        log::warn!("skipping {}-byte cover over the limit: {}", data.len(), path.display());
        return None;
    }
    Some(CoverArt { mime, data: data.to_vec() })
}

// ---------------------------------------------------------------------------
// Đọc
// ---------------------------------------------------------------------------

/// Đọc một tệp: thẻ, bìa, thời lượng, dấu tệp. Tệp có âm thanh mà thẻ hỏng vẫn `Ok` với trường trống.
pub fn read_track(path: &Path) -> Result<ScannedTrack, TagError> {
    guarded(path, || read_track_inner(path))
}

fn guarded<T>(path: &Path, work: impl FnOnce() -> Result<T, TagError>) -> Result<T, TagError> {
    // Tuỳ chọn của lofty theo từng luồng, worker nào cũng phải đặt.
    apply_global_options(GlobalOptions::new().allocation_limit(LOFTY_ALLOCATION_LIMIT));
    panic::catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
        log::error!("tag library panicked: {}", path.display());
        Err(TagError::Panicked(path.to_path_buf()))
    })
}

fn open_reader(path: &Path) -> Result<BufReader<File>, TagError> {
    match File::open(path) {
        Ok(file) => Ok(BufReader::new(file)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(TagError::NotFound(path.to_path_buf())),
        Err(source) => Err(TagError::Io { path: path.to_path_buf(), source }),
    }
}

fn rewind(reader: &mut BufReader<File>, path: &Path) -> Result<(), TagError> {
    reader.seek(SeekFrom::Start(0)).map(|_| ()).map_err(|source| TagError::Io { path: path.to_path_buf(), source })
}

/// Nhận định dạng theo nội dung, không theo đuôi: `.mp3` thật ra là M4A vẫn đi đúng nhánh.
fn detect_format(reader: &mut BufReader<File>, path: &Path) -> Result<FileType, TagError> {
    rewind(reader, path)?;
    let guessed = Probe::new(&mut *reader)
        .guess_file_type()
        .map_err(|source| TagError::Io { path: path.to_path_buf(), source })?
        .file_type();
    rewind(reader, path)?;
    guessed.ok_or_else(|| TagError::UnsupportedFormat(path.to_path_buf()))
}

fn uses_id3(format: FileType) -> bool {
    matches!(format, FileType::Mpeg | FileType::Wav)
}

/// Thẻ `id3`, nhận cả thẻ đọc được một phần. `None` khi không có.
fn read_id3(reader: &mut BufReader<File>, path: &Path) -> Option<id3::Tag> {
    rewind(reader, path).ok()?;
    let result = id3::Tag::read_from2(&mut *reader);
    match result {
        Ok(tag) => Some(tag),
        Err(e) => {
            if !matches!(e.kind, id3::ErrorKind::NoTag) {
                log::warn!("damaged ID3 tag in {}: {e}", path.display());
            }
            e.partial_tag
        }
    }
}

fn read_track_inner(path: &Path) -> Result<ScannedTrack, TagError> {
    let mut reader = open_reader(path)?;
    let stamp = reader
        .get_ref()
        .metadata()
        .map(|m| FileStamp::from_metadata(&m))
        .map_err(|source| TagError::Io { path: path.to_path_buf(), source })?;
    let format = detect_format(&mut reader, path)?;

    let id3_tag = if uses_id3(format) { read_id3(&mut reader, path) } else { None };

    rewind(&mut reader, path)?;
    // MP3, WAV đã có thẻ `id3` thì lofty chỉ cần thời lượng. Không có thì lofty đọc `ID3v1`, `APE`, `RIFF INFO`.
    let options = ParseOptions::new().read_tags(id3_tag.is_none());
    let tagged = Probe::new(&mut reader)
        .set_file_type(format)
        .options(options)
        .read()
        .map_err(|e| TagError::Read { path: path.to_path_buf(), message: e.to_string() })?;
    let duration_ms = u64::try_from(tagged.properties().duration().as_millis()).unwrap_or(u64::MAX);

    let (fields, cover) = match &id3_tag {
        Some(tag) => (fields_from_id3(tag), cover_from_id3(tag, path)),
        None => {
            let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
            let mut fields = tag.map(fields_from_generic).unwrap_or_default();
            let cover = tag.and_then(|t| cover_from_generic(t, path));
            fields.timeline = read_app_value(&mut reader, path, format, TIMELINE);
            (fields, cover)
        }
    };
    let mut fields = fields;
    for value in fields.numbers_mut() {
        if value.as_deref().is_some_and(is_zero) {
            *value = None;
        }
    }
    Ok(ScannedTrack { fields, cover, duration_ms, stamp })
}

fn fields_from_id3(tag: &id3::Tag) -> TagFields {
    let raw = |id: &str| tag.get(id).and_then(|f| f.content().text());
    let text = |id: &str| raw(id).and_then(read_line);
    let (track_no, track_total) = split_positional(raw("TRCK"));
    let (disc_no, disc_total) = split_positional(raw("TPOS"));
    // `COMM` của iTunes (`iTunNORM`, `iTunSMPB`) là số liệu âm thanh, không phải ghi chú.
    let comment = tag.comments().find(|c| !is_sound_key(&c.description)).and_then(|c| read_multiline(&c.text));
    let lyrics = tag.lyrics().next().and_then(|l| read_multiline(&l.text));
    let timeline = tag
        .extended_texts()
        .find(|t| t.description.eq_ignore_ascii_case(TIMELINE))
        .and_then(|t| read_multiline(&t.value));
    TagFields {
        title: text("TIT2"),
        artist: text("TPE1"),
        album: text("TALB"),
        album_artist: text("TPE2"),
        track_no,
        track_total,
        disc_no,
        disc_total,
        // `TYER` trước, nhận `TDRC` bản cũ để lại.
        year: text("TYER").or_else(|| raw("TDRC").and_then(year_of)),
        genre: text("TCON"),
        composer: text("TCOM"),
        publisher: text("TPUB"),
        copyright: text("TCOP"),
        comment,
        bpm: text("TBPM"),
        grouping: text("TIT1"),
        lyrics,
        timeline,
        compilation: text("TCMP").map(|v| v == "1"),
    }
}

fn cover_from_id3(tag: &id3::Tag, path: &Path) -> Option<CoverArt> {
    let picture = tag
        .pictures()
        .find(|p| p.picture_type == id3::frame::PictureType::CoverFront)
        .or_else(|| tag.pictures().next())?;
    capped_cover(picture.mime_type.clone(), &picture.data, path)
}

/// Nối mọi giá trị của khoá: Vorbis và MP4 lưu nhiều nghệ sĩ thành nhiều mục (S17).
fn joined(tag: &Tag, key: ItemKey) -> Option<String> {
    read_line(&tag.get_strings(key).collect::<Vec<_>>().join("\0"))
}

fn fields_from_generic(tag: &Tag) -> TagFields {
    let text = |key: ItemKey| joined(tag, key);
    let date = tag.get_string(ItemKey::Year).or_else(|| tag.get_string(ItemKey::RecordingDate));
    TagFields {
        title: text(ItemKey::TrackTitle),
        artist: text(ItemKey::TrackArtist),
        album: text(ItemKey::AlbumTitle),
        album_artist: text(ItemKey::AlbumArtist),
        track_no: tag.track().map(|v| v.to_string()),
        track_total: tag.track_total().map(|v| v.to_string()),
        disc_no: tag.disk().map(|v| v.to_string()),
        disc_total: tag.disk_total().map(|v| v.to_string()),
        year: date.and_then(year_of),
        genre: text(ItemKey::Genre),
        composer: text(ItemKey::Composer),
        publisher: text(ItemKey::Publisher).or_else(|| text(ItemKey::Label)),
        copyright: text(ItemKey::CopyrightMessage),
        comment: tag.get_string(ItemKey::Comment).and_then(read_multiline),
        bpm: text(ItemKey::Bpm),
        grouping: text(ItemKey::ContentGroup),
        lyrics: tag.get_string(ItemKey::Lyrics).and_then(read_multiline),
        timeline: None,
        compilation: tag.get_string(ItemKey::FlagCompilation).map(|v| v == "1"),
    }
}

fn cover_from_generic(tag: &Tag, path: &Path) -> Option<CoverArt> {
    let picture = tag.get_picture_type(PictureType::CoverFront).or_else(|| tag.pictures().first())?;
    let mime = picture
        .mime_type()
        .map(|m| m.as_str().to_string())
        .or_else(|| sniff_image_mime(picture.data()).map(str::to_string))
        .unwrap_or_else(|| "application/octet-stream".to_string());
    capped_cover(mime, picture.data(), path)
}

// --- Khung của app qua lofty: `ItemKey` chung không có khoá tuỳ ý nên phải đọc ghi trên thẻ riêng của từng định dạng. ---

fn mp4_ident(name: &str) -> AtomIdent<'static> {
    AtomIdent::Freeform { mean: Cow::Borrowed(MP4_FREEFORM_MEAN), name: Cow::Owned(name.to_string()) }
}

fn read_app_value(reader: &mut BufReader<File>, path: &Path, format: FileType, name: &str) -> Option<String> {
    rewind(reader, path).ok()?;
    let options = ParseOptions::new().read_properties(false).read_cover_art(false);
    let raw = match format {
        FileType::Flac => {
            let file = FlacFile::read_from(reader, options).ok()?;
            file.vorbis_comments().and_then(|c| c.get(name)).map(str::to_string)
        }
        FileType::Vorbis => {
            VorbisFile::read_from(reader, options).ok()?.vorbis_comments().get(name).map(str::to_string)
        }
        FileType::Opus => OpusFile::read_from(reader, options).ok()?.vorbis_comments().get(name).map(str::to_string),
        FileType::Mp4 => {
            let file = Mp4File::read_from(reader, options).ok()?;
            let atom = file.ilst()?.get(&mp4_ident(name))?.clone();
            atom.data().find_map(|d| match d {
                AtomData::UTF8(v) | AtomData::UTF16(v) => Some(v.clone()),
                _ => None,
            })
        }
        _ => None,
    };
    raw.as_deref().and_then(read_multiline)
}

/// Khoá thẻ mới phải mang theo khi làm sạch: khung của app và khoá âm thanh, đúng tên như trong tệp.
fn read_kept_values(reader: &mut BufReader<File>, path: &Path, format: FileType) -> Vec<(String, String)> {
    if rewind(reader, path).is_err() {
        return Vec::new();
    }
    let options = ParseOptions::new().read_properties(false).read_cover_art(false);
    let vorbis = |comments: &VorbisComments| -> Vec<(String, String)> {
        comments.items().map(|(key, value)| (key.to_string(), value.to_string())).collect()
    };
    let pairs = match format {
        FileType::Flac => FlacFile::read_from(reader, options).ok().and_then(|f| f.vorbis_comments().map(vorbis)),
        FileType::Vorbis => VorbisFile::read_from(reader, options).ok().map(|f| vorbis(f.vorbis_comments())),
        FileType::Opus => OpusFile::read_from(reader, options).ok().map(|f| vorbis(f.vorbis_comments())),
        FileType::Mp4 => Mp4File::read_from(reader, options).ok().and_then(|file| {
            let ilst = file.ilst()?;
            let pairs = ilst
                .into_iter()
                .filter_map(|atom| {
                    let AtomIdent::Freeform { mean, name } = atom.ident() else { return None };
                    if mean != MP4_FREEFORM_MEAN {
                        return None;
                    }
                    let value = atom.data().find_map(|d| match d {
                        AtomData::UTF8(v) | AtomData::UTF16(v) => Some(v.clone()),
                        _ => None,
                    })?;
                    Some((name.to_string(), value))
                })
                .collect();
            Some(pairs)
        }),
        _ => None,
    };
    pairs
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, _)| is_app_frame(name) || is_sound_key(name))
        .filter_map(|(name, value)| read_multiline(&value).map(|value| (name, value)))
        .collect()
}

// ---------------------------------------------------------------------------
// Ghi
// ---------------------------------------------------------------------------

/// Ghi thẻ an toàn: chép ra `<tên>.groove-tmp` cùng thư mục, ghi trên bản tạm, đọc lại kiểm tra, rồi thay tệp gốc (N3).
/// Lỗi ở đâu thì bản tạm bị xoá và tệp gốc nguyên vẹn. Trả dấu tệp sau khi ghi để lưu vào DB (S6).
///
/// Đổi tên tệp và cập nhật DB là việc của `metadata_ops`.
pub fn write_tags(path: &Path, edit: &MetadataEdit, mode: WriteMode) -> Result<FileStamp, TagError> {
    guarded(path, || write_tags_inner(path, edit, mode))
}

/// Xoá bản tạm khi ra khỏi phạm vi, trừ khi đã đổi tên thành công.
struct TempFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.armed
            && let Err(e) = fs::remove_file(&self.path)
            && e.kind() != io::ErrorKind::NotFound
        {
            log::warn!("cannot remove temp file {}: {e}", self.path.display());
        }
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(TEMP_SUFFIX);
    path.with_file_name(name)
}

fn write_tags_inner(path: &Path, edit: &MetadataEdit, mode: WriteMode) -> Result<FileStamp, TagError> {
    let io_err = |source: io::Error| TagError::Io { path: path.to_path_buf(), source };
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(TagError::NotFound(path.to_path_buf())),
        Err(e) => return Err(io_err(e)),
    };
    // Đổi tên đè lên symlink sẽ thay chính symlink, nên đi tới tệp thật.
    let target =
        if meta.file_type().is_symlink() { fs::canonicalize(path).map_err(io_err)? } else { path.to_path_buf() };
    if fs::metadata(&target).map_err(io_err)?.permissions().readonly() {
        return Err(TagError::ReadOnly(target));
    }

    let fields = edit.fields.normalized();
    let edit = MetadataEdit { fields, cover: edit.cover.clone() };
    if let CoverEdit::Set(data) = &edit.cover {
        if sniff_image_mime(data).is_none() {
            return Err(TagError::UnsupportedCover);
        }
        if data.len() > MAX_COVER_BYTES {
            return Err(TagError::CoverTooLarge);
        }
    }

    let before = read_track_inner(&target)?;
    let temp = TempFile { path: temp_path_for(&target), armed: true };
    fs::copy(&target, &temp.path).map_err(|source| TagError::Io { path: temp.path.clone(), source })?;

    let mut reader = open_reader(&temp.path)?;
    let format = detect_format(&mut reader, &temp.path)?;
    if uses_id3(format) {
        let old = read_id3(&mut reader, &temp.path);
        drop(reader);
        write_id3(&temp.path, format, &edit, mode, &before, old)?;
    } else {
        drop(reader);
        write_lofty(&temp.path, format, &edit, mode, &before)?;
    }

    verify(&temp.path, &edit, &before)?;

    File::options()
        .write(true)
        .open(&temp.path)
        .and_then(|f| f.sync_all())
        .map_err(|source| TagError::Io { path: temp.path.clone(), source })?;
    fs::rename(&temp.path, &target).map_err(|source| TagError::Replace { path: target.clone(), source })?;
    let mut temp = temp;
    temp.armed = false;

    FileStamp::of(&target).map_err(io_err)
}

/// Đọc lại bản tạm: trường đã sửa phải đúng, khung của app phải còn, phần âm thanh phải như cũ.
fn verify(temp: &Path, edit: &MetadataEdit, before: &ScannedTrack) -> Result<(), TagError> {
    let wrong = |field: &'static str| TagError::Verify { path: temp.to_path_buf(), field };
    let after = read_track_inner(temp)?;

    let edited = &edit.fields;
    for (field, wanted, got) in [
        ("title", &edited.title, &after.fields.title),
        ("artist", &edited.artist, &after.fields.artist),
        ("album", &edited.album, &after.fields.album),
        ("album_artist", &edited.album_artist, &after.fields.album_artist),
        ("lyrics", &edited.lyrics, &after.fields.lyrics),
        ("timeline", &edited.timeline, &after.fields.timeline),
    ] {
        if let Some(value) = wanted
            && got.as_deref().unwrap_or("") != value.trim()
        {
            return Err(wrong(field));
        }
    }
    if edited.timeline.is_none() && after.fields.timeline != before.fields.timeline {
        return Err(wrong("timeline"));
    }

    let cover_ok = match &edit.cover {
        CoverEdit::Keep => after.cover.as_ref().map(|c| &c.data) == before.cover.as_ref().map(|c| &c.data),
        CoverEdit::Remove => after.cover.is_none(),
        CoverEdit::Set(data) => after.cover.as_ref().is_some_and(|c| &c.data == data),
    };
    if !cover_ok {
        return Err(wrong("cover"));
    }

    if before.duration_ms.abs_diff(after.duration_ms) > DURATION_TOLERANCE_MS {
        return Err(wrong("duration"));
    }
    Ok(())
}

/// Trường cuối cùng cần ghi theo chế độ.
fn effective_fields(edit: &TagFields, mode: WriteMode, before: &TagFields) -> TagFields {
    match mode {
        WriteMode::CleanKeepKnown => before.overlay(edit),
        WriteMode::Clean => {
            // Khung của app luôn đi theo tệp.
            let mut fields = edit.clone();
            fields.timeline = edit.timeline.clone().or_else(|| before.timeline.clone());
            fields
        }
        WriteMode::Merge => edit.clone(),
    }
}

type FieldGetter = fn(&TagFields) -> &Option<String>;

const ID3_TEXT_FRAMES: &[(&str, FieldGetter)] = &[
    ("TIT2", |f| &f.title),
    ("TPE1", |f| &f.artist),
    ("TALB", |f| &f.album),
    ("TPE2", |f| &f.album_artist),
    ("TCON", |f| &f.genre),
    ("TCOM", |f| &f.composer),
    ("TPUB", |f| &f.publisher),
    ("TCOP", |f| &f.copyright),
    ("TBPM", |f| &f.bpm),
    ("TIT1", |f| &f.grouping),
    ("TYER", |f| &f.year),
];

fn write_id3(
    temp: &Path,
    format: FileType,
    edit: &MetadataEdit,
    mode: WriteMode,
    before: &ScannedTrack,
    old: Option<id3::Tag>,
) -> Result<(), TagError> {
    let effective = effective_fields(&edit.fields, mode, &before.fields);
    let sound: Vec<id3::Frame> =
        old.iter().flat_map(|tag| tag.frames()).filter(|f| is_sound_frame(f)).cloned().collect();

    let mut tag = id3::Tag::new();
    if mode.strips() {
        if let Some(old) = &old {
            // Thẻ mới là cách duy nhất chắc chắn bỏ được rác, nên chép lại phần phải giữ.
            for frame in old.frames() {
                let keep = match frame.content() {
                    id3::Content::ExtendedText(t) => is_app_frame(&t.description),
                    id3::Content::Picture(_) => edit.cover == CoverEdit::Keep,
                    _ => false,
                };
                if keep {
                    tag.add_frame(frame.clone());
                }
            }
        }
    } else if let Some(old) = old {
        tag = old;
    }

    for (id, get) in ID3_TEXT_FRAMES {
        let value = get(&effective);
        if value.is_none() && !mode.strips() {
            continue;
        }
        match non_empty(value) {
            Some(v) => {
                tag.set_text(*id, v);
            }
            None => {
                tag.remove(*id);
            }
        }
    }
    // `TDRC` còn sót sẽ làm năm đã sửa hoặc đã xoá hiện lại.
    if effective.year.is_some() {
        tag.remove("TDRC");
    }

    // Merge chỉ sửa tổng số thì vẫn phải giữ số thứ tự, nên phủ lên giá trị trong tệp.
    write_positional(
        &mut tag,
        "TRCK",
        &effective.track_no,
        &effective.track_total,
        &before.fields.track_no,
        &before.fields.track_total,
        mode,
    );
    write_positional(
        &mut tag,
        "TPOS",
        &effective.disc_no,
        &effective.disc_total,
        &before.fields.disc_no,
        &before.fields.disc_total,
        mode,
    );

    match effective.compilation {
        Some(true) => {
            tag.set_text("TCMP", "1");
        }
        Some(false) => {
            tag.remove("TCMP");
        }
        None => {}
    }

    if let Some(comment) = &effective.comment {
        tag.remove_comment(None, None);
        if !comment.is_empty() {
            tag.add_frame(id3::frame::Comment {
                lang: "eng".into(),
                description: String::new(),
                text: comment.clone(),
            });
        }
    }
    if let Some(lyrics) = &effective.lyrics {
        tag.remove_all_lyrics();
        if !lyrics.is_empty() {
            tag.add_frame(id3::frame::Lyrics { lang: "eng".into(), description: String::new(), text: lyrics.clone() });
        }
    }
    if let Some(timeline) = &effective.timeline {
        let stale: Vec<String> = tag
            .extended_texts()
            .filter(|t| t.description.eq_ignore_ascii_case(TIMELINE))
            .map(|t| t.description.clone())
            .collect();
        for description in stale {
            tag.remove_extended_text(Some(&description), None);
        }
        if !timeline.is_empty() {
            tag.add_frame(id3::frame::ExtendedText { description: TIMELINE.into(), value: timeline.clone() });
        }
    }

    // Thẻ mới không có, còn sửa ghi chú thì xoá mọi `COMM` kể cả Sound Check: trả khung âm thanh về sau cùng.
    for frame in sound {
        if !tag.frames().any(|f| *f == frame) {
            tag.add_frame(frame);
        }
    }

    match &edit.cover {
        CoverEdit::Keep => {}
        CoverEdit::Remove => tag.remove_all_pictures(),
        CoverEdit::Set(data) => {
            tag.remove_all_pictures();
            tag.add_frame(id3::frame::Picture {
                mime_type: sniff_image_mime(data).unwrap_or("image/jpeg").to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "Cover".to_string(),
                data: data.clone(),
            });
        }
    }

    normalize_multi_value_frames(&mut tag);

    tag.write_to_path(temp, id3::Version::Id3v23)
        .map_err(|e| TagError::Write { path: temp.to_path_buf(), message: e.to_string() })?;

    if mode.strips() {
        strip_legacy_containers(temp, format);
    }
    Ok(())
}

fn write_positional(
    tag: &mut id3::Tag,
    id: &str,
    number: &Option<String>,
    total: &Option<String>,
    file_number: &Option<String>,
    file_total: &Option<String>,
    mode: WriteMode,
) {
    if mode == WriteMode::Merge && number.is_none() && total.is_none() {
        return;
    }
    let (number, total) = if mode == WriteMode::Merge {
        (number.as_ref().or(file_number.as_ref()).cloned(), total.as_ref().or(file_total.as_ref()).cloned())
    } else {
        (number.clone(), total.clone())
    };
    match positional(&number, &total) {
        Some(v) => {
            tag.set_text(id, v);
        }
        None => {
            tag.remove(id);
        }
    }
}

/// v2.3 không có `\0` ngăn giá trị: khung còn lại khi Merge cũng đổi sang `; ` (S17).
fn normalize_multi_value_frames(tag: &mut id3::Tag) {
    let multi: Vec<(String, String)> = tag
        .frames()
        .filter_map(|f| {
            let text = f.content().text()?;
            text.contains('\0').then(|| (f.id().to_string(), clean(text)))
        })
        .collect();
    for (id, text) in multi {
        tag.set_text(id, text);
    }
}

/// Bỏ `ID3v1`, `APE` ở cuối tệp, `RIFF INFO` của WAV. Trình gắn thẻ có thể để lại chồng nhiều lớp nên lặp tới khi hết;
/// trần chỉ để tệp dị dạng không làm quay mãi.
fn strip_legacy_containers(path: &Path, format: FileType) {
    if format == FileType::Wav {
        let _ = TagType::RiffInfo.remove_from_path(path, WriteOptions::default());
        return;
    }
    for _ in 0..8 {
        let removed_v1 = id3::v1::Tag::remove_from_path(path).unwrap_or(false);
        let removed_ape = remove_trailing_ape(path).unwrap_or(false);
        if !removed_v1 && !removed_ape {
            return;
        }
    }
}

/// Cắt `APEv2` theo footer 32 byte ở cuối tệp. Tự làm vì `TagType::Ape.remove_from_path` để sót thẻ trên MP3.
fn remove_trailing_ape(path: &Path) -> io::Result<bool> {
    const FOOTER_LEN: u64 = 32;
    const HAS_HEADER: u32 = 1 << 31;
    let mut file = File::options().read(true).write(true).open(path)?;
    let len = file.metadata()?.len();
    if len < FOOTER_LEN {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(len - FOOTER_LEN))?;
    let mut footer = [0u8; FOOTER_LEN as usize];
    file.read_exact(&mut footer)?;
    if &footer[..8] != b"APETAGEX" {
        return Ok(false);
    }
    let field = |at: usize| u32::from_le_bytes([footer[at], footer[at + 1], footer[at + 2], footer[at + 3]]);
    let header = if field(20) & HAS_HEADER != 0 { FOOTER_LEN } else { 0 };
    let total = u64::from(field(12)) + header;
    if total < FOOTER_LEN || total > len {
        return Ok(false);
    }
    file.set_len(len - total)?;
    Ok(true)
}

/// Ghi qua thẻ riêng của định dạng thay vì `TaggedFile`: `TaggedFile` lưu lại từ thẻ chung nên làm rơi mọi khoá lofty không
/// biết, kể cả `TIMELINE`. `Tag::from(thẻ riêng)` giữ phần đó trong `companion_tag`, chuyển ngược lại là còn nguyên.
fn write_lofty(
    temp: &Path,
    format: FileType,
    edit: &MetadataEdit,
    mode: WriteMode,
    before: &ScannedTrack,
) -> Result<(), TagError> {
    let effective = effective_fields(&edit.fields, mode, &before.fields);
    let mut reader = open_reader(temp)?;
    let parse = |e: lofty::error::FileParseError| TagError::Read { path: temp.to_path_buf(), message: e.to_string() };
    // Thẻ mới làm mất khung của app và khoá âm thanh nên đọc trước; Merge đã có `companion_tag`.
    let mut app_values = if mode.strips() { read_kept_values(&mut reader, temp, format) } else { Vec::new() };
    if let Some(timeline) = &effective.timeline {
        app_values.retain(|(name, _)| !name.eq_ignore_ascii_case(TIMELINE));
        app_values.push((TIMELINE.to_string(), timeline.clone()));
    }
    rewind(&mut reader, temp)?;

    let tag_type = format.primary_tag_type();
    let options = WriteOptions::default().remove_others(mode.strips());
    let edited = |tag: Tag| edit_generic(tag, tag_type, &effective, &edit.cover, mode);
    let saved = match format {
        FileType::Flac => {
            let mut file = FlacFile::read_from(&mut reader, ParseOptions::new()).map_err(parse)?;
            drop(reader);
            let mut tag = Tag::from(file.vorbis_comments().cloned().unwrap_or_default());
            // Bìa FLAC nằm ở khối riêng của tệp; `set_vorbis_comments` tách bìa trong thẻ về lại khối riêng.
            for (picture, _) in file.pictures() {
                tag.push_picture(picture.clone());
            }
            file.remove_pictures();
            let mut comments = VorbisComments::from(edited(tag));
            set_comment_values(&mut comments, &app_values);
            file.set_vorbis_comments(comments);
            file.save_to_path(temp, options)
        }
        FileType::Vorbis => {
            let mut file = VorbisFile::read_from(&mut reader, ParseOptions::new()).map_err(parse)?;
            drop(reader);
            let mut comments = VorbisComments::from(edited(Tag::from(file.vorbis_comments().clone())));
            set_comment_values(&mut comments, &app_values);
            file.set_vorbis_comments(comments);
            file.save_to_path(temp, options)
        }
        FileType::Opus => {
            let mut file = OpusFile::read_from(&mut reader, ParseOptions::new()).map_err(parse)?;
            drop(reader);
            let mut comments = VorbisComments::from(edited(Tag::from(file.vorbis_comments().clone())));
            set_comment_values(&mut comments, &app_values);
            file.set_vorbis_comments(comments);
            file.save_to_path(temp, options)
        }
        FileType::Mp4 => {
            let mut file = Mp4File::read_from(&mut reader, ParseOptions::new()).map_err(parse)?;
            drop(reader);
            let mut ilst = Ilst::from(edited(Tag::from(file.ilst().cloned().unwrap_or_default())));
            set_ilst_values(&mut ilst, &app_values);
            file.set_ilst(ilst);
            file.save_to_path(temp, options)
        }
        _ => return Err(TagError::UnsupportedFormat(temp.to_path_buf())),
    };
    saved.map_err(|e| TagError::Write { path: temp.to_path_buf(), message: e.to_string() })
}

/// Xoá hết các khoá trước rồi mới thêm: một khoá có thể mang nhiều giá trị.
fn set_comment_values(comments: &mut VorbisComments, values: &[(String, String)]) {
    for (name, _) in values {
        let _ = comments.remove(name).count();
    }
    for (name, value) in values {
        if !value.is_empty() {
            comments.insert(name.clone(), value.clone());
        }
    }
}

fn set_ilst_values(ilst: &mut Ilst, values: &[(String, String)]) {
    for (name, _) in values {
        let _ = ilst.remove(&mp4_ident(name)).count();
    }
    for (name, value) in values {
        if !value.is_empty() {
            ilst.insert(Atom::new(mp4_ident(name), AtomData::UTF8(value.clone())));
        }
    }
}

/// Áp phần sửa lên thẻ chung. Chế độ clean bắt đầu từ thẻ trống, giữ lại bìa nếu [`CoverEdit::Keep`].
fn edit_generic(existing: Tag, tag_type: TagType, effective: &TagFields, cover: &CoverEdit, mode: WriteMode) -> Tag {
    let mut tag = existing;
    if mode.strips() {
        let pictures: Vec<Picture> = if *cover == CoverEdit::Keep { tag.pictures().to_vec() } else { Vec::new() };
        tag = Tag::new(tag_type);
        for picture in pictures {
            tag.push_picture(picture);
        }
    }

    for (key, value) in [
        (ItemKey::TrackTitle, &effective.title),
        (ItemKey::TrackArtist, &effective.artist),
        (ItemKey::AlbumTitle, &effective.album),
        (ItemKey::AlbumArtist, &effective.album_artist),
        (ItemKey::TrackNumber, &effective.track_no),
        (ItemKey::TrackTotal, &effective.track_total),
        (ItemKey::DiscNumber, &effective.disc_no),
        (ItemKey::DiscTotal, &effective.disc_total),
        (ItemKey::Year, &effective.year),
        (ItemKey::RecordingDate, &effective.year),
        (ItemKey::Genre, &effective.genre),
        (ItemKey::Composer, &effective.composer),
        (publisher_key(tag_type), &effective.publisher),
        (ItemKey::CopyrightMessage, &effective.copyright),
        (ItemKey::Comment, &effective.comment),
        (ItemKey::Bpm, &effective.bpm),
        (ItemKey::ContentGroup, &effective.grouping),
        (ItemKey::Lyrics, &effective.lyrics),
    ] {
        if value.is_none() {
            continue;
        }
        tag.remove_key(key);
        if let Some(v) = non_empty(value) {
            tag.insert_text(key, v.to_string());
        }
    }

    match effective.compilation {
        Some(true) => {
            tag.insert_text(ItemKey::FlagCompilation, "1".to_string());
        }
        Some(false) => tag.remove_key(ItemKey::FlagCompilation),
        None => {}
    }

    match cover {
        CoverEdit::Keep => {}
        CoverEdit::Remove => remove_pictures(&mut tag),
        CoverEdit::Set(data) => {
            remove_pictures(&mut tag);
            let mut builder = Picture::unchecked(data.clone()).pic_type(PictureType::CoverFront);
            if let Some(mime) = sniff_image_mime(data) {
                builder = builder.mime_type(MimeType::from_str(mime));
            }
            tag.push_picture(builder.build());
        }
    }
    tag
}

/// MP4 không có khoá `Publisher`, lofty sẽ âm thầm bỏ; `LABEL` là khoá các app khác dùng cho nó.
fn publisher_key(tag_type: TagType) -> ItemKey {
    if ItemKey::Publisher.map_key(tag_type).is_some() { ItemKey::Publisher } else { ItemKey::Label }
}

fn remove_pictures(tag: &mut Tag) {
    while !tag.pictures().is_empty() {
        tag.remove_picture(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{self, AudioKind, TINY_PNG};
    use std::collections::HashSet;

    const MODES: [WriteMode; 3] = [WriteMode::Clean, WriteMode::CleanKeepKnown, WriteMode::Merge];

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn fields_edit(fields: TagFields) -> MetadataEdit {
        MetadataEdit { fields, cover: CoverEdit::Keep }
    }

    fn title_edit(title: &str) -> MetadataEdit {
        fields_edit(TagFields { title: Some(title.into()), ..Default::default() })
    }

    fn timeline_edit(value: &str) -> MetadataEdit {
        fields_edit(TagFields { timeline: Some(value.into()), ..Default::default() })
    }

    fn frame_ids(path: &Path) -> Vec<String> {
        id3::Tag::read_from_path(path).unwrap().frames().map(|f| f.id().to_string()).collect()
    }

    fn id3_version(path: &Path) -> u8 {
        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[0..3], b"ID3", "tệp phải mở đầu bằng ID3v2");
        bytes[3]
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    fn no_temp_left(dir: &Path) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            assert!(!is_temp_path(&entry.path()), "sót bản tạm: {:?}", entry.path());
        }
    }

    fn write_app_values(path: &Path, format: FileType, values: &[(&str, String)]) {
        let values: Vec<(String, String)> =
            values.iter().map(|(name, value)| (name.to_string(), value.clone())).collect();
        let values = values.as_slice();
        let mut reader = open_reader(path).unwrap();
        match format {
            FileType::Flac => {
                let mut file = FlacFile::read_from(&mut reader, ParseOptions::new()).unwrap();
                drop(reader);
                let mut comments = file.vorbis_comments().cloned().unwrap_or_default();
                set_comment_values(&mut comments, values);
                file.set_vorbis_comments(comments);
                file.save_to_path(path, WriteOptions::default()).unwrap();
            }
            FileType::Vorbis => {
                let mut file = VorbisFile::read_from(&mut reader, ParseOptions::new()).unwrap();
                drop(reader);
                set_comment_values(file.vorbis_comments_mut(), values);
                file.save_to_path(path, WriteOptions::default()).unwrap();
            }
            FileType::Mp4 => {
                let mut file = Mp4File::read_from(&mut reader, ParseOptions::new()).unwrap();
                drop(reader);
                let mut ilst = file.ilst().cloned().unwrap_or_default();
                set_ilst_values(&mut ilst, values);
                file.set_ilst(ilst);
                file.save_to_path(path, WriteOptions::default()).unwrap();
            }
            other => panic!("{other:?}"),
        }
    }

    // --- C1: tệp mẫu ---

    #[test]
    fn fixtures_are_readable_audio_of_about_one_second() {
        let dir = tmp();
        for kind in AudioKind::ALL {
            let path = fixtures::write_sample(dir.path(), "a", kind).unwrap();
            let track = read_track(&path).unwrap();
            assert!((800..1_400).contains(&track.duration_ms), "{kind:?} dài {} ms", track.duration_ms);
            assert_eq!(track.fields, TagFields::default(), "{kind:?}");
            assert_eq!(track.stamp, FileStamp::of(&path).unwrap());
        }
        image::load_from_memory(TINY_PNG).expect("PNG mẫu hợp lệ");
    }

    // --- C2: khứ hồi mọi định dạng ---

    #[test]
    fn every_format_round_trips_all_fields() {
        let dir = tmp();
        let wanted = TagFields {
            title: Some("Tựa đề".into()),
            artist: Some("Nghệ sĩ".into()),
            album: Some("Album".into()),
            album_artist: Some("Album Artist".into()),
            track_no: Some("3".into()),
            track_total: Some("12".into()),
            disc_no: Some("1".into()),
            disc_total: Some("2".into()),
            year: Some("2024".into()),
            genre: Some("Pop".into()),
            composer: Some("Composer".into()),
            publisher: Some("Publisher".into()),
            copyright: Some("(C) 2024".into()),
            comment: Some("Dòng một\nDòng hai".into()),
            bpm: Some("120".into()),
            grouping: Some("Grouping".into()),
            lyrics: Some("Lời một\nLời hai".into()),
            timeline: Some("0:00 Intro\n1:00 Hook".into()),
            compilation: Some(true),
        };
        for kind in AudioKind::ALL {
            for mode in MODES {
                let path = fixtures::write_sample(dir.path(), &format!("rt-{mode:?}"), kind).unwrap();
                let stamp = write_tags(&path, &fields_edit(wanted.clone()), mode).unwrap();
                let got = read_track(&path).unwrap();
                assert_eq!(got.fields, wanted, "{kind:?} {mode:?}");
                assert_eq!(stamp, got.stamp, "{kind:?} {mode:?}");
                assert!((800..1_400).contains(&got.duration_ms), "{kind:?} {mode:?}");
            }
        }
        no_temp_left(dir.path());
    }

    #[test]
    fn audio_payload_survives_every_write() {
        let dir = tmp();
        let edit = MetadataEdit {
            fields: TagFields {
                title: Some("Tựa".into()),
                lyrics: Some("x".repeat(70_000)),
                timeline: Some("0:00 A".into()),
                ..Default::default()
            },
            cover: CoverEdit::Set(TINY_PNG.to_vec()),
        };
        for kind in AudioKind::ALL {
            for mode in MODES {
                let path = fixtures::write_sample(dir.path(), &format!("audio-{mode:?}"), kind).unwrap();
                write_tags(&path, &edit, mode).unwrap();
                let bytes = fs::read(&path).unwrap();
                assert!(contains(&bytes, &fixtures::audio_bytes(kind)), "{kind:?} {mode:?} hỏng âm thanh");
                if matches!(kind, AudioKind::M4a | AudioKind::M4r) {
                    let offset = fixtures::mp4_first_chunk_offset(&bytes).unwrap();
                    let frame = fixtures::mp4_first_frame();
                    assert_eq!(&bytes[offset..offset + frame.len()], frame, "{kind:?} {mode:?} lệch stco");
                }
            }
        }
    }

    // --- C3: N2, khung của app sống qua mọi chế độ ---

    #[test]
    fn timeline_survives_every_mode_on_every_format() {
        let dir = tmp();
        for kind in AudioKind::ALL {
            for mode in MODES {
                let path = fixtures::write_sample(dir.path(), &format!("tl-{mode:?}"), kind).unwrap();
                write_tags(&path, &timeline_edit("0:00 A\n1:00 B"), WriteMode::Merge).unwrap();
                write_tags(&path, &title_edit("Sau"), mode).unwrap();
                let got = read_track(&path).unwrap().fields;
                assert_eq!(got.timeline.as_deref(), Some("0:00 A\n1:00 B"), "{kind:?} {mode:?}");
                assert_eq!(got.title.as_deref(), Some("Sau"), "{kind:?} {mode:?}");
            }
        }
    }

    #[test]
    fn timeline_can_be_pushed_into_a_file_and_cleared() {
        let dir = tmp();
        for kind in AudioKind::ALL {
            for mode in MODES {
                let path = fixtures::write_sample(dir.path(), &format!("push-{mode:?}"), kind).unwrap();
                write_tags(&path, &timeline_edit("0:00 A"), mode).unwrap();
                assert_eq!(read_track(&path).unwrap().fields.timeline.as_deref(), Some("0:00 A"), "{kind:?}");
                write_tags(&path, &timeline_edit(""), mode).unwrap();
                assert_eq!(read_track(&path).unwrap().fields.timeline, None, "{kind:?} {mode:?}");
            }
        }
    }

    #[test]
    fn txxx_timeline_written_by_the_old_app_is_kept() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(dir.path(), "old", &[("TIT2", "Bài"), ("TSSE", "LAME")]).unwrap();
        // Bản Tauri ghi v2.4 (S5), không qua `tags`.
        let mut tag = id3::Tag::read_from_path(&path).unwrap();
        tag.add_frame(id3::frame::ExtendedText { description: "TIMELINE".into(), value: "0:10 Cũ".into() });
        tag.add_frame(id3::frame::ExtendedText { description: "RUBBISH".into(), value: "x".into() });
        tag.write_to_path(&path, id3::Version::Id3v24).unwrap();

        for mode in [WriteMode::CleanKeepKnown, WriteMode::Clean] {
            let album = fields_edit(TagFields { album: Some("A".into()), ..Default::default() });
            write_tags(&path, &album, mode).unwrap();
            let tag = id3::Tag::read_from_path(&path).unwrap();
            let texts: Vec<_> = tag.extended_texts().map(|t| (t.description.as_str(), t.value.as_str())).collect();
            assert_eq!(texts, [("TIMELINE", "0:10 Cũ")], "{mode:?}");
            assert_eq!(id3_version(&path), 3);
        }
    }

    #[test]
    fn merge_keeps_foreign_custom_fields_on_lofty_formats() {
        let dir = tmp();
        for kind in [AudioKind::Flac, AudioKind::Ogg, AudioKind::M4a] {
            let path = fixtures::write_sample(dir.path(), "foreign", kind).unwrap();
            let mut reader = open_reader(&path).unwrap();
            let format = detect_format(&mut reader, &path).unwrap();
            drop(reader);
            write_app_values(&path, format, &[("MYFIELD", "giữ tôi".to_string())]);

            write_tags(&path, &title_edit("T"), WriteMode::Merge).unwrap();
            let mut reader = open_reader(&path).unwrap();
            let kept = read_app_value(&mut reader, &path, format, "MYFIELD");
            assert_eq!(kept.as_deref(), Some("giữ tôi"), "{kind:?} Merge phải giữ");
            drop(reader);

            write_tags(&path, &title_edit("T"), WriteMode::Clean).unwrap();
            let mut reader = open_reader(&path).unwrap();
            assert_eq!(read_app_value(&mut reader, &path, format, "MYFIELD"), None, "{kind:?} Clean phải bỏ");
        }
    }

    /// Làm sạch không làm bài nghe khác đi: ReplayGain, Sound Check, `RVA2` còn nguyên, rác vẫn bỏ; `iTunNORM` không
    /// bị đọc thành ghi chú, sửa ghi chú không xoá nó.
    #[test]
    fn clean_keeps_sound_frames_on_mp3() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(dir.path(), "gain", &[("TIT2", "T"), ("TSSE", "LAME")]).unwrap();
        let mut tag = id3::Tag::read_from_path(&path).unwrap();
        tag.add_frame(id3::frame::ExtendedText {
            description: "REPLAYGAIN_TRACK_GAIN".into(),
            value: "-6.20 dB".into(),
        });
        tag.add_frame(id3::frame::ExtendedText { description: "RUBBISH".into(), value: "x".into() });
        tag.add_frame(id3::frame::Comment {
            lang: "eng".into(),
            description: "iTunNORM".into(),
            text: " 0000044E".into(),
        });
        let rva2 = id3::Content::Unknown(id3::frame::Unknown {
            data: vec![0, 1, 0xFC, 0x00, 0],
            version: id3::Version::Id3v24,
        });
        tag.add_frame(id3::Frame::with_content("RVA2", rva2));
        tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.comment, None, "iTunNORM không phải ghi chú");

        for mode in [WriteMode::Clean, WriteMode::CleanKeepKnown, WriteMode::Merge] {
            let edit = fields_edit(TagFields { comment: Some("Hay".into()), ..Default::default() });
            write_tags(&path, &edit, mode).unwrap();
            let tag = id3::Tag::read_from_path(&path).unwrap();
            let gain =
                tag.extended_texts().find(|t| t.description == "REPLAYGAIN_TRACK_GAIN").map(|t| t.value.as_str());
            assert_eq!(gain, Some("-6.20 dB"), "{mode:?}");
            assert!(tag.comments().any(|c| c.description == "iTunNORM"), "{mode:?}");
            assert!(tag.get("RVA2").is_some(), "{mode:?}");
            assert_eq!(read_track(&path).unwrap().fields.comment.as_deref(), Some("Hay"), "{mode:?}");
            if mode != WriteMode::Merge {
                assert!(!frame_ids(&path).contains(&"TSSE".to_string()), "{mode:?}");
                assert!(tag.extended_texts().all(|t| t.description != "RUBBISH"), "{mode:?}");
            }
        }
    }

    #[test]
    fn clean_keeps_sound_keys_on_lofty_formats() {
        let dir = tmp();
        for kind in [AudioKind::Flac, AudioKind::Ogg, AudioKind::M4a] {
            let path = fixtures::write_sample(dir.path(), "gain", kind).unwrap();
            let mut reader = open_reader(&path).unwrap();
            let format = detect_format(&mut reader, &path).unwrap();
            drop(reader);
            let values = [("REPLAYGAIN_TRACK_GAIN", "-6.20 dB".to_string()), ("MYFIELD", "rác".to_string())];
            write_app_values(&path, format, &values);

            write_tags(&path, &title_edit("T"), WriteMode::Clean).unwrap();
            let mut reader = open_reader(&path).unwrap();
            let gain = read_app_value(&mut reader, &path, format, "REPLAYGAIN_TRACK_GAIN");
            assert_eq!(gain.as_deref(), Some("-6.20 dB"), "{kind:?}");
            assert_eq!(read_app_value(&mut reader, &path, format, "MYFIELD"), None, "{kind:?}");
        }
    }

    /// Số 0 trình khác tự điền đọc thành trống; ô số để trống hay gõ 0 thì xoá khung, không ghi 0.
    #[test]
    fn zero_numbers_read_as_blank_and_are_never_written() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(
            dir.path(),
            "zero",
            &[("TIT2", "T"), ("TRCK", "0/0"), ("TPOS", "0"), ("TYER", "0000"), ("TBPM", "0")],
        )
        .unwrap();
        let fields = read_track(&path).unwrap().fields;
        assert_eq!(
            (fields.track_no, fields.track_total, fields.disc_no, fields.disc_total, fields.year, fields.bpm),
            (None, None, None, None, None, None)
        );

        let edit = fields_edit(TagFields {
            title: Some("T".into()),
            track_no: Some("0".into()),
            track_total: Some(String::new()),
            year: Some("0".into()),
            ..Default::default()
        });
        for mode in [WriteMode::Clean, WriteMode::Merge] {
            write_tags(&path, &edit, mode).unwrap();
            let ids = frame_ids(&path);
            assert!(!ids.contains(&"TRCK".to_string()) && !ids.contains(&"TYER".to_string()), "{mode:?}: {ids:?}");
        }
    }

    // --- C4: các test giữ lại từ bản cũ (`tags.rs:744-992`), đổi sang tệp tự sinh (M6) ---

    #[test]
    fn clean_write_drops_junk_frames_and_legacy_tags() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(
            dir.path(),
            "clean",
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
        )
        .unwrap();
        fixtures::append_id3v1(&path, "Stale", "Stale Album").unwrap();
        assert!(frame_ids(&path).contains(&"TCOP".to_string()));
        assert!(fixtures::has_id3v1(&path).unwrap());

        write_tags(
            &path,
            &fields_edit(TagFields {
                title: Some("Flower Road".into()),
                album: Some("K Pop".into()),
                album_artist: Some("K Pop".into()),
                ..Default::default()
            }),
            WriteMode::Clean,
        )
        .unwrap();

        let after = frame_ids(&path);
        for junk in ["TCOP", "TSSE", "TSOA", "TSO2", "TSOT", "TENC"] {
            assert!(!after.contains(&junk.to_string()), "{junk} còn: {after:?}");
        }
        assert_eq!(id3_version(&path), 3);
        assert!(!fixtures::has_id3v1(&path).unwrap());

        let round = read_track(&path).unwrap().fields;
        assert_eq!(round.album.as_deref(), Some("K Pop"));
        assert_eq!(round.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(round.title.as_deref(), Some("Flower Road"));
        assert_eq!(round.copyright, None);
    }

    #[test]
    fn clean_write_removes_a_trailing_ape_tag() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(dir.path(), "ape", &[("TIT2", "T")]).unwrap();
        let mut ape = lofty::ape::ApeTag::default();
        ape.set_album("Stale".to_string());
        ape.save_to_path(&path, WriteOptions::default()).unwrap();
        assert!(contains(&fs::read(&path).unwrap(), b"APETAGEX"));

        write_tags(&path, &MetadataEdit::default(), WriteMode::CleanKeepKnown).unwrap();
        assert!(!contains(&fs::read(&path).unwrap(), b"APETAGEX"));
    }

    #[test]
    fn clean_keep_known_preserves_title_while_forcing_grouping() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(
            dir.path(),
            "keep",
            &[("TIT2", "Let Me Hear Your Voice"), ("TPE1", "BigBang"), ("TALB", "K Pop"), ("TRCK", "4/12")],
        )
        .unwrap();

        write_tags(
            &path,
            &fields_edit(TagFields {
                album: Some("K Pop".into()),
                album_artist: Some("K Pop".into()),
                compilation: Some(true),
                ..Default::default()
            }),
            WriteMode::CleanKeepKnown,
        )
        .unwrap();

        let after = read_track(&path).unwrap().fields;
        assert_eq!(after.title.as_deref(), Some("Let Me Hear Your Voice"));
        assert_eq!(after.artist.as_deref(), Some("BigBang"));
        assert_eq!(after.track_no.as_deref(), Some("4"));
        assert_eq!(after.track_total.as_deref(), Some("12"));
        assert_eq!(after.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(after.compilation, Some(true));
        assert_eq!(id3_version(&path), 3);
    }

    #[test]
    fn empty_string_clears_a_stubborn_field() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(
            dir.path(),
            "clear",
            &[("TIT2", "Fxxk It"), ("TALB", "K Pop"), ("TCOP", "(C) 2018 YG")],
        )
        .unwrap();
        let edit = fields_edit(TagFields { copyright: Some(String::new()), ..Default::default() });
        write_tags(&path, &edit, WriteMode::CleanKeepKnown).unwrap();
        let after = read_track(&path).unwrap().fields;
        assert_eq!(after.copyright, None);
        assert_eq!(after.title.as_deref(), Some("Fxxk It"));
    }

    #[test]
    fn merge_mode_leaves_untouched_frames_alone() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(
            dir.path(),
            "merge",
            &[("TIT2", "Bad Boy"), ("TALB", "K Pop"), ("TSOA", "K Pop")],
        )
        .unwrap();
        let edit = fields_edit(TagFields { album_artist: Some("K Pop".into()), ..Default::default() });
        write_tags(&path, &edit, WriteMode::Merge).unwrap();
        let after = read_track(&path).unwrap().fields;
        assert_eq!(after.album_artist.as_deref(), Some("K Pop"));
        assert_eq!(after.album.as_deref(), Some("K Pop"));
        assert_eq!(after.title.as_deref(), Some("Bad Boy"));
        assert!(frame_ids(&path).contains(&"TSOA".to_string()));
    }

    #[test]
    fn bulk_album_pass_gives_every_track_one_grouping_key() {
        let dir = tmp();
        let artists = ["", "BigBang", "BoA", "ATD Softs Production"];
        let mut copies = Vec::new();
        for (i, artist) in artists.iter().enumerate() {
            let mut frames = vec![("TIT2", "Track"), ("TALB", "K Pop")];
            if !artist.is_empty() {
                frames.push(("TPE1", *artist));
            }
            copies.push(fixtures::write_mp3_with_frames(dir.path(), &format!("bulk{i}"), &frames).unwrap());
        }
        let key = |p: &PathBuf, fallback: bool| {
            let m = read_track(p).unwrap().fields;
            let artist = if fallback { m.album_artist.or(m.artist) } else { m.album_artist };
            (m.album.unwrap_or_default(), artist.unwrap_or_default())
        };
        let before: HashSet<_> = copies.iter().map(|p| key(p, true)).collect();
        assert_eq!(before.len(), artists.len());

        let edit = fields_edit(TagFields {
            album: Some("K Pop".into()),
            album_artist: Some("K Pop".into()),
            compilation: Some(true),
            ..Default::default()
        });
        for path in &copies {
            write_tags(path, &edit, WriteMode::CleanKeepKnown).unwrap();
        }
        let after: HashSet<_> = copies.iter().map(|p| key(p, false)).collect();
        assert_eq!(after.into_iter().collect::<Vec<_>>(), [("K Pop".to_string(), "K Pop".to_string())]);
        for path in &copies {
            let m = read_track(path).unwrap().fields;
            assert_eq!(m.title.as_deref(), Some("Track"));
            assert_eq!(m.compilation, Some(true));
            assert_eq!(id3_version(path), 3);
        }
    }

    #[test]
    fn positional_frames_round_trip() {
        assert_eq!(positional(&Some("3".into()), &Some("12".into())).as_deref(), Some("3/12"));
        assert_eq!(positional(&Some("3".into()), &None).as_deref(), Some("3"));
        assert_eq!(positional(&None, &Some("12".into())), None);
        assert_eq!(split_positional(Some("3/12")), (Some("3".into()), Some("12".into())));
        assert_eq!(split_positional(Some("3")), (Some("3".into()), None));
        assert_eq!(split_positional(None), (None, None));
    }

    // --- C5: luật khung năm, số thứ tự, lyrics ---

    #[test]
    fn tdrc_is_accepted_and_cleared_with_the_year() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(dir.path(), "y", &[("TDRC", "2019-05-01")]).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.year.as_deref(), Some("2019"));

        write_tags(&path, &MetadataEdit::default(), WriteMode::CleanKeepKnown).unwrap();
        assert!(frame_ids(&path).contains(&"TYER".to_string()), "clean_keep đổi sang TYER");
        assert!(!frame_ids(&path).contains(&"TDRC".to_string()));

        let path = fixtures::write_mp3_with_frames(dir.path(), "y2", &[("TDRC", "2019")]).unwrap();
        let clear = fields_edit(TagFields { year: Some(String::new()), ..Default::default() });
        write_tags(&path, &clear, WriteMode::Merge).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.year, None, "TDRC không được làm năm hiện lại");
    }

    #[test]
    fn merge_of_only_a_total_keeps_the_number() {
        let dir = tmp();
        let path = fixtures::write_mp3_with_frames(dir.path(), "pos", &[("TRCK", "4/10")]).unwrap();
        let edit = fields_edit(TagFields { track_total: Some("12".into()), ..Default::default() });
        write_tags(&path, &edit, WriteMode::Merge).unwrap();
        let got = read_track(&path).unwrap().fields;
        assert_eq!((got.track_no.as_deref(), got.track_total.as_deref()), (Some("4"), Some("12")));
    }

    #[test]
    fn id3v1_only_file_keeps_its_values_on_clean_keep() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "v1", AudioKind::Mp3).unwrap();
        fixtures::append_id3v1(&path, "Title1", "Album1").unwrap();
        assert_eq!(read_track(&path).unwrap().fields.album.as_deref(), Some("Album1"));

        write_tags(&path, &MetadataEdit::default(), WriteMode::CleanKeepKnown).unwrap();
        assert!(!fixtures::has_id3v1(&path).unwrap());
        assert_eq!(read_track(&path).unwrap().fields.album.as_deref(), Some("Album1"));
    }

    #[test]
    fn clean_without_lyrics_drops_them_for_metadata_ops_to_restore() {
        let dir = tmp();
        for kind in AudioKind::ALL {
            let path = fixtures::write_sample(dir.path(), "ly", kind).unwrap();
            let lyrics = fields_edit(TagFields { lyrics: Some("Lời".into()), ..Default::default() });
            write_tags(&path, &lyrics, WriteMode::Merge).unwrap();
            write_tags(&path, &MetadataEdit::default(), WriteMode::CleanKeepKnown).unwrap();
            assert_eq!(read_track(&path).unwrap().fields.lyrics.as_deref(), Some("Lời"), "{kind:?}");
            write_tags(&path, &MetadataEdit::default(), WriteMode::Clean).unwrap();
            assert_eq!(read_track(&path).unwrap().fields.lyrics, None, "{kind:?}");
        }
    }

    // --- C6: S17 ---

    #[test]
    fn multi_value_frames_become_semicolons_instead_of_sticking_together() {
        assert_eq!(clean("A\0B\0"), "A; B");
        let dir = tmp();
        for (stem, mode) in [("mv1", WriteMode::CleanKeepKnown), ("mv2", WriteMode::Merge)] {
            let path = fixtures::write_sample(dir.path(), stem, AudioKind::Mp3).unwrap();
            let mut tag = id3::Tag::new();
            tag.set_text_values("TPE1", ["A", "B"]);
            tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
            assert_eq!(read_track(&path).unwrap().fields.artist.as_deref(), Some("A; B"), "đọc từ tệp v2.4 thật");

            let album = fields_edit(TagFields { album: Some("X".into()), ..Default::default() });
            write_tags(&path, &album, mode).unwrap();
            let raw = id3::Tag::read_from_path(&path).unwrap();
            assert_eq!(raw.get("TPE1").unwrap().content().text(), Some("A; B"), "{mode:?} ghi v2.3 không còn \\0");
            assert_eq!(id3_version(&path), 3);
        }
    }

    #[test]
    fn vorbis_multi_artist_is_joined() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "mv", AudioKind::Flac).unwrap();
        let mut reader = open_reader(&path).unwrap();
        let mut file = FlacFile::read_from(&mut reader, ParseOptions::new()).unwrap();
        drop(reader);
        let mut comments = VorbisComments::new();
        comments.push("ARTIST".into(), "A".into());
        comments.push("ARTIST".into(), "B".into());
        file.set_vorbis_comments(comments);
        file.save_to_path(&path, WriteOptions::default()).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.artist.as_deref(), Some("A; B"));
    }

    // --- C7: bìa ---

    #[test]
    fn cover_set_keep_remove_on_every_format_and_mode() {
        let dir = tmp();
        for kind in AudioKind::ALL {
            for mode in MODES {
                let path = fixtures::write_sample(dir.path(), &format!("cv-{mode:?}"), kind).unwrap();
                let set = MetadataEdit { fields: TagFields::default(), cover: CoverEdit::Set(TINY_PNG.to_vec()) };
                write_tags(&path, &set, mode).unwrap();
                let cover = read_track(&path).unwrap().cover.unwrap();
                assert_eq!((cover.mime.as_str(), cover.data.as_slice()), ("image/png", TINY_PNG), "{kind:?} {mode:?}");

                write_tags(&path, &MetadataEdit::default(), mode).unwrap();
                assert!(read_track(&path).unwrap().cover.is_some(), "{kind:?} {mode:?} Keep");

                let remove = MetadataEdit { fields: TagFields::default(), cover: CoverEdit::Remove };
                write_tags(&path, &remove, mode).unwrap();
                assert!(read_track(&path).unwrap().cover.is_none(), "{kind:?} {mode:?} Remove");
            }
        }
    }

    #[test]
    fn cover_that_is_not_an_image_is_refused_and_mime_is_sniffed() {
        assert_eq!(sniff_image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image_mime(b"RIFF0000WEBPxxxx"), Some("image/webp"));
        assert_eq!(sniff_image_mime(b"GIF89a"), Some("image/gif"));
        assert_eq!(sniff_image_mime(b"BM0000"), Some("image/bmp"));
        assert_eq!(sniff_image_mime(b"nope"), None);

        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "bad", AudioKind::Mp3).unwrap();
        let edit = MetadataEdit { fields: TagFields::default(), cover: CoverEdit::Set(b"nope".to_vec()) };
        assert!(matches!(write_tags(&path, &edit, WriteMode::Merge), Err(TagError::UnsupportedCover)));
    }

    // --- C8: N3, ghi an toàn ---

    #[test]
    fn failed_write_leaves_the_original_untouched_and_no_temp() {
        let dir = tmp();
        let path = dir.path().join("rác.mp3");
        fs::write(&path, b"khong phai nhac").unwrap();
        assert!(write_tags(&path, &title_edit("T"), WriteMode::CleanKeepKnown).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"khong phai nhac");
        no_temp_left(dir.path());

        let missing = dir.path().join("không có.mp3");
        assert!(matches!(write_tags(&missing, &title_edit("T"), WriteMode::Merge), Err(TagError::NotFound(_))));
        assert!(matches!(read_track(&missing), Err(TagError::NotFound(_))));
    }

    #[test]
    fn verify_catches_a_field_that_did_not_land() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "v", AudioKind::Mp3).unwrap();
        let before = read_track(&path).unwrap();
        // Tệp chưa có tiêu đề mà phần sửa đòi có: chính là tình huống ghi trượt.
        assert!(matches!(verify(&path, &title_edit("T"), &before), Err(TagError::Verify { field: "title", .. })));

        let lost = ScannedTrack {
            fields: TagFields { timeline: Some("0:00 A".into()), ..Default::default() },
            ..before.clone()
        };
        let result = verify(&path, &MetadataEdit::default(), &lost);
        assert!(matches!(result, Err(TagError::Verify { field: "timeline", .. })));

        let longer = ScannedTrack { duration_ms: before.duration_ms + 5_000, ..before };
        let result = verify(&path, &MetadataEdit::default(), &longer);
        assert!(matches!(result, Err(TagError::Verify { field: "duration", .. })));
    }

    #[test]
    fn stale_temp_from_a_crash_is_replaced_and_removed() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "s", AudioKind::Mp3).unwrap();
        fs::write(temp_path_for(&path), b"sot lai").unwrap();
        write_tags(&path, &MetadataEdit::default(), WriteMode::CleanKeepKnown).unwrap();
        no_temp_left(dir.path());
    }

    #[test]
    fn read_only_file_is_refused() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "ro", AudioKind::Mp3).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms.clone()).unwrap();
        assert!(matches!(write_tags(&path, &MetadataEdit::default(), WriteMode::Merge), Err(TagError::ReadOnly(_))));
        no_temp_left(dir.path());
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&path, perms).unwrap();
    }

    /// Đang phát: `File::open` của Rust có `FILE_SHARE_DELETE`, handle cũ vẫn đọc được bản cũ (N3).
    #[test]
    fn open_handle_keeps_reading_while_the_file_is_replaced() {
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "play", AudioKind::Mp3).unwrap();
        let original = fs::read(&path).unwrap();
        let mut playing = File::open(&path).unwrap();

        write_tags(&path, &title_edit("Mới"), WriteMode::Merge).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.title.as_deref(), Some("Mới"));

        let mut seen = Vec::new();
        playing.read_to_end(&mut seen).unwrap();
        assert_eq!(seen, original);
    }

    /// App khác mở không cho xoá: báo lỗi rõ, tệp gốc nguyên vẹn (đặc tả 5.8).
    #[cfg(windows)]
    #[test]
    fn file_locked_by_another_app_reports_replace_error() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let dir = tmp();
        let path = fixtures::write_sample(dir.path(), "lock", AudioKind::Mp3).unwrap();
        let original = fs::read(&path).unwrap();
        let lock = File::options().read(true).share_mode(FILE_SHARE_READ).open(&path).unwrap();

        let result = write_tags(&path, &title_edit("T"), WriteMode::Merge);
        assert!(matches!(result, Err(TagError::Replace { .. })), "{result:?}");
        drop(lock);
        assert_eq!(fs::read(&path).unwrap(), original);
        no_temp_left(dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_stays_a_symlink() {
        let dir = tmp();
        let real = fixtures::write_sample(dir.path(), "real", AudioKind::Mp3).unwrap();
        let link = dir.path().join("link.mp3");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_tags(&link, &title_edit("T"), WriteMode::Merge).unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(read_track(&real).unwrap().fields.title.as_deref(), Some("T"));
    }

    // --- C9: S25, tệp độc ---

    #[test]
    fn damaged_files_never_panic_out() {
        let dir = tmp();
        let path = dir.path().join("damaged");
        let edit =
            fields_edit(TagFields { title: Some("T".into()), timeline: Some("0:00 A".into()), ..Default::default() });
        for kind in AudioKind::ALL {
            let source = fixtures::write_sample(dir.path(), "src", kind).unwrap();
            write_tags(&source, &edit, WriteMode::Merge).unwrap();
            let good = fs::read(&source).unwrap();

            // Cắt cụt ở nhiều độ dài.
            let step = (good.len() / 64).max(1);
            for len in (0..good.len()).step_by(step) {
                fs::write(&path, &good[..len]).unwrap();
                assert!(!matches!(read_track(&path), Err(TagError::Panicked(_))), "{kind:?} cắt ở {len}");
            }
            // Lật byte ở phần đầu tệp, nơi có cấu trúc. Sinh số cố định để lặp lại được.
            let mut state = 0x2545_F491u32;
            for _ in 0..200 {
                let mut bad = good.clone();
                for _ in 0..8 {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    let at = state as usize % bad.len().min(2_048);
                    bad[at] = (state >> 8) as u8;
                }
                fs::write(&path, &bad).unwrap();
                assert!(!matches!(read_track(&path), Err(TagError::Panicked(_))), "{kind:?}");
            }
        }
    }

    /// Bìa 20 MiB vượt trần cấp phát mặc định 16 MiB của lofty nhưng trong trần của ta.
    #[test]
    fn large_cover_round_trips_and_oversized_is_refused() {
        let dir = tmp();
        let mut large = TINY_PNG.to_vec();
        large.resize(20 * 1024 * 1024, 0);
        for kind in AudioKind::ALL {
            let path = fixtures::write_sample(dir.path(), "big", kind).unwrap();
            let edit = MetadataEdit { fields: TagFields::default(), cover: CoverEdit::Set(large.clone()) };
            let result = write_tags(&path, &edit, WriteMode::Merge);
            if kind == AudioKind::Flac {
                // Khối siêu dữ liệu FLAC tối đa 16 MiB: báo lỗi, tệp gốc nguyên vẹn.
                assert!(matches!(result, Err(TagError::Write { .. })), "{result:?}");
                assert_eq!(fs::read(&path).unwrap(), fixtures::sample_bytes(kind));
                continue;
            }
            result.unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            assert_eq!(read_track(&path).unwrap().cover.unwrap().data.len(), large.len(), "{kind:?}");
        }

        let mut huge = TINY_PNG.to_vec();
        huge.resize(MAX_COVER_BYTES + 1, 0);
        let path = fixtures::write_sample(dir.path(), "huge", AudioKind::Mp3).unwrap();
        let edit = MetadataEdit { fields: TagFields::default(), cover: CoverEdit::Set(huge) };
        assert!(matches!(write_tags(&path, &edit, WriteMode::Merge), Err(TagError::CoverTooLarge)));
    }

    /// Soát tay bằng ffprobe: ghi thẻ lên tệp mẫu và mọi tệp `real.*` (tự mã hoá, không phải thư viện thật) ở `GROOVE_DUMP`.
    #[test]
    #[ignore = "soát tay"]
    fn dump_for_ffprobe() {
        let out = PathBuf::from(std::env::var("GROOVE_DUMP").unwrap());
        let edit = MetadataEdit {
            fields: TagFields {
                title: Some("Tựa".into()),
                timeline: Some("0:00 A".into()),
                lyrics: Some("x".repeat(70_000)),
                ..Default::default()
            },
            cover: CoverEdit::Set(TINY_PNG.to_vec()),
        };
        for kind in AudioKind::ALL {
            fixtures::write_sample(&out, "raw", kind).unwrap();
            let path = fixtures::write_sample(&out, "tagged", kind).unwrap();
            write_tags(&path, &edit, WriteMode::CleanKeepKnown).unwrap();
        }
        for entry in fs::read_dir(&out).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with("real.") {
                write_tags(&entry.path(), &edit, WriteMode::CleanKeepKnown).unwrap();
                write_tags(&entry.path(), &title_edit("Lần hai"), WriteMode::CleanKeepKnown).unwrap();
            }
        }
    }

    #[test]
    fn panic_becomes_an_error() {
        let result: Result<(), TagError> = guarded(Path::new("x.mp3"), || panic!("tệp độc"));
        assert!(matches!(result, Err(TagError::Panicked(_))));
    }

    #[test]
    fn oversized_values_are_capped_when_reading() {
        assert_eq!(read_line(&"a".repeat(MAX_FIELD_CHARS + 10)).unwrap().chars().count(), MAX_FIELD_CHARS);
        assert_eq!(read_multiline(&"é".repeat(MAX_MULTILINE_CHARS + 10)).unwrap().chars().count(), MAX_MULTILINE_CHARS);
        assert!(capped_cover("image/png".into(), &vec![0u8; MAX_COVER_BYTES + 1], Path::new("x")).is_none());
    }

    // --- C10: đường dẫn, nhận định dạng ---

    #[test]
    fn temp_paths() {
        assert!(is_temp_path(&temp_path_for(Path::new("dir/a.mp3"))));
        assert_eq!(temp_path_for(Path::new("dir/a.mp3")), Path::new("dir/a.mp3.groove-tmp"));
        assert!(!is_temp_path(Path::new("a.mp3")));
    }

    #[test]
    fn format_comes_from_content_not_extension() {
        let dir = tmp();
        let path = dir.path().join("nhầm-đuôi.mp3");
        fs::write(&path, fixtures::sample_bytes(AudioKind::Flac)).unwrap();
        write_tags(&path, &title_edit("T"), WriteMode::Merge).unwrap();
        assert_eq!(read_track(&path).unwrap().fields.title.as_deref(), Some("T"));
        assert_eq!(&fs::read(&path).unwrap()[..4], b"fLaC", "không bị chèn ID3 trước FLAC");
    }

    #[test]
    fn write_mode_serde_is_snake_case() {
        assert_eq!(serde_json::to_string(&WriteMode::CleanKeepKnown).unwrap(), "\"clean_keep_known\"");
        assert_eq!(serde_json::from_str::<WriteMode>("\"merge\"").unwrap(), WriteMode::Merge);
        assert_eq!(WriteMode::default(), WriteMode::CleanKeepKnown);
    }
}
