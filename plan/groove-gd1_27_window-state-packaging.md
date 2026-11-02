# groove-gd1_27 — window-state-packaging

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~500 dòng |
| Phụ thuộc | groove-gd1_25 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Không (`embed-resource` đã khai ở task 01) |
| File đụng tới | `app/window_state.rs`, `packaging/*`; ở repo gốc: `build.ps1`, `build.sh`, `init.*`, `start.*` (nhánh groove) |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 27. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_25 (02/11/2026)

<!-- handoff: groove-gd1_25 -->

- **Đã có sẵn để dùng**: nhận tệp xong hết ở `open_paths`: `argv` (nhiều tệp một tiến trình hay nhiều tiến trình đều đúng) và `on_open_urls` trên macOS. Task này chỉ khai liên kết theo đặc tả 5.6: NSIS ProgID, `OpenWithProgids`; `CFBundleDocumentTypes`; `.desktop` `MimeType`, `Exec=groove %F`. Danh sách đuôi: `groove_core::model::AUDIO_EXTENSIONS`.
- **Đổi so với lúc viết spec này**: `app::run` gọi `open_paths::run_app(application, closure)` thay cho `.run(closure)`; lời gọi của task này vẫn thêm một dòng trong closure như cũ.
- **Khi đóng gói**: phiên sau kiểm exe phiên chính cùng thư mục (`instance::sys::prepare_server`); bản cài ở thư mục khác bản đang chạy thì không chuyển được, app mở phiên riêng sau 5 giây. Bộ cài gỡ, cập nhật phải đóng phiên cũ trước.
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
## 2b. Phân vai

Task `opus`, không tách subagent: `[opus]` toàn bộ `window_state.rs`, `build.rs`, `packaging/*`, nhánh groove ở repo gốc. `[owner-qa]`: nhìn thật trên máy hai màn, macOS, Ubuntu (`plan/_backlog.md`).

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | Màn đã lưu còn, vùng kéo trên màn ⇒ về đúng chỗ, đúng màn | `[opus-unit]` `keeps_position_on_saved_display` |
| C2 | Id màn đổi (HMONITOR sau khởi động lại), vị trí còn trên màn ⇒ giữ chỗ | `[opus-unit]` `stale_display_id_…` |
| C3 | Màn đã rút ⇒ giữa màn chính, giữ cỡ | `[opus-unit]` `missing_display_…` |
| C4 | Ngoài vùng, biên 100 × 16 px của vùng kéo ⇒ giữa màn chính | `[opus-unit]` `off_screen_…`, `visible_strip_boundary` |
| C5 | Vắt ngang hai màn ⇒ màn đã lưu, không thì màn chứa nhiều hơn | `[opus-unit]` `straddling_…` |
| C6 | Wayland ⇒ chỉ giữ cỡ | `[opus-unit]` `wayland_keeps_only_size` |
| C7 | Cỡ quá nhỏ, quá lớn, số rác, không có màn ⇒ kẹp, bỏ qua, không panic | `[opus-unit]` `sizes_are_clamped_…`, `no_display_still_plans` |
| C8 | Phóng to, toàn màn ⇒ lưu bounds thường; mở lại phóng to, cả khi màn mất | `[opus-unit]` `placement_keeps_restore_bounds`, `maximized_survives_fallback` |
| C9 | Cửa sổ ảo mở bằng `gpui_kit::open_window`, đổi cỡ ⇒ báo vị trí mới kèm id màn | `[opus-unit]` `resize_reports_placement` |
| C10 | Khai báo gói khớp `AUDIO_EXTENSIONS`, `Exec=groove %F`, G-2 | `[opus-unit]` `tests/packaging.rs` |
| C11 | Bản cài Windows: cài, liên kết, không đổi mặc định, exe bị giữ thì dừng mã 2, gỡ sạch | `[opus-run]` `packaging/windows/installer-e2e.ps1` |
| C12 | Mở, kéo, đóng, mở lại trên màn thật; `.app`; `.deb` | `[owner-qa]` |

## 8. Nợ chuyển task khác

- Task 29: `src-tauri` gỡ được, icon đã chép sang `crates/groove-app/assets/app`, `packaging/macos`, `packaging/linux`.

## 9. Bàn giao thực tế

- **Lưu, mở vị trí**: `window_state::restore(options, cx)` là lời gọi duy nhất thêm vào closure của `run_app`, đứng trước `open_window` vì GPUI không có lệnh dời cửa sổ sau khi mở. Nó tự đăng ký `observe_new::<Shell>` để lưu, nên không cần lời gọi thứ hai.
- **Lưu mọi lần đổi vào `Settings`, không chỉ lúc đóng**: nút đóng ở `views/app_bar.rs:127` đi `remove_window`, không qua `on_window_should_close`. Ghi đĩa do `JsonSaver` gom, xả lúc thoát.
- **`--display` thắng và không ghi đè cài đặt**: cửa sổ thử không làm bẩn vị trí thật.
- **Gói**: `packaging/windows/{groove.nsi,build-installer.ps1,installer-e2e.ps1}`, `packaging/macos/{Info.plist,build-macos.sh,groove.icns}`, `packaging/linux/{groove.desktop,build-deb.sh,groove.png}`, `packaging/licenses/*`. `crates/groove-app/build.rs` nhúng icon exe. Repo gốc: `build.ps1 -Project groove`, `bash build.sh --project groove`, `start.* groove`; `init.*` đã có groove từ trước, không sửa.
- **Đã chạy trên Windows, cây đã gộp `master`**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (lần 1 đỏ `player::tests::volume_and_mute_reach_the_engine`; lần 2 đỏ ba test `engine::tests` khi máy đang tải; lần 3 xanh hết; không sửa, đúng lớp test chập chờn agent khác đang xử lý), `cargo deny check licenses`, dựng bản cài thật, `installer-e2e.ps1` xanh 27 mục.
- **Chưa chạy**: mở cửa sổ thật (máy một màn, `tools/run.ps1` từ chối), `build-macos.sh`, `build-deb.sh` (chỉ `bash -n`), `plutil`, `dpkg-deb`, cài `.deb`, `.dmg`, nhấp đúp tệp sau khi cài. Luồng `restore` với `update_settings` thật không có test tự động vì `install_settings` là hàm riêng của `app.rs`; test C9 thay bằng hàm lưu giả.
- **Soi cải tiến**: bảo mật: lệnh mở bọc ngoặc kép `"%1"`, chỉ ghi HKCU. UI/UX: phiên đang chạy khi cài thì hỏi đóng, không giết. RAM: không thêm luồng, không thêm phụ thuộc. Pro: không áp.
- **Opus chọn 02/11/2026**: xem các dòng `groove-gd1_27` ở `plan/confirm.md`.

## 10. Đề xuất cải tiến

Không có đề xuất ngoài phạm vi đáng ghi.
