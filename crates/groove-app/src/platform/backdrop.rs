//! linux-chrome (03/11/2026): nền cột trái theo việc OS có làm mờ phía sau cửa sổ được hay không.
//!
//! Windows (acrylic), macOS (`NSVisualEffectView`) luôn mờ ⇒ cột trái nền alpha 0,88 (`translucent_surface`).
//! gpui-pre-linux 0.3.10 chỉ xin mờ qua `org_kde_kwin_blur` trên Wayland (KWin có; GNOME / Mutter — GNOME 50 không quảng bá
//! giao thức mờ nào —, wlroots không có), X11 không xin gì: ở đó `Blurred` chỉ là trong suốt trơn, chữ cửa sổ phía sau lộ
//! rõ. Không có mờ thì cột trái đục hơn: alpha 0,94 như khối nổi trên Linux (D-10, chủ dự án chốt 03/11/2026).

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
