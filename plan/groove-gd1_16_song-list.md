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

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `Player::spawn(PlayerDeps { library, output: Box::new(FakeOutput), .. }, cx)` với `app::update_settings` đã có, và `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác.
- **Danh sách bài**: bấm dòng `play_list(track, list, cx)`, bấm đúng bài đang phát tự giữ tiếng (S2). Dòng đang phát so `current_track()`; bars theo `is_playing()`.

## 0. Bàn giao từ groove-gd1_15 (02/11/2026)

<!-- handoff: groove-gd1_15 -->

- **Đã có sẵn để dùng**: `views/library_page.rs` (`LibraryPage`) đã ở `Slot::Library`; vào danh sách bằng `LibraryPageEvent::Open` → `app::navigate(Route::Category(ref))`. `app::library(cx)` trả `Library` dùng chung (`songs(CategoryRef)`, `category(CategoryRef)` cho đầu danh sách: bìa, accent, tên, `song_count`). Chi tiết: `plan/groove-gd1_15_library-grid.md` mục 9.
- **Không gắn `SongList` vào slot ở task này**: nhánh `Route::Category(..)` trong `LibraryPage::render` đang là chữ giữ chỗ và là việc của task 17 thay bằng view `SongList` bạn xuất (task 15 không sửa được tệp của 16, 16 không sửa `library_page.rs`). Xuất `SongList::new(reference, cx)` (hoặc tương đương) thật gọn để một dòng là gắn được.
- **Dùng lại**: `views::cover_image::{CoverImage, CoverKind, parse_color}`, `views::category_card::{accent_of, cover_path}` cho bìa 160 / 80 px và quầng accent ở đầu danh sách.
- **Bẫy đã gặp**: `occlude()` làm `group_hover` tắt; test view với thư viện thật cần `cx.executor().allow_parking()`.

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
