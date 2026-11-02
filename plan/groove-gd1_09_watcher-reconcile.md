# groove-gd1_09 — watcher-reconcile

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~700 dòng |
| Phụ thuộc | groove-gd1_08 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/{watcher,reconcile}.rs` (N1, N4, S6) |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Đối chiếu**: `Library::song_file_states()`, `Library::watched_roots()`; ghi qua `writer().transaction` với `queries::{move_song, delete_song_by_path, set_song_file_state, upsert_song}`. `move_song` giữ `id`, đích là bài khác thì `QueryError::PathConflict`. Phát `LibraryEvent::Moved` sau commit.
- **Thiếu trong schema**: N1 cần `missing_since`, N9 cần `excluded_paths`, `schema.rs` (04) ở `user_version = 1` chưa có. Task này cần `missing_since` thì phải thêm cột và cách nâng phiên bản; `migrate::open_existing` hiện chỉ nhận đúng các phiên bản đã biết.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 09. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N1, N4, S6 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).