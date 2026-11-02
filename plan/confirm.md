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

## groove-gd1_03

- [groove-gd1_03] Cỡ cửa sổ nhỏ nhất 560 × 360 — không đặt trần như bản Tauri (800 × 600 mặc định, không ghi cỡ nhỏ nhất) — cột trái mở 230 px + ô tìm + ba nút cửa sổ cần chỗ, không có trần thì kéo nhỏ là vỡ bố cục.
- [groove-gd1_03] Nền cột trái dùng `translucent_surface` (alpha 0,88 tông tối, tông sáng đục) — alpha 0,4 như `--nav-bg` bản cũ — materiant đã kiểm tương phản chữ ≥ 4,5:1 trên nền trắng xấu nhất ở 0,88; 0,4 trượt. Đổi thì sửa một dòng ở `views/side_nav.rs`.
- [groove-gd1_03] Tên mục: Thư viện, Đang phát, Danh sách phát, Cài đặt (en: My Music, Now Playing, Playlists, Settings) — dịch sát từng chữ "Nhạc của tôi" — khớp tên lệnh `go_to_library` ở đặc tả 1.9 và ngắn hơn trong cột 56 / 230 px.
- [groove-gd1_03] Tiêu đề cửa sổ `<trang> | Groove X` — `GrooveX` liền như dòng Now Playing của bản cũ — đặc tả 1.1 ghi "theo trang"; tên app lấy từ `common.app_name` để một chỗ.
- [groove-gd1_03] Một `Slot::Library` cho My Music, Playlists và danh sách bài của category — slot riêng cho từng route — đặc tả 1.2: hai route chung một màn với `mode`; view nghe `RouteChanged` để chọn lưới hay danh sách.
- [groove-gd1_03] Nút đóng bằng `window.remove_window()` + `cx.on_window_closed` thoát app — hit-test `WindowControlArea::Close` của OS — bản này chạy cùng một mã trên ba OS và click trong GPUI test được; mất Snap Layouts của nút phóng to trên Windows 11.
