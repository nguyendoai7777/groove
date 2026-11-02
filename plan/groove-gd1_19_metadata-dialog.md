# groove-gd1_19 — metadata-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~850 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_18, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/metadata/{dialog,fields,cover_pane,raw_frames}.rs`, `i18n/*/metadata.toml` |

## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đổi so với bản cũ**: `RawFrame { id, description, value, known }` không còn `name`: nhãn khung tra theo `id` qua i18n vùng `metadata`, `description` là mô tả của `TXXX`. Số khung rác là số dòng `known == false`; `TXXX:TIMELINE` có `known == true`.
- **Bìa**: dùng `CoverEdit { Keep, Remove, Set(Vec<u8>) }`, không còn `data:` URI.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 19. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).