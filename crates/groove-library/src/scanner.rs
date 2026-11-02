//! groove-gd1_08: ScanOutcome, walkdir (N1, S26).
//!
//! Chỉ liệt kê, không đọc thẻ: tệp OneDrive chưa tải không bị kéo về chỉ vì được quét (S26). Chỗ không xem được ghi lại đủ
//! trong `ScanOutcome`, để caller không bao giờ hiểu "không thấy" thành "đã xoá" (N1).

use std::fs;
use std::path::{Path, PathBuf};

use groove_core::model::is_audio_path;
use groove_core::paths;
use walkdir::WalkDir;

use crate::tags::{FileStamp, is_temp_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub stamp: FileStamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFailure {
    pub path: PathBuf,
    pub message: String,
}

/// `files` xếp theo đường dẫn, nên "bài đầu tiên có bìa" không đổi giữa các lần quét.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanOutcome {
    pub files: Vec<ScannedFile>,
    /// Không đọc được, bên trong chưa rõ có gì.
    pub failed_dirs: Vec<ScanFailure>,
    /// Thấy tên nhưng không lấy được dấu tệp.
    pub failed_files: Vec<ScanFailure>,
    /// Liên kết tới thư mục không đi theo (S26), hoặc liên kết hỏng. Bản cũ có đi theo, nên dòng cũ có thể nằm dưới đây.
    pub skipped_links: Vec<PathBuf>,
}

impl ScanOutcome {
    /// Tệp có thể vẫn còn dù không nằm trong `files`: chỗ chứa nó chưa được xem. Dòng như vậy không được xoá (N1).
    pub fn may_still_exist(&self, path: &Path) -> bool {
        self.failed_dirs.iter().chain(&self.failed_files).any(|failure| paths::is_under_path(path, &failure.path))
            || self.skipped_links.iter().any(|link| paths::is_under_path(path, link))
    }

    /// Lỗi của chính gốc: cả cây chưa được xem.
    pub fn root_failure(&self, root: &Path) -> Option<&ScanFailure> {
        let root = paths::normalize_path(root);
        self.failed_dirs.iter().find(|failure| paths::normalize_path(&failure.path) == root)
    }
}

/// Đệ quy, đuôi nhạc không phân biệt hoa thường, bỏ bản tạm khi ghi thẻ. Không theo liên kết thư mục nên vòng lặp không làm
/// tràn stack (S26). Gốc không tồn tại, không đọc được hoặc không phải thư mục thành một dòng ở `failed_dirs`.
/// Chạm đĩa, chỉ gọi ở luồng nền.
pub fn scan(root: &Path) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    match fs::metadata(root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            outcome.failed_dirs.push(failure(root, "not a directory"));
            return outcome;
        }
        Err(err) => {
            outcome.failed_dirs.push(failure(root, err));
            return outcome;
        }
    }
    for entry in WalkDir::new(root).follow_links(false) {
        match entry {
            Ok(entry) if entry.file_type().is_symlink() => match fs::metadata(entry.path()) {
                Ok(meta) if meta.is_file() => add_file(&mut outcome, entry.path()),
                _ => outcome.skipped_links.push(entry.into_path()),
            },
            Ok(entry) if entry.file_type().is_file() => add_file(&mut outcome, entry.path()),
            Ok(_) => {}
            Err(err) => outcome.failed_dirs.push(failure(err.path().unwrap_or(root), &err)),
        }
    }
    outcome.files.sort_by(|a, b| a.path.cmp(&b.path));
    outcome
}

fn add_file(outcome: &mut ScanOutcome, path: &Path) {
    if !is_audio_path(path) || is_temp_path(path) {
        return;
    }
    match FileStamp::of(path) {
        Ok(stamp) => outcome.files.push(ScannedFile { path: path.to_owned(), stamp }),
        Err(err) => outcome.failed_files.push(failure(path, err)),
    }
}

fn failure(path: &Path, message: impl ToString) -> ScanFailure {
    ScanFailure { path: path.to_owned(), message: message.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn names(outcome: &ScanOutcome) -> Vec<String> {
        outcome.files.iter().map(|file| file.path.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn finds_audio_recursively_in_a_stable_order() {
        let dir = TempDir::new().unwrap();
        for name in ["b.mp3", "a/c.FLAC", "a/deep/d.m4a", "e.wav", "f.ogg", "g.m4r"] {
            touch(&dir.path().join(name));
        }
        for name in ["note.txt", "cover.jpg", "noext", "h.mp3.groove-tmp", "a/i.mp3x"] {
            touch(&dir.path().join(name));
        }
        let outcome = scan(dir.path());
        assert_eq!(names(&outcome), ["c.FLAC", "d.m4a", "b.mp3", "e.wav", "f.ogg", "g.m4r"]);
        assert!(outcome.failed_dirs.is_empty() && outcome.failed_files.is_empty());
        let meta = fs::metadata(&outcome.files[0].path).unwrap();
        assert_eq!(outcome.files[0].stamp.size, meta.len());
    }

    #[test]
    fn missing_root_is_a_failure_not_an_empty_folder() {
        // N1: bản cũ nuốt lỗi `read_dir`, nên gốc mất giống hệt gốc rỗng.
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("gone");
        let outcome = scan(&root);
        assert!(outcome.files.is_empty());
        assert!(outcome.root_failure(&root).is_some());
        assert!(outcome.may_still_exist(&root.join("a").join("b.mp3")));
        assert!(!outcome.may_still_exist(&dir.path().join("gone2").join("b.mp3")));
    }

    #[test]
    fn a_file_root_is_a_failure() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("a.mp3");
        touch(&file);
        assert!(scan(&file).root_failure(&file).is_some());
    }

    #[test]
    fn empty_folder_has_no_failure() {
        let dir = TempDir::new().unwrap();
        assert_eq!(scan(dir.path()), ScanOutcome::default());
    }

    #[cfg(unix)]
    fn link_dir(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn link_dir(target: &Path, link: &Path) -> bool {
        // Junction không cần quyền quản trị như symlink.
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|out| out.status.success())
    }

    #[test]
    fn a_link_cycle_ends_and_protects_what_is_behind_it() {
        // S26: bản cũ đệ quy theo liên kết, vòng lặp làm tràn stack.
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        touch(&root.join("a.mp3"));
        let link = root.join("loop");
        if !link_dir(&root, &link) {
            eprintln!("skipped: cannot create a directory link here");
            return;
        }
        let outcome = scan(&root);
        assert_eq!(names(&outcome), ["a.mp3"]);
        assert_eq!(outcome.skipped_links, std::slice::from_ref(&link));
        assert!(outcome.may_still_exist(&link.join("a.mp3")));
        assert!(!outcome.may_still_exist(&root.join("a.mp3")));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_file_is_indexed() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let target = dir.path().join("outside.mp3");
        fs::create_dir(&root).unwrap();
        touch(&target);
        std::os::unix::fs::symlink(&target, root.join("linked.mp3")).unwrap();
        assert_eq!(names(&scan(&root)), ["linked.mp3"]);
    }
}
