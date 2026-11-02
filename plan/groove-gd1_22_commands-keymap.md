# groove-gd1_22 — commands-keymap

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_02, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `core/{keymap,cli}.rs`, `app/commands.rs` (M4, M5, S21) |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Lệnh**: `toggle_play`, `next`, `prev`, `play_random`, `toggle_shuffle`, `toggle_repeat`, `toggle_mute`; `seek_by(±seek_step as f64, cx)`, `adjust_volume(±volume_step as i16, cx)`, step đọc từ `app::settings(cx)`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 22. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `settings::{KeyBinding, DEFAULT_KEYBINDINGS, default_keybindings}`, `Settings::keybindings()` (mặc định khi `custom_keybindings` là `None`).
- **Không kiểm ở task 02**: lệnh lạ và phím trùng (M5) thuộc `keymap`. `KeyBinding` chỉ là hai chuỗi, tải tệp không loại dòng sai lệnh nên `keymap` phải chịu được.
- **Nguồn bảng mặc định**: dùng `DEFAULT_KEYBINDINGS`, không chép lại vào `keymap.rs`; thêm lệnh mặc định mới thì ghi mục 8, bảng thuộc `settings.rs`.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi M4, M5, S21 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 6. Nghiệm thu

Phủ bằng test `[opus-unit]`. `core/keymap.rs`, `core/cli.rs`: luật khớp, M5, S21 phần chữ. `#[gpui::test]` trong `app/commands.rs`, cửa sổ ảo, `FakeOutput`, DB tạm:

- C1 mọi phím mặc định chạy đúng lệnh. C2 `meta` là `ctrl`. C3 chuỗi quên sau 800 ms.
- C4 (M4) trong ô nhập chỉ `Ctrl+K` chạy, chữ vẫn vào ô. C5 (M4) dialog mở, kể cả mất focus, không phím nào chạy; đóng thì chạy lại.
- C6 thành phần đang focus giữ phím của nó. C7 nút đến bằng Tab: Space, Enter bấm nút; nút đến bằng chuột: Space phát / dừng.
- C8 (M5) đổi phím áp ngay, dòng sai không làm hỏng dòng đúng. C9 (S21) ba thao tác nhanh đều chạy.
- C10..C13 (S21) `/queue`, `/play`, `/playall`, `/pause`, `/volume`, `/theme`, `/help` làm thật; chọn bài từ kết quả tìm giữ nguyên hàng đợi.
- `[owner-qa]` chạy thật: máy phiên này chỉ một màn nên chưa mở cửa sổ, chưa bấm phím trên app thật.

## 9. Bàn giao thực tế

**Kiểm** (cwd worktree): `cargo build`, `clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, `cargo deny check licenses` xanh. Test `commands::` chạy lặp 3 lần xanh. Không ra loa, không chạm DB thật, không đăng ký phím toàn cục OS: phím chỉ nghe trong cửa sổ của app.

**Chưa kiểm**: `AppHost` (nối `commands` với `app::*`) chỉ biên dịch; `app::settings`, `Shell` là `Global` riêng của `app.rs` nên test dùng `FakeHost`. `tools/run.ps1` từ chối vì máy một màn, không ép. Chờ kiểm tay: `plan/_backlog.md`.

**API đã khoá**:

- `groove_core::keymap`: `Command` (`ALL`, `name`, `from_name`, `works_in_text_input`), `Chord`, `KeySequence` (`parse`, `label`), `Keymap` (`from_bindings`, `sequence_for`), `KeyMatcher::press`, `KeyScope`, `validate` -> `KeyIssue`, `parse_json` -> `KeyJsonError`, `to_json`, `SEQUENCE_TIMEOUT`, `MAX_SEQUENCE`.
- `groove_core::cli`: `COMMANDS`, `CliSpec` (`completion`), `CliArg`, `CliCommand`, `CliError`, `is_cli`, `filter`, `parse`, `resolve`.
- `crate::commands`: `install`, `run`, `run_cli` -> `Task<CliOutcome>`, `CliOutcome`, `play_search_result`, `key_label`, `QUICK_ACTIONS`.
- `app.rs`: đúng một lời gọi `crate::commands::install(cx)` trong `run`.

**Quyết định** (Opus chọn 02/11/2026, ghi cả `plan/confirm.md`):

- Phím đi qua `cx.intercept_keystrokes` + `window.context_stack()`, không `KeyBinding` của GPUI: chuỗi GPUI cố định 1 giây (đặc tả 800 ms), đổi phím lúc chạy phải xoá cả phím của materiant, `meta` phải gán đôi.
- Dialog mở thì `Ctrl+K` cũng tắt; đang ở ô nhập ngoài dialog thì `Ctrl+K` chạy.
- Space, Enter trên phần tử đến bằng Tab là bấm phần tử; focus do chuột thì Space là phát / dừng. GPUI không cho biết nguồn focus nên tự nhớ phần tử sau phím Tab.
- `/playall`: phát lại danh sách hiện tại từ đầu, như bản cũ; trống thì `NoMatch`. Thư viện không có "tất cả bài".
- `/theme` không đối số đổi qua lại sáng, tối; nhận thêm `system`. `/queue <tên>` thêm bài khớp đầu tiên. `/volume` nhận số nguyên, kẹp 0-100, chữ thì báo lỗi.
- Nhận cả `left`, `arrowleft`, thứ tự tiền tố tuỳ ý; `meta+k` là `ctrl+k`. Báo thêm `UnreachableSequence` (`g` và `g m` cùng tồn tại).
- `dialogShortcuts` không chuyển sang (S21).

**Soi cải tiến**: bảo mật: log chỉ ghi số dòng sai, không ghi nội dung. UI/UX: dùng được hết bằng bàn phím (C7); lỗi có biến thể cho task 23, 24 báo. RAM: không thêm phụ thuộc, không luồng mới; so bảng phím chỉ khi có phím. Pro: không áp.

## 10. Đề xuất cải tiến

- [ux] Giữ Space làm phát / dừng nhấp nháy theo nhịp lặp phím, như bản cũ — `KeystrokeEvent` của GPUI không có `is_held`, `app/commands.rs:on_keystroke` — bỏ lặp cho `toggle_play` khi GPUI lộ cờ này — nhỏ — S — P3. Chưa làm: không có API, không phải lỗi mới.
