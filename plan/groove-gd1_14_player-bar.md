# groove-gd1_14 — player-bar

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
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

## 2b. Phân vai
| Phần việc | Vai |
|---|---|
| `seek_bar`, `marquee`, `playing_bars`, `player_bar`, i18n `player`, icon `Pause`, một lời gọi trong `run` | `[sonnet]` (task Sonnet theo D-14, không giao subagent) |
| Nhìn thanh thật, nghe, rút thiết bị | `[owner-qa]` |

## 6. Nghiệm thu
| Case | Cho / Khi / Thì | Phủ |
|---|---|---|
| C1 | Không có bài / mở thanh / mờ, không bấm được, vẽ được dù thiếu thư viện, bìa | `[sonnet-test]` `bar_without_a_song_is_inert` |
| C2 | Âm lượng 0, 1, 35, 36, 65, 66, 100, tắt tiếng / chọn icon / đúng ngưỡng 0-mute, 1-35, 36-65, trên 65 | `[sonnet-test]` `volume_icon_follows_the_thresholds` |
| C3 | Timeline bắt đầu ở 30 s / bấm trên thanh / nơi bấm khớp nơi vẽ (khúc ẩn danh từ 0) | `[sonnet-test]` `click_matches_what_is_drawn_when_timeline_starts_late` |
| C4 | Nhấn, kéo, thả / thanh tua / kéo chỉ đổi xem trước, `seek` đúng một lần khi thả | `[sonnet-test]` `scrub_seeks_only_on_release`, `dragging_seeks_once_on_release` |
| C5 | Rê lên khúc có tên / nhãn / `<tên> (<đầu> - <cuối>)`; khúc ẩn danh, không timeline không nhãn | `[sonnet-test]` `label_names_the_segment_under_the_cursor`, `anonymous_bars_have_no_label` |
| C6 | `TrackFailed` hoặc `OutputFailed` / player phát sự kiện / mỗi sự kiện một toast; chưa bỏ cuộc là cảnh báo, bỏ cuộc là lỗi, tên trống dùng chữ thay | `[sonnet-test]` `failure_events_become_toasts`, `track_failure_toast_*`, `output_failure_toast_*` |
| C7 | Chữ rộng hơn khung / marquee / chạy; vòng lặp nối liền; không tràn thì đứng | `[sonnet-test]` `marquee::tests` |
| C8 | Nhìn thật: bìa, nút, marquee, bánh xe âm lượng, toast thật | `[owner-qa]` (`plan/_backlog.md`) |

## 8. Nợ chuyển task khác
- `Player` (task 13, `player.rs`) chưa có `volume()` / `is_muted()` công khai: thanh đọc `app::settings(cx)`. Đủ cho app thật vì `Player::apply_settings` `notify` khi đổi; chỉ test phải thay bằng hàm giả (`PrefsFn`). Có hàm đọc thì đổi `app_prefs` ở `player_bar.rs`, một chỗ.
- Phím tua / chỉnh âm lượng (`seek_step`, `volume_step`) thuộc task 22: `Player::seek_by`, `adjust_volume` đã có; thanh tua chưa nhận phím mũi tên khi focus (không phải việc của task 14).

## 9. Bàn giao thực tế
- **Soát (cwd = worktree, `CARGO_TARGET_DIR=groove/target/gd1_14`)**: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` sạch, `cargo deny check licenses` ok, `cargo test --no-fail-fast` xanh trừ hai test engine nhạy giờ của `groove-audio` (`idle_closes_stream_and_keeps_position`, `position_excludes_audio_still_in_the_ring`) lúc đỏ lúc xanh, không dính mã task này (ghi `plan/_backlog.md`). Test của task: 35 test `views::` xanh, gồm 16 test mới.
- **Đã làm**: `seek_bar.rs` (`SeekBar`, `Scrub`, `segment_at`, `segment_label`, `thickness`), `marquee.rs` (`Marquee`, `overflows`, `offset`), `playing_bars.rs` (`PlayingBars`, `bar_height`), `player_bar.rs` (`PlayerBar`, `install`, `volume_icon`, `track_failed_toast`, `output_failed_toast`), `i18n/{vi,en}/player.toml`, icon `Pause` (`assets/icons/pause.svg` + một dòng `groove_icons!`), một lời gọi `views::player_bar::install(handle, cx)` trong `app::run` sau `player::install`.
- **N5**: `PlayerBar` đăng ký `subscribe_in` vào `PlayerEvent`; `TrackFailed` thành toast Warning (đã tự sang bài kế) hoặc Danger (`gave_up`, nối thêm câu "dừng sau nhiều lỗi liên tiếp"), `OutputFailed` thành toast Danger 8 s kèm cách gỡ. `kind` ra chữ riêng cho từng `AudioErrorKind`.
- **S20**: thanh vẽ và đổi chuột sang thời gian cùng bằng `timeline::segments_to_bar` / `x_to_time` trên một danh sách khúc; `Player::seek` chỉ gọi khi thả chuột.
- **Opus chọn 02/11/2026** (cũng ở `plan/confirm.md`): Play đổi icon tức thì thay cho biến hình 250 ms (đặc tả cho phép); marquee đứng ở đầu dòng khi rê chuột và chạy lại từ đầu khi rời (animation GPUI không tạm dừng giữa chừng); số giờ không nhấp nháy (đặc tả mục 4); thanh không bài thì chặn cả nút Settings (đặc tả "không bấm được"; Settings vẫn mở được từ cột trái); thêm icon `Pause`; âm lượng đọc từ `app::settings`.
- **Chưa kiểm**: nhìn thật và nghe thật (C8) — `[owner-qa]` ở `plan/_backlog.md`; máy dev chỉ một màn nên không mở cửa sổ. Chưa đo RAM.
- **Sửa kèm**: `plan/_backlog.md` đang còn dấu xung đột `<<<<<<<` / `>>>>>>>` từ một lần gộp trước (dòng `groove-gd1_15` trùng, dòng `groove-gd1_08` bị kẹp); đã giải, giữ đủ ba dòng.

## 10. Đề xuất cải tiến
| Loại | Vấn đề | Bằng chứng | Đề xuất | Giá trị | Cỡ | Ưu tiên |
|---|---|---|---|---|---|---|
| ux | Thanh tua không dùng được bằng bàn phím khi focus | `seek_bar.rs` không đăng ký phím; chỉ con trỏ | `Left`/`Right` gọi `Player::seek_by(seek_step)` khi thanh có focus | trợ năng | S | P2 (task 22 đã giữ `seek_step`) |
| ux | Marquee đứng khi rê chuột nhưng chạy lại từ đầu | `marquee.rs`: `with_animation` không tạm dừng | giữ độ lệch lúc dừng bằng `Instant` khi GPUI cho đặt pha | mượt hơn | S | P3 |
| pro | Không áp | | | | | |

Không làm trong lượt: cả ba đều ngoài phạm vi hoặc cần GPUI hỗ trợ; bảo mật không áp (không chạm tệp, IPC, đường dẫn); RAM: chỉ có animation khi tiêu đề tràn (30 fps tối đa), bốn thanh nhảy khi phát (20 fps, `PlayingBars` chưa gắn ở đâu), không giữ thêm bộ nhớ ngoài `LruImageCache` 4 bìa.
