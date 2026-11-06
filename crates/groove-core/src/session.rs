//! groove-gd1_02: session.json.
//!
//! Chỉ giữ tham chiếu bài, không kèm lyrics, bìa như localStorage của bản cũ (S12). Khôi phục: nạp lại bài ở vị trí cũ, không
//! tự phát, nên không có trường "đang phát" (đặc tả 1.4). Ghi nền qua `settings::JsonSaver`: vị trí mỗi 5 giây, khi dừng, khi
//! thoát (`SAVE_INTERVAL`).

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::{RepeatMode, TrackRef};
use crate::paths;
use crate::settings::{load_json, write_json_atomic};

/// Nhịp ghi vị trí khi đang phát (S12).
pub const SAVE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    pub current: Option<TrackRef>,
    pub position_sec: f64,
    pub original: Vec<TrackRef>,
    pub history: Vec<TrackRef>,
    pub queue: Vec<TrackRef>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Session {
    /// Đọc `paths::session_file`, không lỗi: có gì bất thường thì ghi log và về phiên trống.
    pub fn load() -> Session {
        match paths::session_file() {
            Ok(path) => Session::load_from(&path),
            Err(err) => {
                log::warn!("session path: {err}");
                Session::default()
            }
        }
    }

    pub fn load_from(path: &Path) -> Session {
        let mut session: Session = load_json(path);
        session.sanitize();
        session
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&paths::session_file()?)
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        write_json_atomic(path, self)
    }

    /// Giữ các bất biến của hàng đợi: mỗi bài xuất hiện một lần trong history + current + queue, shuffle và repeat không cùng
    /// bật (đặc tả 1.4). Tệp sửa tay hoặc ghi dở vẫn ra phiên dùng được.
    pub fn sanitize(&mut self) {
        if !self.position_sec.is_finite() || self.position_sec < 0.0 || self.current.is_none() {
            self.position_sec = 0.0;
        }
        if self.shuffle && self.repeat != RepeatMode::Off {
            self.repeat = RepeatMode::Off;
        }

        let mut seen = HashSet::new();
        if let Some(current) = &self.current {
            seen.insert(current.clone());
        }
        self.queue.retain(|track| seen.insert(track.clone()));
        // Bài vừa ở history vừa ở queue thì giữ ở queue: chưa nghe lại thì không mất lượt.
        self.history.retain(|track| seen.insert(track.clone()));

        let mut seen = HashSet::new();
        self.original.retain(|track| seen.insert(track.clone()));
    }

    /// Bỏ bài không còn trong thư viện (N5). Mất bài đang phát thì không còn bài nào được chọn, vị trí về 0; hàng đợi giữ nguyên.
    pub fn retain_tracks(&mut self, mut keep: impl FnMut(&TrackRef) -> bool) {
        self.original.retain(&mut keep);
        self.history.retain(&mut keep);
        self.queue.retain(&mut keep);
        if self.current.as_ref().is_some_and(|track| !keep(track)) {
            self.current = None;
            self.position_sec = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(ids: &[i64]) -> Vec<TrackRef> {
        ids.iter().copied().map(TrackRef::Library).collect()
    }

    fn sample() -> Session {
        Session {
            current: Some(TrackRef::Library(2)),
            position_sec: 61.5,
            original: lib(&[1, 2, 3, 4]),
            history: lib(&[1]),
            queue: vec![TrackRef::Library(3), TrackRef::from_path("/m/ngoài.mp3")],
            shuffle: false,
            repeat: RepeatMode::One,
        }
    }

    #[test]
    fn missing_file_is_an_empty_session() {
        let dir = crate::test_dir::TestDir::new();
        assert_eq!(Session::load_from(&dir.path().join("session.json")), Session::default());
    }

    #[test]
    fn round_trip_keeps_library_and_external_tracks() {
        let dir = crate::test_dir::TestDir::new();
        let path = dir.path().join("session.json");
        sample().save_to(&path).unwrap();
        assert_eq!(Session::load_from(&path), sample());
    }

    #[test]
    fn json_holds_references_only_with_camel_case_keys() {
        let json = serde_json::to_value(sample()).unwrap();
        assert_eq!(json["positionSec"], 61.5);
        assert_eq!(json["current"]["library"], 2);
        assert_eq!(json["repeat"], "one");
        assert_eq!(json["queue"][1]["external"], "/m/ngoài.mp3");
        assert!(json.get("lyrics").is_none());
    }

    #[test]
    fn corrupt_file_is_an_empty_session() {
        let dir = crate::test_dir::TestDir::new();
        let path = dir.path().join("session.json");
        std::fs::write(&path, "{ cut off").unwrap();
        assert_eq!(Session::load_from(&path), Session::default());
    }

    #[test]
    fn a_bad_key_keeps_the_rest_of_the_session() {
        let dir = crate::test_dir::TestDir::new();
        let path = dir.path().join("session.json");
        std::fs::write(&path, r#"{"current": {"library": 5}, "positionSec": 9, "queue": "oops"}"#).unwrap();
        let s = Session::load_from(&path);
        assert_eq!(s.current, Some(TrackRef::Library(5)));
        assert_eq!(s.position_sec, 9.0);
        assert!(s.queue.is_empty());
    }

    #[test]
    fn sanitize_makes_every_track_appear_once() {
        let mut s = Session {
            current: Some(TrackRef::Library(1)),
            original: lib(&[1, 1, 2, 3]),
            history: lib(&[1, 2, 4, 4]),
            queue: lib(&[1, 2, 3, 3]),
            ..Session::default()
        };
        s.sanitize();
        assert_eq!(s.original, lib(&[1, 2, 3]));
        assert_eq!(s.queue, lib(&[2, 3]));
        assert_eq!(s.history, lib(&[4]));
    }

    #[test]
    fn sanitize_treats_path_spellings_as_one_track() {
        let mut s = Session {
            current: Some(TrackRef::from_path(r"C:\Music\a.mp3")),
            queue: vec![TrackRef::from_path("C:/Music/a.mp3")],
            ..Session::default()
        };
        s.sanitize();
        assert!(s.queue.is_empty());
    }

    #[test]
    fn sanitize_fixes_position_and_shuffle_repeat_conflict() {
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let mut s = Session { current: Some(TrackRef::Library(1)), position_sec: bad, ..Session::default() };
            s.sanitize();
            assert_eq!(s.position_sec, 0.0);
        }
        let mut s = Session { position_sec: 5.0, shuffle: true, repeat: RepeatMode::One, ..Session::default() };
        s.sanitize();
        assert_eq!(s.position_sec, 0.0, "không có bài thì không có vị trí");
        assert!(s.shuffle);
        assert_eq!(s.repeat, RepeatMode::Off);
    }

    #[test]
    fn retain_tracks_drops_dead_tracks_everywhere() {
        let mut s = sample();
        s.retain_tracks(|track| *track != TrackRef::Library(3) && *track != TrackRef::Library(1));
        assert_eq!(s.original, lib(&[2, 4]));
        assert!(s.history.is_empty());
        assert_eq!(s.queue, vec![TrackRef::from_path("/m/ngoài.mp3")]);
        assert_eq!(s.current, Some(TrackRef::Library(2)));
        assert_eq!(s.position_sec, 61.5);
    }

    #[test]
    fn losing_the_current_track_resets_the_position() {
        let mut s = sample();
        s.retain_tracks(|track| *track != TrackRef::Library(2));
        assert_eq!(s.current, None);
        assert_eq!(s.position_sec, 0.0);
        assert_eq!(s.queue.len(), 2);
    }
}
