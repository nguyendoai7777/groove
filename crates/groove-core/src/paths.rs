//! Đường dẫn theo OS và so sánh đường dẫn.
//!
//! So sánh theo đoạn, không phân biệt hoa thường trên Windows và macOS, phân biệt trên Linux; port nguyên từ
//! `src-tauri/src/paths.rs` kèm test (đặc tả 1.11). Đây là chuẩn hoá chuỗi, không phải `fs::canonicalize`: phải chạy được
//! với đường dẫn không còn tồn tại, như tệp trong sự kiện xoá.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Thư mục của app trong thư mục dữ liệu, cache của OS (đặc tả 2.3).
pub const APP_DIR: &str = "Groove X";
/// Thư mục của bản Tauri, chỉ đọc để chuyển đổi (đặc tả 2.3).
pub const LEGACY_APP_DIR: &str = "com.mrdxd.groovex";
const LEGACY_DB_FILE: &str = "groovex.db";
/// Chỉ bản debug (M1): cần dữ liệu trống khi thử thì trỏ biến này sang thư mục tạm, cả cache và log đi theo.
const DATA_DIR_VAR: &str = "GROOVE_DATA_DIR";

/// Windows `%APPDATA%\Groove X`, macOS `~/Library/Application Support/Groove X`, Linux `~/.local/share/Groove X`.
pub fn data_dir() -> io::Result<PathBuf> {
    resolve_data_dir(dev_override(), dirs::data_dir())
}

/// Bìa và dữ liệu tính lại được. Windows `%LOCALAPPDATA%\Groove X`, macOS `~/Library/Caches/Groove X`, Linux
/// `~/.cache/Groove X`. Có `GROOVE_DATA_DIR` thì là `<đó>/cache`.
pub fn cache_dir() -> io::Result<PathBuf> {
    resolve_cache_dir(dev_override(), dirs::cache_dir())
}

pub fn library_db() -> io::Result<PathBuf> {
    data_dir().map(|d| d.join("library.db"))
}

pub fn settings_file() -> io::Result<PathBuf> {
    data_dir().map(|d| d.join("settings.json"))
}

pub fn session_file() -> io::Result<PathBuf> {
    data_dir().map(|d| d.join("session.json"))
}

/// `<cache>/covers`, chứa `<hash>_<256|512>.jpg` (đặc tả 2.2).
pub fn covers_dir() -> io::Result<PathBuf> {
    cache_dir().map(|d| d.join("covers"))
}

pub fn logs_dir() -> io::Result<PathBuf> {
    data_dir().map(|d| d.join("logs"))
}

/// DB của bản Tauri, không kiểm tệp có tồn tại. Chạy thử từ app Claude thì AppData bị ảo hoá MSIX: không thấy tệp ở đó
/// không có nghĩa là máy không có DB.
pub fn legacy_db() -> io::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| not_found("data dir"))?;
    Ok(base.join(LEGACY_APP_DIR).join(LEGACY_DB_FILE))
}

fn dev_override() -> Option<OsString> {
    if cfg!(debug_assertions) { std::env::var_os(DATA_DIR_VAR).filter(|v| !v.is_empty()) } else { None }
}

fn resolve_data_dir(over: Option<OsString>, os_dir: Option<PathBuf>) -> io::Result<PathBuf> {
    match over {
        Some(dir) => Ok(dir.into()),
        None => os_dir.map(|d| d.join(APP_DIR)).ok_or_else(|| not_found("data dir")),
    }
}

fn resolve_cache_dir(over: Option<OsString>, os_dir: Option<PathBuf>) -> io::Result<PathBuf> {
    match over {
        Some(dir) => Ok(PathBuf::from(dir).join("cache")),
        None => os_dir.map(|d| d.join(APP_DIR)).ok_or_else(|| not_found("cache dir")),
    }
}

fn not_found(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("OS did not report a {what}"))
}

/// Hệ tệp của nền tảng có coi `A.mp3` và `a.mp3` là một tệp không.
const CASE_INSENSITIVE: bool = cfg!(any(target_os = "windows", target_os = "macos"));

/// Dạng để so sánh: một kiểu dấu ngăn, không dấu ngăn cuối, gập hoa thường nếu hệ tệp gập.
pub fn normalize(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let trimmed = unified.trim_end_matches('/');

    // Gốc trần ("/" hoặc "C:/") cắt xong là rỗng, nên giữ dấu ngăn.
    let base = if trimmed.is_empty() { unified.as_str() } else { trimmed };

    if CASE_INSENSITIVE { base.to_lowercase() } else { base.to_string() }
}

pub fn normalize_path(path: &Path) -> String {
    normalize(&path.to_string_lossy())
}

/// `child` là chính `parent` hoặc nằm dưới nó. So cả đoạn: `C:\Music Videos` không nằm trong `C:\Music`.
pub fn is_under(child: &str, parent: &str) -> bool {
    let child = normalize(child);
    let parent = normalize(parent);

    if child == parent {
        return true;
    }
    // `parent` là gốc hệ tệp thì đã có sẵn dấu ngăn cuối.
    let boundary = if parent.ends_with('/') { parent.clone() } else { format!("{parent}/") };
    child.starts_with(&boundary)
}

pub fn is_under_path(child: &Path, parent: &Path) -> bool {
    is_under(&child.to_string_lossy(), &parent.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_style_does_not_matter() {
        assert!(is_under(r"C:\Music\Pop\a.mp3", r"C:\Music"));
        assert!(is_under("C:/Music/Pop/a.mp3", r"C:\Music"));
        assert!(is_under(r"C:\Music\Pop\a.mp3", "C:/Music"));
    }

    #[test]
    fn a_folder_contains_itself() {
        assert!(is_under(r"C:\Music", r"C:\Music"));
        assert!(is_under(r"C:\Music\", r"C:\Music"));
        assert!(is_under("/Users/me/Music", "/Users/me/Music/"));
    }

    #[test]
    fn a_shared_prefix_is_not_containment() {
        // Lỗi test này giữ: "Music Videos" bắt đầu bằng "Music".
        assert!(!is_under(r"C:\Music Videos\x.mp3", r"C:\Music"));
        assert!(!is_under("/Users/me/Music Videos", "/Users/me/Music"));
        assert!(!is_under(r"C:\Music", r"C:\Music\Pop"));
    }

    #[test]
    fn macos_style_absolute_paths_work() {
        assert!(is_under("/Users/me/Music/Pop/a.mp3", "/Users/me/Music"));
        assert!(is_under("/Users/me/Music", "/"));
        assert!(!is_under("/Users/me/Music", "/Volumes"));
    }

    #[test]
    fn case_follows_the_platform() {
        let mixed = is_under(r"c:\music\pop\a.mp3", r"C:\Music");
        assert_eq!(mixed, CASE_INSENSITIVE);
    }

    #[test]
    fn trailing_separators_are_ignored() {
        assert_eq!(normalize(r"C:\Music\"), normalize(r"C:\Music"));
        assert_eq!(normalize("/Users/me/Music/"), normalize("/Users/me/Music"));
        // Gốc không được chuẩn hoá thành chuỗi rỗng.
        assert!(!normalize("/").is_empty());
    }

    #[test]
    fn app_dirs_use_the_os_dir_without_override() {
        let data = resolve_data_dir(None, Some("/os/data".into())).unwrap();
        assert_eq!(data, Path::new("/os/data").join(APP_DIR));
        let cache = resolve_cache_dir(None, Some("/os/cache".into())).unwrap();
        assert_eq!(cache, Path::new("/os/cache").join(APP_DIR));
    }

    #[test]
    fn override_moves_data_and_cache_together() {
        let over = || Some(OsString::from("/tmp/g"));
        assert_eq!(resolve_data_dir(over(), None).unwrap(), Path::new("/tmp/g"));
        assert_eq!(resolve_cache_dir(over(), None).unwrap(), Path::new("/tmp/g").join("cache"));
    }

    #[test]
    fn missing_os_dir_is_an_error() {
        assert_eq!(resolve_data_dir(None, None).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(resolve_cache_dir(None, None).unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn legacy_db_is_outside_the_new_app_dir() {
        let legacy = legacy_db().unwrap();
        assert!(legacy.ends_with(Path::new(LEGACY_APP_DIR).join(LEGACY_DB_FILE)));
    }
}
