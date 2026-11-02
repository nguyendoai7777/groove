# groove-gd1_08 — indexer-import

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_06, groove-gd1_07 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/{scanner,indexer}.rs` |

## 0. Bàn giao từ groove-gd1_07 (02/11/2026)

<!-- handoff: groove-gd1_07 -->

- **Đã có sẵn để dùng**: `groove_library::covers::CoverStore`: `default_location()`, `save(&self, &[u8]) -> Result<CoverRecord, CoverError>`, `has_files(hash)`, `path(hash, CoverSize)`; hàm `cover_hash(&[u8])`. `Clone + Send + Sync`, gọi được từ worker. API đủ ở `plan/groove-gd1_07_covers.md` mục 9.
- **Phải làm**: `save` không chạm DB. Indexer tự `INSERT OR IGNORE INTO covers` từ `CoverRecord` và gán `cover_hash`, ở `LibraryActor` như mọi lệnh ghi.
- **Tránh giải mã lặp**: album 12 bài có 12 bản bìa giống nhau. Tính `cover_hash` trước; đã có dòng `covers` và `has_files(hash)` đúng thì bỏ qua `save`. Cache có thể bị xoá trong khi DB còn dòng, nên vẫn phải soát `has_files`.
- **Giữ RAM**: mỗi `save` đỉnh bằng một lần giải mã, trần 192 MiB. Chỉ cho một luồng giải mã bìa tại một thời điểm, không chạy theo số worker đọc thẻ. Gọi ngoài transaction.
- **Ảnh hỏng**: `save` trả `Err` kể cả khi decoder panic, không cần bọc `catch_unwind`. `Err` chỉ mất bìa, bài vẫn vào thư viện với `cover_hash = NULL`, ghi `log::warn!`.
- **Bìa album**: `save` nhận byte gốc, ảnh nhỏ không phóng to nên `_512.jpg` có thể nhỏ hơn 512 và không vuông.
- **Soát lại khi làm**: `cargo test -p groove-library --test covers_migrate`; nhập lại một thư mục không tăng số tệp ở thư mục bìa.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 08. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).