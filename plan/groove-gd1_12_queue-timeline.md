# groove-gd1_12 — queue-timeline

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~550 dòng |
| Phụ thuộc | groove-gd1_02 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `core/{queue,timeline}.rs` (bảng 1.4, S4, S20) |

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 12. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `groove_core::model::{TrackRef, RepeatMode}`, không khai lại trong `queue.rs`. `TrackRef` băm và so sánh theo đường dẫn chuẩn hoá, dùng được làm khoá `HashSet` (N6).
- **Khớp với phiên**: `session::Session { current, position_sec, original, history, queue, shuffle, repeat }` là dạng lưu; `Session::sanitize` đã giữ bất biến mỗi bài một lần và shuffle, repeat không cùng bật. Hàng đợi của task này nên dựng được từ các trường đó và ngược lại.
- **Chi tiết**: `plan/groove-gd1_02_settings-session.md` mục 9.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi S4, S20 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 8. Nợ chuyển task khác

- groove-gd1_14: `views/seek_bar` vẽ, rê và tua đều qua `timeline::{segments_to_bar, x_to_time, time_to_x}` với cùng một `Vec<BarSegment>`, khe 2 px; `segment: None` thì không hiện nhãn. Phần "chỉ tua khi thả" của S20 nằm ở view này, không có trong core. Nhãn dùng `timeline::format_time`.
- groove-gd1_17: S2 đã có ở `Queue::play_list` (trùng bài vẫn thay hàng đợi, trả `Resume`); Play all chỉ cần luôn truyền danh sách.
- groove-gd1_20: `timeline::parse` để xem trước; lưu văn bản nguyên gốc, không lưu `Segment`.
- groove-gd1_21: `timeline::active_index`, `Queue::{visible, current_index, visible_len}` cho hộp đoạn và bảng hàng đợi.
- groove-gd1_22: `/queue` gọi `Queue::append`, `play_random` gọi `Queue::play_random`, "Tìm" trong bảng lệnh (S21) gọi `Queue::play_track`.
- groove-gd1_25: mở nhiều tệp gọi `Queue::append` (N6).

## 9. Bàn giao thực tế

- `core/queue.rs` 380 dòng ngoài test, 40 test; `core/timeline.rs` 180 dòng ngoài test, 22 test. Không thêm phụ thuộc, không có chữ giao diện. `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` theo `-p groove-core`: xanh, 120 test (trước đó 58); `cargo deny check licenses` xanh. Không build crate khác theo yêu cầu của lượt này.
- **API đã khoá, queue**: `Queue::{new, with_seed, from_session, write_to, current, original, history, queue, shuffle, repeat, visible, current_index, visible_len, play_list, play_track, append, next_track, prev_track, track_ended, play_random, toggle_shuffle, set_shuffle, toggle_repeat, set_repeat, retain_tracks, clear}`; `QueueOutcome::{Play(TrackRef), Resume, Replay, SeekToStart, Stop, Nothing}`. Tên `next_track`, `prev_track` vì clippy cấm `next` trùng `Iterator`.
- **API đã khoá, timeline**: `Segment { start, end, title }`, `parse(text, duration_sec)`, `parse_time`, `format_time`, `active_index`, `BarSegment { x, width, start, end, segment }` với `fill(time)`, `segments_to_bar(segments, duration_sec, width, gap)`, `x_to_time(bar, x)`, `time_to_x(bar, time)`.
- **Lỗi có test tái hiện**: S4: `s4_*`, `next_under_shuffle_plays_exactly_the_order_shown`; S20: `s20_*`; kèm N5, N6, N8, S2 ở mức hàng đợi (`n5_*`, `n6_*`, `n8_*`, `s2_*`). Mỗi dòng bảng 1.4 có test; `invariants_hold_under_any_sequence_of_actions` chạy 15 000 thao tác ngẫu nhiên, giữ mỗi bài một lần và shuffle, repeat không cùng bật.
- **Opus chọn 02/11/2026** (ghi `plan/confirm.md`): thứ tự khi tắt shuffle; bật shuffle giữ bài đang chờ ngoài original; `append` không đụng original; repeat khi original rỗng vòng qua các bài đã nghe; dòng timeline bốn phần trở lên bị bỏ qua. Chọn nhỏ: `play_random` không chọn lại bài đang phát (bản cũ có thể, thành ra không làm gì); `from_session` không xáo lại; không có bài mà bật shuffle vẫn đi cùng một đường, không chỉ đổi cờ; `fill` của khúc rỗng là 0.
- Không chạy thật: task này không có UI, không có đường chạy riêng.
- Soi: bảo mật, Pro không áp. UX: `next_track` không còn phụ thuộc original nên tệp ngoài không kẹt ở cuối hàng đợi (N6). RAM, hiệu năng: `append` ban đầu so từng cặp, mỗi lần so lại chuẩn hoá đường dẫn; đã đổi sang tập hợp ngay trong task, chưa đo vì không có số trước, sau nào ở thực tế.

## 10. Đề xuất cải tiến

Không có.