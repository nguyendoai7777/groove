//! groove-gd1_25: mở tệp từ OS (N6, S22, đặc tả 1.12, 5.6).
//!
//! Ba nguồn đổ về một chỗ: `argv` của phiên đầu, phiên sau chuyển qua `instance`, Apple Event trên macOS. Nhận hết rồi mới áp
//! sau khi player sẵn sàng, không hẹn giờ cứng; `Player::play_paths` tự chờ phiên đọc xong và thắng bài của phiên.
//!
//! Chọn nhiều tệp trong Explorer có thể bật mỗi tệp một tiến trình, nên yêu cầu về sát nhau được gộp: tệp đầu phát, tệp sau
//! vào hàng đợi, thay vì tệp nào cũng cướp bài của tệp trước.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver};
use groove_core::model::is_audio_path;
use materiant::gpui_kit::{App, Application};

use crate::{instance, player};

/// Yên lặng từng này thì coi như hết đợt mở. Thêm vào độ trễ của lần mở một tệp, focus thì không phải chờ.
const MERGE_QUIET: Duration = Duration::from_millis(200);
/// Trần số lần chờ, để đợt mở dài vẫn bắt đầu phát sau khoảng 1 giây.
const MERGE_STEPS: usize = 5;

/// Một yêu cầu đã lọc.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenRequest {
    pub paths: Vec<PathBuf>,
    /// Đưa cửa sổ lên trước.
    pub activate: bool,
    /// Mở trình phát thu nhỏ (menu dock: `groove --pip`, jump list Windows `--dock-action 0`).
    pub pip: bool,
}

/// Cờ mở trình phát thu nhỏ. `--dock-action <n>` là lệnh jump list của gpui-pre-windows (`set_dock_menu`); menu dock chỉ có
/// một mục là PiP.
pub const PIP_FLAG: &str = "--pip";
const DOCK_ACTION_FLAG: &str = "--dock-action";

/// Tiến trình được mở để bật trình phát thu nhỏ.
pub fn wants_pip(args: &[OsString]) -> bool {
    args.iter().skip(1).any(|arg| arg == PIP_FLAG || arg == DOCK_ACTION_FLAG)
}

/// Đối số không phải cờ, bỏ `argv[0]`. Cờ của `dev.rs` có giá trị đi kèm không phải tệp.
pub fn positional_args(args: &[OsString]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--display" || arg == "--keys" || arg == DOCK_ACTION_FLAG {
            iter.next();
        } else if !arg.to_str().is_some_and(|text| text.starts_with("--")) {
            out.push(PathBuf::from(arg));
        }
    }
    out
}

/// Lọc và làm tuyệt đối. Đầu vào có thể từ tiến trình khác nên không tin: chỉ đuôi âm thanh (không phân biệt hoa thường, S22),
/// tương đối thì ghép `cwd` tuyệt đối, bỏ thiết bị Windows. Không chạm đĩa: tệp đã mất do player báo lỗi cho người dùng.
pub fn resolve(paths: Vec<PathBuf>, cwd: &Path) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| is_audio_path(path))
        .filter_map(|path| {
            let joined = if path.is_absolute() {
                path
            } else if cwd.is_absolute() {
                cwd.join(path)
            } else {
                return None;
            };
            // `C:bài.mp3` ghép xong vẫn tương đối theo ổ đĩa.
            (joined.is_absolute() && is_plain_file_path(&joined)).then(|| lexically_clean(&joined))
        })
        .collect()
}

/// Không NUL, không `\\.\COM1.mp3`, `\\?\GLOBALROOT\…`: mở thiết bị có thể treo luồng decoder.
fn is_plain_file_path(path: &Path) -> bool {
    let device = path.components().next().is_some_and(|first| match first {
        Component::Prefix(prefix) => matches!(prefix.kind(), Prefix::DeviceNS(_) | Prefix::Verbatim(_)),
        _ => false,
    });
    !device && !path.as_os_str().as_encoded_bytes().contains(&0)
}

/// Bỏ `.` và `..`: cùng một tệp phải cùng một `TrackRef`, cùng dòng trong thư viện.
fn lexically_clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `file:///Users/a%20b/x.mp3` thành đường dẫn. URL khác `file`, host lạ, UTF-8 hỏng: `None`.
fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.get(..7).filter(|scheme| scheme.eq_ignore_ascii_case("file://")).map(|_| &url[7..])?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| bytes.get(i + 1..i + 3)).flatten();
        match hex.and_then(|hex| std::str::from_utf8(hex).ok()).and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
            Some(byte) => {
                decoded.push(byte);
                i += 3;
            }
            None => {
                decoded.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn resolve_urls(urls: &[String]) -> Vec<PathBuf> {
    // URL tệp luôn tuyệt đối, không cần `cwd`.
    resolve(urls.iter().filter_map(|url| file_url_to_path(url)).collect(), Path::new(""))
}

/// Thay cho `Application::run` trong `app::run`, là lời gọi duy nhất của task này ở đó. Phải bọc `run` vì `on_open_urls` đăng ký
/// **trước** `run` (đặc tả 5.6): macOS gửi tệp của lần mở đầu trước `applicationDidFinishLaunching`. `on_ready` là phần khởi tạo
/// của app; bơm yêu cầu cài sau nó nên player đã có.
pub fn run_app(application: Application, on_ready: impl FnOnce(&mut App) + 'static) {
    let (sender, receiver) = instance::take_incoming().unwrap_or_else(mpsc::unbounded);
    application.on_open_urls(move |urls| {
        let paths = resolve_urls(&urls);
        let _ = sender.unbounded_send(OpenRequest { paths, activate: true, pip: false });
    });
    let args: Vec<OsString> = std::env::args_os().collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let initial = resolve(positional_args(&args), &cwd);
    application.run(move |cx: &mut App| {
        on_ready(cx);
        install(initial, receiver, bring_to_front, cx);
    });
}

/// Đưa cửa sổ chính lên trước (không phải cửa sổ PiP). Phiên sau đã gọi `AllowSetForegroundWindow`.
fn bring_to_front(cx: &mut App) {
    crate::app::bring_main_to_front(cx);
}

/// Bơm yêu cầu vào player cho tới hết đời app. `initial` đứng đầu đợt đầu tiên, trước mọi thứ phiên khác gửi tới.
fn install(
    initial: Vec<PathBuf>,
    mut receiver: UnboundedReceiver<OpenRequest>,
    front: impl Fn(&mut App) + 'static,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let mut batch = initial;
        loop {
            if batch.is_empty() {
                let Some(request) = receiver.next().await else { return };
                if request.activate {
                    cx.update(|cx| front(cx));
                }
                if request.pip {
                    cx.update(crate::views::pip::open);
                }
                batch = request.paths;
                if batch.is_empty() {
                    continue;
                }
            }
            for _ in 0..MERGE_STEPS {
                cx.background_executor().timer(MERGE_QUIET).await;
                let mut quiet = true;
                while let Ok(request) = receiver.try_recv() {
                    quiet = false;
                    if request.activate {
                        cx.update(|cx| front(cx));
                    }
                    if request.pip {
                        cx.update(crate::views::pip::open);
                    }
                    batch.extend(request.paths);
                }
                if quiet {
                    break;
                }
            }
            let paths = std::mem::take(&mut batch);
            log::info!("open: {} paths", paths.len());
            cx.update(|cx| player::player(cx).update(cx, |player, cx| player.play_paths(paths, cx)));
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    use futures::channel::mpsc::UnboundedSender;
    use gpui::TestAppContext;
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::model::TrackRef;
    use groove_library::fixtures::{self, AudioKind};
    use materiant::gpui_kit::Entity;
    use tempfile::TempDir;

    use super::*;
    use crate::player::{Player, PlayerDeps};

    fn abs(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn positional_args_skip_exe_and_dev_flags() {
        let args = os(&["groove", "--display", "secondary", "a.mp3", "--background", "--keys", "x y", "b.flac"]);
        assert_eq!(positional_args(&args), [PathBuf::from("a.mp3"), PathBuf::from("b.flac")]);
        assert!(positional_args(&os(&["groove"])).is_empty());
    }

    /// pip — `--pip` (Linux `Actions=`) và `--dock-action 0` (jump list Windows) mở trình phát thu nhỏ, không thành tệp.
    #[test]
    fn dock_flags_open_pip_and_are_not_files() {
        assert!(wants_pip(&os(&["groove", "--pip"])));
        assert!(wants_pip(&os(&["groove.exe", "--dock-action", "0"])));
        assert!(!wants_pip(&os(&["groove", "a.mp3"])));
        assert!(!wants_pip(&os(&["--pip"])), "argv[0] không phải cờ");
        assert!(positional_args(&os(&["groove.exe", "--dock-action", "0", "a.mp3"])) == [PathBuf::from("a.mp3")]);
    }

    /// S22: đuôi không phân biệt hoa thường, đủ sáu đuôi của liên kết, đuôi khác bị bỏ.
    #[test]
    fn resolve_keeps_only_audio() {
        let names = ["BÀI.MP3", "a.flac", "a.wav", "a.m4a", "a.m4r", "a.OGG", "a.exe", "a.lnk", "mp3", "a.mp3.txt"];
        let got = resolve(names.iter().map(|n| abs(n)).collect(), Path::new(""));
        assert_eq!(got, names[..6].iter().map(|n| abs(n)).collect::<Vec<_>>());
    }

    #[test]
    fn resolve_joins_relative_with_cwd_and_cleans() {
        let cwd = abs("nhạc");
        assert_eq!(resolve(vec!["a.mp3".into()], &cwd), [cwd.join("a.mp3")]);
        assert_eq!(resolve(vec![Path::new(".").join("sub").join("..").join("a.mp3")], &cwd), [cwd.join("a.mp3")]);
        assert_eq!(resolve(vec![abs("b.mp3")], &cwd), [abs("b.mp3")]);
        // `cwd` không tuyệt đối: dữ liệu hỏng, không đoán theo thư mục của phiên chính.
        assert!(resolve(vec!["a.mp3".into()], Path::new("rel")).is_empty());
        assert!(resolve(vec!["a.mp3".into()], Path::new("")).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn resolve_refuses_devices_and_drive_relative_paths() {
        let cwd = Path::new(r"C:\nhạc");
        for bad in [r"\\.\COM1.mp3", r"\\?\GLOBALROOT\Device\x.mp3", "C:a.mp3"] {
            assert!(resolve(vec![bad.into()], cwd).is_empty(), "{bad}");
        }
        assert_eq!(resolve(vec![r"\\máy\chung\a.mp3".into()], cwd), [PathBuf::from(r"\\máy\chung\a.mp3")]);
    }

    #[test]
    fn nul_in_a_path_is_refused() {
        assert!(resolve(vec![abs("a\0b.mp3")], Path::new("")).is_empty());
    }

    #[test]
    fn file_urls_decode() {
        assert_eq!(file_url_to_path("file:///Users/a%20b/B%C3%80I.mp3"), Some(PathBuf::from("/Users/a b/BÀI.mp3")));
        assert_eq!(file_url_to_path("FILE://localhost/x.mp3"), Some(PathBuf::from("/x.mp3")));
        assert_eq!(file_url_to_path("file:///100%.mp3"), Some(PathBuf::from("/100%.mp3")));
        assert_eq!(file_url_to_path("https://x/y.mp3"), None);
        assert_eq!(file_url_to_path("file://server/y.mp3"), None);
        assert_eq!(file_url_to_path("file:///%FF.mp3"), None);
        assert_eq!(file_url_to_path("fi"), None);
    }

    #[cfg(unix)]
    #[test]
    fn urls_go_through_the_same_filter() {
        let urls = ["file:///a.mp3".to_owned(), "file:///a.exe".to_owned(), "file:///x%00.mp3".to_owned()];
        assert_eq!(resolve_urls(&urls), [PathBuf::from("/a.mp3")]);
    }

    struct Rig {
        dir: TempDir,
        fake: FakeOutput,
        player: Entity<Player>,
        sender: UnboundedSender<OpenRequest>,
        fronts: Rc<Cell<usize>>,
    }

    /// `FakeOutput`, không thư viện, không phiên: không ra loa, không chạm DB thật.
    fn rig(cx: &mut TestAppContext, initial: impl FnOnce(&Path) -> Vec<PathBuf>) -> Rig {
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let fake = FakeOutput::new(48_000, 2);
        let deps = PlayerDeps {
            library: None,
            output: Box::new(fake.clone()),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: None,
        };
        let player = cx.update(|cx| player::install_for_test(deps, cx));
        let (sender, receiver) = mpsc::unbounded();
        let fronts = Rc::new(Cell::new(0));
        let count = fronts.clone();
        let initial = initial(dir.path());
        cx.update(|cx| install(initial, receiver, move |_| count.set(count.get() + 1), cx));
        Rig { dir, fake, player, sender, fronts }
    }

    impl Rig {
        fn wav(&self, name: &str) -> PathBuf {
            fixtures::write_sample(self.dir.path(), name, AudioKind::Wav).unwrap()
        }

        fn open(&self, paths: Vec<PathBuf>) {
            self.sender.unbounded_send(OpenRequest { paths, activate: true, pip: false }).unwrap();
        }

        /// Chạy vòng sự kiện, đẩy đồng hồ giả qua cửa sổ gộp, kéo mẫu cho tới khi điều kiện đúng.
        fn until(&self, cx: &mut TestAppContext, what: &str, condition: impl Fn(&Player) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                cx.run_until_parked();
                cx.executor().advance_clock(MERGE_QUIET);
                cx.run_until_parked();
                if self.player.read_with(cx, |p, _| condition(p)) {
                    return;
                }
                assert!(Instant::now() < deadline, "timed out: {what}");
                let _ = self.fake.pull(480);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn ext(path: &Path) -> TrackRef {
        TrackRef::External(path.to_path_buf())
    }

    /// N6 + Explorer: mỗi tệp ngoài thư viện một yêu cầu, về sát nhau: tệp đầu phát, tệp sau vào hàng đợi, không tệp nào mất.
    #[gpui::test]
    fn requests_close_together_become_one_queue(cx: &mut TestAppContext) {
        let rig = rig(cx, |_| Vec::new());
        let (a, b, c) = (rig.wav("a"), rig.wav("b"), rig.wav("c"));
        rig.open(vec![a.clone()]);
        rig.open(vec![b.clone()]);
        rig.open(vec![c.clone()]);
        rig.until(cx, "a plays", |p| p.current_track() == Some(&ext(&a)) && p.is_playing());
        let queue = rig.player.read_with(cx, |p, _| p.queue().queue().to_vec());
        assert_eq!(queue, [ext(&b), ext(&c)]);
        assert_eq!(rig.fronts.get(), 3);
    }

    /// N6: tệp ngoài thứ hai mở khi tệp ngoài thứ nhất đang phát thì phát, không "không có gì xảy ra".
    #[gpui::test]
    fn second_external_open_later_takes_over(cx: &mut TestAppContext) {
        let rig = rig(cx, |_| Vec::new());
        let (a, b) = (rig.wav("a"), rig.wav("b"));
        rig.open(vec![a.clone()]);
        rig.until(cx, "a plays", |p| p.current_track() == Some(&ext(&a)) && p.is_playing());
        rig.open(vec![b.clone()]);
        rig.until(cx, "b plays", |p| p.current_track() == Some(&ext(&b)) && p.is_playing());
        assert_eq!(rig.player.read_with(cx, |p, _| p.queue().history().to_vec()), [ext(&a)]);
    }

    /// S22: `argv` của phiên đầu đứng trước yêu cầu phiên khác gửi tới trong lúc khởi động, không hẹn giờ, không focus thêm.
    #[gpui::test]
    fn initial_paths_come_first(cx: &mut TestAppContext) {
        let rig = rig(cx, |dir| vec![fixtures::write_sample(dir, "first", AudioKind::Wav).unwrap()]);
        let first = rig.dir.path().join("first.wav");
        let later = rig.wav("later");
        rig.open(vec![later.clone()]);
        rig.until(cx, "first plays", |p| p.current_track() == Some(&ext(&first)) && p.is_playing());
        assert_eq!(rig.player.read_with(cx, |p, _| p.queue().queue().to_vec()), [ext(&later)]);
    }

    /// Mở app lần hai không kèm tệp: focus, không đụng bài đang phát.
    #[gpui::test]
    fn empty_request_only_focuses(cx: &mut TestAppContext) {
        let rig = rig(cx, |_| Vec::new());
        rig.open(Vec::new());
        cx.run_until_parked();
        assert_eq!(rig.fronts.get(), 1);
        assert_eq!(rig.player.read_with(cx, |p, _| p.current_track().cloned()), None);
    }

    /// `install` thật không có cửa sổ thì `bring_to_front` không làm gì, không panic.
    #[gpui::test]
    fn bring_to_front_without_window_is_harmless(cx: &mut TestAppContext) {
        cx.update(bring_to_front);
    }
}
