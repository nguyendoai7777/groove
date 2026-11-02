//! groove-gd1_04: schema user_version = 1.
//!
//! Đặc tả 2.2. Schema không phụ thuộc hàm SQL tự đăng ký, nên tool khác mở được DB. Khoá dẫn xuất (`path_key`,
//! `search_key`, `group_key`) chỉ tính ở đây để migration, indexer và truy vấn dùng cùng một công thức.

use groove_core::paths;
use rusqlite::Connection;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub const SCHEMA_VERSION: i32 = 1;

/// `group_key` của album không có bài nào. `AUTOINCREMENT` bắt đầu từ 1 nên không trùng thư mục thật.
pub const NO_FOLDER_ID: i64 = 0;

const CREATE_TABLES: &str = "
CREATE TABLE folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT UNIQUE NOT NULL,
    cover_hash TEXT
);
CREATE TABLE albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    artist TEXT,
    group_key TEXT NOT NULL,
    cover_hash TEXT,
    cover_is_custom INTEGER NOT NULL DEFAULT 0,
    UNIQUE(name, group_key)
);
CREATE TABLE songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT,
    artist TEXT,
    album_artist TEXT,
    album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    path_key TEXT UNIQUE NOT NULL,
    filename TEXT NOT NULL,
    duration INTEGER NOT NULL,
    track_no INTEGER,
    disc_no INTEGER,
    lyrics TEXT,
    timeline TEXT,
    cover_hash TEXT,
    file_mtime INTEGER,
    file_size INTEGER,
    search_key TEXT NOT NULL DEFAULT '',
    tags_read_at INTEGER
);
CREATE TABLE watched_roots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL
);
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    color TEXT NOT NULL
);
CREATE TABLE playlist_songs (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY(playlist_id, song_id)
);
CREATE TABLE covers (
    hash TEXT PRIMARY KEY,
    accent_color TEXT NOT NULL,
    width INTEGER,
    height INTEGER
);
CREATE INDEX idx_songs_folder_id ON songs(folder_id);
CREATE INDEX idx_songs_album_id ON songs(album_id);
CREATE INDEX idx_songs_cover_hash ON songs(cover_hash);
CREATE INDEX idx_playlist_songs_song_id ON playlist_songs(song_id);
";

/// Tạo mọi bảng và đặt `user_version`. Gọi trong transaction để không bao giờ có DB dở dang: `user_version` cũng rollback.
pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(CREATE_TABLES)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
}

/// Theo kết nối, mọi kết nối mới đều phải gọi (trừ `journal_mode`, lưu trong tệp).
pub fn apply_pragmas(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )
}

pub fn user_version(conn: &Connection) -> rusqlite::Result<i32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Không dấu, hạ chữ, gộp khoảng trắng (S18). Giữ dấu câu: `-` chỉ khớp tên có `-`, không khớp tất cả như bản cũ.
/// Truy vấn tìm kiếm đi qua đúng hàm này rồi `instr(search_key, ?)`.
pub fn normalize_text(input: &str) -> String {
    let folded: String = input
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'đ' { 'd' } else { c })
        .nfc()
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Các trường cách nhau bằng xuống dòng: truy vấn đã chuẩn hoá không chứa nó, nên không khớp vắt qua hai trường.
pub fn search_key(title: Option<&str>, artist: Option<&str>, album: Option<&str>, filename: &str) -> String {
    [title, artist, album, Some(filename)]
        .into_iter()
        .flatten()
        .map(normalize_text)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `album_artist` đã chuẩn hoá khi có, không có thì `folder:<folder_id>` (đặc tả 2.2, S1).
pub fn album_group_key(album_artist: Option<&str>, folder_id: i64) -> String {
    match album_artist.map(normalize_text).filter(|artist| !artist.is_empty()) {
        Some(artist) => artist,
        None => format!("folder:{folder_id}"),
    }
}

/// Khoá UNIQUE của bài (M2): `file_path` giữ nguyên để mở tệp, so sánh luôn qua cột này.
pub fn path_key(file_path: &str) -> String {
    paths::normalize(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_removes_vietnamese_marks_and_case() {
        assert_eq!(normalize_text("Đường Về Nhà"), "duong ve nha");
        assert_eq!(normalize_text("TÌNH CA Ả Ơi"), "tinh ca a oi");
        // Dựng sẵn và tổ hợp cho cùng một kết quả.
        assert_eq!(normalize_text("Nguye\u{302}\u{303}n"), normalize_text("Nguyễn"));
    }

    #[test]
    fn normalize_keeps_punctuation_and_collapses_spaces() {
        assert_eq!(normalize_text("  AC/DC \t  Live - 1992 "), "ac/dc live - 1992");
        assert_eq!(normalize_text("-"), "-");
        assert_eq!(normalize_text("   "), "");
    }

    #[test]
    fn search_key_does_not_match_across_fields() {
        let key = search_key(Some("Hello"), Some("World"), None, "a.mp3");
        assert_eq!(key, "hello\nworld\na.mp3");
        assert!(!key.contains(&normalize_text("hello world")));
    }

    #[test]
    fn search_key_skips_missing_fields() {
        assert_eq!(search_key(None, Some(" "), Some("Áo"), "b.flac"), "ao\nb.flac");
    }

    #[test]
    fn group_key_prefers_album_artist() {
        assert_eq!(album_group_key(Some("Sơn Tùng"), 3), "son tung");
        assert_eq!(album_group_key(Some("  "), 3), "folder:3");
        assert_eq!(album_group_key(None, 7), "folder:7");
    }

    #[test]
    fn path_key_unifies_separators() {
        assert_eq!(path_key(r"C:\Music\a.mp3"), path_key("C:/Music/a.mp3"));
    }
}
