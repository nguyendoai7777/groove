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
- [groove-gd1_11] Preamp theo đỉnh đáp ứng thật của chuỗi 10 dải — `-max(gain dương)` như S23 — toàn +10 dB đo được đỉnh +15,06 dB, công thức S23 vẫn rè; một dải thì hai cách như nhau.

## groove-gd1_04

- [groove-gd1_04] Thêm `songs.path_key` UNIQUE, bỏ UNIQUE trên `file_path` — giữ cả hai — M2 cần, cột kia thừa. Đặc tả 2.2 chưa ghi cột này.
- [groove-gd1_04] DB cũ trùng `path_key` thì gộp về `id` nhỏ nhất, lấp lyrics, timeline — dừng chuyển đổi — trùng do khác hoa thường là chính lỗi M2, dừng thì người dùng kẹt. Ngoại lệ duy nhất của "giữ nguyên `id`".
- [groove-gd1_04] Chụp bằng `VACUUM INTO` chỉ đọc — chép tệp như đặc tả ghi — vẫn nhất quán khi bản Tauri đang mở DB.
- [groove-gd1_04] `position` playlist cũ theo tên tệp — theo thứ tự thêm — bản cũ hiện theo tên tệp, không lưu thứ tự thêm.
- [groove-gd1_04] DB cũ hỏng trả lỗi cho caller, không tự tạo thư viện trống — tự tạo rồi im lặng — tạo rồi thì không bao giờ thử chuyển lại.
- [groove-gd1_04] `search_key` giữ dấu câu theo S18 — bỏ như bản cũ — `AC/DC` không còn khớp `acdc`, đổi lại `-` không khớp tất cả.

## groove-gd1_12

- [groove-gd1_12] Tắt shuffle: queue = phần sau bài hiện tại, rồi vòng về các bài phía trước chưa nghe, cuối cùng là bài ngoài original — xếp đúng thứ tự original từ đầu — Next vẫn ra bài kế như bảng 1.4, mà S4 không mất bài phía trước.
- [groove-gd1_12] Bật shuffle: bài chưa phát của original cộng bài đang chờ ngoài original — chỉ xáo phần queue hiện có như câu đầu S4 — bảng 1.4 và dòng "phát kèm danh sách" đều lấy mọi bài còn lại, đồng thời không mất bài thêm tay.
- [groove-gd1_12] `append` không đụng original, vòng lặp repeat không gồm bài thêm tay — thêm cả vào original — bảng 1.4 ghi nạp lại từ original, và original là danh sách nguồn.
- [groove-gd1_12] Repeat ở cuối khi original rỗng (toàn tệp ngoài): vòng qua history + bài hiện tại — xoá history, chỉ lặp bài hiện tại — bảng 1.4 không có ca này, bản cũ thoát sớm (N6), xoá thì mất bài đã nghe.
- [groove-gd1_12] Dòng timeline bốn phần thời gian trở lên hoặc phần rỗng (`1:2:3:4 x`, `:30 x`) bị bỏ qua — coi là đoạn ở giây 0 hoặc giây 30 như bản cũ — đặc tả 1.6 ghi dòng khác bỏ qua.

## groove-gd1_07

- [groove-gd1_07] `hash` là SHA-256 byte ảnh gốc — băm điểm ảnh sau giải mã — tính được trước khi giải mã nên tránh giải mã lặp; cùng ảnh khác mã hoá sẽ thành hai bộ tệp, chấp nhận.
- [groove-gd1_07] Tệp bìa giữ tỉ lệ, không phóng to, `_512.jpg` có thể nhỏ hơn 512 — cắt vuông hoặc phóng to cho đúng cỡ — cắt là việc của lớp hiển thị, phóng to chỉ tốn đĩa.
- [groove-gd1_07] Trần tệp vào 64 MiB, giải mã 192 MiB (RGB 8000 × 8000), cạnh 16 384 — trần thấp hơn để bớt RAM — bìa quá trần sẽ mất, đổi trần chỉ sửa một hằng.
- [groove-gd1_07] Accent tính theo alpha như canvas cũ, JPEG phủ alpha lên nền đen — coi trong suốt là đen khi tính màu — logo nền trong suốt vẫn ra đúng màu logo.
