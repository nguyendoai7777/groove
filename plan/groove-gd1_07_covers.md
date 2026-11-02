# groove-gd1_07 — covers

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~350 dòng |
| Phụ thuộc | groove-gd1_04 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/covers.rs` |

## 0. Bàn giao từ groove-gd1_04 (02/11/2026)

<!-- handoff: groove-gd1_04 -->

- **Phải làm**: `impl db::migrate::CoverSink`: `store(&mut self, image: &[u8]) -> anyhow::Result<CoverRecord>`, ghi `<hash>_<256|512>.jpg`, trả `CoverRecord { hash, accent_color, width, height }`. Migration tự chèn dòng `covers` và gán `cover_hash`, sink không chạm DB.
- `image` là byte ảnh gốc đã giải base64, có thể hỏng: trả `Err`. Migration bắt cả lỗi và panic, chỉ mất bìa dòng đó.
- **Bảng đã có**: `covers(hash PK, accent_color NN, width, height)`, `cover_hash` ở `songs`, `albums`, `folders`, `albums.cover_is_custom`.
- **Soát lại khi làm**: sink chạy trong transaction của migration nên đừng mở kết nối DB khác; `covers_dir()` có thể chưa tồn tại, tự tạo. Test mẫu: `crates/groove-library/tests/migrate_legacy.rs`.
Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 07. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).