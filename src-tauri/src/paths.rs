//! Path comparison that behaves the same on every platform GrooveX ships on.
//!
//! Three places need to ask "is this file inside that folder?" — the import scan,
//! the watched-root bookkeeping and the watcher's event filter. Each had grown its
//! own Windows-shaped version that lowercased the string and rewrote `/` as `\`,
//! which quietly breaks on macOS: `/Users/me/Music` would be compared as
//! `\users\me\music` against paths the OS reports with forward slashes.
//!
//! Case sensitivity follows the platform's usual filesystem rather than the
//! string: Windows (NTFS) and macOS (APFS, case-insensitive by default) compare
//! without regard to case, Linux compares exactly.

use std::path::Path;

/// Whether the platform's filesystem treats `A.mp3` and `a.mp3` as the same file.
const CASE_INSENSITIVE: bool = cfg!(any(target_os = "windows", target_os = "macos"));

/// Canonical form for comparison: one separator style, no trailing separator, and
/// case folded where the filesystem would fold it.
///
/// This is a string-level normalisation, not `fs::canonicalize` — it must work for
/// paths that no longer exist, such as a file reported by a delete event.
pub fn normalize(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let trimmed = unified.trim_end_matches('/');

    // A bare root ("/" or "C:/") trims away to nothing, so keep the separator.
    let base = if trimmed.is_empty() { unified.as_str() } else { trimmed };

    if CASE_INSENSITIVE {
        base.to_lowercase()
    } else {
        base.to_string()
    }
}

pub fn normalize_path(path: &Path) -> String {
    normalize(&path.to_string_lossy())
}

/// True when `child` is `parent` itself or sits somewhere beneath it.
///
/// Compares whole segments, so `C:\Music Videos` is correctly reported as *not*
/// being inside `C:\Music` despite the shared prefix.
pub fn is_under(child: &str, parent: &str) -> bool {
    let child = normalize(child);
    let parent = normalize(parent);

    if child == parent {
        return true;
    }
    // `parent` may already end in a separator when it is a filesystem root.
    let boundary = if parent.ends_with('/') {
        parent.clone()
    } else {
        format!("{parent}/")
    };
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
        // The bug this guards: "Music Videos" starts with "Music".
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
        // A root must not normalise away to an empty string.
        assert!(!normalize("/").is_empty());
    }
}
