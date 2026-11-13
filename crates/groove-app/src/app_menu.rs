//! groove-gd2_05: thanh menu của macOS — menu "Groovex" với Giới thiệu, Kiểm tra cập nhật…, Cài đặt…, Ẩn, Thoát (⌘H, ⌘Q).
//! Giới thiệu / Kiểm tra cập nhật mở Settings ở thẻ Giới thiệu; Kiểm tra cập nhật hỏi server ngay (`update::check_now`).
//! Windows / Linux không có thanh menu chung: các hàm ở đây không làm gì.

use materiant::gpui_kit::{self, App};
// `actions!` gọi `gpui::…`.
use materiant::gpui_kit as gpui;

gpui_kit::actions!(groove_app_menu, [About, CheckForUpdates, OpenSettings, HideApp, QuitApp]);

/// Đăng ký action + phím + menu; gọi một lần trong `app::run`, sau khi khung mở.
pub fn install(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        use materiant::gpui_kit::KeyBinding;

        cx.on_action(|_: &About, cx| crate::app::open_about(cx));
        cx.on_action(|_: &CheckForUpdates, cx| {
            crate::app::open_about(cx);
            crate::update::check_now(true, cx);
        });
        cx.on_action(|_: &OpenSettings, cx| crate::app::open_settings(cx));
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &QuitApp, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", QuitApp, None), KeyBinding::new("cmd-h", HideApp, None)]);
        set_menus(cx);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

/// Dựng lại menu theo ngôn ngữ đang dùng (`app` gọi khi đổi ngôn ngữ).
pub fn set_menus(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        use groove_core::t;
        use materiant::gpui_kit::{Menu, MenuItem};

        let menu = Menu::new(t!("common.app_name")).items([
            MenuItem::action(t!("update.menu.about"), About),
            MenuItem::separator(),
            MenuItem::action(t!("update.menu.check"), CheckForUpdates),
            MenuItem::action(t!("update.menu.settings"), OpenSettings),
            MenuItem::separator(),
            MenuItem::action(t!("update.menu.hide"), HideApp),
            MenuItem::action(t!("update.menu.quit"), QuitApp),
        ]);
        cx.set_menus([menu]);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}
