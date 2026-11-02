# groove-gd1_13 — player-controller

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_10, groove-gd1_11, groove-gd1_12 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/player.rs`, `app/session_sync.rs` (N5, N6, N8) |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 13. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N5, N6, N8 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).