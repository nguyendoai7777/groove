# groove-gd1_02 — settings-session

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~400 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `core/{settings,session,model}.rs` |

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Nợ chuyển sang đây**: gọi `i18n::set_language` theo cài đặt (`i18n::Language` serde `vi`, `en`, `Language::detect` theo OS); giữ hằng đuôi tệp nhạc một chỗ ở `core/model.rs` (S22); đường dẫn tệp: `paths::settings_file`, `paths::session_file`.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 02. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`, một tác giả; không có phần `[sonnet]` hay `[owner-qa]`.

## 6. Nghiệm thu

Mọi case `[opus-unit]`, tên test ghi trong ngoặc, xanh trên Windows. Test `#[cfg(unix)]` chưa chạy ở máy này.

- C1. Cho thiếu tệp, khi tải, thì ra mặc định 75 / 5 / 2 / `Flat` / `my_music`, không sinh tệp (`defaults_match_the_old_app`, `missing_file_*`).
- C2. Cho `Settings`, `Session` đủ trường, khi lưu rồi tải, thì bằng nhau (`round_trip_*`).
- C3. Khoá JSON camelCase, enum snake_case (`json_keys_*`, `json_holds_references_only_*`, `json_shape_is_stable`).
- C4. Cho tệp hỏng hoặc không phải object, khi tải, thì ra mặc định và tệp cũ thành `.bad` (`corrupt_file_*`).
- C5. Cho một khoá sai kiểu, khi tải, thì chỉ mất khoá đó (`a_wrongly_typed_key_*`, `a_bad_key_*`); thiếu khoá thì lấy mặc định (`missing_keys_*`).
- C6. Giá trị ngoài khoảng được kẹp về (`out_of_range_*`, `non_finite_*`).
- C7. Ghi nguyên tử, không sót tệp tạm (`saving_leaves_no_temp_files_*`).
- C8. EQ: bảng preset, kéo dải thành Custom, bass boost giữ preset Bass, hệ số và kẹp ±10 (`preset_table_*`, `dragging_a_band_*`, `bass_boost_keeps_*`, `effective_gains_*`). M7 (`choosing_custom_again_*`).
- C9. `JsonSaver` gộp nhiều lần lưu, ghi sau debounce, `flush` và `Drop` ghi nốt (`saver_*`, `dropping_the_saver_*`).
- C10. Hai tệp ngoài là hai bài, hai cách viết một đường dẫn là một bài kể cả khi băm, N6 (`two_external_files_*`, `spellings_of_one_path_*`).
- C11. S22: đuôi không phân biệt hoa thường, đủ `mp3, flac, wav, m4a, m4r, ogg` (`extensions_match_*`).
- C12. Phiên: mỗi bài một lần, shuffle và repeat không cùng bật, vị trí hợp lệ (`sanitize_*`); bỏ bài chết, N5 (`retain_tracks_*`, `losing_the_current_track_*`).
- C13. `apply_language` đưa cài đặt vào `i18n` (`apply_language_*`).

## 8. Nợ chuyển task khác

- groove-gd1_03: gọi `Settings::load()` + `apply_language()` khi mở app, đổi `theme`, `lastActiveRoute`, `sideNavCollapsed` sang UI.
- groove-gd1_12: `queue.rs` dùng `model::{TrackRef, RepeatMode}`, không khai lại.
- groove-gd1_13: `session_sync` dùng `Session`, `JsonSaver`, `SAVE_INTERVAL`, `retain_tracks`.
- groove-gd1_22: `keymap` kiểm tra `KeyBinding` (M5), lấy bảng mặc định từ `settings::DEFAULT_KEYBINDINGS`.
- groove-gd1_24: thao tác EQ qua `EqSettings::{select_preset, set_band, set_bass_boost}`; nhãn preset qua i18n theo `EqPreset`.
- groove-gd1_27: `Settings::window` (`WindowPlacement`).

## 9. Bàn giao thực tế

- `core/model.rs`, `core/settings.rs`, `core/session.rs`: 151 + 443 + 92 dòng ngoài test, vượt ước lượng 400 vì thêm `JsonSaver` và tải chịu lỗi từng khoá, vẫn dưới trần 900. 58 test của `groove-core` xanh trên Windows (36 test mới, thêm 1 test `#[cfg(unix)]` chưa chạy ở máy này).
- Soát: `cargo clippy -p groove-core --all-targets -- -D warnings`, `cargo test -p groove-core`, `cargo deny check licenses` xanh. Không build `groove-app` theo yêu cầu (máy đang chạy song song); không đổi API công khai có sẵn nên không có gì ở crate khác bị ảnh hưởng.
- Opus chọn 02/11/2026, ghi ở `plan/confirm.md`: theme mặc định `dark`; `customKeybindings = null` là bảng mặc định; tải chịu lỗi từng khoá + `.bad`; `JsonSaver` ở core. Chọn nhỏ khác: `eq` là object lồng trong `settings.json`; `Song` không serde; `WindowPlacement.displayId` là `u64` theo `gpui-pre` 0.3.10 (`DisplayId(u64)`).
- `i18n::set_language` chưa được gọi ở đường chạy thật: `Settings::apply_language` có sẵn, nơi gọi là `app/app.rs` của task 03 (mục 8), vì `main.rs` không được sửa.
- `WriteMode` không nằm ở `model.rs`: bảng sở hữu ghi `TrackRef, CategoryRef, Song, đuôi tệp`, task 06 đang chạy song song từ trước khi có tệp này nên không dựa vào đây được; chỗ khai `WriteMode` do task 06 chốt.
- Không chạy thật: task này không có UI, không có đường chạy riêng. Soi: bảo mật, Pro không áp; UX: tệp hỏng được giữ thành `.bad`, không mất âm thầm; RAM: một luồng `JsonSaver` nhỏ, chỉ giữ bản JSON mới nhất. `TrackRef::External` ghi đường dẫn không UTF-8 dạng lossy (`serialize_path_lossy`) để không làm hỏng cả lần lưu phiên.

## 10. Đề xuất cải tiến

Không có.
