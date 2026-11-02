# groove-gd1_24 — settings-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~850 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_11, groove-gd1_17, groove-gd1_22 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/settings/{dialog,general,eq_panel,eq_curve,keybindings,roots}.rs`, `i18n/*/settings.toml` |

## 0. Bàn giao từ groove-gd1_22 (02/11/2026)

<!-- handoff: groove-gd1_22 -->

- **Đã có sẵn để dùng** (`groove_core::keymap`): `parse_json(text) -> Result<Vec<KeyBinding>, KeyJsonError>` soát hình dạng; `validate(&bindings) -> Vec<KeyIssue>` soát nội dung (M5); `to_json(&bindings)` cho nút Đặt lại mặc định với `settings::default_keybindings()`; `Command::ALL` + `.name()` cho gợi ý lệnh.
- **OK**: `parse_json` lỗi, hoặc `validate` không rỗng thì nhảy về tab Phím tắt, không đóng (M5). Chữ báo lỗi map từ biến thể `KeyJsonError`, `KeyIssue` sang vùng `settings`; `index` tính từ 0.
- **Áp phím**: chỉ cần `app::update_settings(cx, |s| s.custom_keybindings = Some(list))`. `commands` tự so ở phím kế, không có hàm nạp lại.
- **Cần biết**: hộp Settings là dialog, phím toàn cục tự tắt khi mở.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 24. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

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
