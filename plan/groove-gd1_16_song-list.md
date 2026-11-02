# groove-gd1_16 — song-list

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~700 dòng |
| Phụ thuộc | groove-gd1_03, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{song_list,song_row}.rs`, `i18n/*/song_list.toml` |

## 0. Bàn giao từ groove-gd1_03 (02/11/2026)

<!-- handoff: groove-gd1_03 -->

- **Chỗ đặt**: danh sách bài của một category nằm trong `Slot::Library` cùng task 15 (đặc tả 1.2); task 15 sở hữu `library_page.rs` và là bên gắn `SongList` vào slot, task này chỉ cần xuất view `SongList` nhận `CategoryRef`. Điều hướng vào / ra: `app::navigate(Route::Category(..), cx)`, `app::go_back(cx)`.
- **Chữ ký ở `app::`** (chi tiết `plan/groove-gd1_03_app-shell.md` mục 9): `Slot::{PlayerBar, Library, NowPlaying}`, `install_slot(slot, view, cx)`, `navigate(Route, cx)`, `current_route(cx)`, `settings(cx)`, `update_settings(cx, |s| …)`, `shell(cx)` phát `ShellEvent`.
- **Icon sẵn**: `GrooveIcon::{Play, Artist, Song, Edit, LyricsBubble, Delete}`.
- **Bẫy**: `#[derive(IntoElement)]` ở bản không test cần `use materiant::gpui_kit as gpui;`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 16. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).