//! window-buttons (03/11/2026): ba nút cửa sổ tự vẽ của thanh trên (thu nhỏ, phóng to / khôi phục, đóng).
//!
//! Nút là phần tử bấm thường của GPUI (`on_click`), không phải vùng `WindowControlArea` để OS hit-test: chủ dự án bấm trên màn
//! thật thì thu nhỏ / phóng to không ăn qua đường `HTMINBUTTON` / `HTMAXBUTTON` của gpui-pre-windows (hit-test dựa vào vị trí
//! chuột GPUI nhớ từ lần di chuột trước), trong khi nút bấm thường của app luôn chạy. `zoom_window` của gpui-pre-windows chỉ
//! `SW_MAXIMIZE`, nên Windows gọi thẳng `ShowWindowAsync` để bấm lần hai khôi phục cỡ cũ; đóng gửi `WM_CLOSE` như nút tiêu
//! đề thật (đi qua `on_window_should_close`). Nền khác: API cửa sổ của GPUI.

use materiant::gpui_kit::Window;

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

pub fn close(window: &mut Window) {
    #[cfg(windows)]
    if let Some(hwnd) = sys::hwnd(window) {
        sys::close(hwnd);
        return;
    }
    window.remove_window();
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
