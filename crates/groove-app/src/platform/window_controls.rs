//! window-buttons (03/11/2026): ba nút cửa sổ tự vẽ của thanh trên (thu nhỏ, phóng to / khôi phục, đóng).
//!
//! Nút là phần tử bấm thường của GPUI (`on_click`), không phải vùng `WindowControlArea` để OS hit-test: thanh trên nhúng
//! `cached`, mà gpui-pre 0.3.10 không chép vùng `WindowControlArea` khi dùng lại khung của view `cached` — trên màn thật
//! (có hoạt ảnh, rê chuột) `HTMINBUTTON` / `HTMAXBUTTON` mất, bấm không ăn. Nút bấm thường đi đường mọi nút khác của app.
//! `zoom_window` của gpui-pre-windows chỉ `SW_MAXIMIZE`, nên Windows gọi thẳng `ShowWindowAsync` để bấm lần hai khôi phục cỡ
//! cũ; đóng gửi `WM_CLOSE` như nút tiêu đề thật (đi qua `on_window_should_close`). Nền khác: API cửa sổ của GPUI.

//!
//! macos-captions (09/11/2026): macOS vẫn không có thanh tiêu đề OS (`titlebar: None`, chủ dự án: thanh trên tự vẽ, tự vẽ cả ba
//! đèn giao thông — `views::traffic_lights`). gpui-pre-macos 0.3.10 khi `titlebar: None` tạo NSWindow chỉ với
//! `Titled | FullSizeContentView`, thiếu `Miniaturizable` / `Resizable` / `Closable` ⇒ `miniaturize:`, `zoom:` bị AppKit
//! bỏ qua, kéo mép không đổi cỡ. [`prepare`] thêm ba cờ đó rồi ẩn ba nút native mà cờ làm hiện ra.

use materiant::gpui_kit::Window;

/// macOS: ba nút cửa sổ là đèn giao thông tự vẽ ở góc trái trên; nền khác: ba nút kiểu Windows ở góc phải thanh trên.
pub const TRAFFIC_LIGHTS: bool = cfg!(target_os = "macos");

/// Gọi một lần sau khi mở cửa sổ chính (macOS: cờ NSWindow, ẩn nút native). Nền khác không làm gì.
pub fn prepare(window: &Window) {
    #[cfg(target_os = "macos")]
    mac::prepare(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn minimize(window: &Window) {
    window.minimize_window();
}

/// Phóng to, hoặc khôi phục khi đang phóng to.
pub fn toggle_maximize(window: &Window) {
    #[cfg(windows)]
    if let Some(hwnd) = sys::hwnd(window) {
        sys::toggle_maximize(hwnd);
        return;
    }
    window.zoom_window();
}

#[cfg(windows)]
pub fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    sys::hwnd(window)
}

pub fn close(window: &mut Window) {
    #[cfg(windows)]
    if let Some(hwnd) = sys::hwnd(window) {
        sys::close(hwnd);
        return;
    }
    window.remove_window();
}

#[cfg(target_os = "macos")]
mod mac {
    use materiant::gpui_kit::Window;
    use objc2_app_kit::{NSView, NSWindowButton, NSWindowStyleMask};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn prepare(window: &Window) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
        // SAFETY: `ns_view` là NSView đang sống của cửa sổ GPUI vừa mở, gọi trên luồng chính.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let Some(ns_window) = view.window() else { return };
        ns_window.setStyleMask(
            ns_window.styleMask()
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable,
        );
        for kind in [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton] {
            if let Some(button) = ns_window.standardWindowButton(kind) {
                button.setHidden(true);
            }
        }
    }
}

#[cfg(windows)]
mod sys {
    use materiant::gpui_kit::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsZoomed, PostMessageW, SW_MAXIMIZE, SW_RESTORE, ShowWindowAsync, WM_CLOSE,
    };

    pub fn hwnd(window: &Window) -> Option<HWND> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
            _ => None,
        }
    }

    pub fn toggle_maximize(hwnd: HWND) {
        // SAFETY: `hwnd` là cửa sổ đang sống của tiến trình này (vừa lấy từ `Window` đang vẽ).
        unsafe {
            let show = if IsZoomed(hwnd) != 0 { SW_RESTORE } else { SW_MAXIMIZE };
            ShowWindowAsync(hwnd, show);
        }
    }

    pub fn close(hwnd: HWND) {
        // SAFETY: như trên; `WM_CLOSE` gửi bất đồng bộ, GPUI xử lý ở vòng thông điệp kế.
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}
