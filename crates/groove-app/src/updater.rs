//! linux-blur-update (13/11/2026): kiểm bản mới, nút Cập nhật ở thanh trên.
//!
//! Mỗi bản build ghi kèm bộ cài một tệp `groovex-latest-<nền>.json` ([`Manifest`]: phiên bản, tên tệp bộ cài, cỡ, sha256 —
//! `packaging/*` sinh ra), trang tải đặt cả hai ở [`MANIFEST_BASE`]. App hỏi tệp của đúng nền mình ([`platform_key`]) ở luồng
//! riêng, [`FIRST_CHECK`] sau khi mở rồi mỗi [`CHECK_EVERY`]; bản trên trang mới hơn bản đang chạy thì thanh trên hiện nút
//! Cập nhật ([`button`]). Mỗi nền một tệp riêng: bộ cài của nền nào chưa lên trang thì nền đó chưa thấy nút.
//!
//! Bấm: tải bộ cài vào `<cache>/updates/` (luồng riêng, soát cỡ + sha256, tệp dở `.part` bị xoá khi lỗi) rồi giao cho OS:
//! - Windows: chạy bộ cài NSIS rồi thoát (bộ cài thay exe đang chạy). Bản chạy thẳng (`_portable.zip`, không có
//!   `uninstall.exe` cạnh exe) không tải gì, mở trang tải.
//! - macOS: `open` tệp `.dmg` (Finder gắn ổ, người dùng kéo vào Applications như lần cài đầu).
//! - Linux: exe là `/usr/bin/groove` của gói `.deb` ⇒ `pkexec apt-get install` (hộp mật khẩu của polkit), xong thì nút thành
//!   Khởi động lại; không có `pkexec` / lỗi ⇒ `xdg-open` tệp `.deb` (App Center). Exe nằm chỗ khác (bản dev) ⇒ mở trang tải.
//!
//! Bản debug chỉ kiểm khi có `GROOVE_UPDATE_URL` (địa chỉ tệp manifest; đường dẫn tệp trên đĩa cũng được, `file` trong đó
//! tính từ thư mục của manifest) — chạy thử nút mà không cần trang thật.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use groove_core::{paths, t};
use materiant::gpui_kit::{App, AppContext, Entity, Global, SharedString, Window};
use materiant::toast::{Toast, ToastKind, ToastSpec};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Thư mục trên trang tải chứa bộ cài + manifest của bản mới nhất.
pub const MANIFEST_BASE: &str = "https://groovex.sandtechs.com/download/";
/// Trang tải, khi app không tự cài được (bản chạy thẳng, bản dev).
pub const DOWNLOAD_PAGE: &str = "https://groovex.sandtechs.com/#download";
/// Lần hỏi đầu: sau khi app mở xong, không tranh mạng / đĩa với lúc nạp thư viện.
const FIRST_CHECK: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const TIMEOUT: Duration = Duration::from_secs(20);
/// Tải bộ cài: mạng chậm vẫn kịp (~60 MB).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Bộ cài lớn hơn thế này là manifest hỏng.
const MAX_SIZE: u64 = 512 * 1024 * 1024;
const UPDATE_URL_VAR: &str = "GROOVE_UPDATE_URL";

/// `groovex-latest-<nền>.json`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    /// Tên tệp bộ cài, cạnh manifest (hoặc địa chỉ đầy đủ).
    pub file: String,
    pub size: u64,
    /// Hex chữ thường.
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Available(Manifest),
    /// `received` / `manifest.size` byte.
    Downloading {
        manifest: Manifest,
        received: u64,
    },
    Installing(Manifest),
    /// Linux: gói mới đã cài, chờ khởi động lại.
    Installed(Manifest),
}

pub struct Updater {
    state: UpdateState,
    /// Địa chỉ manifest đang dùng (tính `file` tương đối).
    manifest_url: String,
}

struct UpdaterHandle(Entity<Updater>);

impl Global for UpdaterHandle {}

/// Khoá nền: tên tệp trên trang tải là `groovex-latest-<khoá>.<đuôi>`. Nền chưa phát hành ⇒ `None`.
pub fn platform_key() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("win-x64-setup")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("macos-x64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-amd64")
    } else {
        None
    }
}

fn manifest_url() -> Option<String> {
    if cfg!(debug_assertions) {
        // Bản debug (start.sh) không hỏi trang thật: phiên bản dev luôn cũ hơn bản phát hành.
        return std::env::var(UPDATE_URL_VAR).ok().filter(|v| !v.is_empty());
    }
    platform_key().map(|key| format!("{MANIFEST_BASE}groovex-latest-{key}.json"))
}

/// `remote` mới hơn `current` (semver; đọc không được ⇒ không).
pub fn is_newer(current: &str, remote: &str) -> bool {
    match (semver::Version::parse(current), semver::Version::parse(remote.trim().trim_start_matches('v'))) {
        (Ok(current), Ok(remote)) => remote > current,
        _ => false,
    }
}

/// Địa chỉ bộ cài: `file` đầy đủ thì giữ, không thì cạnh manifest.
fn resolve(manifest_url: &str, file: &str) -> String {
    if file.contains("://") {
        return file.to_owned();
    }
    match manifest_url.rfind(['/', '\\']) {
        Some(i) => format!("{}{file}", &manifest_url[..=i]),
        None => file.to_owned(),
    }
}

/// Tên tệp an toàn để ghi vào thư mục tải (không thoát khỏi thư mục).
fn local_name(file: &str) -> String {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
    if clean.trim_matches('.').is_empty() { "groovex-update".into() } else { clean }
}

fn agent(timeout: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .user_agent(concat!("Groovex/", env!("CARGO_PKG_VERSION")))
        .tls_config(tls)
        .build()
        .into()
}

/// Mở `url` để đọc: HTTP(S), hoặc (chỉ bản debug) đường dẫn trên đĩa.
fn open(url: &str, timeout: Duration) -> io::Result<Box<dyn Read + Send>> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let response = agent(timeout).get(url).call().map_err(io::Error::other)?;
        return Ok(Box::new(response.into_body().into_reader()));
    }
    if cfg!(debug_assertions) {
        return Ok(Box::new(std::fs::File::open(url.trim_start_matches("file://"))?));
    }
    Err(io::Error::other("not an http(s) url"))
}

fn fetch_manifest(url: &str) -> io::Result<Manifest> {
    let mut body = String::new();
    open(url, TIMEOUT)?.take(64 * 1024).read_to_string(&mut body)?;
    let manifest: Manifest = serde_json::from_str(&body).map_err(io::Error::other)?;
    if manifest.size == 0 || manifest.size > MAX_SIZE || manifest.sha256.len() != 64 {
        return Err(io::Error::other("bad manifest"));
    }
    Ok(manifest)
}

/// Tải về `<dir>/<tên>`, soát cỡ + sha256. `progress` nhận số byte đã nhận.
fn download(url: &str, manifest: &Manifest, dir: &Path, progress: impl Fn(u64)) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let target = dir.join(local_name(&manifest.file));
    let part = target.with_extension("part");
    let result = (|| {
        let mut reader = open(url, DOWNLOAD_TIMEOUT)?.take(manifest.size + 1);
        let mut out = std::fs::File::create(&part)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut received = 0u64;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            received += n as u64;
            progress(received);
        }
        out.sync_all()?;
        if received != manifest.size {
            return Err(io::Error::other(format!("size {received} != {}", manifest.size)));
        }
        let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !digest.eq_ignore_ascii_case(&manifest.sha256) {
            return Err(io::Error::other("sha256 mismatch"));
        }
        std::fs::rename(&part, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| target)
}

/// App tự cài được bản mới ở chỗ đang chạy không (không thì mở trang tải).
fn can_install() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    if cfg!(windows) {
        exe.parent().is_some_and(|dir| dir.join("uninstall.exe").exists())
    } else if cfg!(target_os = "linux") {
        exe == Path::new("/usr/bin/groove")
    } else {
        true
    }
}

/// Bộ cài đã giao cho OS, app làm gì tiếp.
enum Handoff {
    /// Windows: bộ cài đang chạy, thoát để nó thay exe.
    #[cfg_attr(not(windows), allow(dead_code))]
    Quit,
    /// Linux: gói đã cài, chờ người dùng khởi động lại.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Installed,
    /// macOS / Linux dự phòng: OS mở tệp, người dùng tự làm nốt.
    #[cfg_attr(windows, allow(dead_code))]
    Opened,
}

/// Chặn (Linux chờ `pkexec`): gọi ở luồng riêng.
fn hand_off(installer: &Path) -> io::Result<Handoff> {
    #[cfg(windows)]
    {
        std::process::Command::new(installer).spawn()?;
        Ok(Handoff::Quit)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(installer).status()?;
        Ok(Handoff::Opened)
    }
    #[cfg(target_os = "linux")]
    {
        // `pkexec` trả 126 / 127 khi người dùng huỷ hộp mật khẩu / không được phép: báo huỷ, không mở App Center.
        match std::process::Command::new("pkexec")
            .args(["apt-get", "install", "-y", "--allow-downgrades"])
            .arg(installer)
            .status()
        {
            Ok(status) if status.success() => Ok(Handoff::Installed),
            Ok(status) if matches!(status.code(), Some(126 | 127)) => Err(io::Error::other("cancelled")),
            result => {
                log::warn!("updater: pkexec apt-get install failed: {result:?}, opening the package");
                std::process::Command::new("xdg-open").arg(installer).spawn()?;
                Ok(Handoff::Opened)
            }
        }
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = installer;
        Err(io::Error::other("unsupported"))
    }
}

/// Chạy lại app sau khi tiến trình này thoát (khoá một phiên còn giữ tới lúc thoát), rồi thoát.
fn restart(cx: &mut App) {
    #[cfg(unix)]
    if let Ok(exe) = std::env::current_exe() {
        let script = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec \"$0\"";
        let spawned =
            std::process::Command::new("sh").args(["-c", script]).arg(&exe).arg(std::process::id().to_string()).spawn();
        if let Err(e) = spawned {
            log::warn!("updater: cannot relaunch: {e}");
        }
    }
    cx.quit();
}

pub fn install(cx: &mut App) {
    let Some(url) = manifest_url() else { return };
    let updater = cx.new(|_| Updater { state: UpdateState::Idle, manifest_url: url.clone() });
    cx.set_global(UpdaterHandle(updater.clone()));
    // Thanh trên (`Island<Shell>`) chỉ dựng lại khi Shell `notify`.
    let shell = crate::app::shell(cx);
    shell.update(cx, |_, cx| cx.observe(&updater, |_, _, cx| cx.notify()).detach());

    cx.spawn(async move |cx| {
        cx.background_executor().timer(FIRST_CHECK).await;
        loop {
            let (tx, rx) = oneshot::channel();
            let url = url.clone();
            std::thread::spawn(move || {
                let _ = tx.send(fetch_manifest(&url));
            });
            match rx.await {
                Ok(Ok(manifest)) if is_newer(env!("CARGO_PKG_VERSION"), &manifest.version) => {
                    updater.update(cx, |this, cx| {
                        // Đang tải / cài / chờ khởi động lại thì giữ nguyên.
                        if matches!(this.state, UpdateState::Idle | UpdateState::Available(_)) {
                            if this.state != UpdateState::Available(manifest.clone()) {
                                log::info!("updater: version {} is available", manifest.version);
                            }
                            this.state = UpdateState::Available(manifest);
                            cx.notify();
                        }
                    });
                }
                Ok(Ok(_)) => log::debug!("updater: up to date"),
                Ok(Err(e)) => log::info!("updater: check failed: {e}"),
                Err(_) => {}
            }
            cx.background_executor().timer(CHECK_EVERY).await;
        }
    })
    .detach();
}

/// Nút trên thanh trên: chữ, chú thích, đang bận (khoá bấm).
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub label: SharedString,
    pub tooltip: SharedString,
    pub busy: bool,
}

/// `None` ⇒ không có nút.
pub fn button(cx: &App) -> Option<Button> {
    let state = &cx.try_global::<UpdaterHandle>()?.0.read(cx).state;
    Some(match state {
        UpdateState::Idle => return None,
        UpdateState::Available(m) => Button {
            label: t!("shell.update.button").into(),
            tooltip: if can_install() {
                t!("shell.update.available", version = m.version.as_str())
            } else {
                t!("shell.update.open_page", version = m.version.as_str())
            }
            .into(),
            busy: false,
        },
        UpdateState::Downloading { manifest, received } => Button {
            label: format!("{}%", (received * 100 / manifest.size.max(1)).min(100)).into(),
            tooltip: t!("shell.update.downloading", version = manifest.version.as_str()).into(),
            busy: true,
        },
        UpdateState::Installing(m) => Button {
            label: t!("shell.update.installing").into(),
            tooltip: t!("shell.update.downloading", version = m.version.as_str()).into(),
            busy: true,
        },
        UpdateState::Installed(m) => Button {
            label: t!("shell.update.restart").into(),
            tooltip: t!("shell.update.restart_tooltip", version = m.version.as_str()).into(),
            busy: false,
        },
    })
}

/// Bấm nút.
pub fn click(window: &mut Window, cx: &mut App) {
    let Some(updater) = cx.try_global::<UpdaterHandle>().map(|h| h.0.clone()) else { return };
    let (state, manifest_url) = {
        let this = updater.read(cx);
        (this.state.clone(), this.manifest_url.clone())
    };
    match state {
        UpdateState::Available(_) if !can_install() => {
            log::info!("updater: cannot install in place, opening the download page");
            cx.open_url(DOWNLOAD_PAGE);
        }
        UpdateState::Available(manifest) => start(updater, manifest, &manifest_url, window, cx),
        UpdateState::Installed(_) => restart(cx),
        _ => {}
    }
}

enum Event {
    Received(u64),
    Downloaded(io::Result<PathBuf>),
    HandedOff(io::Result<Handoff>),
}

fn start(updater: Entity<Updater>, manifest: Manifest, manifest_url: &str, window: &mut Window, cx: &mut App) {
    let dir = match paths::cache_dir() {
        Ok(dir) => dir.join("updates"),
        Err(e) => return fail(window, cx, &e),
    };
    let url = resolve(manifest_url, &manifest.file);
    set(&updater, UpdateState::Downloading { manifest: manifest.clone(), received: 0 }, cx);
    log::info!("updater: downloading version {}", manifest.version);

    let (tx, mut rx) = mpsc::unbounded();
    let job = manifest.clone();
    std::thread::spawn(move || {
        let step = (job.size / 100).max(1);
        let last = std::cell::Cell::new(0u64);
        let downloaded = download(&url, &job, &dir, |received| {
            // Mỗi 1 % một lần: thanh trên không dựng lại theo từng khối 64 KB.
            if received - last.get() >= step {
                last.set(received);
                let _ = tx.unbounded_send(Event::Received(received));
            }
        });
        let installer = match downloaded {
            Ok(path) => {
                let _ = tx.unbounded_send(Event::Downloaded(Ok(path.clone())));
                path
            }
            Err(e) => {
                let _ = tx.unbounded_send(Event::Downloaded(Err(e)));
                return;
            }
        };
        let _ = tx.unbounded_send(Event::HandedOff(hand_off(&installer)));
    });

    let window = window.window_handle();
    cx.spawn(async move |cx| {
        while let Some(event) = rx.next().await {
            let next = match event {
                Event::Received(received) => UpdateState::Downloading { manifest: manifest.clone(), received },
                Event::Downloaded(Ok(_)) => UpdateState::Installing(manifest.clone()),
                Event::HandedOff(Ok(Handoff::Quit)) => {
                    log::info!("updater: installer started, quitting");
                    cx.update(|cx| cx.quit());
                    return;
                }
                Event::HandedOff(Ok(Handoff::Installed)) => {
                    log::info!("updater: version {} installed", manifest.version);
                    UpdateState::Installed(manifest.clone())
                }
                Event::HandedOff(Ok(Handoff::Opened)) => UpdateState::Available(manifest.clone()),
                Event::Downloaded(Err(e)) | Event::HandedOff(Err(e)) => {
                    let _ = window.update(cx, |_, window, cx| fail(window, cx, &e));
                    UpdateState::Available(manifest.clone())
                }
            };
            cx.update(|cx| set(&updater, next, cx));
        }
    })
    .detach();
}

fn set(updater: &Entity<Updater>, state: UpdateState, cx: &mut App) {
    updater.update(cx, |this, cx| {
        if this.state != state {
            this.state = state;
            cx.notify();
        }
    });
}

fn fail(window: &mut Window, cx: &mut App, error: &io::Error) {
    log::warn!("updater: {error}");
    if error.to_string() == "cancelled" {
        return;
    }
    let message = t!("shell.update.failed", error = error.to_string());
    Toast::push(window, cx, ToastSpec::new(ToastKind::Danger, message));
}

#[cfg(test)]
mod tests {
    use super::*;
    use groove_core::test_dir::TestDir;

    #[test]
    fn newer_versions_compare_by_semver() {
        assert!(is_newer("0.2.0", "0.2.1"));
        assert!(is_newer("0.2.0", "v0.10.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.2.0", "rác"));
    }

    #[test]
    fn installer_sits_next_to_the_manifest() {
        let base = "https://groovex.sandtechs.com/download/groovex-latest-linux-amd64.json";
        assert_eq!(
            resolve(base, "groovex-latest-linux-amd64.deb"),
            "https://groovex.sandtechs.com/download/groovex-latest-linux-amd64.deb"
        );
        assert_eq!(resolve(base, "https://cdn.example/x.deb"), "https://cdn.example/x.deb");
        assert_eq!(resolve("/tmp/u/m.json", "a.deb"), "/tmp/u/a.deb");
    }

    #[test]
    fn local_name_stays_inside_the_folder() {
        assert_eq!(local_name("../../etc/passwd"), "passwd");
        assert_eq!(local_name("a/b\\c d.exe"), "cd.exe");
        assert_eq!(local_name(".."), "groovex-update");
    }

    #[test]
    fn download_checks_size_and_sha256() {
        let dir = TestDir::new();
        let src = dir.path().join("src.bin");
        std::fs::write(&src, b"groovex").unwrap();
        let sha = "c6f3b7b6a1a1b9f5d1b3c9d0b7e9a7e1c1d6f4b0c1f7a5f4f8c3b1c9e3a2f0d1";
        let good = Manifest {
            version: "9.9.9".into(),
            file: "src.bin".into(),
            size: 7,
            sha256: Sha256::digest(b"groovex").iter().map(|b| format!("{b:02x}")).collect(),
        };
        let out = dir.path().join("out");
        let url = src.to_string_lossy().into_owned();
        let path = download(&url, &good, &out, |_| {}).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"groovex");

        let bad_sha = Manifest { sha256: sha.into(), ..good.clone() };
        assert!(download(&url, &bad_sha, &out, |_| {}).is_err());
        let bad_size = Manifest { size: 6, ..good };
        assert!(download(&url, &bad_size, &out, |_| {}).is_err());
        assert!(!out.join("src.part").exists(), "tệp dở bị xoá");
    }

    #[test]
    fn manifest_reads_camel_case() {
        let dir = TestDir::new();
        let path = dir.path().join("m.json");
        let sha = "a".repeat(64);
        std::fs::write(&path, format!(r#"{{"version":"1.2.3","file":"x.deb","size":10,"sha256":"{sha}"}}"#)).unwrap();
        let manifest = fetch_manifest(&path.to_string_lossy()).unwrap();
        assert_eq!(manifest.version, "1.2.3");
        std::fs::write(&path, r#"{"version":"1.2.3","file":"x.deb","size":0,"sha256":"aa"}"#).unwrap();
        assert!(fetch_manifest(&path.to_string_lossy()).is_err());
    }
}
