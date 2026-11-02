# groove-gd1_18 — metadata-ops

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~600 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_06 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/metadata_ops.rs` (N2, N3, S5, S7, S8) |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Hàm cho task này**: `Library::song_text(id)`, `song_ids(CategoryRef)`; `queries::{set_song_lyrics, set_song_timeline, set_song_file_state, move_song, upsert_song}` qua `writer().transaction`. Ghi tệp xong mới gửi transaction, không ghi tệp trong closure (N7).
- **Bẫy**: `upsert_song` giữ lyrics, timeline DB khi tệp rỗng, nên muốn xoá thật thì dùng `set_song_lyrics(.., None)`, `set_song_timeline(.., None)`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 18. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N2, N3, S5, S7, S8 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).