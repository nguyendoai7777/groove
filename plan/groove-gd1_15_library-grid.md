# groove-gd1_15 — library-grid

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_03, groove-gd1_05, materiant-gd5_04 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{library_page,category_card,cover_image}.rs`, `i18n/*/library.toml` |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Lưới**: `Library::categories()` trả `Categories { albums, folders }`, `playlists()` trả `Vec<CategorySummary>`, đã sắp theo tên không dấu. `CategorySummary`: `reference, name, artist, path, color, song_count, cover_hash, accent_color`; trường không áp cho loại đó là `None`. Bìa: `CoverStore::path(hash, size)`.
- **Trạng thái mở**: `Library::status()`, `OpenStatus::Failed(OpenFailure)` phải báo cho người dùng. `TooNew`: chỉ báo cần cập nhật app. `LegacyFailed`: hai nút `retry_open()` và `start_fresh()`. Lúc `Opening` việc đọc tự chờ, không lỗi.
- **Nạp lại**: nghe `LibraryEvent::{Opened, Synced, PlaylistsChanged}` qua `subscribe()`.

## 0. Bàn giao từ groove-gd1_03 (02/11/2026)

<!-- handoff: groove-gd1_03 -->

- **Chỗ đặt**: `Slot::Library`, vẽ ở My Music, Playlists **và** `Route::Category(..)` (đặc tả 1.2: chung một màn, `mode` lưới / danh sách). View đọc `app::current_route(cx)` và nghe `ShellEvent::RouteChanged` qua `app::shell(cx).subscribe`; vào danh sách thì `app::navigate(Route::Category(CategoryRef::…), cx)`, nút lùi trên thanh trên đã chạy theo lịch sử.
- **Chữ ký ở `app::`** (chi tiết `plan/groove-gd1_03_app-shell.md` mục 9): `Slot::{PlayerBar, Library, NowPlaying}`, `install_slot(slot, view, cx)`, `navigate(Route, cx)`, `current_route(cx)`, `settings(cx)`, `update_settings(cx, |s| …)`, `shell(cx)` phát `ShellEvent`.
- **Vùng nội dung**: đục (`surface`), cao = cửa sổ trừ thanh trên 36 px và thanh phát 90 px; slot nằm trong `div` `flex_1 min_h_0`, view dùng `size_full()`.
- **Icon sẵn**: `GrooveIcon::{DefaultCover, Folder, Delete, Play, Album, Song}`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 15. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).