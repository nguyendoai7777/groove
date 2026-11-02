# Backlog — việc treo không làm được trong lượt

Mỗi gạch một việc + LÝ DO chưa làm được (thiếu máy, dịch vụ trả phí, chờ quyết định — D-8: đề xuất đã xác nhận thì làm
ngay, không vào đây). Có task thì trỏ tới tệp task, đừng chép lại. `run-task` mục 5, `close-phase` mục 3 ghi thêm.

## groove

- **[viết lại · owner-qa] Soát nền mờ sidenav trên macOS sau khi viết lại bằng GPUI (G-1)** — lý do: bản Tauri mờ đúng trên Windows, nhưng trên macOS vùng đáng ra mờ lại xám trắng; không debug bản Tauri vì sẽ bỏ, và máy Windows không kiểm được. Dự đoán từ đọc mã, chưa chạy thử: GPUI (`gpui-pre-macos` 0.3.10 `window.rs`, `blurred_view_update_layer`) bóc màu phủ của `NSVisualEffectView` nên sẽ hết xám. Bước: cửa sổ `WindowBackgroundAppearance::Blurred`, sidenav nền alpha, vùng nội dung đục; chạy trên Mac ở cả chế độ sáng / tối của hệ thống, trên hình nền sáng và tối. Mong đợi: thấy desktop nhoè, không ngả xám trắng, chữ sidenav đọc được, mờ không lem ra ngoài góc bo cửa sổ. Còn xám ⇒ sửa ngay trong task viết lại (D-11).
- **[groove-gd1_01 · owner-qa] Mở cửa sổ ở màn DELL** — lý do: lúc làm task máy chỉ có một màn. Bước: có màn phụ, `cargo build -p groove-app`, `pwsh tools/run.ps1`. Mong đợi: cửa sổ ở màn phụ, `foreground_unchanged=True`.
- **[groove-gd1_10 · owner-qa] Nghe thật engine âm thanh** — lý do: test tự động không phát ra loa, không đổi thiết bị trên máy dùng thật. Bước và mong đợi: `plan/groove-gd1_10_audio-engine.md` mục 9.
- **[groove-gd1_10 · owner-qa] Linux có tiếng trên Ubuntu (G-2)** — lý do: `libasound` đã chấp nhận và bật mặc định nhưng chưa chạy thử, chưa coi Linux là đạt; từ Windows không kiểm được. Bước: máy Ubuntu, `libasound2-dev`, lệnh `audio_check play` ở mục 9 của task.

- **[groove-gd1_06 · owner-qa] Mở tệp đã ghi thẻ bằng Explorer và một player khác** — lý do: phải nhìn trên máy thật; thấy đúng album, bìa, ID3v2.3 hiện trong Explorer.

- **[groove-gd1_03 · owner-qa] Nhìn khung cửa sổ thật (C10)** — lý do: máy dev chỉ một màn, `tools/run.ps1` từ chối. Bước: máy hai màn, `cargo build -p groove-app` rồi `pwsh tools/run.ps1 -Keys "wait@500 shot@shell"` (xoá thư mục `-Out` sau khi xem). Mong đợi: cột trái mờ lộ desktop (Windows; macOS còn là mục viết lại ở trên), nội dung và thanh phát đục, kéo được thanh trên, thu nhỏ / phóng to / đóng chạy, nút đóng hover đỏ, nút thu cột trái nội suy 300 ms, chữ khử răng cưa xám do nền Blurred (đặc tả mục 4, dòng cuối) chấp nhận được hay cần tuỳ chọn tắt mờ.
