# groove-gd2_04 — shortcuts-editor

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (09/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | M — ~850 dòng (keymap, registry phím toàn cục, trang danh sách, hộp thêm / sửa, i18n) + test |
| Phụ thuộc | groove-gd1_22, groove-gd1_24 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, G-4, G-6, N7 |
| Phụ thuộc mới | `global-hotkey` 0.9 (MIT OR Apache-2.0, chỉ Windows / macOS) |
| File đụng tới | MỚI `crates/groove-app/src/platform/global_keys.rs`, `crates/groove-app/src/views/settings/key_editor.rs`; SỬA `crates/groove-core/src/keymap.rs`, `crates/groove-core/src/settings.rs`, `crates/groove-app/src/views/settings/keybindings.rs`, `crates/groove-app/src/views/settings/dialog.rs`, `crates/groove-app/src/app.rs`, `i18n/{vi,en}/settings.toml` |

## 1. Làm gì

- Trang Settings → Phím tắt bỏ ô JSON: danh sách phím tắt (tên, chip phím, nhãn "Toàn cục", nút sửa / xoá), nút Thêm, nút Đặt lại
  mặc định. Huỷ / bấm nền bỏ mọi thay đổi như cũ; OK lưu.
- Hộp thêm / sửa (Dialog lồng trong hộp Settings): ô tên hiển thị, dropdown chức năng (`Command::ALL`, tên qua i18n), ô ghi tổ hợp
  (bấm để ghi, nhận chuỗi hai phím `g m` trong 800 ms), công tắc "Toàn cục".
- Cảnh báo: trùng phím với dòng khác, chuỗi bị phím đơn che (`g` vs `g m`); phím toàn cục: tổ hợp không hợp lệ cho toàn cục (chuỗi,
  không phím bổ trợ), OS từ chối đăng ký (app khác giữ). Hiện ở hộp sửa và dưới dòng trong danh sách.
- Phím toàn cục thật: đăng ký với OS (Windows `RegisterHotKey`, macOS Carbon qua `global-hotkey`), bấm ở app khác vẫn chạy lệnh;
  tạm gỡ khi đang ghi phím.
- `KeyBinding` thêm `label`, `global` (`#[serde(default)]`): settings cũ đọc được, settings mới bỏ trường trống khi ghi.

## 2. Không làm trong task này

- Danh sách loại trừ theo app như fastcast (chủ dự án: không làm).
- Lịch sử phím / Quay lại của fastcast.
- Phím toàn cục trên Linux (portal `GlobalShortcuts` như fastcast, cần máy Ubuntu thử): công tắc tắt, ghi chú "chưa hỗ trợ" — backlog.

## 2b. Phân vai

- [opus] cả task (nghiệp vụ phím, registry, nối dây).
- [owner-qa] bấm phím toàn cục thật từ app khác (desktop ẩn không nhận phím của bàn phím thật); macOS.

## 3. Kiểu dữ liệu

```rust
pub struct KeyBinding {
    pub key: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub global: bool,
}
```

`keymap`: `KeySequence::global_issue() -> Option<GlobalIssue>` (`Sequence`, `NoModifier`); `conflict(bindings, editing, key) ->
Option<Conflict>` (`Same { index }`, `ShadowedBy { index }`, `Shadows { index }`); `Command::label()` qua i18n
`settings.keys.command.<name>`. Bỏ `parse_json`, `to_json`, `KeyJsonError` (hết dùng).

## 4. Code

- `platform/global_keys.rs`: `install(cx)`, `apply(&[KeyBinding], cx)` (gỡ hết, đăng ký bộ mới, nhớ lỗi từng phím),
  `failure(key) -> Option<String>`, `probe(&KeySequence) -> Result<(), String>` (đăng ký thử rồi gỡ; phím của mình thì Ok),
  `suspend(bool)`. Sự kiện `GlobalHotKeyEvent` ⇒ kênh ⇒ `commands::run` trên UI thread. Linux: bản rỗng `supported() = false`.
- `views/settings/key_editor.rs`: entity `KeyEditor` (nháp một dòng, bộ ghi phím qua `intercept_keystrokes`), phát
  `KeyEditorEvent::{Save(KeyBinding), Cancel}`.
- `views/settings/keybindings.rs`: `KeybindingsPanel` giữ `Vec<KeyBinding>` nháp, vẽ danh sách, mở `KeyEditor` trong `Dialog`.
- `dialog.rs` OK: lưu nháp rồi `global_keys::apply`.

## 5. Đo

RAM nghỉ trước / sau (desktop ẩn, release + debug-assertions): thêm crate không được làm tăng quá 1 MB.

## 6. Nghiệm thu

- C1 [đơn vị] settings cũ `[{key, command}]` đọc được, `label` rỗng, `global = false`; ghi lại không thêm trường rỗng (`TestDir`).
- C2 [đơn vị] `conflict`: trùng phím (cả viết khác `left` / `arrowleft`), chuỗi bị phím đơn che, phím đơn che chuỗi, bỏ qua chính dòng đang sửa.
- C3 [đơn vị] `global_issue`: `g m` và `space` không dùng toàn cục được, `ctrl+f7`, `f7` được.
- C4 [view] thêm dòng qua hộp (chọn lệnh, ghi phím, đặt toàn cục) rồi OK: lưu đúng; Huỷ hộp Settings bỏ dòng.
- C5 [view] sửa / xoá dòng; Đặt lại mặc định; về mặc định thì ghi `None`.
- C6 [view] ghi phím `g` rồi `m` trong 800 ms ra `g m`; Esc huỷ ghi; trùng phím hiện cảnh báo.
- C7 [Windows] OS từ chối: một manager khác giữ `ctrl+alt+shift+f23` ⇒ `probe` / `apply` báo lỗi.
- C8 [owner-qa] bật toàn cục cho `ctrl+f7` → `next_track`, focus app khác, bấm `Ctrl+F7`: chuyển bài.

## 7. Ghi chú UI

```
Phím tắt
┌──────────────────────────────────────────────────────────┐
│ Mở tìm kiếm                        [Ctrl] [K]     ✎  🗑   │
│ Bài kế · Toàn cục                  [Ctrl] [F7]    ✎  🗑   │
│ ⚠ Ctrl+F7 đang bị ứng dụng khác giữ                      │
└──────────────────────────────────────────────────────────┘
[Thêm phím tắt]                         [Đặt lại mặc định]
```

Khoá i18n vùng `settings.keys.*`. Chip `Kbd` mỗi phím của chuỗi một khối. Một độ đậm (G-6), màu nhấn từ theme.

## 8. Nợ chuyển task khác

- Phím toàn cục trên Linux qua portal `GlobalShortcuts` (backlog).

## 9. Bàn giao thực tế

- **Đã làm**: trang danh sách (`views/settings/keybindings.rs`), hộp thêm / sửa lồng (`views/settings/key_editor.rs`), phím toàn
  cục (`platform/global_keys.rs`, `Registry<B: Backend>` + bản thật `global-hotkey` trên Windows / macOS, bản rỗng Linux),
  `keymap::{conflict, Conflict, GlobalIssue, KeySequence::global_issue, Command::label}`, `KeyBinding::{label, global, new}`.
  Bỏ `keymap::{parse_json, to_json, KeyJsonError}` và khoá i18n `settings.keys.error.*` (hết ô JSON).
- **Quyết định (D-9)**:
  - Cảnh báo không chặn lưu (hộp sửa lẫn OK của Settings): người dùng có thể đang đổi dần hai dòng trùng; dòng trước thắng như
    `Keymap`. Thiếu phím / chức năng thì nút Lưu của hộp sửa không đóng hộp.
  - Phím toàn cục chỉ nhận một tổ hợp có Ctrl / Alt, hoặc F1–F24 (`GlobalIssue`): phím trơn giữ toàn cục làm mất phím gõ chữ ở app khác.
  - Dòng toàn cục cũng vào `Keymap` trong app: OS giữ được thì `WM_HOTKEY` chạy lệnh; OS từ chối thì phím vẫn chạy khi cửa sổ focus.
  - macOS: `ctrl` của groove (gộp Ctrl / Cmd, `commands::chord_of`) đăng ký thành Cmd.
  - Ghi phím: phím sau tới trong 800 ms nối thành chuỗi (cùng luật `KeyMatcher`), Enter xong, Esc huỷ; Enter / Esc trơn nên
    không gán được, như bản JSON cũ không ai gán.
  - OS từ chối: hộp sửa đăng ký thử (`probe`) khi bật Toàn cục; danh sách đọc lỗi của lần đăng ký gần nhất (sau OK).
  - Linux chưa có phím toàn cục (portal của fastcast cần máy Ubuntu): công tắc tắt + chữ "chưa hỗ trợ"; `plan/_backlog.md`.
- **Kiểm**: `cargo build`, `clippy --all-targets -D warnings`, `cargo test` (cả workspace), `cargo deny check licenses` xanh.
  Test mới: `settings::tests::old_keybindings_file_reads_and_new_fields_round_trip` (C1, `TestDir`),
  `keymap::tests::{conflict_finds_same_and_shadowed_keys, global_needs_one_chord_with_a_modifier_or_function_key}` (C2, C3),
  `dialog::tests::{c6_add_through_the_editor_then_ok_saves_label_and_global, c6b_recording_makes_sequences_and_warns_on_conflicts,
  c7_edit_remove_and_reset_then_ok}` (C4–C6), `global_keys::tests::os_refuses_a_combination_held_by_someone_else` (C7, OS thật,
  `Ctrl+Alt+Shift+F23`).
- **Chạy thật** (`tools/measure-hidden.ps1`, desktop ẩn): chụp danh sách + hộp sửa theme sáng / tối, danh sách có dòng trùng / chuỗi
  bị che / toàn cục sai dạng hiện đủ cảnh báo; ô ghi đang ghi. Sửa sau khi xem: nền rê dòng đầu / cuối bo theo khung, lề chữ gợi ý,
  nền rê ô ghi.
- **Đo**: RAM nghỉ bản debug cùng lượt — master 78,6 MB working set / 69,6 MB private, task 78,3 / 69,4 MB (không đổi).
- **Gặp**: dòng log `ERROR gpui-pre/src/window.rs:2005: RefCell already borrowed` lúc mở app trong desktop ẩn có cả ở bản master
  (không do task này), chưa truy.
- **Owner-qa**: `plan/_backlog.md` dòng `[groove-gd2_04 · owner-qa]`.

## 10. Đề xuất cải tiến
