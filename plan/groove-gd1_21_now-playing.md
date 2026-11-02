# groove-gd1_21 — now-playing

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~800 dòng |
| Phụ thuộc | groove-gd1_14, groove-gd1_20 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{now_playing,timeline_box,lyrics_panel,queue_panel}.rs`, `i18n/*/now_playing.toml` |

## 0. Bàn giao từ groove-gd1_14 (02/11/2026)

<!-- handoff: groove-gd1_14 -->

- **Đã có sẵn để dùng**: `views::seek_bar::SeekBar` (một hàng giờ trái, thanh chia khúc, thời lượng; tự quan sát `player::player(cx)`, kéo chỉ tua khi thả, nhãn khi rê khúc có tên) nếu màn Now Playing cần thanh tua riêng; `views::marquee::Marquee::new(id, text)` cho tiêu đề dài; `views::playing_bars::PlayingBars::new(active)` cho dòng đang phát trong hàng đợi (chưa gắn ở đâu).
- **Hàm thuần dùng lại**: `seek_bar::{segment_at, segment_label}` cho hộp đoạn timeline; `timeline::segments_to_bar` / `x_to_time` (S20) là nguồn chung nơi bấm và nơi vẽ.
- **Chỗ đặt**: thanh phát nằm ở `Slot::PlayerBar`, bấm bìa hoặc chữ đã `app::navigate(Route::NowPlaying, cx)`; màn này không phải vẽ lại điều khiển phát.
- **Toast lỗi phát** đã do `PlayerBar` đẩy (`TrackFailed`, `OutputFailed`); màn Now Playing đừng đẩy trùng.
- **Bẫy**: `#[derive(IntoElement)]` cần `use materiant::gpui_kit as gpui;`, nhưng alias đó làm `use gpui::…` trong `mod tests` mơ hồ: ở tệp không derive thì bỏ alias. Test view dùng `player::install_for_test` với `library: None`, `FakeOutput`, `session_file: None`, `cx.executor().allow_parking()`; mẫu ở `views/player_bar.rs` mục `tests` (`rig`, `open_bar`).
- **Soát lại khi làm**: `cargo test -p groove-app views::player_bar` còn xanh sau khi đụng `seek_bar.rs`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 21. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).