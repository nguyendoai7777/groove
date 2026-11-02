# groove-gd1_22 — commands-keymap

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
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
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `Player::spawn(PlayerDeps { library, output: Box::new(FakeOutput), .. }, cx)` với `app::update_settings` đã có, và `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác.
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
