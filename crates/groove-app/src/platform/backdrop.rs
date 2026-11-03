//! linux-chrome (03/11/2026): nền cột trái theo việc OS có làm mờ phía sau cửa sổ được hay không.
//!
//! Windows (acrylic), macOS (`NSVisualEffectView`) luôn mờ ⇒ cột trái nền alpha 0,88 (`translucent_surface`).
//! gpui-pre-linux 0.3.10 chỉ xin mờ qua `org_kde_kwin_blur` trên Wayland (KWin có; GNOME / Mutter — GNOME 50 không quảng bá
//! giao thức mờ nào —, wlroots không có), X11 không xin gì: ở đó `Blurred` chỉ là trong suốt trơn, chữ cửa sổ phía sau lộ
//! rõ. Không có mờ thì cột trái đục hơn: alpha 0,94 (`FLOATING_SURFACE_ALPHA` Linux, nền trong suốt app tự chọn — D-10).
//!
//! drag-lag (03/11/2026): [`smooth_blur`] — `Blurred` của gpui-pre-windows 0.3.10 là acrylic
//! (`ACCENT_ENABLE_ACRYLICBLURBEHIND`), mà acrylic trên Windows 10 1903+ và Windows 11 trước 22H2 (build < 22621) giật khi kéo /
//! đổi cỡ cửa sổ (DWM vẽ lại lớp acrylic mỗi lần dời; window-vibrancy ghi rõ "bad performance when resizing/dragging"). Ở các
//! bản đó cửa sổ chính đổi sang blur-behind kiểu Aero (`ACCENT_ENABLE_BLURBEHIND`) như bản Tauri (`window_vibrancy::apply_blur`):
//! vẫn mờ, không hạt nhiễu acrylic, kéo mượt. Windows 11 22H2+ giữ acrylic.

use materiant::gpui_kit::{App, Hsla};
use materiant::style::FLOATING_SURFACE_ALPHA;
use materiant::theme::ActiveTheme;

/// Nền cột trái: alpha 0,88 khi cửa sổ mờ, 0,94 khi chỉ trong suốt. Tông sáng giữ đục như `translucent_surface`.
pub fn side_nav_surface(cx: &App) -> Hsla {
    if blur_available() {
        return materiant::window::translucent_surface(cx);
    }
    let theme = cx.theme();
    let surface = theme.colors().surface;
    if theme.is_dark() { surface.opacity(FLOATING_SURFACE_ALPHA) } else { surface }
}

/// Đổi nền mờ của cửa sổ `Blurred` sang blur-behind ở bản Windows có acrylic giật khi kéo. Gọi một lần sau khi mở cửa sổ (GPUI
/// chỉ đặt nền lúc tạo). Nền khác không làm gì.
pub fn smooth_blur(window: &materiant::gpui_kit::Window) {
    #[cfg(windows)]
    if let Some(hwnd) = super::window_controls::hwnd(window)
        && sys::acrylic_lags(sys::build_number())
    {
        sys::blur_behind(hwnd);
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
mod sys {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

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

    /// `SetWindowCompositionAttribute` (user32, không công khai — như gpui-pre-windows, window-vibrancy): blur-behind, không tô màu.
    pub fn blur_behind(hwnd: HWND) {
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
        fn acrylic_kept_from_windows_11_22h2() {
            assert!(super::acrylic_lags(19045), "Windows 10 22H2");
            assert!(super::acrylic_lags(22000), "Windows 11 21H2");
            assert!(!super::acrylic_lags(22621));
            assert!(!super::acrylic_lags(26100));
            assert!(super::build_number() > 0);
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn blur_available() -> bool {
    true
}

/// Đoán theo môi trường như gpui-pre-linux chọn nền: có `WAYLAND_DISPLAY` thì Wayland, và chỉ KWin (Plasma) có
/// `org_kde_kwin_blur`. Đoán sai về phía KWin tắt hiệu ứng mờ thì chỉ là trong suốt 0,88 như trước.
#[cfg(target_os = "linux")]
pub fn blur_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        wayland && kwin_desktop(&desktop)
    })
}

#[cfg(target_os = "linux")]
fn kwin_desktop(xdg_current_desktop: &str) -> bool {
    xdg_current_desktop.split(':').any(|name| name.eq_ignore_ascii_case("KDE"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn only_plasma_counts_as_kwin() {
        assert!(super::kwin_desktop("KDE"));
        assert!(super::kwin_desktop("X-Generic:KDE"));
        assert!(!super::kwin_desktop("ubuntu:GNOME"));
        assert!(!super::kwin_desktop("sway"));
        assert!(!super::kwin_desktop(""));
    }
}
