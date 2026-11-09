//! linux-chrome (03/11/2026): nền cột trái theo việc OS có làm mờ phía sau cửa sổ được hay không.
//!
//! Windows (acrylic), macOS (`NSVisualEffectView`) luôn mờ ⇒ cột trái nền alpha 0,88 (`translucent_surface`).
//! gpui-pre-linux 0.3.10 chỉ xin mờ qua `org_kde_kwin_blur` trên Wayland (KWin có; GNOME / Mutter — GNOME 50 không quảng bá
//! giao thức mờ nào —, wlroots không có), X11 không xin gì: ở đó `Blurred` chỉ là trong suốt trơn, chữ cửa sổ phía sau lộ
//! rõ. Không có mờ thì cột trái đục hơn: alpha 0,96 tông tối, đục tông sáng (nền trong suốt app tự chọn — D-10).
//!
//! gnome-blur (09/11/2026): GNOME có extension Shell của Groovex ([`gnome`]) thì phía sau cửa sổ chính mờ thật ⇒ cột trái
//! alpha 0,72 cả hai tông, như launcher của fastcast.
//!
//! drag-lag (03/11/2026): [`smooth_blur`] — `Blurred` của gpui-pre-windows 0.3.10 là acrylic
//! (`ACCENT_ENABLE_ACRYLICBLURBEHIND`), mà acrylic trên Windows 10 1903+ và Windows 11 trước 22H2 (build < 22621) giật khi kéo /
//! đổi cỡ cửa sổ (DWM vẽ lại lớp acrylic mỗi lần dời; window-vibrancy ghi rõ "bad performance when resizing/dragging"). Ở các
//! bản đó cửa sổ chính đổi sang blur-behind kiểu Aero (`ACCENT_ENABLE_BLURBEHIND`) như bản Tauri (`window_vibrancy::apply_blur`):
//! vẫn mờ, không hạt nhiễu acrylic, kéo mượt. Windows 11 22H2+ giữ acrylic.
//!
//! blur-region (03/11/2026): blur-behind không tự giới hạn trong cửa sổ — trên Windows 10 thật lớp mờ tràn ra một vùng cỡ màn
//! bên phải / dưới cửa sổ, và phủ viền đổi cỡ vô hình 8 px (trái, phải, dưới) thành vệt xám quanh khung. Nay vùng mờ đặt đúng
//! bằng vùng client (`DwmEnableBlurBehindWindow` + `hRgnBlur`), đặt lại mỗi `WM_SIZE` (đổi cỡ, phóng to / khôi phục), rồi đặt
//! lại accent. Bóng DWM quanh khung (vẽ trong viền vô hình) tắt bằng `DWMWA_NCRENDERING_POLICY = DISABLED`.
//!
//! blur-frame (04/11/2026): accent blur-behind phủ cả khung cửa sổ, `hRgnBlur` không cắt được — viền đổi cỡ vô hình 8 px ở
//! dưới / trái / phải vẫn thành một dải mờ quanh cửa sổ. Nay khi không phóng to, `WM_NCCALCSIZE` cho vùng client bằng cả khung
//! (không còn viền vô hình), và subclass tự hit-test [`RESIZE_EDGE`] px mép trong trái / phải / dưới thành `HTLEFT`… để vẫn kéo
//! mép đổi cỡ (mép trên GPUI đã tự lo). Phóng to giữ cách tính của GPUI (thụt viền để không tràn màn). Lúc cài, cửa sổ co lại
//! đúng bằng vùng client cũ để cỡ đã lưu không lớn dần mỗi lần mở. Log `backdrop: blur-behind …` ghi lúc cài để đối chiếu.
//!
//! Kiểm tay trên màn thật: `GROOVE_BLUR=acrylic` giữ acrylic của GPUI (so độ mượt khi kéo), `GROOVE_WINDOW_SHADOW=on` giữ bóng.

use materiant::gpui_kit::{AnyWindowHandle, App, Hsla};
use materiant::theme::ActiveTheme;
use materiant::window::TRANSLUCENT_SURFACE_ALPHA;

/// Ai làm mờ phía sau cửa sổ chính.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blur {
    /// Compositor theo `Blurred` của GPUI: acrylic / blur-behind (Windows), `NSVisualEffectView` (macOS), KWin.
    Compositor,
    /// Extension GNOME Shell của Groovex ([`gnome`]): bán kính 40, mờ hơn hẳn acrylic.
    Shell,
    /// Chỉ trong suốt trơn: GNOME chưa nạp extension / bật tương phản cao, wlroots, X11.
    None,
}

/// Alpha cột trái khi extension GNOME Shell làm mờ, cả hai tông — như launcher của fastcast (`BLURRED_SURFACE_ALPHA`): kiểu
/// Raycast, nhìn ra mờ; thấp hơn ~0,65 thì nền sáng phía sau làm chữ phụ tông tối khó đọc.
pub const SHELL_BLUR_ALPHA: f32 = 0.72;

/// Alpha cột trái tông tối khi không có mờ: 0,94 cũ vẫn lộ chữ cửa sổ dưới (ảnh chủ dự án, 09/11/2026). Tông sáng đục.
pub const UNBLURRED_ALPHA: f32 = 0.96;

/// Nền cột trái theo việc phía sau có mờ thật hay không.
pub fn side_nav_surface(cx: &App) -> Hsla {
    let theme = cx.theme();
    theme.colors().surface.opacity(side_nav_alpha(blur(), theme.is_dark()))
}

fn side_nav_alpha(blur: Blur, dark: bool) -> f32 {
    match (blur, dark) {
        (Blur::Shell, _) => SHELL_BLUR_ALPHA,
        (_, false) => 1.0,
        (Blur::Compositor, true) => TRANSLUCENT_SURFACE_ALPHA,
        (Blur::None, true) => UNBLURRED_ALPHA,
    }
}

#[cfg(not(target_os = "linux"))]
fn blur() -> Blur {
    Blur::Compositor
}

#[cfg(target_os = "linux")]
fn blur() -> Blur {
    if gnome::blur() {
        Blur::Shell
    } else if kwin_blur() {
        Blur::Compositor
    } else {
        Blur::None
    }
}

/// Theo dõi lớp mờ của extension GNOME Shell cho cửa sổ chính, vẽ lại khi nó bật / tắt. OS khác, Linux ngoài GNOME: không làm gì.
pub fn watch_shell_blur(window: AnyWindowHandle, cx: &mut App) {
    #[cfg(target_os = "linux")]
    gnome::watch(window, cx);
    #[cfg(not(target_os = "linux"))]
    let _ = (window, cx);
}

/// Đổi nền mờ của cửa sổ `Blurred` sang blur-behind (đúng bằng vùng client) ở bản Windows có acrylic giật khi kéo, và tắt bóng
/// DWM quanh khung. Gọi một lần sau khi mở cửa sổ (GPUI chỉ đặt nền lúc tạo). Nền khác không làm gì.
pub fn smooth_blur(window: &materiant::gpui_kit::Window) {
    #[cfg(windows)]
    if let Some(hwnd) = super::window_controls::hwnd(window) {
        if !env_is("GROOVE_WINDOW_SHADOW", "on") {
            sys::disable_frame_shadow(hwnd);
        }
        if sys::acrylic_lags(sys::build_number()) && !env_is("GROOVE_BLUR", "acrylic") {
            sys::install_blur_behind(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
fn env_is(name: &str, value: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v.eq_ignore_ascii_case(value))
}

#[cfg(windows)]
mod sys {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::{
        DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY,
        DwmEnableBlurBehindWindow, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, IsZoomed,
        SPI_SETWORKAREA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
        SetWindowPos, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_SETTINGCHANGE, WM_SIZE,
    };

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDpiForWindow(hwnd: HWND) -> u32;
    }

    /// Bề dày vùng kéo mép đổi cỡ bên trong cửa sổ (px logic), thay viền vô hình 8 px của Windows.
    const RESIZE_EDGE: i32 = 6;

    const SUBCLASS_ID: usize = 0x626c_7572;

    /// Bản đầu có acrylic mượt khi kéo (Windows 11 22H2).
    const SMOOTH_ACRYLIC_BUILD: u32 = 22621;
    const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
    const WCA_ACCENT_POLICY: u32 = 0x13;

    pub fn acrylic_lags(build: u32) -> bool {
        build < SMOOTH_ACRYLIC_BUILD
    }

    #[repr(C)]
    struct OsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
    }

    /// Số build thật (`GetVersionEx` nói dối khi exe không khai manifest tương thích). Đọc lỗi thì 0 (coi như bản cũ).
    pub fn build_number() -> u32 {
        let mut info = OsVersionInfo {
            size: size_of::<OsVersionInfo>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            csd: [0; 128],
        };
        // SAFETY: `info` đúng bố cục `OSVERSIONINFOW`, `size` đã điền.
        if unsafe { RtlGetVersion(&mut info) } == 0 { info.build } else { 0 }
    }

    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }

    #[repr(C)]
    struct CompositionData {
        attribute: u32,
        data: *mut core::ffi::c_void,
        size: usize,
    }

    /// Tắt vẽ phần ngoài client của DWM: mất bóng / viền 1 px quanh khung (vệt xám trong viền đổi cỡ vô hình).
    pub fn disable_frame_shadow(hwnd: HWND) {
        let policy = DWMNCRP_DISABLED;
        // SAFETY: `hwnd` là cửa sổ đang sống; con trỏ tới biến cục bộ đúng cỡ.
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_NCRENDERING_POLICY as u32,
                (&raw const policy).cast(),
                size_of::<i32>() as u32,
            );
        }
    }

    /// Bật blur-behind vừa khít vùng client và giữ nó khít khi đổi cỡ (subclass nghe `WM_SIZE`).
    pub fn install_blur_behind(hwnd: HWND) {
        // SAFETY: subclass không giữ dữ liệu; `SetWindowSubclass` không gửi thông điệp. `SetWindowPos` sau đó chạy lồng wndproc
        // (`WM_NCCALCSIZE`, `WM_SIZE`) ngoài mọi `update` của GPUI (người gọi đang trong `handle.update` nhưng GPUI hoãn sự kiện
        // cửa sổ tới vòng sau).
        unsafe {
            let mut window = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let mut client = window;
            let measured = GetWindowRect(hwnd, &mut window) != 0 && GetClientRect(hwnd, &mut client) != 0;
            SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
            if measured && IsZoomed(hwnd) == 0 {
                // Bỏ viền vô hình, khung mới = vùng client GPUI định khi đặt cỡ đã lưu: nó chia đôi phần chênh khung − client
                // cho hai bên (`calculate_window_rect`), kể cả 8 px viền dưới thành 4 trên / 4 dưới. Đặt theo đúng cách chia đó thì
                // cỡ và vị trí lưu lại khớp cỡ đã mở, không trôi mỗi lần mở.
                let extra_w = (window.right - window.left) - client.right;
                let extra_h = (window.bottom - window.top) - client.bottom;
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    window.left + extra_w / 2,
                    window.top + extra_h / 2,
                    client.right,
                    client.bottom,
                    SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            } else {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                );
            }
        }
        sync_border_offset(hwnd);
        apply(hwnd);
        log::info!("backdrop: blur-behind on, invisible resize border removed (build {})", build_number());
    }

    /// GPUI trừ "viền" (khung − client, đo lúc tạo) khi đổi qua lại cỡ đã lưu; viền nay là 0 nên bắt nó đo lại, không thì cỡ lưu
    /// hụt 16 × 8 px và cửa sổ nhỏ dần mỗi lần mở. GPUI chỉ đo lại ở `WM_SETTINGCHANGE` (`wParam` ≠ 0) — `SPI_SETWORKAREA` không
    /// đụng gì khác của nó. Gọi khi không phóng to (lúc phóng to GPUI thụt viền, đo lúc đó sẽ sai cỡ thường).
    fn sync_border_offset(hwnd: HWND) {
        // SAFETY: `hwnd` là cửa sổ đang sống; gọi ngoài mọi lượt xử lý của GPUI cho cửa sổ này.
        unsafe {
            SendMessageW(hwnd, WM_SETTINGCHANGE, SPI_SETWORKAREA as WPARAM, 0);
        }
    }

    /// Mép đổi cỡ bên trong khung cho điểm `(x, y)` toạ độ client, `None` khi không ở mép.
    pub fn resize_edge(x: i32, y: i32, width: i32, height: i32, edge: i32) -> Option<u32> {
        let left = x < edge;
        let right = x >= width - edge;
        let bottom = y >= height - edge;
        match (left, right, bottom) {
            (true, _, true) => Some(HTBOTTOMLEFT),
            (_, true, true) => Some(HTBOTTOMRIGHT),
            (true, _, _) => Some(HTLEFT),
            (_, true, _) => Some(HTRIGHT),
            (_, _, true) => Some(HTBOTTOM),
            _ => None,
        }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _: usize,
        _: usize,
    ) -> LRESULT {
        // SAFETY: chuyển tiếp nguyên tham số cho wndproc của GPUI trước, rồi đặt lại vùng mờ theo cỡ mới.
        unsafe {
            if message == WM_NCDESTROY {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                return DefSubclassProc(hwnd, message, wparam, lparam);
            }
            match message {
                // Không phóng to: vùng client = cả khung (giữ nguyên `rgrc[0]`). Phóng to: để GPUI thụt viền như cũ.
                WM_NCCALCSIZE if wparam != 0 && IsZoomed(hwnd) == 0 => return 0,
                WM_NCHITTEST if IsZoomed(hwnd) == 0 => {
                    let hit = DefSubclassProc(hwnd, message, wparam, lparam);
                    if hit != HTCLIENT as LRESULT {
                        return hit;
                    }
                    let mut point =
                        POINT { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 };
                    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    if ScreenToClient(hwnd, &mut point) == 0 || GetClientRect(hwnd, &mut client) == 0 {
                        return hit;
                    }
                    let edge = RESIZE_EDGE * GetDpiForWindow(hwnd) as i32 / 96;
                    return match resize_edge(point.x, point.y, client.right, client.bottom, edge) {
                        Some(area) => area as LRESULT,
                        None => hit,
                    };
                }
                _ => {}
            }
            let result = DefSubclassProc(hwnd, message, wparam, lparam);
            if message == WM_SIZE {
                if IsZoomed(hwnd) == 0 {
                    sync_border_offset(hwnd);
                }
                apply(hwnd);
            }
            result
        }
    }

    /// Vùng mờ = vùng client hiện tại, rồi accent blur-behind.
    fn apply(hwnd: HWND) {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: `hwnd` là cửa sổ đang sống; region tạo ra được DWM chép, xoá ngay sau lời gọi.
        unsafe {
            if GetClientRect(hwnd, &mut client) == 0 {
                return;
            }
            let region = CreateRectRgn(0, 0, client.right, client.bottom);
            let blur = DWM_BLURBEHIND {
                dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                fEnable: 1,
                hRgnBlur: region,
                fTransitionOnMaximized: 0,
            };
            DwmEnableBlurBehindWindow(hwnd, &blur);
            if !region.is_null() {
                DeleteObject(region);
            }
        }
        blur_behind(hwnd);
    }

    /// `SetWindowCompositionAttribute` (user32, không công khai — như gpui-pre-windows, window-vibrancy): blur-behind, không tô màu.
    fn blur_behind(hwnd: HWND) {
        type SetAttribute = unsafe extern "system" fn(HWND, *mut CompositionData) -> i32;
        // SAFETY: user32 đã nạp sẵn trong mọi tiến trình có cửa sổ; hàm lấy đúng chữ ký đã biết; `hwnd` là cửa sổ đang sống.
        unsafe {
            let user32 = GetModuleHandleA(c"user32.dll".as_ptr().cast());
            if user32.is_null() {
                return;
            }
            let Some(raw) = GetProcAddress(user32, c"SetWindowCompositionAttribute".as_ptr().cast()) else { return };
            let set_attribute: SetAttribute = std::mem::transmute(raw);
            let mut accent =
                AccentPolicy { state: ACCENT_ENABLE_BLURBEHIND, flags: 0, gradient_color: 0, animation_id: 0 };
            let mut data = CompositionData {
                attribute: WCA_ACCENT_POLICY,
                data: (&raw mut accent).cast(),
                size: size_of::<AccentPolicy>(),
            };
            set_attribute(hwnd, &mut data);
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn resize_edges_inside_the_frame() {
            use windows_sys::Win32::UI::WindowsAndMessaging::{HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT};
            let at = |x, y| super::resize_edge(x, y, 800, 600, 6);
            assert_eq!(at(0, 300), Some(HTLEFT));
            assert_eq!(at(5, 300), Some(HTLEFT));
            assert_eq!(at(6, 300), None);
            assert_eq!(at(799, 300), Some(HTRIGHT));
            assert_eq!(at(400, 599), Some(HTBOTTOM));
            assert_eq!(at(2, 598), Some(HTBOTTOMLEFT));
            assert_eq!(at(797, 598), Some(HTBOTTOMRIGHT));
            assert_eq!(at(400, 0), None, "mép trên do GPUI lo");
        }

        #[test]
        fn acrylic_kept_from_windows_11_22h2() {
            assert!(super::acrylic_lags(19045), "Windows 10 22H2");
            assert!(super::acrylic_lags(22000), "Windows 11 21H2");
            assert!(!super::acrylic_lags(22621));
            assert!(!super::acrylic_lags(26100));
            assert!(super::build_number() > 0);
        }
    }
}

/// Đoán theo môi trường như gpui-pre-linux chọn nền: có `WAYLAND_DISPLAY` thì Wayland, và chỉ KWin (Plasma) có
/// `org_kde_kwin_blur`. Đoán sai về phía KWin tắt hiệu ứng mờ thì chỉ là trong suốt 0,88 như trước.
#[cfg(target_os = "linux")]
fn kwin_blur() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
        wayland && desktop_has(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(), "KDE")
    })
}

/// `XDG_CURRENT_DESKTOP` (danh sách cách bằng `:`) có tên `name`.
#[cfg(target_os = "linux")]
fn desktop_has(xdg_current_desktop: &str, name: &str) -> bool {
    xdg_current_desktop.split(':').any(|part| part.eq_ignore_ascii_case(name))
}

/// gnome-blur (09/11/2026): extension GNOME Shell của Groovex (`packaging/linux/gnome-shell/<UUID>/extension.js`) gắn lớp
/// `Shell.BlurEffect` dưới cửa sổ chính — Mutter không có giao thức cho app xin mờ, GPUI chỉ xin qua `org_kde_kwin_blur`.
/// Thuộc tính D-Bus `Blur` nói phía sau có mờ thật không (tương phản cao ⇒ false). App có thể mở trước khi Shell nạp
/// extension (đăng nhập), tương phản cao đổi lúc đang chạy ⇒ nghe `NameOwnerChanged` + `PropertiesChanged`, mỗi lần đọc lại
/// `Blur`; đổi thì vẽ lại cả cửa sổ (cột trái nằm trong view `cached`). Không dò theo nhịp. Tên phải khớp `extension.js`.
#[cfg(target_os = "linux")]
pub mod gnome {
    use std::sync::atomic::{AtomicBool, Ordering};

    use futures::StreamExt;
    use futures::channel::mpsc;
    use materiant::gpui_kit::{AnyWindowHandle, App};
    use zbus::message::Type;
    use zbus::zvariant::OwnedValue;
    use zbus::{Connection, MatchRule, MessageStream};

    /// Thư mục của extension trong `gnome-shell/extensions/`.
    pub const UUID: &str = "groovex-shell@com.sandtechs.desktop.groovex";
    pub const BUS_NAME: &str = "com.sandtechs.desktop.groovex.Shell";
    pub const OBJECT_PATH: &str = "/com/sandtechs/desktop/groovex/Shell";
    pub const INTERFACE: &str = "com.sandtechs.desktop.groovex.Shell1";

    const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
    const SHELL: &str = "org.gnome.Shell";
    const SHELL_PATH: &str = "/org/gnome/Shell";
    const EXTENSIONS: &str = "org.gnome.Shell.Extensions";
    /// `ExtensionState.ACTIVE` của GNOME Shell 45+.
    const ACTIVE: f64 = 1.0;
    /// Mốc trong thư mục dữ liệu: đã bật extension một lần, người dùng tự tắt sau đó là ý của họ.
    const MARKER: &str = "shell-extension-enabled";

    static BLUR: AtomicBool = AtomicBool::new(false);

    /// Giá trị `Blur` gần nhất của extension (chưa biết, không phải GNOME ⇒ false).
    pub fn blur() -> bool {
        BLUR.load(Ordering::Relaxed)
    }

    pub fn watch(window: AnyWindowHandle, cx: &mut App) {
        if !super::desktop_has(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(), "GNOME") {
            return;
        }
        let (sender, mut receiver) = mpsc::unbounded();
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = follow(sender).await {
                    log::debug!("backdrop: GNOME Shell blur unavailable: {e}");
                }
            })
            .detach();
        cx.spawn(async move |cx| {
            while let Some(blur) = receiver.next().await {
                if BLUR.swap(blur, Ordering::Relaxed) != blur {
                    log::info!("backdrop: GNOME Shell blur {}", if blur { "on" } else { "off" });
                    let _ = window.update(cx, |_, window, _| window.refresh());
                }
            }
        })
        .detach();
    }

    async fn follow(sender: mpsc::UnboundedSender<bool>) -> zbus::Result<()> {
        let conn = Connection::session().await?;
        enable_once(&conn).await;
        let owner = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.DBus")?
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .add_arg(BUS_NAME)?
            .build();
        let changed = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(PROPERTIES)?
            .member("PropertiesChanged")?
            .path(OBJECT_PATH)?
            .build();
        // Đặt luật nghe trước rồi mới đọc: extension nạp giữa hai bước vẫn không lỡ.
        let owner = MessageStream::for_match_rule(owner, &conn, None).await?;
        let changed = MessageStream::for_match_rule(changed, &conn, None).await?;
        let mut events = futures::stream::select(owner, changed);
        loop {
            if sender.unbounded_send(read_blur(&conn).await).is_err() {
                return Ok(());
            }
            if events.next().await.is_none() {
                return Ok(());
            }
        }
    }

    /// Extension không chạy / bản không có `Blur` ⇒ false.
    async fn read_blur(conn: &Connection) -> bool {
        let reply = conn.call_method(Some(BUS_NAME), OBJECT_PATH, Some(PROPERTIES), "Get", &(INTERFACE, "Blur")).await;
        reply
            .and_then(|r| r.body().deserialize::<OwnedValue>())
            .ok()
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    }

    /// Gói `.deb` cài extension vào `/usr/share/gnome-shell/extensions/`, GNOME không tự bật extension mới ⇒ bật một lần. Shell
    /// chỉ quét thư mục extension lúc đăng nhập: cài xong mà chưa đăng nhập lại thì Shell chưa biết, lần chạy sau thử lại.
    async fn enable_once(conn: &Connection) {
        let Ok(marker) = groove_core::paths::data_dir().map(|d| d.join(MARKER)) else { return };
        if marker.exists() {
            return;
        }
        let info = conn.call_method(Some(SHELL), SHELL_PATH, Some(EXTENSIONS), "GetExtensionInfo", &(UUID,)).await;
        let info: std::collections::HashMap<String, OwnedValue> = match info.and_then(|r| r.body().deserialize()) {
            Ok(info) => info,
            Err(e) => return log::debug!("backdrop: no GNOME Shell ({e})"),
        };
        if info.is_empty() {
            log::info!(
                "backdrop: GNOME Shell does not know the Groovex extension yet (not installed, or log in again)"
            );
            return;
        }
        // `d` ở bản Shell cũ; kiểu khác thì coi như chưa bật (`EnableExtension` lần nữa vô hại).
        let state =
            info.get("state").and_then(|v| f64::try_from(v).ok().or_else(|| u32::try_from(v).ok().map(f64::from)));
        let enabled = state == Some(ACTIVE) || {
            let reply = conn.call_method(Some(SHELL), SHELL_PATH, Some(EXTENSIONS), "EnableExtension", &(UUID,)).await;
            reply.and_then(|r| r.body().deserialize::<bool>()).unwrap_or(false)
        };
        log::info!("backdrop: Groovex shell extension state {state:?}, enabled {enabled}");
        if enabled
            && let Err(e) =
                std::fs::create_dir_all(marker.parent().unwrap_or(&marker)).and_then(|()| std::fs::write(&marker, b""))
        {
            log::warn!("backdrop: cannot write the extension marker ({})", e.kind());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Blur, SHELL_BLUR_ALPHA, TRANSLUCENT_SURFACE_ALPHA, UNBLURRED_ALPHA, side_nav_alpha};

    #[test]
    fn side_nav_is_clearer_only_when_something_blurs_behind() {
        assert_eq!(side_nav_alpha(Blur::Shell, true), SHELL_BLUR_ALPHA);
        assert_eq!(side_nav_alpha(Blur::Shell, false), SHELL_BLUR_ALPHA, "extension GNOME mờ đủ cho cả tông sáng");
        assert_eq!(side_nav_alpha(Blur::Compositor, true), TRANSLUCENT_SURFACE_ALPHA);
        assert_eq!(side_nav_alpha(Blur::Compositor, false), 1.0);
        assert_eq!(side_nav_alpha(Blur::None, true), UNBLURRED_ALPHA);
        assert_eq!(side_nav_alpha(Blur::None, false), 1.0);
        const { assert!(SHELL_BLUR_ALPHA < TRANSLUCENT_SURFACE_ALPHA && TRANSLUCENT_SURFACE_ALPHA < UNBLURRED_ALPHA) };
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_names_match_any_entry() {
        use super::desktop_has;
        assert!(desktop_has("KDE", "KDE"));
        assert!(desktop_has("X-Generic:KDE", "KDE"));
        assert!(!desktop_has("ubuntu:GNOME", "KDE"));
        assert!(desktop_has("ubuntu:GNOME", "GNOME"));
        assert!(!desktop_has("sway", "GNOME"));
        assert!(!desktop_has("", "KDE"));
    }

    /// Tên D-Bus, UUID khớp `extension.js` / `metadata.json` của gói.
    #[cfg(target_os = "linux")]
    #[test]
    fn names_match_the_shell_extension() {
        use super::gnome::{BUS_NAME, INTERFACE, OBJECT_PATH, UUID};
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/linux/gnome-shell/");
        let js = std::fs::read_to_string(format!("{dir}{UUID}/extension.js")).expect("extension.js");
        let meta = std::fs::read_to_string(format!("{dir}{UUID}/metadata.json")).expect("metadata.json");
        assert!(js.contains(&format!("'{BUS_NAME}'")));
        assert!(js.contains(&format!("'{OBJECT_PATH}'")));
        assert!(js.contains(&format!("\"{INTERFACE}\"")));
        assert!(js.contains("'groove', 'groove.dev'"), "app_id của bản cài và bản debug (`window_state`)");
        assert!(meta.contains(&format!("\"uuid\": \"{UUID}\"")));
    }
}
