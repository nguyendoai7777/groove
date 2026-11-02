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

## 0. Bàn giao từ groove-gd1_24 (02/11/2026)

<!-- handoff: groove-gd1_24 -->

- **Đã có**: `views/settings/*`, `settings::dialog::install` gọi trong `app::run`; `app.rs` có thêm `Slot::Settings` và hai hàm test `install_settings_for_test`, `set_shell_for_test`. Hộp có ba tab (Cài đặt chung, Phím tắt, Thư viện), nối `ShellEvent::OpenSettings` (nút đáy cột trái, lệnh).
- **Đổi so với đặc tả 1.10**: thêm tab Thư viện (danh sách gốc, Thêm, Quét lại, Bỏ); Esc và ✕ coi như Huỷ; bước tua, bước âm lượng áp ngay như EQ; không có ô ngôn ngữ, theme; ô JSON không gợi ý tên lệnh, thay bằng danh sách lệnh dưới ô. Danh sách ở mục "Quyết định tự chọn" của mục 9 và `plan/confirm.md`.
- **Chưa kiểm**: nhìn hộp thật (đường cong khớp núm trượt, bố cục 800 × 480, chữ vi / en) — `[owner-qa]` ở `_backlog.md`. Cần ghi vào bảng so bản cũ.
- **Bảng so bản cũ**: S6 / N9 (bỏ gốc) đã có đường vào từ Settings; M3, M5, M7 đã làm; S23 (preamp) nghe được ngay khi kéo.
- **Số đo**: hộp dựng một lần lúc khởi động, chưa đo RAM; đo cùng lượt số đo RAM của task này.

## 0. Bàn giao từ groove-gd1_23 (02/11/2026)

<!-- handoff: groove-gd1_23 -->

- **Đã có**: `views/command_palette.rs`, `command_palette::install` gọi trong `app::run`; `app.rs` có thêm `Slot::CommandPalette`.
- **Chưa kiểm**: nhìn bảng thật và `ShellEvent::OpenSearch` nối thật từ ô tìm, `Ctrl+K` (`[owner-qa]` ở `_backlog.md`).
- **Cần biết**: khung đệm 24 px của `Dialog` là hằng của materiant, bảng lệnh hẹp hơn bản cũ một chút; muốn khít thì đề xuất tuỳ chọn đệm cho `Dialog`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 29. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_26 (02/11/2026)

<!-- handoff: groove-gd1_26 -->

- **Đã có sẵn để dùng**: icon taskbar và logo đã chép vào `crates/groove-app/assets/{taskbar,media}`, gỡ `src-tauri` không ảnh hưởng.
- **Soát lại khi làm**: ba mục `[groove-gd1_26 · owner-qa]` ở `plan/_backlog.md`; biên dịch trọn `groove-app` trên macOS, Linux; đo RAM có thêm hai luồng `taskbar`, `media-session`.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.
- Sửa `main.rs` (nợ từ `groove-gd1_25`): `std::env::args()` panic nếu `argv` không phải Unicode, phiên đầu mở tệp tên hỏng là sập. Đổi sang `args_os` + `to_string_lossy`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
