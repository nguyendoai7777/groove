# groove-gd1_03 — app-shell

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~700 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_01, groove-gd1_02 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/{app,route,icons}.rs`, `views/{side_nav,app_bar}.rs`, `assets/icons/*`, `i18n/*/shell.toml` |

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `groove_core::settings::{Settings, ThemeChoice, StartPage, JsonSaver}`, `groove_core::model::{CategoryRef, TrackRef}`.
- **Nợ chuyển sang đây**: khi mở app, trước khi tạo cửa sổ, gọi `Settings::load()` rồi `apply_language()` (task 02 không gọi được vì `main.rs` không sửa). Đây là chỗ đọc đĩa duy nhất được phép trên UI thread (N7).
- **Dùng khi dựng khung**: `settings.theme` (`ThemeChoice`, cùng giá trị serde với `materiant::ThemeMode`, tự đổi kiểu), `settings.last_active_route` (`StartPage`, đổi sang `Route`; `Route::Category(CategoryRef)` không lưu), `settings.side_nav_collapsed`.
- **Lưu**: một `JsonSaver::spawn(paths::settings_file()?, debounce)`, gọi `.save(&settings)` khi đổi; `Drop` ghi nốt khi thoát.

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Nợ chuyển sang đây**: `app/app.rs` đang là cửa sổ trống còn thanh tiêu đề OS, đổi sang không viền; `run(dev: DevArgs)` giữ `display_id`, `focus: !dev.background`, `dev::replay` để `tools/run.ps1` chạy được; chừa chỗ một lời gọi khởi tạo cho từng task sau. Vùng i18n `shell`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 03. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
