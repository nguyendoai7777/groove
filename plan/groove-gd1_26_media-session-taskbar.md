# groove-gd1_26 — media-session-taskbar

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~550 dòng |
| Phụ thuộc | groove-gd1_22 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/platform/{media_session,taskbar_windows}.rs`, `i18n/*/platform.toml` |

## 0. Bàn giao từ groove-gd1_22 (02/11/2026)

<!-- handoff: groove-gd1_22 -->

- **Đã có sẵn để dùng**: `crate::commands::run(Command, cx)`; `Command::{PrevTrack, TogglePlay, NextTrack}` ở `groove_core::keymap`. Dispatcher chung với phím tắt, gọi từ UI thread: sự kiện SMTC, `WM_COMMAND` phải chuyển về UI thread trước.
- **Không có trong `Command`**: Play, Pause tách riêng, tua tuyệt đối. Media session gọi thẳng `player(cx).update(cx, |p, cx| p.play(cx))`, `pause`, `seek`: `Command` là danh sách lệnh gán phím, thêm vào là lộ ra JSON phím tắt.
- **Cần biết**: `commands::run` không qua luật ngữ cảnh M4, nên phím media vẫn chạy khi dialog đang mở, đúng ý vì chúng đến từ OS.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 26. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).