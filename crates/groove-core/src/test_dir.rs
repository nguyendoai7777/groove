//! Thư mục làm việc cho test — thay `tempfile::TempDir`.
//!
//! `TempDir` mỗi lần tạo một thư mục ngẫu nhiên trong `%TEMP%`; trên Windows SQLite (`library.db` + `-wal` / `-shm`) còn
//! mở lúc nó drop thì xoá thất bại im lặng ⇒ mỗi lượt `cargo test` sót vài trăm thư mục (đã gom tới 37 nghìn thư mục,
//! làm bước nạp hồ sơ người dùng lúc đăng nhập Windows chậm hẳn). Ở đây mỗi test có **một** thư mục cố định
//! `<repo>/target/test-tmp/<crate>/<tên test>[-n]`: tạo mới (xoá sạch bản cũ) lúc đầu test, cố xoá lúc drop; xoá không
//! được thì lần chạy sau dọn. Chạy bao nhiêu lần cũng chỉ một bộ, không đụng `%TEMP%`, `clean.*` của workspace dọn theo `target/`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::Location;
use std::path::{Path, PathBuf};

/// Gốc chung: `target/test-tmp` của repo (hoặc worktree) chứa crate này.
fn root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().and_then(Path::parent).unwrap_or(manifest).join("target").join("test-tmp")
}

thread_local! {
    /// Số thư mục test hiện tại đã xin, theo khoá — test xin nhiều thư mục thì thêm hậu tố `-2`, `-3`…
    static TAKEN: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

/// Tên an toàn cho tên thư mục trên mọi OS.
fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Thư mục của một test; xem đầu mô-đun.
#[derive(Debug)]
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    /// Thư mục riêng của test đang chạy: tên lấy từ tên luồng của libtest (`module::tests::ten_test`) + crate của nơi gọi.
    /// Luồng không mang tên test (chạy một luồng trên `main`) ⇒ theo tệp + dòng gọi, vẫn cố định giữa các lần chạy.
    #[track_caller]
    #[allow(clippy::new_without_default, reason = "tên thư mục lấy từ nơi gọi")]
    pub fn new() -> TestDir {
        let caller = Location::caller();
        let file = caller.file().replace('\\', "/");
        let krate = file.split('/').skip_while(|p| *p != "crates").nth(1).unwrap_or("crate");
        let stem = file.rsplit('/').next().unwrap_or("test").trim_end_matches(".rs");
        // Tên tệp gọi vào khoá để hai binary test (unit + `tests/*.rs`) trùng tên hàm không giẫm thư mục nhau.
        let test = match std::thread::current().name() {
            Some(name) if name != "main" => name.to_string(),
            _ => format!("line{}", caller.line()),
        };
        let key = format!("{krate}/{stem}-{}", sanitize(&test));
        let n = TAKEN.with(|t| {
            let mut t = t.borrow_mut();
            let n = t.entry(key.clone()).or_insert(0);
            *n += 1;
            *n
        });
        let name = if n == 1 { key } else { format!("{key}-{n}") };
        let path = root().join(name);
        fresh(&path);
        TestDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Xoá bản cũ (lần chạy trước còn sót) rồi tạo thư mục rỗng. Windows: tệp vừa đóng có thể còn khoá giây lát ⇒ thử lại.
fn fresh(path: &Path) {
    for attempt in 0..20 {
        match std::fs::remove_dir_all(path) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) if attempt == 19 => panic!("test dir {} is still in use: {e}", path.display()),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    std::fs::create_dir_all(path).unwrap_or_else(|e| panic!("cannot create test dir {}: {e}", path.display()));
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // Còn mở (SQLite, watcher) thì để đó: lần chạy sau `fresh` dọn; số thư mục không tăng theo số lần chạy.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_test_gets_the_same_dir_every_run_and_starts_empty() {
        let first = TestDir::new();
        std::fs::write(first.path().join("library.db"), b"x").unwrap();
        let path = first.path().to_path_buf();
        assert!(
            path.ends_with(
                "groove-core/test_dir-test_dir__tests__same_test_gets_the_same_dir_every_run_and_starts_empty"
            )
        );
        // Giả lần chạy sau: tệp còn sót (drop không xoá được) ⇒ `fresh` xoá sạch.
        std::mem::forget(first);
        TAKEN.with(|t| t.borrow_mut().clear());
        let again = TestDir::new();
        assert_eq!(again.path(), path);
        assert_eq!(std::fs::read_dir(again.path()).unwrap().count(), 0);
        // Thư mục thứ hai trong cùng test có hậu tố riêng.
        let second = TestDir::new();
        assert_ne!(second.path(), again.path());
        drop(second);
        drop(again);
        assert!(!path.exists());
    }
}
