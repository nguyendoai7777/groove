//! groove-gd2_05: tự cập nhật (crate `updater` của materiant, cùng hợp đồng server `cloud` với fastcast).
//!
//! - Soát 10 s sau khi mở rồi mỗi 6 giờ (hoặc tới hạn 6 giờ tính từ lần soát cuối trong sổ `<data>/update/state.json`).
//! - Có bản patch / minor ⇒ thanh trên hiện nút "Cập nhật lên v<x>" (rê: phím ⌘D / Ctrl+D), bấm hay phím ⇒ tải, soát sha256
//!   và cỡ, cài (`updater::install`), app thoát; bản mới tự bật với `--updated` ⇒ hộp "Có gì mới" ([`whats_new`]). Bản major
//!   hay bản không tự cài được (dev, `.zip` chạy thẳng, `.deb`) ⇒ nút mở trang tải.
//! - Settings → Giới thiệu: phiên bản + trạng thái + Kiểm tra / Cập nhật; menu macOS "Kiểm tra cập nhật…" (`app_menu`).
//!
//! Server: `https://cloud.groovex.sandtechs.com` (D-27); bản debug đọc `GROOVE_CLOUD_URL` (server local / server giả
//! `materiant/updater/tools/fake-server.mjs`), tin khoá dev và url `http://`. UI thread không chạm đĩa (N7): sổ, dọn tệp, tải,
//! cài đều ở `background_executor`.

pub mod whats_new;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use groove_core::{paths, t};
use materiant::gpui_kit::{App, Global};
use updater::check::{self, Outcome};
use updater::install::{self, Layout};
use updater::{AppSpec, Manifest};

/// Khoá công khai cập nhật prod của groove (`cloud/secrets/groove/update-prod-key.json`, sinh 13/11/2026).
const PROD_UPDATE_KEY: [u8; 32] = [
    0x9c, 0x90, 0xb6, 0x29, 0x67, 0xb2, 0x2b, 0xd1, 0x21, 0xd5, 0xf2, 0x6a, 0x0e, 0xae, 0x95, 0x28, //
    0x74, 0x72, 0xf5, 0x7a, 0xdb, 0xe0, 0xc6, 0xe3, 0xd9, 0xaf, 0x50, 0x57, 0xc5, 0x21, 0xb7, 0xf3,
];

pub const SPEC: AppSpec = AppSpec {
    id: "groove",
    version: env!("CARGO_PKG_VERSION"),
    prod_key: PROD_UPDATE_KEY,
    dev_build: cfg!(debug_assertions),
    plan: "free",
    mac_bundle: "Groovex.app",
    exe: "groove",
    linux_dir: "groove",
    download_page: "https://groovex.sandtechs.com",
};

const PROD_SERVER: &str = "https://cloud.groovex.sandtechs.com";
const SERVER_VAR: &str = "GROOVE_CLOUD_URL";
const PROGRESS_TICK: Duration = Duration::from_millis(150);

fn server_url() -> String {
    if cfg!(debug_assertions)
        && let Some(url) = std::env::var(SERVER_VAR).ok().filter(|v| !v.is_empty())
    {
        return url;
    }
    PROD_SERVER.to_owned()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available(Arc<Manifest>),
    Downloading {
        manifest: Arc<Manifest>,
        fraction: f32,
    },
    /// Đã cài / giao cho bộ cài: app đang thoát.
    Installing(Arc<Manifest>),
    /// Hỏi / tải / cài lỗi (log có chi tiết). Còn manifest ⇒ bấm lại thử lại.
    Failed(Option<Arc<Manifest>>),
}

struct Updater {
    status: Status,
    /// `<data>/update`; `None` khi không có thư mục dữ liệu (test) ⇒ không ghi sổ, không cài.
    dir: Option<PathBuf>,
}

impl Global for Updater {}

/// Thứ nút trên thanh trên cần vẽ.
#[derive(Debug, Clone, PartialEq)]
pub struct Badge {
    pub version: String,
    /// `Some` khi đang tải / cài (0..=1).
    pub progress: Option<f32>,
    pub installing: bool,
    /// Major / không tự cài được ⇒ bấm mở trang tải.
    pub opens_page: bool,
}

/// `app::run`: đọc sổ (luồng nền), lên lịch soát; bản vừa cập nhật ⇒ hộp "Có gì mới".
pub fn install(window: materiant::gpui_kit::AnyWindowHandle, cx: &mut App) {
    let dir = paths::data_dir().ok().map(|d| d.join("update"));
    cx.set_global(Updater { status: Status::Idle, dir: dir.clone() });
    whats_new::install(window, cx);
    let (Some(dir), Some(_)) = (dir, SPEC.target()) else { return };
    let startup = cx.background_executor().spawn(async move { check::startup(&SPEC, &dir) });
    let executor = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let startup = startup.await;
        if let Some(m) = startup.pending {
            log::info!("update: {} pending from the last check", m.version);
            cx.update(|cx| set_status(Status::Available(Arc::new(m)), cx));
        }
        executor.timer(startup.first_check).await;
        loop {
            cx.update(|cx| check_now(false, cx));
            executor.timer(check::CHECK_EVERY).await;
        }
    })
    .detach();
}

pub fn status(cx: &App) -> Status {
    cx.try_global::<Updater>().map_or(Status::Idle, |u| u.status.clone())
}

fn set_status(status: Status, cx: &mut App) {
    if cx.has_global::<Updater>() {
        cx.global_mut::<Updater>().status = status;
    }
    // Thanh trên nằm trong phần `cached` của khung: báo khung + làm mới cửa sổ (trang Giới thiệu đọc lại).
    if let Some(shell) = crate::app::try_shell(cx) {
        shell.update(cx, |_, cx| cx.notify());
    }
    cx.refresh_windows();
}

fn installable() -> bool {
    Layout::detect(&SPEC, None).is_some()
}

pub fn badge(cx: &App) -> Option<Badge> {
    let (manifest, progress, installing) = match status(cx) {
        Status::Available(m) | Status::Failed(Some(m)) => (m, None, false),
        Status::Downloading { manifest, fraction } => (manifest, Some(fraction), false),
        Status::Installing(m) => (m, Some(1.0), true),
        _ => return None,
    };
    Some(Badge {
        version: manifest.version.clone(),
        progress,
        installing,
        opens_page: !manifest.kind.installs_in_app() || !installable(),
    })
}

/// Có bản để bấm cài / mở trang (phím ⌘D / Ctrl+D chỉ ăn lúc này).
pub fn can_install(cx: &App) -> bool {
    matches!(status(cx), Status::Available(_) | Status::Failed(Some(_)))
}

/// Hỏi server ngay. `manual` (Giới thiệu, menu macOS): lỗi thì báo người dùng.
pub fn check_now(manual: bool, cx: &mut App) {
    let Some(u) = cx.try_global::<Updater>() else { return };
    if matches!(u.status, Status::Checking | Status::Downloading { .. } | Status::Installing(_)) {
        return;
    }
    let (before, dir) = (u.status.clone(), u.dir.clone());
    set_status(Status::Checking, cx);
    let server = server_url();
    let task = cx.background_executor().spawn(async move { check::check(&SPEC, &server, dir.as_deref()) });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let status = match result {
                Ok(Outcome::UpToDate) => {
                    log::info!("update: up to date ({})", SPEC.version);
                    Status::UpToDate
                }
                Ok(Outcome::Available(m)) => {
                    log::info!("update: {} available ({:?})", m.version, m.kind);
                    Status::Available(Arc::new(*m))
                }
                Err(e) => {
                    log::warn!("update: check failed: {e}");
                    if manual {
                        toast_error(t!("update.check_failed"), cx);
                        Status::Failed(None)
                    } else {
                        // Tự soát lỗi (mạng…) ⇒ giữ trạng thái cũ, thử lại lần sau.
                        before
                    }
                }
            };
            set_status(status, cx);
        });
    })
    .detach();
}

/// Nút Update / ⌘D / nút Cập nhật ở Giới thiệu: tải + cài rồi thoát, hoặc mở trang tải.
pub fn start_install(cx: &mut App) {
    let Some(u) = cx.try_global::<Updater>() else { return };
    let manifest = match &u.status {
        Status::Available(m) | Status::Failed(Some(m)) => m.clone(),
        _ => return,
    };
    let layout = Layout::detect(&SPEC, None);
    let (Some(dir), true, Some(layout)) = (u.dir.clone(), manifest.kind.installs_in_app(), layout.clone()) else {
        log::info!(
            "update: {} — open the download page (kind {:?}, self-install: {})",
            manifest.version,
            manifest.kind,
            layout.is_some()
        );
        cx.open_url(SPEC.download_page);
        return;
    };
    set_status(Status::Downloading { manifest: manifest.clone(), fraction: 0.0 }, cx);
    let got = Arc::new(AtomicU64::new(0));
    let job = {
        let (manifest, got) = (manifest.clone(), got.clone());
        cx.background_executor().spawn(async move {
            let file = install::download(&manifest, &dir, |n| got.store(n, Ordering::Relaxed))?;
            updater::notes::save_pending(&dir, &manifest.notes);
            install::apply(&SPEC, &layout, &file, &dir, std::process::id())
        })
    };
    let executor = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let mut job = std::pin::pin!(job);
        let result = loop {
            match updater::job_or_tick(&mut job, executor.timer(PROGRESS_TICK)).await {
                Some(result) => break result,
                None => {
                    let fraction = (got.load(Ordering::Relaxed) as f32 / manifest.size.max(1) as f32).min(1.0);
                    let m = manifest.clone();
                    cx.update(|cx| set_status(Status::Downloading { manifest: m, fraction }, cx));
                }
            }
        };
        cx.update(|cx| match result {
            Ok(()) => {
                log::info!("update: {} installed, quitting so it can start", manifest.version);
                set_status(Status::Installing(manifest.clone()), cx);
                // Bản mới chờ tiến trình này thoát (macOS / Linux) hay bộ cài chờ exe hết khoá (Windows).
                cx.quit();
            }
            Err(e) => {
                log::error!("update: install {} failed: {e}", manifest.version);
                set_status(Status::Failed(Some(manifest.clone())), cx);
                toast_error(t!("update.failed"), cx);
            }
        });
    })
    .detach();
}

fn toast_error(text: &'static str, cx: &mut App) {
    let spec = materiant::toast::ToastSpec::new(materiant::toast::ToastKind::Danger, text);
    crate::library_ctl::with_window(cx, move |window, cx| {
        materiant::toast::Toast::push(window, cx, spec);
    });
}

/// Chữ phím của lệnh cập nhật.
pub fn shortcut_label() -> &'static str {
    if cfg!(target_os = "macos") { "⌘D" } else { "Ctrl+D" }
}

/// Phím cài bản mới: ⌘D (macOS) / Ctrl+D, không Shift / Alt.
pub fn is_install_key(keystroke: &materiant::gpui_kit::Keystroke) -> bool {
    let m = &keystroke.modifiers;
    let primary = if cfg!(target_os = "macos") { m.platform && !m.control } else { m.control && !m.platform };
    keystroke.key == "d" && primary && !m.alt && !m.shift
}

#[cfg(test)]
pub fn set_for_test(status: Status, cx: &mut App) {
    cx.set_global(Updater { status, dir: None });
    cx.refresh_windows();
}

#[cfg(test)]
pub fn test_manifest(version: &str) -> Manifest {
    let target = updater::Target { os: "macos", arch: "arm64", plan: "free" };
    updater::testing::manifest(&SPEC, target, SPEC.version, version, updater::Kind::Minor)
}

#[cfg(test)]
mod tests {
    use materiant::gpui_kit::Keystroke;

    use super::*;

    #[test]
    fn install_key_is_cmd_or_ctrl_d() {
        let key = |s: &str| Keystroke::parse(Box::leak(s.to_owned().into_boxed_str())).unwrap();
        let primary = if cfg!(target_os = "macos") { "cmd-d" } else { "ctrl-d" };
        assert!(is_install_key(&key(primary)));
        assert!(!is_install_key(&key("d")));
        assert!(!is_install_key(&key(&format!("shift-{primary}"))));
        assert!(!is_install_key(&key(if cfg!(target_os = "macos") { "cmd-e" } else { "ctrl-e" })));
    }

    #[test]
    fn spec_matches_the_build() {
        assert!(updater::Version::parse(SPEC.version).is_some());
        assert_ne!(SPEC.prod_key, updater::DEV_KEY);
    }
}
