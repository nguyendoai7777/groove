# groove-gd1_12 — queue-timeline

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~550 dòng |
| Phụ thuộc | groove-gd1_02 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `core/{queue,timeline}.rs` (bảng 1.4, S4, S20) |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 12. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `groove_core::model::{TrackRef, RepeatMode}`, không khai lại trong `queue.rs`. `TrackRef` băm và so sánh theo đường dẫn chuẩn hoá, dùng được làm khoá `HashSet` (N6).
- **Khớp với phiên**: `session::Session { current, position_sec, original, history, queue, shuffle, repeat }` là dạng lưu; `Session::sanitize` đã giữ bất biến mỗi bài một lần và shuffle, repeat không cùng bật. Hàng đợi của task này nên dựng được từ các trường đó và ngược lại.
- **Chi tiết**: `plan/groove-gd1_02_settings-session.md` mục 9.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi S4, S20 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).