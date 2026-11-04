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
