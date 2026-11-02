# groove-gd1_21 — now-playing

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~800 dòng |
| Phụ thuộc | groove-gd1_14, groove-gd1_20 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{now_playing,timeline_box,lyrics_panel,queue_panel}.rs`, `i18n/*/now_playing.toml` |

## 0. Bàn giao từ groove-gd1_20 (02/11/2026)

<!-- handoff: groove-gd1_20 -->

- **Đã có sẵn để dùng**: `views::lyrics_dialog::TextDialog` (một lõi cho lyrics và timeline). `cx.new(|cx| TextDialog::lyrics(ops.clone(), window, cx))` và `timeline_dialog::timeline(ops.clone(), window, cx)`; giữ hai `Entity` trong view, vẽ bằng `.child(entity)` (hộp tự trống khi đóng). Nút Edit / Add Lyrics gọi `entity.update(cx, |d, cx| d.open(track, window, cx))` với `TrackRef` bài đang phát; nút Timeline tương tự. Lưu xong hộp tự `refresh_current` và đóng, đổi bài tự đóng, lỗi tự toast: view không phải làm thêm.
- **Cần `MetadataOps`**: app chưa giữ chung. Dựng một bản `MetadataOps::new(app::library().clone(), CoverStore::default_location()?)` (Clone được, mỗi bản một luồng) dùng cho cả hai hộp; chi tiết mục 8 của `plan/groove-gd1_20_lyrics-timeline-dialogs.md`.
- **Bẫy**: ô timeline chưa mono (TextArea thiếu font). Kiểm tay còn nợ ở `_backlog.md`. Test view dùng `install_for_test` với thư viện thật trong thư mục tạm (mẫu `rig` ở `lyrics_dialog.rs`, mục `tests`).
- **Soát lại khi làm**: `cargo test -p groove-app views::lyrics_dialog`.

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
## 8. Thiếu từ làn khác / cần Opus xem
- Không có chỗ nào phải quyết định nghiệp vụ. Không thiếu API của làn khác.
- `ShellHandle` riêng của `app.rs` nên `NowPlaying::new` nhận `route` + hàm đặt tiêu đề (`app::set_title_override`) làm tham số để test không cần `Shell`.

## 9. Bàn giao thực tế (02/11/2026)
- **Đã làm**: `views/{now_playing,timeline_box,lyrics_panel,queue_panel}.rs`, `i18n/{vi,en}/now_playing.toml`; `app::{metadata_ops, try_metadata_ops}` (global `MetadataOpsHandle`, dựng lười, đặt trong `set_library`) + một lời gọi `now_playing::install(handle, cx)` trong `run`.
- **Gộp `metadata_ops` với task 17**: chữ ký `app::metadata_ops(cx: &App) -> MetadataOps` như đã quyết; nếu 17 cũng thêm thì giữ một bản (bản này có thêm `try_metadata_ops`, `set_library` đặt global).
- **Bố cục**: cột trái 25 % (180-300), giữa còn lại, phải 32 % (240-340); quầng cao 450 px độ đục 0,3; khung dùng nền bán trong suốt thay `backdrop-blur`.
- **Hàng đợi**: tra tên bằng `Library::songs_for_tracks` ở luồng nền, nhớ theo id (shuffle không tra lại); bài đã xoá khỏi thư viện hiện "Bài không rõ". Dòng đã phát opacity 0,25 (rê 0,5); đổi bài cuộn vào giữa trừ khi vừa bấm trong bảng.
- **Quyết định đã chốt**: xem 5 dòng `[groove-gd1_21]` ở `plan/confirm.md`.
- **Case**: C1 hàng đợi đủ bài + bài hiện tại, C2 ba nút mở đúng hộp, C3 timeline hiện đoạn, C4 bấm dòng hàng đợi phát bài đó, C5 tiêu đề cửa sổ chỉ đặt khi ở màn này, hàm thuần (lyric_lines, row_state, build_rows, take_center, progress_label, window_title) đều có test: `[sonnet-test]` xanh; `cargo test -p groove-app views::` 103 pass.
- **Chờ kiểm tay**: `[owner-qa]` nhìn màn thật, ghi ở `plan/_backlog.md`.
- **Soi 4 lăng kính**: bảo mật — không đường dẫn từ ngoài, truy vấn accent tham số hoá; UX — dùng hết bằng bàn phím (VirtualList), vi/en đủ; RAM — cache bìa 4 ảnh, chỉ bìa đang phát, hàng đợi không giữ ảnh; Pro — không áp.
