//! Log ra file `<data_dir>/logs/<tiến trình>.log` (+ stderr khi build debug). Tự viết thay `env_logger` / `tracing`
//! để khỏi mang regex / định dạng thời gian. Quá `ROTATE_BYTES` lúc khởi động ⇒ đổi thành `.old.log`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, Log, Metadata, Record};

pub const ROTATE_BYTES: u64 = 1 << 20;

struct FileLogger {
    file: Option<Mutex<File>>,
    stderr: bool,
    level: Level,
}

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= self.level
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let level = if is_gpui_teardown(r) { Level::Debug } else { r.level() };
        if level > self.level {
            return;
        }
        let origin = origin(r.target(), r.file(), r.line());
        let line = format!("{} {level:<5} {origin}: {}\n", utc_timestamp(unix_now()), r.args());
        if let Some(f) = &self.file
            && let Ok(mut f) = f.lock()
        {
            let _ = f.write_all(line.as_bytes());
        }
        if self.stderr {
            let _ = io::stderr().write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(f) = &self.file
            && let Ok(mut f) = f.lock()
        {
            let _ = f.flush();
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn is_gpui_teardown(r: &Record) -> bool {
    r.level() == Level::Error
        && r.target().is_empty()
        && r.file().is_some_and(|file| file.contains("gpui-pre"))
        && is_teardown_message(&r.args().to_string())
}

/// Dấu vết GPUI gọi vào cửa sổ vừa đóng, không mất gì, không sửa được từ ngoài thư viện ⇒ hạ `ERROR` xuống `DEBUG` để khỏi che
/// lỗi thật: `window not found` — `WM_ACTIVATE` của cửa sổ đang active chạy callback (hoãn qua executor) sau khi cửa sổ đã gỡ;
/// `Invalid window handle` — luồng vsync `RedrawWindow` đúng lúc HWND vừa huỷ, `RevokeDragDrop` / `DestroyWindow` sau khi OS
/// đã tự huỷ (nút X), `PostMessage` vào cửa sổ ẩn của platform lúc thoát.
fn is_teardown_message(message: &str) -> bool {
    message == "window not found" || message.starts_with("Invalid window handle")
}

/// Nơi ghi của một dòng log. `log_err` của GPUI để trống `target` (tìm crate theo thư mục `crates/`, thư viện từ
/// crates.io không có) ⇒ dùng tệp + dòng; chỉ giữ ba đoạn cuối để log không lộ đường dẫn máy build.
fn origin(target: &str, file: Option<&str>, line: Option<u32>) -> String {
    match (target, file) {
        ("", Some(file)) => {
            let parts: Vec<&str> = file.split(['/', '\\']).collect();
            format!("{}:{}", parts[parts.len().saturating_sub(3)..].join("/"), line.unwrap_or(0))
        }
        _ => target.to_owned(),
    }
}

/// Bật log cho tiến trình `name` (`groove`). Không mở được file thì chỉ ghi stderr. Trả đường dẫn file log.
/// Mức log: `GROOVE_LOG=debug|info|warn|error`, mặc định `info`. Bản release ghi cả stderr khi có
/// `GROOVE_LOG_STDERR` (test tích hợp đọc log của exe con qua ống).
pub fn init(name: &str) -> Option<PathBuf> {
    let level = std::env::var("GROOVE_LOG").ok().and_then(|s| s.parse().ok()).unwrap_or(Level::Info);
    let path = crate::paths::logs_dir().ok().map(|d| d.join(format!("{name}.log")));
    let file = path.as_deref().and_then(|p| open_rotated(p).ok());
    let stderr = cfg!(debug_assertions) || std::env::var_os("GROOVE_LOG_STDERR").is_some();
    let logger = FileLogger { file: file.map(Mutex::new), stderr, level };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level.to_level_filter());
    }
    path
}

fn open_rotated(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        let _ = fs::rename(path, path.with_extension("old.log"));
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// `2026-10-29T12:34:56Z` từ giây Unix (UTC, lịch Gregory — thuật toán `civil_from_days` của H. Hinnant).
pub fn utc_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_timestamp(1_793_277_296), "2026-10-29T12:34:56Z");
    }

    #[test]
    fn only_window_teardown_messages_are_demoted() {
        assert!(is_teardown_message("window not found"));
        assert!(is_teardown_message("Invalid window handle. (0x80070578)"));
        assert!(is_teardown_message("Invalid window handle (0x80040102)"));
        assert!(!is_teardown_message("RefCell already borrowed"));
        assert!(!is_teardown_message("Failed to resize renderer"));
    }

    #[test]
    fn empty_target_logs_short_file_and_line() {
        let file = r"C:\Users\someone\.cargo\registry\gpui-pre-0.3.10\src/window.rs";
        assert_eq!(origin("", Some(file), Some(2074)), "gpui-pre-0.3.10/src/window.rs:2074");
        assert_eq!(origin("groove::app", Some(file), Some(1)), "groove::app");
        assert_eq!(origin("", None, None), "");
    }

    #[test]
    fn rotation_moves_big_file_aside() {
        let dir = crate::test_dir::TestDir::new();
        let path = dir.path().join("logs").join("groove.log");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![b'x'; ROTATE_BYTES as usize + 1]).unwrap();
        let mut f = open_rotated(&path).unwrap();
        writeln!(f, "fresh").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "fresh\n");
        assert!(path.with_extension("old.log").exists());
    }
}
