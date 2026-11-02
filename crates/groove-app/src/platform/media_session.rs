//! groove-gd1_26: souvlaki (đặc tả 5.4): SMTC trên Windows, `MPNowPlayingInfoCenter` trên macOS, MPRIS qua `zbus` trên Linux.
//!
//! Một chiều: [`Tracker`] so trạng thái `Player` với lần báo trước, đổi thành [`Update`]. Chiều về: sự kiện của OS vào kênh, về UI
//! thread rồi mới gọi `commands::run` (Prev, Next, Toggle chung dispatcher với phím tắt) hoặc thẳng `Player` (Play, Pause tách
//! riêng, tua tuyệt đối không có trong `Command`).
//!
//! Windows, Linux gọi souvlaki trên luồng `media-session`: `set_metadata` của Windows đọc tệp bìa bằng `join()` chặn luồng (N7), và
//! gọi WinRT trên UI thread có thể chạy lồng wndproc của GPUI. macOS gọi trên main thread, ảnh bìa souvlaki tự nạp ở hàng đợi nền.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc;
use groove_core::keymap::Command;
use groove_core::model::Song;
use groove_library::covers::{CoverSize, CoverStore};
use materiant::gpui_kit::{AnyWindowHandle, App, Entity};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
};

use crate::player::{Player, player};

/// Tên MPRIS: `org.mpris.MediaPlayer2.groovex`.
const DBUS_NAME: &str = "groovex";
const DISPLAY_NAME: &str = "Groove X";
/// Vị trí lệch dự đoán quá mức này là người dùng vừa tua, phải báo. Nhịp thường thì OS tự chạy đồng hồ, không báo 4 lần mỗi giây.
const POSITION_DRIFT_SEC: f64 = 1.0;

#[derive(Clone, Debug, PartialEq)]
struct TrackInfo {
    title: String,
    artist: String,
    album: String,
    cover: Option<PathBuf>,
    duration_sec: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackState {
    Stopped,
    Paused,
    Playing,
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    track: Option<TrackInfo>,
    playing: bool,
    position_sec: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum Update {
    Metadata(Option<TrackInfo>),
    Playback { state: PlaybackState, position_sec: f64 },
}

/// Lần báo cuối. `Player` báo 4 lần mỗi giây khi phát, phần lớn không đổi gì OS cần biết.
struct Tracker {
    track: Option<TrackInfo>,
    state: PlaybackState,
    position_sec: f64,
    sent_at: Instant,
}

impl Tracker {
    fn new(now: Instant) -> Tracker {
        Tracker { track: None, state: PlaybackState::Stopped, position_sec: 0.0, sent_at: now }
    }

    fn update(&mut self, snapshot: Snapshot, now: Instant) -> Vec<Update> {
        let state = match (&snapshot.track, snapshot.playing) {
            (None, _) => PlaybackState::Stopped,
            (Some(_), true) => PlaybackState::Playing,
            (Some(_), false) => PlaybackState::Paused,
        };
        let expected = match self.state {
            PlaybackState::Playing => self.position_sec + now.duration_since(self.sent_at).as_secs_f64(),
            PlaybackState::Paused | PlaybackState::Stopped => self.position_sec,
        };
        let mut updates = Vec::new();
        let new_track = snapshot.track != self.track;
        if new_track {
            self.track = snapshot.track.clone();
            updates.push(Update::Metadata(snapshot.track));
        }
        let jumped = state != PlaybackState::Stopped && (snapshot.position_sec - expected).abs() > POSITION_DRIFT_SEC;
        if new_track || state != self.state || jumped {
            self.state = state;
            self.position_sec = snapshot.position_sec;
            self.sent_at = now;
            updates.push(Update::Playback { state, position_sec: snapshot.position_sec });
        }
        updates
    }
}

/// `cover_hash` đến từ DB rồi ghép vào đường dẫn: chỉ nhận hex, như `views::category_card::cover_path`.
fn cover_of(song: &Song, covers: Option<&CoverStore>) -> Option<PathBuf> {
    let hash =
        song.cover_hash.as_deref().filter(|hash| !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit()))?;
    Some(covers?.path(hash, CoverSize::Large))
}

fn snapshot(player: &Player, covers: Option<&CoverStore>) -> Snapshot {
    let track = player.current_song().map(|song| TrackInfo {
        title: song.display_title().to_owned(),
        artist: song.artist.clone().unwrap_or_default(),
        album: song.album.clone().unwrap_or_default(),
        cover: cover_of(song, covers),
        duration_sec: player.duration_sec(),
    });
    Snapshot { track, playing: player.is_playing(), position_sec: player.position_sec() }
}

// ---- Sự kiện của OS ----

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Run(Command),
    Play,
    Pause,
    Stop,
    SeekTo(f64),
    SeekBy(f64),
    Raise,
}

fn action_for(event: &MediaControlEvent) -> Option<Action> {
    let signed = |direction: &SeekDirection, by: &Duration| match direction {
        SeekDirection::Forward => by.as_secs_f64(),
        SeekDirection::Backward => -by.as_secs_f64(),
    };
    Some(match event {
        MediaControlEvent::Play => Action::Play,
        MediaControlEvent::Pause => Action::Pause,
        MediaControlEvent::Toggle => Action::Run(Command::TogglePlay),
        MediaControlEvent::Next => Action::Run(Command::NextTrack),
        MediaControlEvent::Previous => Action::Run(Command::PrevTrack),
        MediaControlEvent::Stop => Action::Stop,
        MediaControlEvent::Seek(SeekDirection::Forward) => Action::Run(Command::SeekForward),
        MediaControlEvent::Seek(SeekDirection::Backward) => Action::Run(Command::SeekBackward),
        MediaControlEvent::SeekBy(direction, by) => Action::SeekBy(signed(direction, by)),
        MediaControlEvent::SetPosition(MediaPosition(position)) => Action::SeekTo(position.as_secs_f64()),
        MediaControlEvent::Raise => Action::Raise,
        // Âm lượng là của app, tắt app từ OS, mở URI: ngoài đặc tả.
        _ => return None,
    })
}

fn apply(action: Action, window: AnyWindowHandle, cx: &mut App) {
    let target = player(cx);
    match action {
        Action::Run(command) => crate::commands::run(command, cx),
        Action::Play => target.update(cx, |p, cx| p.play(cx)),
        Action::Pause => target.update(cx, |p, cx| p.pause(cx)),
        Action::Stop => target.update(cx, |p, cx| {
            p.pause(cx);
            p.seek(0.0, cx);
        }),
        Action::SeekTo(sec) => target.update(cx, |p, cx| p.seek(sec, cx)),
        Action::SeekBy(delta) => target.update(cx, |p, cx| p.seek_by(delta, cx)),
        Action::Raise => {
            let _ = window.update(cx, |_, window, _| window.activate_window());
        }
    }
}

// ---- Gọi souvlaki ----

/// URL tệp theo MPRIS và `NSURL`: `file://` + đường dẫn đã mã hoá; thư mục dữ liệu có dấu cách (`Groove X`).
#[cfg(any(not(windows), test))]
fn file_url(path: &str) -> String {
    let mut url = String::with_capacity(path.len() + 7);
    url.push_str("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => url.push(byte as char),
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// Souvlaki trên Windows cắt `file://` rồi đưa phần còn lại cho `StorageFile`: không mã hoá, phải dấu `\`.
#[cfg(windows)]
fn cover_url(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace('/', "\\"))
}

#[cfg(not(windows))]
fn cover_url(path: &Path) -> String {
    file_url(&path.to_string_lossy())
}

/// Windows: souvlaki không xoá được ảnh, bài không bìa sẽ giữ ảnh bài trước, và tệp bìa mất thì cả `set_metadata` hỏng. Thay bằng logo.
#[cfg(windows)]
fn usable_cover(track: Option<&TrackInfo>) -> Option<PathBuf> {
    static NO_COVER: &[u8] = include_bytes!("../../assets/media/no_cover.png");
    if let Some(cover) = track.and_then(|track| track.cover.as_ref()).filter(|cover| cover.is_file()) {
        return Some(cover.clone());
    }
    let dir = groove_core::paths::cache_dir().ok()?;
    let path = dir.join("media-session-no-cover.png");
    if !path.is_file() {
        std::fs::create_dir_all(&dir).ok()?;
        let temp = dir.join(format!("media-session-no-cover.{}.tmp", std::process::id()));
        std::fs::write(&temp, NO_COVER).ok()?;
        std::fs::rename(&temp, &path).ok()?;
    }
    Some(path)
}

/// macOS, Linux: không có ảnh là xoá ảnh; tệp mất thì OS bỏ qua.
#[cfg(not(windows))]
fn usable_cover(track: Option<&TrackInfo>) -> Option<PathBuf> {
    track.and_then(|track| track.cover.clone())
}

fn publish(controls: &mut MediaControls, update: Update) {
    match update {
        Update::Metadata(track) => {
            let cover = usable_cover(track.as_ref()).map(|path| cover_url(&path));
            let empty = String::new();
            let (title, artist, album) = match &track {
                Some(track) => (&track.title, &track.artist, &track.album),
                None => (&empty, &empty, &empty),
            };
            let duration = track
                .as_ref()
                .map(|track| track.duration_sec)
                .filter(|sec| sec.is_finite() && *sec > 0.0)
                .and_then(|sec| Duration::try_from_secs_f64(sec).ok());
            // `None` là giữ giá trị cũ trên Windows, nên chữ trống vẫn phải gửi `Some`.
            let metadata = |cover_url| MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                album: Some(album),
                cover_url,
                duration,
            };
            if let Err(err) = controls.set_metadata(metadata(cover.as_deref())) {
                log::warn!("media session metadata: {err:?}");
                // Ảnh hỏng không được làm mất tên bài.
                if cover.is_some()
                    && let Err(err) = controls.set_metadata(metadata(None))
                {
                    log::warn!("media session metadata without cover: {err:?}");
                }
            }
        }
        Update::Playback { state, position_sec } => {
            let progress = Some(MediaPosition(Duration::try_from_secs_f64(position_sec.max(0.0)).unwrap_or_default()));
            let playback = match state {
                PlaybackState::Stopped => MediaPlayback::Stopped,
                PlaybackState::Paused => MediaPlayback::Paused { progress },
                PlaybackState::Playing => MediaPlayback::Playing { progress },
            };
            if let Err(err) = controls.set_playback(playback) {
                log::warn!("media session playback: {err:?}");
            }
        }
    }
}

/// Không dựng được thì app chạy tiếp, chỉ thiếu media session: máy Linux không có D-Bus, SMTC bị tắt.
fn open_controls(hwnd: Option<isize>, events: mpsc::UnboundedSender<MediaControlEvent>) -> Option<MediaControls> {
    if cfg!(windows) && hwnd.is_none() {
        // souvlaki `expect` HWND trên Windows.
        log::warn!("media session needs a window");
        return None;
    }
    let config = PlatformConfig {
        dbus_name: DBUS_NAME,
        display_name: DISPLAY_NAME,
        hwnd: hwnd.map(|hwnd| hwnd as *mut std::ffi::c_void),
    };
    let mut controls = MediaControls::new(config).map_err(|err| log::warn!("media session: {err:?}")).ok()?;
    // Handler chạy ở luồng của OS, chỉ đẩy vào kênh.
    let attached = controls.attach(move |event| {
        let _ = events.unbounded_send(event);
    });
    attached.map_err(|err| log::warn!("media session attach: {err:?}")).ok()?;
    Some(controls)
}

#[cfg(not(target_os = "macos"))]
fn spawn_publisher(
    hwnd: Option<isize>,
    events: mpsc::UnboundedSender<MediaControlEvent>,
) -> impl FnMut(Update) + 'static {
    let (tx, rx) = crossbeam_channel::unbounded::<Update>();
    let started = std::thread::Builder::new().name("media-session".into()).spawn(move || {
        let Some(mut controls) = open_controls(hwnd, events) else { return };
        for update in rx {
            publish(&mut controls, update);
        }
    });
    if let Err(err) = started {
        log::warn!("cannot start the media session thread: {err}");
    }
    move |update| {
        let _ = tx.send(update);
    }
}

#[cfg(target_os = "macos")]
fn spawn_publisher(
    hwnd: Option<isize>,
    events: mpsc::UnboundedSender<MediaControlEvent>,
) -> impl FnMut(Update) + 'static {
    let mut controls = open_controls(hwnd, events);
    move |update| {
        if let Some(controls) = &mut controls {
            publish(controls, update);
        }
    }
}

// ---- Nối vào app ----

/// Theo `player`, báo cho `sink` phần đổi. Chạy trên UI thread.
fn follow_player(
    player: &Entity<Player>,
    covers: Option<CoverStore>,
    mut sink: impl FnMut(Update) + 'static,
    cx: &mut App,
) {
    let mut tracker = Tracker::new(Instant::now());
    for update in tracker.update(snapshot(player.read(cx), covers.as_ref()), Instant::now()) {
        sink(update);
    }
    cx.observe(player, move |player, cx| {
        for update in tracker.update(snapshot(player.read(cx), covers.as_ref()), Instant::now()) {
            sink(update);
        }
    })
    .detach();
}

/// Media session và nút taskbar cho cửa sổ chính. Gọi một lần trong `app::run`, sau `commands::install`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    #[cfg(windows)]
    let hwnd = super::taskbar_windows::hwnd_of(window, cx);
    #[cfg(not(windows))]
    let hwnd = None;
    #[cfg(windows)]
    if let Some(hwnd) = hwnd {
        super::taskbar_windows::install(hwnd, window, cx);
    }

    let (events, mut event_rx) = mpsc::unbounded();
    let sink = spawn_publisher(hwnd, events);
    cx.spawn(async move |cx| {
        while let Some(event) = event_rx.next().await {
            if let Some(action) = action_for(&event) {
                cx.update(|cx| apply(action, window, cx));
            }
        }
    })
    .detach();

    let covers = CoverStore::default_location().map_err(|err| log::warn!("cannot locate covers: {err}")).ok();
    let target = player(cx);
    follow_player(&target, covers, sink, cx);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{TestAppContext, VisualContext as _};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_library::fixtures::{self, AudioKind};
    use tempfile::TempDir;

    use super::*;
    use crate::player::{PlayerDeps, install_for_test};

    fn info(title: &str) -> TrackInfo {
        TrackInfo { title: title.into(), artist: String::new(), album: String::new(), cover: None, duration_sec: 100.0 }
    }

    fn playing(title: &str, position_sec: f64) -> Snapshot {
        Snapshot { track: Some(info(title)), playing: true, position_sec }
    }

    #[test]
    fn nothing_to_say_without_a_track() {
        let now = Instant::now();
        let mut tracker = Tracker::new(now);
        assert_eq!(tracker.update(Snapshot { track: None, playing: false, position_sec: 0.0 }, now), []);
    }

    #[test]
    fn new_track_sends_metadata_then_playback() {
        let now = Instant::now();
        let mut tracker = Tracker::new(now);
        assert_eq!(
            tracker.update(playing("a", 0.0), now),
            [Update::Metadata(Some(info("a"))), Update::Playback { state: PlaybackState::Playing, position_sec: 0.0 }]
        );
    }

    #[test]
    fn steady_playback_stays_quiet_but_a_seek_is_reported() {
        let start = Instant::now();
        let mut tracker = Tracker::new(start);
        tracker.update(playing("a", 0.0), start);
        for tick in 1..=40 {
            let now = start + Duration::from_millis(250 * tick);
            let position = 0.25 * tick as f64;
            assert_eq!(tracker.update(playing("a", position), now), [], "nhịp {tick}");
        }
        let now = start + Duration::from_secs(11);
        assert_eq!(
            tracker.update(playing("a", 70.0), now),
            [Update::Playback { state: PlaybackState::Playing, position_sec: 70.0 }]
        );
        // Sau cú tua, mốc dự đoán chạy từ chỗ mới.
        assert_eq!(tracker.update(playing("a", 71.0), now + Duration::from_secs(1)), []);
    }

    #[test]
    fn pause_resume_seek_while_paused_and_clearing() {
        let start = Instant::now();
        let mut tracker = Tracker::new(start);
        tracker.update(playing("a", 5.0), start);
        let paused = |position_sec| Snapshot { track: Some(info("a")), playing: false, position_sec };
        assert_eq!(
            tracker.update(paused(6.0), start + Duration::from_secs(1)),
            [Update::Playback { state: PlaybackState::Paused, position_sec: 6.0 }]
        );
        // Dừng lâu, vị trí không đổi: im.
        assert_eq!(tracker.update(paused(6.0), start + Duration::from_secs(60)), []);
        assert_eq!(
            tracker.update(paused(30.0), start + Duration::from_secs(61)),
            [Update::Playback { state: PlaybackState::Paused, position_sec: 30.0 }]
        );
        assert_eq!(
            tracker
                .update(Snapshot { track: None, playing: false, position_sec: 0.0 }, start + Duration::from_secs(62)),
            [Update::Metadata(None), Update::Playback { state: PlaybackState::Stopped, position_sec: 0.0 }]
        );
    }

    #[test]
    fn duration_arriving_later_republishes_metadata() {
        let now = Instant::now();
        let mut tracker = Tracker::new(now);
        tracker.update(playing("a", 0.0), now);
        let mut longer = playing("a", 0.0);
        longer.track.as_mut().unwrap().duration_sec = 200.0;
        let updates = tracker.update(longer.clone(), now);
        assert_eq!(updates[0], Update::Metadata(longer.track));
    }

    #[test]
    fn os_events_map_to_actions() {
        let cases = [
            (MediaControlEvent::Play, Some(Action::Play)),
            (MediaControlEvent::Pause, Some(Action::Pause)),
            (MediaControlEvent::Toggle, Some(Action::Run(Command::TogglePlay))),
            (MediaControlEvent::Next, Some(Action::Run(Command::NextTrack))),
            (MediaControlEvent::Previous, Some(Action::Run(Command::PrevTrack))),
            (MediaControlEvent::Stop, Some(Action::Stop)),
            (MediaControlEvent::Seek(SeekDirection::Forward), Some(Action::Run(Command::SeekForward))),
            (MediaControlEvent::Seek(SeekDirection::Backward), Some(Action::Run(Command::SeekBackward))),
            (MediaControlEvent::SeekBy(SeekDirection::Backward, Duration::from_secs(5)), Some(Action::SeekBy(-5.0))),
            (MediaControlEvent::SetPosition(MediaPosition(Duration::from_secs(9))), Some(Action::SeekTo(9.0))),
            (MediaControlEvent::Raise, Some(Action::Raise)),
            (MediaControlEvent::SetVolume(0.5), None),
            (MediaControlEvent::Quit, None),
            (MediaControlEvent::OpenUri("file:///evil".into()), None),
        ];
        for (event, action) in cases {
            assert_eq!(action_for(&event), action, "{event:?}");
        }
    }

    #[test]
    fn cover_comes_from_the_store_and_rejects_odd_hashes() {
        let store = CoverStore::new("covers");
        let mut song = Song::external("a.mp3");
        assert_eq!(cover_of(&song, Some(&store)), None);
        song.cover_hash = Some("abc".into());
        assert_eq!(cover_of(&song, Some(&store)), Some(store.path("abc", CoverSize::Large)));
        assert_eq!(cover_of(&song, None), None);
        for bad in ["", "../../secret", "a/b", "a\\b"] {
            song.cover_hash = Some(bad.into());
            assert_eq!(cover_of(&song, Some(&store)), None, "{bad}");
        }
    }

    #[test]
    fn file_url_escapes_for_mpris_and_nsurl() {
        assert_eq!(
            file_url("/home/a b/.cache/Groove X/covers/ab_512.jpg"),
            "file:///home/a%20b/.cache/Groove%20X/covers/ab_512.jpg"
        );
        assert_eq!(file_url("/tmp/nhạc"), "file:///tmp/nh%E1%BA%A1c");
    }

    #[cfg(windows)]
    #[test]
    fn windows_cover_url_stays_a_plain_path() {
        assert_eq!(cover_url(Path::new("C:/x y/a.jpg")), "file://C:\\x y\\a.jpg");
    }

    /// Luồng thật: bài vào, dừng, tua, xoá thì `sink` nhận đúng, âm thanh là `FakeOutput`, không đụng DB.
    #[gpui::test]
    fn follows_the_real_player(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let deps = PlayerDeps {
            library: None,
            output: Box::new(FakeOutput::new(48_000, 2)),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: None,
        };
        let seen = Rc::new(RefCell::new(Vec::new()));
        let target = cx.update(|cx| {
            let target = install_for_test(deps, cx);
            let sink = {
                let seen = seen.clone();
                move |update| seen.borrow_mut().push(update)
            };
            follow_player(&target, None, sink, cx);
            target
        });
        let wait = |cx: &mut TestAppContext, what: &str, done: &dyn Fn(&[Update]) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                cx.run_until_parked();
                if done(&seen.borrow()) {
                    return;
                }
                assert!(Instant::now() < deadline, "hết giờ chờ {what}: {:?}", seen.borrow());
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let last_state = |updates: &[Update]| {
            updates.iter().rev().find_map(|update| match update {
                Update::Playback { state, .. } => Some(*state),
                Update::Metadata(_) => None,
            })
        };

        let path = fixtures::write_sample(dir.path(), "a", AudioKind::Wav).unwrap();
        target.update(cx, |p, cx| p.play_paths(vec![path], cx));
        wait(cx, "metadata", &|updates| {
            updates.iter().any(|update| matches!(update, Update::Metadata(Some(track)) if !track.title.is_empty()))
        });

        target.update(cx, |p, cx| p.pause(cx));
        wait(cx, "paused", &|updates| last_state(updates) == Some(PlaybackState::Paused));

        target.update(cx, |p, cx| p.clear(cx));
        wait(cx, "cleared", &|updates| {
            updates.contains(&Update::Metadata(None)) && last_state(updates) == Some(PlaybackState::Stopped)
        });
    }

    #[gpui::test]
    fn actions_drive_the_player(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let deps = PlayerDeps {
            library: None,
            output: Box::new(FakeOutput::new(48_000, 2)),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: None,
        };
        let target = cx.update(|cx| install_for_test(deps, cx));
        let window = cx.add_empty_window().window_handle();
        let path = fixtures::write_sample(dir.path(), "a", AudioKind::Wav).unwrap();
        target.update(cx, |p, cx| p.play_paths(vec![path], cx));
        let until = |cx: &mut TestAppContext, what: &str, done: &dyn Fn(&Player) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                cx.run_until_parked();
                if target.read_with(cx, |p, _| done(p)) {
                    return;
                }
                assert!(Instant::now() < deadline, "hết giờ chờ {what}");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        until(cx, "playing", &|p| p.is_playing() && p.duration_sec() > 0.0);
        cx.update(|cx| apply(Action::Pause, window, cx));
        until(cx, "paused", &|p| !p.is_playing());
        cx.update(|cx| apply(Action::Play, window, cx));
        until(cx, "resumed", &|p| p.is_playing());
        cx.update(|cx| apply(Action::SeekTo(0.5), window, cx));
        cx.update(|cx| apply(Action::Stop, window, cx));
        until(cx, "stopped", &|p| !p.is_playing() && p.position_sec() < 0.2);
    }

    /// Không phát tiếng, không hiện cửa sổ: SMTC thật gọi từ luồng riêng trên HWND của luồng khác.
    /// `cargo test -p groove-app media_session -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "đăng ký phiên SMTC thật với OS"]
    fn smtc_accepts_a_worker_thread() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow};
        let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
        // SAFETY: lớp hệ thống, không `WS_VISIBLE`.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        let raw = hwnd as isize;
        std::thread::spawn(move || {
            let (events, _rx) = mpsc::unbounded();
            let mut controls = open_controls(Some(raw), events).expect("SMTC từ luồng riêng");
            let mut track = info("a");
            track.cover = Some(PathBuf::from("missing-cover.jpg"));
            publish(&mut controls, Update::Metadata(Some(track)));
            publish(&mut controls, Update::Playback { state: PlaybackState::Paused, position_sec: 1.0 });
            publish(&mut controls, Update::Metadata(None));
            publish(&mut controls, Update::Playback { state: PlaybackState::Stopped, position_sec: 0.0 });
            // `publish` chỉ ghi log khi lỗi: ảnh thay thế phải nạp được thật.
            let cover = cover_url(&usable_cover(None).expect("ảnh thay thế"));
            controls
                .set_metadata(MediaMetadata { title: Some("a"), cover_url: Some(&cover), ..MediaMetadata::default() })
                .expect("SMTC nhận ảnh thay thế");
        })
        .join()
        .unwrap();
        // SAFETY: cửa sổ của test.
        unsafe { DestroyWindow(hwnd) };
    }
}
