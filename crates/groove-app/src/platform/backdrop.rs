//! linux-chrome (03/11/2026): nền cột trái theo việc OS có làm mờ phía sau cửa sổ được hay không.
//!
//! Windows (acrylic), macOS (`NSVisualEffectView`) luôn mờ ⇒ cửa sổ `Blurred`, cột trái nền alpha
//! (`translucent_surface`). gpui-pre-linux 0.3.10 chỉ xin mờ qua `org_kde_kwin_blur` trên Wayland (KWin có, GNOME / Mutter
//! và mọi compositor wlroots không có), X11 không xin gì: ở đó `Blurred` chỉ là trong suốt trơn, nhìn xuyên thấy rõ chữ cửa
//! sổ phía sau. Không có mờ thì cửa sổ đục (`Opaque`) và cột trái nền `surface` đặc (`plan/confirm.md` mục `linux-chrome`).

use materiant::gpui_kit::{App, Hsla, WindowBackgroundAppearance};
use materiant::theme::ActiveTheme;

/// Nền cửa sổ chính.
pub fn window_background() -> WindowBackgroundAppearance {
    if blur_available() { WindowBackgroundAppearance::Blurred } else { WindowBackgroundAppearance::Opaque }
}

/// Nền cột trái: alpha khi cửa sổ mờ, `surface` đục khi không.
pub fn side_nav_surface(cx: &App) -> Hsla {
    if blur_available() { materiant::window::translucent_surface(cx) } else { cx.theme().colors().surface }
}

#[cfg(not(target_os = "linux"))]
pub fn blur_available() -> bool {
    true
}

/// Đoán theo môi trường như gpui-pre-linux chọn nền: có `WAYLAND_DISPLAY` thì Wayland, và chỉ KWin (Plasma) có
/// `org_kde_kwin_blur`. Đoán sai về phía KWin thiếu giao thức thì vẫn như cũ (trong suốt trơn) — không tệ hơn trước.
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
