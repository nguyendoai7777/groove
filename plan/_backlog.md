# Backlog — việc treo không làm được trong lượt

Mỗi gạch một việc + LÝ DO chưa làm được (thiếu máy, dịch vụ trả phí, chờ quyết định — D-8: đề xuất đã xác nhận thì làm
ngay, không vào đây). Có task thì trỏ tới tệp task, đừng chép lại. `run-task` mục 5, `close-phase` mục 3 ghi thêm.

## groove

- **[viết lại · owner-qa] Soát nền mờ sidenav trên macOS sau khi viết lại bằng GPUI (G-1)** — lý do: bản Tauri mờ đúng trên Windows, nhưng trên macOS vùng đáng ra mờ lại xám trắng; không debug bản Tauri vì sẽ bỏ, và máy Windows không kiểm được. Dự đoán từ đọc mã, chưa chạy thử: GPUI (`gpui-pre-macos` 0.3.10 `window.rs`, `blurred_view_update_layer`) bóc màu phủ của `NSVisualEffectView` nên sẽ hết xám. Bước: cửa sổ `WindowBackgroundAppearance::Blurred`, sidenav nền alpha, vùng nội dung đục; chạy trên Mac ở cả chế độ sáng / tối của hệ thống, trên hình nền sáng và tối. Mong đợi: thấy desktop nhoè, không ngả xám trắng, chữ sidenav đọc được, mờ không lem ra ngoài góc bo cửa sổ. Còn xám ⇒ sửa ngay trong task viết lại (D-11).
