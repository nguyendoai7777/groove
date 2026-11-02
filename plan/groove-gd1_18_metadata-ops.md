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

## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đã có sẵn để dùng**: `tags::write_tags(path, &MetadataEdit, WriteMode) -> FileStamp` đã ghi qua `<tên>.groove-tmp`, đọc lại kiểm tra, thay tệp, xoá tạm khi lỗi; `read_track`, `read_raw_frames`. Chi tiết: mục 9 của `groove-gd1_06`.
- **Quy ước sửa**: `None` không đụng, `Some("")` xoá. `CoverEdit::Keep` giữ bìa ở mọi chế độ. `TIMELINE` tự được giữ ở mọi chế độ.
- **Còn lại ở task này (N2, N3)**:
  - `Clean` không có lyrics trong phần sửa là mất lyrics: phải đọc từ DB (tệp ngoài thư viện: từ `read_track`) rồi truyền vào `fields.lyrics`.
  - DB có timeline mà tệp không có: truyền `fields.timeline` để đẩy ngược vào tệp.
  - Kiểm tra tên mới, đổi tên, cập nhật DB trong transaction chưa làm; `write_tags` chỉ thay nội dung, đường dẫn giữ nguyên.
  - Lưu `FileStamp` trả về vào DB để watcher bỏ qua (S6).
- **Lỗi cho người dùng**: `TagError` hiển thị bằng tiếng Anh để ghi log; toast tra theo biến thể qua i18n. `Replace` là tệp đang bị app khác giữ, `ReadOnly`, `CoverTooLarge`, `UnsupportedCover`, `Verify` nên có câu riêng.
- **Bẫy**: bìa quá 16 MiB trên FLAC là `TagError::Write` (trần định dạng).
- **Soát lại khi làm**: `cargo test -p groove-library tags`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 18. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N2, N3, S5, S7, S8 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).