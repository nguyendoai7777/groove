# groove-gd1_24 — settings-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~850 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_11, groove-gd1_17, groove-gd1_22 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/settings/{dialog,general,eq_panel,eq_curve,keybindings,roots}.rs`, `i18n/*/settings.toml` |

## 0. Bàn giao từ groove-gd1_17 (02/11/2026)

<!-- handoff: groove-gd1_17 -->

- **Đã có sẵn để dùng**: `library_ctl::ctl(cx) -> Option<Entity<LibraryCtl>>`. Danh sách gốc đọc bằng `Library::watched_roots()`; bỏ gốc bằng `LibraryCtl::forget_root(path, cx)`: xoá bài dưới gốc, không đụng tệp, tự báo watcher. Chi tiết: mục 9 của `plan/groove-gd1_17_library-actions.md`.
- **Thêm gốc**: `LibraryCtl::pick_and_import(cx)`, đã có toast và cờ đang nhập.
- **Hộp thoại**: dialog của materiant là khai báo, chủ vẽ làm con; `LibraryPage::add_overlay` chỉ vẽ trong `Slot::Library`, nên hộp mở được ở Now Playing phải gắn ở chỗ khác.
- **Bẫy đã gặp**: mở hộp từ `on_select` của menu phải hoãn (`library_ctl::with_window`), cửa sổ đang bị mượn nên `window.update` lỗi.

## 0. Bàn giao từ groove-gd1_22 (02/11/2026)

<!-- handoff: groove-gd1_22 -->

- **Đã có sẵn để dùng** (`groove_core::keymap`): `parse_json(text) -> Result<Vec<KeyBinding>, KeyJsonError>` soát hình dạng; `validate(&bindings) -> Vec<KeyIssue>` soát nội dung (M5); `to_json(&bindings)` cho nút Đặt lại mặc định với `settings::default_keybindings()`; `Command::ALL` + `.name()` cho gợi ý lệnh.
- **OK**: `parse_json` lỗi, hoặc `validate` không rỗng thì nhảy về tab Phím tắt, không đóng (M5). Chữ báo lỗi map từ biến thể `KeyJsonError`, `KeyIssue` sang vùng `settings`; `index` tính từ 0.
- **Áp phím**: chỉ cần `app::update_settings(cx, |s| s.custom_keybindings = Some(list))`. `commands` tự so ở phím kế, không có hàm nạp lại.
- **Cần biết**: hộp Settings là dialog, phím toàn cục tự tắt khi mở.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 24. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_11 (02/11/2026)

<!-- handoff: groove-gd1_11 -->

- **Không cần gọi `dsp` từ view**: view đổi `EqSettings`; task 13 chuyển thành `SetEq`. Kéo thanh trượt gửi liên tục được, DSP tự trượt 10 ms.
- **Cần biết**: preamp tự động làm cả bài nhỏ đi khi tăng dải (S23). Đường cong vẫn vẽ từ `eq.gains`, không tính preamp, không tính bass boost.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `settings::{EqSettings, EqPreset, EQ_BANDS_HZ, EQ_GAIN_LIMIT_DB, BASS_BOOST_MAX_DB}`. Không tự viết luật EQ trong view: `select_preset`, `set_band`, `set_bass_boost` đã làm đúng đặc tả 1.10 và M7.
- **Đường cong** vẽ từ `eq.gains` (không tính bass boost); DSP nhận `effective_gains()`.
- **Nhãn preset** do view map `EqPreset` sang `t!`, `EqPreset::ALL` đúng thứ tự bản cũ. Ngôn ngữ đổi xong gọi `Settings::apply_language`. Phím tắt: `Settings::custom_keybindings`, `None` là mặc định.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Task `sonnet` theo bảng thuộc tính, nhưng phiên này do Opus làm trọn (lệnh giao của chủ dự án). Không có phần nghiệp vụ ngoài giao diện: luật EQ, soát phím, đọc / xoá gốc đều là hàm có sẵn của `core` và `library_ctl`.

| Phần việc | Vai |
| --- | --- |
| `dialog.rs` (hộp, ba tab, OK / Huỷ, nối `Slot::Settings`), `general.rs`, `eq_panel.rs`, `eq_curve.rs`, `keybindings.rs`, `roots.rs`, catalog `settings` | `[sonnet]` (làm bởi Opus) |
| Test chứng minh EQ tới engine bằng `FakeOutput` | `[opus-unit]` |
| Nhìn hộp thật (pixel, đường cong khớp núm trượt, hộp chọn thư mục thật) | `[owner-qa]` |

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | Player phát sóng sin 440 Hz qua `FakeOutput`; kéo mười dải xuống -10 dB bằng `EqPanel::apply` (đường của thanh trượt) thì độ lớn đầu ra giảm dưới 0,6 lần; Huỷ thì về trên 0,85 lần | `[opus-unit]` `c1_eq_reaches_the_engine_and_cancel_restores_it` |
| C1b | Chọn mẫu Bass strong thì tiếng đổi rõ (preamp tự động), chọn Flat thì về | `[opus-unit]` `c1b_preset_reaches_the_engine` |
| C2 | Mở hộp thì ô, thanh, JSON hiện đúng giá trị đang có, về tab đầu | `[sonnet-test]` `c2_open_shows_the_current_values` |
| C3 | Gõ bước tua áp ngay; ô trống, 0 bị bỏ qua | `[sonnet-test]` `c3_steps_apply_while_typing_and_ignore_invalid_input` |
| C4 | Huỷ trả bước tua, EQ, bỏ nháp phím tắt | `[sonnet-test]` `c4_cancel_restores_everything_and_drops_the_key_draft` |
| C5 | Bấm nền không đóng (M3), Esc coi như Huỷ, mở lại thì nháp cũ không còn | `[sonnet-test]` `c5_escape_cancels_and_backdrop_does_not_close` |
| C6 | OK với JSON sai (cú pháp, không phải mảng, lệnh lạ, thiếu trường) nhảy về tab Phím tắt, báo lỗi, không đóng, không lưu; gõ tiếp xoá lời báo (M5) | `[sonnet-test]` `c6_ok_with_bad_json_jumps_to_the_keys_tab_and_stays`, `keybindings::tests` |
| C7 | OK với phím hợp lệ lưu và đóng; không đổi thì không ghi; bằng mặc định thì ghi `None` | `[sonnet-test]` `c7_ok_saves_valid_keybindings_and_closes` |
| C8 | Tab Thư viện: liệt kê gốc từ DB, Thêm qua `Picker` rồi nhập, Bỏ hỏi lại lần hai, chỉ xoá bài, tệp còn | `[sonnet-test]` `c8_roots_tab_lists_adds_and_forgets_roots` |
| C9 | Bấm nút Settings ở khung mở hộp trong `Slot::Settings`; Esc đóng | `[sonnet-test]` `c9_the_settings_button_opens_the_dialog_in_its_slot` |
| C10 | Nhìn hộp thật: tab, đường cong khít núm, chữ vi / en, cửa sổ nhỏ nhất | `[owner-qa]` (`_backlog.md`) |

## 8. Việc thiếu từ làn khác

- Không có. Cần chỗ treo hộp trong cây cửa sổ nên thêm `Slot::Settings` vào `app.rs` (xem mục 9, `plan/confirm.md`).

## 9. Bàn giao thực tế (02/11/2026)

- **Đã làm**: `views/settings/{dialog,general,eq_panel,eq_curve,keybindings,roots}.rs`, `i18n/{vi,en}/settings.toml`. `SettingsDialog` 800 px, ba tab (Cài đặt chung, Phím tắt, Thư viện), nghe `ShellEvent::OpenSettings`, `settings::dialog::install(handle, cx)` gọi một lần trong `app::run` (sau `now_playing::install`).
- **Sửa `app.rs` (ngoài danh sách tệp, đã ghi `plan/confirm.md`)**: `Slot::Settings`, một dòng vẽ trong `Shell::render`, một lời gọi `install`; hai hàm `#[cfg(test)]` `install_settings_for_test`, `set_shell_for_test` cho test của view (global của khung là riêng tư).
- **EQ**: bảng giữ một bản `EqSettings` để vẽ, cùng một phép đổi chạy trên nó và trên `Player::update_eq` (lưu + gửi engine, DSP tự trượt 10 ms). Luật EQ không viết lại (`select_preset`, `set_band`, `set_bass_boost`). Test C1 chứng minh giá trị tới engine bằng độ lớn đầu ra của `FakeOutput` (sóng sin tự sinh, chờ theo điều kiện, bỏ khung im lặng để không chập chờn lúc máy bận). Không dùng `apply_settings`: mọi đổi âm thanh của hộp đã đi qua `update_eq`; bước tua, bước âm lượng không đổi engine.
- **Hành vi**: EQ, bước tua, bước âm lượng áp ngay; OK soát `keymap::parse_json` rồi `validate` (M5), sai thì về tab Phím tắt, lời báo dưới ô theo biến thể `KeyJsonError`, `KeyIssue` (phần tử đếm từ 1); đúng thì `app::update_settings` (`Some(list)`, `None` nếu bằng mặc định, không ghi nếu không đổi). Huỷ, Esc, nút ✕ trả bước tua, bước âm lượng, EQ (qua player) về lúc mở, bỏ nháp. Bấm nền không đóng (M3). Phím toàn cục tự tắt khi hộp mở (`commands`).
- **Tab Thư viện**: `Library::watched_roots`, Thêm = `LibraryCtl::pick_and_import` (hộp chọn qua `Picker`), Quét lại = `rescan`, Bỏ = `forget_root` sau lần bấm xác nhận. Danh sách đọc lại theo `LibraryCtlEvent::Notice`; không sửa `library_ctl.rs`.
- **Quyết định tự chọn**: xem các dòng `[groove-gd1_24]` trong `plan/confirm.md`.
- **Soát** (trên cây đã gộp `master` mới nhất lúc gộp, gồm `claude/flaky-audio`, `claude/gd1_27`, `claude/drop-rfd`): `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast` (groove-app 268 qua, 3 bỏ qua; mọi crate xanh), `cargo deny check licenses`, `cargo fmt --all --check`: xanh. Ở cây gộp trước đó `cargo test --workspace` đỏ ba test engine chập chờn đã biết, chạy lại xanh; chưa gặp lại sau khi gộp bản sửa của `flaky-audio`.
- **Test của task từng chập chờn dưới tải, đã sửa**: `c1b_preset_reaches_the_engine` đỏ khi máy bận vì lấy mốc "phẳng" ngay lúc âm lượng còn nội suy từ 0, và vì kéo mẫu nhanh gấp mười lần thời gian thật. Giờ kéo đúng nhịp 10 ms, bỏ nửa giây đầu rồi lấy trung bình mười khung, mọi điều kiện bỏ khung im lặng; chạy `settings::` đồng thời tám tiến trình bốn vòng liền, 32 / 32 xanh.
- **Cỡ**: sáu tệp (trần 7), khoảng 1.060 dòng code không tính test, gồm chú thích tiếng Việt, vượt trần 900. Chấp nhận: một lát dọc, tách ra thì đường cong, danh sách gốc, ô JSON mỗi cái đều dính `dialog.rs`.
- **Mới ở tầng nghiệp vụ**: không có; không gặp nghiệp vụ khó vượt phạm vi giao diện.
- **Soi cải tiến**: bảo mật không áp (không IPC; bỏ gốc không xoá tệp, hỏi lại hai lần bấm; JSON phím qua `validate` trước khi ghi). UX: dùng hết bằng bàn phím (Tab, ↑ ↓ trên cột tab và thanh trượt, Esc), lời báo lỗi kèm cách gỡ, vi / en cùng bộ khoá, cửa sổ thấp thì thân hộp cuộn. RAM: hộp dựng một lần lúc khởi động (hai `NumberField`, một `TextArea`, vài entity nhỏ), không phụ thuộc mới, chưa đo. Pro không áp.
- **Chờ kiểm tay** `[owner-qa]` (đã vào `plan/_backlog.md`): nhìn hộp thật vì máy dev một màn.

## 10. Đề xuất cải tiến

- `[ux]` Ô JSON phím tắt không gợi ý tên lệnh khi gõ `"command":` và không tự đóng ngoặc — `TextArea` của materiant chưa có chỗ chèn gợi ý — thêm hook gợi ý vào `TextArea` rồi nối `Command::ALL` — gõ nhanh hơn, ít sai tên — M — P3 (cần đổi materiant, không làm trong lượt).
- `[ux]` Hộp Settings chưa có ô ngôn ngữ, theme — `Settings::language`, `theme` đã có, `/theme` mới đổi được theme — thêm hai `Select` vào tab Cài đặt chung — đổi ngôn ngữ không cần xoá `settings.json` — S — P3 (đặc tả 1.10 không liệt kê, chờ chủ dự án).
