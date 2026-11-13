//! pip (03/11/2026): giữ cửa sổ trình phát thu nhỏ nằm trên mọi cửa sổ khác, nền nào làm được thì làm.
//!
//! - **Windows**: `SetWindowPos(HWND_TOPMOST)` sau khi mở. Không dùng `WindowKind::PopUp` của gpui-pre-windows 0.3.10: nó dựng
//!   cửa sổ `WS_EX_TOOLWINDOW` không khung (mất nút trên taskbar, Alt+Tab). `SetWindowPos` gửi `WM_WINDOWPOS*` đồng bộ vào
//!   wndproc của GPUI, nên gọi từ task foreground lúc `App` không bị mượn (bẫy "RefCell already borrowed" như
//!   `taskbar_windows`).
//! - **macOS**: `NSWindow` thường nâng lên `NSFloatingWindowLevel`, theo mọi Space và cạnh app toàn màn hình. Không dùng
//!   `WindowKind::Floating` / `PopUp`: GPUI dựng `NSPanel`, mà panel mặc định ẩn khi app mất focus (`hidesOnDeactivate`) — PiP
//!   biến mất đúng lúc người dùng sang app khác.
//! - **Linux**: Wayland không có giao thức cho app tự ghim cửa sổ lên trên (GNOME / Mutter, KWin đều do người dùng chọn: chuột
//!   phải → "Always on Top", như PiP của Chrome). pip-above (13/11/2026): trên GNOME, extension Shell của Groovex (bản 3) nhận
//!   `KeepAbove(title)` và `make_above()` cửa sổ Groovex có đúng tiêu đề đó (chưa hiện thì áp lúc hiện) — [`super::backdrop::gnome::keep_above`].
//!   Extension chưa nạp (vừa cài, chưa đăng nhập lại) / desktop khác: cửa sổ thường.

use materiant::gpui_kit::{App, Window};

/// Xin giữ `window` trên cùng. `false` khi nền này không làm được (Linux) hoặc không lấy được cửa sổ gốc.
pub fn keep_on_top(window: &Window, cx: &mut App) -> bool {
    sys::keep_on_top(window, cx)
}

#[cfg(windows)]
mod sys {
    use materiant::gpui_kit::{App, Window};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    use crate::platform::window_controls;

    pub fn keep_on_top(window: &Window, cx: &mut App) -> bool {
        let Some(hwnd) = window_controls::hwnd(window) else { return false };
        // `HWND` là con trỏ thô (không `Send`): chỉ đi trong task foreground của cùng luồng UI.
        let hwnd = hwnd as isize;
        cx.spawn(async move |_| {
            // SAFETY: cửa sổ của tiến trình này, trên luồng UI; đã đóng thì `SetWindowPos` chỉ trả lỗi.
            unsafe {
                SetWindowPos(hwnd as _, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        })
        .detach();
        true
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use materiant::gpui_kit::{App, Window};
    use objc2_app_kit::{NSFloatingWindowLevel, NSView, NSWindowCollectionBehavior};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn keep_on_top(window: &Window, _: &mut App) -> bool {
        let Ok(handle) = HasWindowHandle::window_handle(window) else { return false };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return false };
        // SAFETY: `ns_view` là NSView đang sống của cửa sổ GPUI vừa mở, gọi trên luồng chính.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let Some(ns_window) = view.window() else { return false };
        ns_window.setLevel(NSFloatingWindowLevel);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        true
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use materiant::gpui_kit::{App, Window};

    pub fn keep_on_top(_: &Window, cx: &mut App) -> bool {
        // Đúng chữ `PipView` đặt làm tiêu đề cửa sổ ở lần vẽ đầu.
        crate::platform::backdrop::gnome::keep_above(groove_core::t!("player.pip.window_title").to_string(), cx)
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    use materiant::gpui_kit::{App, Window};

    pub fn keep_on_top(_: &Window, _: &mut App) -> bool {
        false
    }
}
