# groove-gd1_26 — media-session-taskbar

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
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

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 26. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
## 2b. Phân vai

Task `opus`, làm trọn một mình. `[owner-qa]`: nút taskbar thật, phím media thật, macOS, Linux.

## 6. Nghiệm thu

- C1 Cho bài đang phát, khi `Player` báo 4 lần mỗi giây, thì OS không nhận gì thêm; tua, dừng, đổi bài thì nhận ngay. `[opus-unit]` `Tracker`, `follows_the_real_player`.
- C2 Cho sự kiện OS Play, Pause, Toggle, Next, Previous, Stop, tua, khi tới thì về UI thread rồi vào `commands::run` hoặc `Player`. `[opus-unit]` `os_events_map_to_actions`, `actions_drive_the_player`.
- C3 Cho không có bài, thì Prev, Next tắt, nút giữa bật; đang phát nút giữa là Pause. `[opus-unit]` `buttons_follow_track_and_playback`.
- C4 Cho `WM_COMMAND` + `THBN_CLICKED`, `TaskbarButtonCreated`, `WM_DPICHANGED` trên cửa sổ ẩn, thì thành sự kiện đúng; huỷ cửa sổ thì nhả `Shared`. `[opus-unit]` `subclass_turns_messages_into_events_and_cleans_up`.
- C5 Cho icon `.ico`, thì nạp được ở 96-288 DPI, tệp hỏng không sập. `[opus-unit]`.
- C6 Cho SMTC thật, gọi từ luồng riêng trên HWND ẩn của luồng khác, thì nhận cả metadata và ảnh. `[opus-run]` `smtc_accepts_a_worker_thread` (`--ignored`).
- C7 Nút taskbar hiện và bấm được, phím media thật, macOS, Linux. `[owner-qa]` `plan/_backlog.md`.

## 9. Bàn giao thực tế

**Đã làm**: `platform::media_session::install(handle, cx)` là lời gọi duy nhất trong `app::run`, tự cài cả taskbar. Icon chép sang `crates/groove-app/assets/taskbar/`, logo cho bài không bìa ở `assets/media/no_cover.png`: task 29 gỡ `src-tauri` không ảnh hưởng.

**Luồng**: không gọi Shell, WinRT trên UI thread (bẫy "RefCell already borrowed"). Taskbar: STA riêng có vòng `GetMessage`, không thức khi nghỉ; subclass chỉ đẩy vào kênh. Media session: luồng riêng trên Windows, Linux; macOS ở main thread. Sự kiện OS về UI thread qua `cx.spawn`.

**Đã kiểm**: xem mục 6. Hai test `--ignored`: SMTC đã chạy xanh trên cửa sổ ẩn; `thumb_buttons_on_a_real_window` chưa chạy vì mở cửa sổ thật. Trên Windows, `app` chạy thật chưa mở (máy một màn).

**Chưa kiểm được**: `ThumbBarAddButtons` thành công trên cửa sổ có nút taskbar (cửa sổ ẩn chỉ cho Shell trả lỗi, nên thứ tự vtable hai hàm này chưa được xác nhận bằng chạy thật); macOS, Linux chưa chạy. Biên dịch chéo cả `groove-app` không được vì thiếu trình biên dịch C (`rusqlite` bundled); phần gọi souvlaki đã `cargo check` riêng bằng crate tạm cho `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, phần còn lại của tệp chưa biên dịch trên hai OS đó.

**Quyết định** (Opus chọn 02/11/2026, ghi cả `plan/confirm.md`): nút giữa luôn bật như bản cũ; Windows dùng logo cho bài không bìa; tệp ngoài chưa có ảnh trên media session; `Raise` mở cửa sổ, `SetVolume`, `Quit`, `OpenUri` bỏ qua.

**Hạn chế đã biết**: đổi ngôn ngữ thì tooltip nút taskbar giữ chữ cũ tới lần nút đổi trạng thái kế tiếp: `app.rs` không cho quan sát `Settings` từ ngoài. Cỡ vượt ước lượng ~550 dòng: khoảng 1.300 dòng, gần nửa là test; không tách vì hai tệp đã là phạm vi của task. Ảnh logo được ghi vào `cache_dir` khi cần, cả lúc chạy test `--ignored` SMTC.

**Test nhạy gặp lúc gộp**: `app::tests::window_title_follows_route_and_override` (race ngôn ngữ từ test của task 21), `refresh_roots_starts_watching_an_imported_folder` (nhạy giờ, CPU 100%). Đều ngoài tệp của task, mỗi cái đỏ một lần, ghi `plan/_backlog.md`, không sửa. Xung đột `app.rs`, `_backlog.md`, `confirm.md` khi gộp 21, 23: giữ cả hai phía.

**Soi cải tiến**: bảo mật: `cover_hash` chỉ nhận hex, log không ghi đường dẫn, `OpenUri` bỏ qua. UI/UX: DPI theo `WM_DPICHANGED`, Explorer khởi động lại thì thêm lại nút, chạy quyền admin có mở lọc UIPI. RAM: thêm hai luồng nghỉ chờ, diff trước khi gửi nên không có việc theo nhịp 250 ms; chưa đo RAM vì không mở app. Pro: không áp.

## 10. Đề xuất cải tiến

- [ux] Tooltip taskbar đổi ngay khi đổi ngôn ngữ — `app.rs` không lộ `Settings` để quan sát — `app.rs` thêm cách nghe đổi ngôn ngữ, gửi lại trạng thái — nhỏ — S — P3. Chưa làm: ngoại lệ `app.rs` chỉ cho một lời gọi trong `run`.
- [ux] Ảnh bìa tệp ngoài thư viện trên media session — bìa chỉ ở bộ nhớ (`Player::external_cover`) — ghi tệp tạm khi cần — nhỏ — S — P3.
