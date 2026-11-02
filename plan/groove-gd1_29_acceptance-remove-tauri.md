# groove-gd1_29 — acceptance-remove-tauri

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~100 dòng |
| Phụ thuộc | tất cả |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | Bảng so bản cũ, số đo RAM, gỡ `src/`, `src-tauri/`, Node; viết `groove/CLAUDE.md`, `groove/ROADMAP.md` |

## 0. Bàn giao từ groove-gd1_23 (02/11/2026)

<!-- handoff: groove-gd1_23 -->

- **Đã có**: `views/command_palette.rs`, `command_palette::install` gọi trong `app::run`; `app.rs` có thêm `Slot::CommandPalette`.
- **Chưa kiểm**: nhìn bảng thật và `ShellEvent::OpenSearch` nối thật từ ô tìm, `Ctrl+K` (`[owner-qa]` ở `_backlog.md`).
- **Cần biết**: khung đệm 24 px của `Dialog` là hằng của materiant, bảng lệnh hẹp hơn bản cũ một chút; muốn khít thì đề xuất tuỳ chọn đệm cho `Dialog`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 29. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.
- Sửa `main.rs` (nợ từ `groove-gd1_25`): `std::env::args()` panic nếu `argv` không phải Unicode, phiên đầu mở tệp tên hỏng là sập. Đổi sang `args_os` + `to_string_lossy`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).