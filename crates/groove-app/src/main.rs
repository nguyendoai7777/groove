//! Groove X: trình phát nhạc GPUI + materiant. Đặc tả: `git show 1a2480f:docs/rewrite-spec.md`.
//!
//! Mọi `mod` khai sẵn ở groove-gd1_01, task sau không sửa tệp này; khởi tạo của app nằm ở `app::run`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod dev;
mod icons;
mod instance;
mod library_ctl;
mod open_paths;
mod platform;
mod player;
mod route;
mod session_sync;
mod views;
mod window_state;

fn main() {
    groove_core::logging::init("groove");
    if instance::forward_to_primary() {
        return;
    }
    // `args_os`: `args()` panic khi có tham số không phải Unicode, mà tên tệp từ OS có thể như vậy. Đường dẫn tệp mở được đọc
    // riêng ở `instance`, `open_paths`; đây chỉ là cờ chạy thử.
    let args: Vec<String> = std::env::args_os().map(|arg| arg.to_string_lossy().into_owned()).collect();
    app::run(dev::DevArgs::parse(&args));
}
