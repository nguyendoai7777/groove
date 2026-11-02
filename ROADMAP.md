# groove — lộ trình

Viết lại trình phát nhạc Groove X từ Tauri + Vue sang Rust + GPUI + materiant (G-1). Đặc tả, lỗi cần sửa, bảng task và làn
song song: `docs/rewrite-spec.md`. Task: `plan/groove-gd1_*.md`, trạng thái ở từng tệp task.

## GĐ1 — viết lại (02/11/2026)

| Task | Trạng thái |
| --- | --- |
| 01-27 | ✅, đã gộp `master` |
| 28 `localstorage-import` (tuỳ chọn) | ☐ chưa làm: bản mới chạy với mặc định, `legacy_ls.rs` là tệp rỗng, phụ thuộc `rusty-leveldb` đã khai |
| 29 `acceptance-remove-tauri` | ☐ (quy ước không có trạng thái "chờ") mã, bảng so bản cũ (`docs/parity.md`), số đo RAM đã gộp; gỡ Tauri nằm ở nhánh `claude/remove-tauri`, chờ chủ dự án xem app thật |

Còn lại trước khi đóng GĐ1:

1. Chủ dự án xem app thật ở máy hai màn: các dòng `[owner-qa]` ở `plan/_backlog.md`, bắt đầu từ dòng đầu tiên.
2. Gộp `claude/remove-tauri` (lệnh ở dòng `[owner-qa]` đầu tiên), đánh dấu task 29 ✅, rồi `close-phase`.
3. macOS, Ubuntu: dựng gói, chạy test một phiên, media session (các dòng `[groove-gd1_25]`, `[groove-gd1_26]`, `[groove-gd1_27]`).

## Sau GĐ1 (chưa lập task)

- Task 28 nếu chủ dự án muốn giữ EQ, phím tắt của bản Tauri.
- Giao diện xoá bài mất vĩnh viễn trước hạn 30 ngày (`reconcile::forget_missing` đã có, chưa có đường vào từ UI; `plan/confirm.md`).
- Đo RAM với thư viện thật và khi đang phát (`docs/parity.md`, mục Số đo RAM).
