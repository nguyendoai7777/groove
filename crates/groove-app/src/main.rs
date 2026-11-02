//! Groove X: trình phát nhạc GPUI + materiant. Đặc tả: `docs/rewrite-spec.md`.
//!
//! Mọi `mod` khai sẵn ở groove-gd1_01, task sau không sửa tệp này; khởi tạo của app nằm ở `app::run`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Các làn làm song song, view và logic có trước khi nối vào `app.rs`. Gỡ ở groove-gd1_29.
#![allow(dead_code)]

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
    let args: Vec<String> = std::env::args().collect();
    app::run(dev::DevArgs::parse(&args));
}
