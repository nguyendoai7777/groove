//! groove-gd1_13: ghi session.json nền.
//!
//! Giữ một [`JsonSaver`] riêng cho `session.json` (bộ lưu của `settings.json` nằm ở `app.rs`, không dùng chung tệp). `save` chỉ
//! tuần tự hoá rồi gửi sang luồng ghi, UI thread không chạm đĩa (N7). Nhịp gọi do `player.rs` quyết: mỗi 5 giây khi phát, khi
//! đổi hàng đợi, khi dừng, khi thoát (S12).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use groove_core::model::TrackRef;
use groove_core::queue::Queue;
use groove_core::session::Session;
use groove_core::settings::JsonSaver;

/// Gom các thao tác liên tiếp trên hàng đợi thành một lần ghi.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Đầu ghi phiên. Không mở được luồng ghi thì app vẫn chạy, chỉ không lưu.
pub struct SessionSync {
    saver: Option<JsonSaver>,
    session: Session,
}

impl SessionSync {
    pub fn new(path: Option<PathBuf>) -> SessionSync {
        let saver = path.and_then(|path| {
            JsonSaver::spawn(path, SAVE_DEBOUNCE).map_err(|err| log::warn!("session saver: {err}")).ok()
        });
        SessionSync { saver, session: Session::default() }
    }

    pub fn save(&mut self, queue: &Queue, position_sec: f64) {
        queue.write_to(&mut self.session);
        self.session.position_sec = position_sec;
        self.session.sanitize();
        if let Some(saver) = &self.saver {
            saver.save(&self.session);
        }
    }

    /// Chặn tới khi lần lưu đang chờ xuống đĩa. Chỉ gọi khi thoát.
    pub fn flush(&self) {
        if let Some(saver) = &self.saver {
            saver.flush();
        }
    }
}

/// Mọi bài cần soát của phiên vừa đọc, mỗi bài một lần.
pub fn tracks_of(queue: &Queue) -> Vec<TrackRef> {
    let mut seen = HashSet::new();
    queue.visible().chain(queue.original()).filter(|track| seen.insert((*track).clone())).cloned().collect()
}

/// Tệp ngoài không còn trên đĩa. Có chạm đĩa: gọi ở luồng nền.
pub fn missing_external(tracks: &[TrackRef]) -> HashSet<TrackRef> {
    tracks.iter().filter(|track| matches!(track, TrackRef::External(path) if !path.exists())).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use groove_core::model::RepeatMode;

    #[test]
    fn saved_session_round_trips_without_playing_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut queue = Queue::with_seed(1);
        queue.play_list(TrackRef::Library(2), vec![TrackRef::Library(1), TrackRef::Library(2), TrackRef::Library(3)]);
        queue.set_repeat(RepeatMode::One);

        let mut sync = SessionSync::new(Some(path.clone()));
        sync.save(&queue, 12.5);
        sync.flush();

        let back = Session::load_from(&path);
        assert_eq!(back.current, Some(TrackRef::Library(2)));
        assert_eq!(back.position_sec, 12.5);
        assert_eq!(back.queue, vec![TrackRef::Library(3)]);
        assert_eq!(back.repeat, RepeatMode::One);
    }

    #[test]
    fn saver_failure_is_not_fatal() {
        let mut sync = SessionSync::new(None);
        sync.save(&Queue::with_seed(1), 1.0);
        sync.flush();
    }

    #[test]
    fn only_missing_external_files_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let there = dir.path().join("a.mp3");
        std::fs::write(&there, b"x").unwrap();
        let gone = TrackRef::from_path(dir.path().join("b.mp3"));
        let tracks = vec![TrackRef::Library(1), TrackRef::from_path(there), gone.clone()];
        assert_eq!(missing_external(&tracks), HashSet::from([gone]));
    }

    #[test]
    fn tracks_of_lists_each_track_once() {
        let mut queue = Queue::with_seed(1);
        queue.play_list(TrackRef::Library(1), vec![TrackRef::Library(1), TrackRef::Library(2)]);
        assert_eq!(tracks_of(&queue), vec![TrackRef::Library(1), TrackRef::Library(2)]);
    }
}
