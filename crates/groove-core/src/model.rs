//! groove-gd1_02: TrackRef, CategoryRef, Song, hằng đuôi tệp nhạc (S22).

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, Serializer};

use crate::paths;

/// Đuôi tệp nhạc, một chỗ duy nhất cho liên kết định dạng, quét thư viện, watcher và mở tệp (S22, đặc tả 5.6).
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "m4a", "m4r", "ogg"];

/// Đuôi có phải tệp nhạc không, không phân biệt hoa thường: `BÀI.MP3` vẫn nhận (S22).
pub fn is_audio_extension(ext: &str) -> bool {
    AUDIO_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext))
}

pub fn is_audio_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(is_audio_extension)
}

/// Tham chiếu tới một bài. Hàng đợi và phiên chỉ giữ kiểu này, bài thật tra từ thư viện lúc phát (N5).
///
/// Tệp ngoài thư viện so theo đường dẫn đã chuẩn hoá, không dùng id giả (N6). JSON: `{"library":12}`,
/// `{"external":"C:\\a.mp3"}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackRef {
    Library(i64),
    #[serde(serialize_with = "serialize_path_lossy")]
    External(PathBuf),
}

impl TrackRef {
    /// Tham chiếu tới tệp theo đường dẫn. Tra xem tệp có trong thư viện hay không là việc của `groove-library`.
    pub fn from_path(path: impl Into<PathBuf>) -> TrackRef {
        TrackRef::External(path.into())
    }
}

impl PartialEq for TrackRef {
    fn eq(&self, other: &TrackRef) -> bool {
        match (self, other) {
            (TrackRef::Library(a), TrackRef::Library(b)) => a == b,
            (TrackRef::External(a), TrackRef::External(b)) => paths::normalize_path(a) == paths::normalize_path(b),
            _ => false,
        }
    }
}

impl Eq for TrackRef {}

// Phải khớp `eq`: hai cách viết của một đường dẫn cho cùng một mã băm.
impl Hash for TrackRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            TrackRef::Library(id) => {
                state.write_u8(0);
                id.hash(state);
            }
            TrackRef::External(path) => {
                state.write_u8(1);
                paths::normalize_path(path).hash(state);
            }
        }
    }
}

// Mặc định `PathBuf` báo lỗi khi tên không phải UTF-8 (Linux), làm hỏng cả lần lưu phiên. Bản lossy trỏ tới tệp không tồn
// tại, lần mở sau rơi vào đường lỗi bình thường của tệp mất (N5).
fn serialize_path_lossy<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&path.to_string_lossy())
}

/// Nhóm bài, truyền nguyên kiểu thay vì tìm theo tên (S3). Id giữ nguyên qua chuyển đổi DB cũ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryRef {
    Album(i64),
    Folder(i64),
    Playlist(i64),
}

/// Lặp một bài khi hết tự nhiên, cộng với quay vòng khi bấm Next ở cuối (đặc tả 1.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
}

/// Một dòng bài trong danh sách, hàng đợi, thanh phát. Không kèm lyrics, timeline, bìa gốc: chỉ nạp khi là bài đang phát
/// (S16). Không lưu ra đĩa.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub track: TrackRef,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub album_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub path: PathBuf,
    pub filename: String,
    pub duration_sec: f64,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub cover_hash: Option<String>,
}

impl Song {
    /// Tệp ngoài thư viện trước khi đọc thẻ: chỉ biết đường dẫn.
    pub fn external(path: impl Into<PathBuf>) -> Song {
        let path = path.into();
        let filename = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Song {
            track: TrackRef::External(path.clone()),
            title: None,
            artist: None,
            album: None,
            album_artist: None,
            album_id: None,
            folder_id: None,
            path,
            filename,
            duration_sec: 0.0,
            track_no: None,
            disc_no: None,
            cover_hash: None,
        }
    }

    /// Tiêu đề hiển thị: tiêu đề rỗng thì dùng filename bỏ đuôi (đặc tả 1.2).
    pub fn display_title(&self) -> &str {
        match non_blank(self.title.as_deref()) {
            Some(title) => title,
            None => Path::new(&self.filename).file_stem().and_then(|stem| stem.to_str()).unwrap_or(&self.filename),
        }
    }

    /// Nghệ sĩ hiển thị; `None` thì giao diện tự chọn chữ thay (`—` ở danh sách, `Unknown Artist` ở thanh phát).
    pub fn display_artist(&self) -> Option<&str> {
        non_blank(self.artist.as_deref())
    }
}

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.filter(|t| !t.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn extensions_match_the_association_list_in_any_case() {
        for ext in ["mp3", "flac", "wav", "m4a", "m4r", "ogg"] {
            assert!(is_audio_extension(ext));
            assert!(is_audio_extension(&ext.to_uppercase()));
        }
        assert!(!is_audio_extension("txt"));
        assert!(is_audio_path(Path::new("C:/Nhạc/BÀI.MP3")));
        assert!(!is_audio_path(Path::new("C:/Nhạc/mp3")));
        assert!(!is_audio_path(Path::new("C:/Nhạc/a.groove-tmp")));
    }

    #[test]
    fn two_external_files_are_different_tracks() {
        // N6: bản cũ cho mọi tệp ngoài id -1 nên tệp thứ hai không phát.
        assert_ne!(TrackRef::from_path("/m/a.mp3"), TrackRef::from_path("/m/b.mp3"));
        assert_eq!(TrackRef::from_path("/m/a.mp3"), TrackRef::from_path("/m/a.mp3"));
        assert_ne!(TrackRef::Library(1), TrackRef::from_path("/m/1"));
    }

    #[test]
    fn spellings_of_one_path_are_one_track_and_hash_alike() {
        let set: HashSet<TrackRef> =
            [TrackRef::from_path(r"C:\Music\a.mp3"), TrackRef::from_path("C:/Music/a.mp3")].into_iter().collect();
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn json_shape_is_stable() {
        assert_eq!(serde_json::to_string(&TrackRef::Library(12)).unwrap(), r#"{"library":12}"#);
        assert_eq!(serde_json::to_string(&TrackRef::from_path("/a.mp3")).unwrap(), r#"{"external":"/a.mp3"}"#);
        assert_eq!(serde_json::to_string(&CategoryRef::Playlist(3)).unwrap(), r#"{"playlist":3}"#);
        assert_eq!(serde_json::to_string(&RepeatMode::One).unwrap(), r#""one""#);
        let back: TrackRef = serde_json::from_str(r#"{"external":"/a.mp3"}"#).unwrap();
        assert_eq!(back, TrackRef::from_path("/a.mp3"));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_does_not_break_serialization() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let track = TrackRef::from_path(OsStr::from_bytes(b"/m/\xFF.mp3"));
        assert!(serde_json::to_string(&track).is_ok());
    }

    #[test]
    fn display_title_falls_back_to_the_filename_stem() {
        let mut song = Song::external("/m/Bài hát.tên.mp3");
        assert_eq!(song.filename, "Bài hát.tên.mp3");
        assert_eq!(song.display_title(), "Bài hát.tên");
        song.title = Some("   ".into());
        assert_eq!(song.display_title(), "Bài hát.tên");
        song.title = Some("Tiêu đề".into());
        assert_eq!(song.display_title(), "Tiêu đề");
    }

    #[test]
    fn blank_artist_is_none() {
        let mut song = Song::external("/m/a.mp3");
        assert_eq!(song.display_artist(), None);
        song.artist = Some(" ".into());
        assert_eq!(song.display_artist(), None);
        song.artist = Some("A".into());
        assert_eq!(song.display_artist(), Some("A"));
    }
}
