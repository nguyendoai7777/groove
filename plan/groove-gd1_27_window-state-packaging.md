# groove-gd1_27 — window-state-packaging

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~500 dòng |
| Phụ thuộc | groove-gd1_25 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/window_state.rs`, `packaging/*`; ở repo gốc: `build.ps1`, `build.sh`, `init.*`, `start.*` (nhánh groove) |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 27. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_25 (02/11/2026)

<!-- handoff: groove-gd1_25 -->

- **Đã có sẵn để dùng**: nhận tệp xong hết ở `open_paths`: `argv` (nhiều tệp một tiến trình hay nhiều tiến trình đều đúng) và `on_open_urls` trên macOS. Task này chỉ khai liên kết theo đặc tả 5.6: NSIS ProgID, `OpenWithProgids`; `CFBundleDocumentTypes`; `.desktop` `MimeType`, `Exec=groove %F`. Danh sách đuôi: `groove_core::model::AUDIO_EXTENSIONS`.
- **Đổi so với lúc viết spec này**: `app::run` gọi `open_paths::run_app(application, closure)` thay cho `.run(closure)`; lời gọi của task này vẫn thêm một dòng trong closure như cũ.
- **Khi đóng gói**: phiên sau kiểm exe phiên chính cùng thư mục (`instance::sys::prepare_server`); bản cài ở thư mục khác bản đang chạy thì không chuyển được, app mở phiên riêng sau 2 giây. Bộ cài gỡ, cập nhật phải đóng phiên cũ trước.
- **Bẫy đã gặp**: chạy thử không kèm `GROOVE_DATA_DIR` thì mọi bản debug chung endpoint `-dev`, lần chạy sau chỉ đánh thức lần trước rồi thoát. `activate_window` của GPUI bấm giả phím Alt.
- **Soát lại khi làm**: kiểm tay `groove-gd1_25` ở `plan/_backlog.md`, mục macOS cần bản `.app` của task này.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `Settings::window: Option<WindowPlacement { x, y, width, height, maximized, display_id }>`; `display_id` là `u64::from(DisplayId)`. Task này chỉ đọc ghi trường đó, không sửa `settings.rs`.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).