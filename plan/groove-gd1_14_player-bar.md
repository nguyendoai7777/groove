# groove-gd1_14 — player-bar

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~800 dòng |
| Phụ thuộc | groove-gd1_03, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{player_bar,seek_bar,marquee,playing_bars}.rs`, `i18n/*/player.toml` |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Việc của task này (N5)**: `PlayerEvent::TrackFailed { title, kind, gave_up, .. }` phải thành toast, `OutputFailed` cũng vậy; task 13 chỉ phát sự kiện vì chữ thuộc vùng `player`. `kind` là `AudioErrorKind`.
- **Thanh phát**: `current_song()`, `is_playing()`, `position_sec()`, `duration_sec()`, `timeline()`, `queue().shuffle()`, `queue().repeat()`; điều khiển `toggle_play`, `next`, `prev`, `toggle_shuffle`, `toggle_repeat`, `seek` (chỉ gọi khi thả, S20), `set_volume`, `adjust_volume`, `toggle_mute`. Bìa tệp ngoài: `external_cover()`. Đừng dùng `app::update_settings` cho âm lượng: phải qua `Player` mới tới engine.

## 0. Bàn giao từ groove-gd1_03 (02/11/2026)

<!-- handoff: groove-gd1_03 -->

- **Chỗ đặt**: `Slot::PlayerBar`, vùng đáy cao 90 px, nền `surface` đục, viền trên; một lời gọi `app::install_slot(Slot::PlayerBar, view, cx)` trong `run`. Slot trống vẫn chừa 90 px.
- **Chữ ký ở `app::`** (chi tiết `plan/groove-gd1_03_app-shell.md` mục 9): `Slot::{PlayerBar, Library, NowPlaying}`, `install_slot(slot, view, cx)`, `navigate(Route, cx)`, `current_route(cx)`, `settings(cx)`, `update_settings(cx, |s| …)`, `shell(cx)` phát `ShellEvent`.
- **Icon sẵn**: `icons::GrooveIcon::{Shuffle, Loop, LoopBadge, Prev, Next, Play, VolumeMute, VolumeMin, VolumeMedium, VolumeMax}` (`impl From<GrooveIcon> for IconSource`, viewBox 36 × 36 và 46 × 46 cho bộ media / volume giữ nguyên từ sprite cũ). Thiếu icon thì thêm tệp vào `crates/groove-app/assets/icons/` + một dòng `groove_icons!` (tệp `icons.rs` do task 03 sở hữu, task này được thêm dòng).
- **Cài đặt**: âm lượng, tắt tiếng, seek / volume step qua `app::update_settings`; đã có `JsonSaver` duy nhất, không tạo cái khác.
- **Bẫy**: `#[derive(IntoElement)]` ở bản không test cần `use materiant::gpui_kit as gpui;`; bọc `.occlude()` quanh phần tử bấm được nằm trong vùng kéo cửa sổ.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 14. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
