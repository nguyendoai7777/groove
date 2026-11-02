# Chờ chủ dự án xác nhận

Việc **đã làm** trong lúc chủ dự án vắng (ngủ / bận) mà Claude không chắc chủ dự án đồng ý — đổi API, lệch bản web có
chủ ý, đánh đổi RAM / CPU, đổi luật, chọn phương án thay chủ dự án. Mỗi gạch: `- [<task>] <việc đã chọn> — <phương án
khác> — <vì sao chọn>`. Chủ dự án đọc: đồng ý ⇒ xoá gạch (quyết định lớn thì chép vào `docs/decisions.md`); không đồng
ý ⇒ ghi lại yêu cầu ngay dưới gạch để session sau làm lại.

Session thêm gạch bằng `Edit` chèn vào mục của task mình (không `Write` cả tệp — nhiều session cùng ghi).

## groove-gd1_01

- [groove-gd1_01] `cpal` optional trên Linux sau feature `linux-alsa`, tắt mặc định: bản Linux chưa có tiếng tới khi chủ dự án quyết `libasound` (LGPL) — khai thẳng cho mọi OS — chưa được phép link LGPL khi chưa hỏi. Cần chủ dự án quyết trước task 10.
- [groove-gd1_01] Khai thêm phụ thuộc ngoài mục 5 của đặc tả (`base64`, `sha2`, `unicode-normalization`, `crossbeam-channel`, `futures`, `anyhow`, `thiserror`, `rusty-leveldb`, `raw-window-handle`, `objc2*`, `embed-resource`) — để task sau tự thêm — Luật song song cấm task sau sửa `Cargo.*`.
- [groove-gd1_01] `app/app.rs` là ngoại lệ của Luật song song: task sau thêm đúng một lời gọi khởi tạo — mỗi task một hook rỗng khai sẵn — chưa có API thật để khai, xung đột một dòng dễ giải.

## groove-gd1_02

- [groove-gd1_02] Theme mặc định `dark` — `system` như materiant — bản cũ chạy tối cố định, giữ cảm giác khi chuyển sang.
- [groove-gd1_02] `customKeybindings = null` là dùng bảng mặc định — ghi cả bảng vào tệp — phím mặc định mới của bản sau tới được người chưa từng sửa.
- [groove-gd1_02] Tệp cấu hình sai kiểu một khoá thì chỉ mất khoá đó, tệp hỏng hẳn giữ lại thành `.bad` — về mặc định và ghi đè — không mất cấu hình âm thầm. Thêm so với đặc tả.
- [groove-gd1_02] Thêm `settings::JsonSaver` (luồng nền, debounce) ở core — để từng task app tự viết — task 03, 13, 24 đều cần và N7, S12 yêu cầu ghi nền.

## groove-gd1_04

- [groove-gd1_04] Thêm `songs.path_key` UNIQUE, bỏ UNIQUE trên `file_path` — giữ cả hai — M2 cần, cột kia thừa. Đặc tả 2.2 chưa ghi cột này.
- [groove-gd1_04] DB cũ trùng `path_key` thì gộp về `id` nhỏ nhất, lấp lyrics, timeline — dừng chuyển đổi — trùng do khác hoa thường là chính lỗi M2, dừng thì người dùng kẹt. Ngoại lệ duy nhất của "giữ nguyên `id`".
- [groove-gd1_04] Chụp bằng `VACUUM INTO` chỉ đọc — chép tệp như đặc tả ghi — vẫn nhất quán khi bản Tauri đang mở DB.
- [groove-gd1_04] `position` playlist cũ theo tên tệp — theo thứ tự thêm — bản cũ hiện theo tên tệp, không lưu thứ tự thêm.
- [groove-gd1_04] DB cũ hỏng trả lỗi cho caller, không tự tạo thư viện trống — tự tạo rồi im lặng — tạo rồi thì không bao giờ thử chuyển lại.
- [groove-gd1_04] `search_key` giữ dấu câu theo S18 — bỏ như bản cũ — `AC/DC` không còn khớp `acdc`, đổi lại `-` không khớp tất cả.
