# groove-gd1_13 — player-controller

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_10, groove-gd1_11, groove-gd1_12 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/player.rs`, `app/session_sync.rs` (N5, N6, N8) |

## 0. Bàn giao từ groove-gd1_10 (02/11/2026)

<!-- handoff: groove-gd1_10 -->

- **Đã có sẵn để dùng**: `groove_audio::engine::{AudioEngine, AudioEvents, AudioCmd, AudioEvent, AudioErrorKind, EngineOptions}`; `AudioEngine::spawn(output::default_output(), EngineOptions::default())`. Chi tiết: `plan/groove-gd1_10_audio-engine.md` mục 9.
- **Cách nối**: `next_generation()` cho mỗi Load; nhận sự kiện bằng `events.next().await`, đã lọc `generation` cũ, không cần lọc lại. `is_playing()` là nguồn duy nhất của trạng thái phát; sau `Ended`, `Error` nó là `false`.
- **N5**: `Error { kind: Output }` là lỗi thiết bị, không phải lỗi tệp: không tự sang bài, kẻo chạy hết hàng đợi. Các `kind` khác mới tính vào 3 lỗi liên tiếp.
- **Tua sau `Ended`**: Seek không tự phát, gửi thêm `Play` (repeat one).
- **EQ**: `SetEq(EqParams::from(&settings))`, xem bàn giao của groove-gd1_11.
- **Test**: `output::FakeOutput` tự kéo mẫu bằng `pull`, không ra loa; đặt `stall_timeout` thật lớn khi test tự kéo.
- **Soát lại khi làm**: `Loaded.duration_sec` là 0 khi tệp không khai độ dài.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 13. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_11 (02/11/2026)

<!-- handoff: groove-gd1_11 -->

- **Đã có sẵn để dùng**: `groove_audio::dsp::EqParams`; `EqParams::from(&settings.eq)` lấy `effective_gains()`, đã có bass boost, đừng cộng lần nữa.
- **Việc của task này**: gửi `SetEq` lúc mở app (cả khi phẳng) và mỗi khi EQ đổi. Đổi gain tự trượt 10 ms trong DSP, gửi liên tục khi kéo thanh trượt được.
- **Cần biết**: preamp tự động làm cả bài nhỏ đi khi tăng dải, đúng S23; chi tiết ở `plan/groove-gd1_11_audio-dsp.md` mục 9.

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `session::{Session, SAVE_INTERVAL}`, `settings::JsonSaver` cho `session_sync`: `JsonSaver::spawn(paths::session_file()?, debounce)`, `.save(&session)` không chạm đĩa, `.flush()` khi thoát (chặn).
- **Khôi phục**: `Session::load()` lúc mở app, `retain_tracks` để bỏ bài không còn trong thư viện (N5); mất bài hiện tại thì `current = None`, hàng đợi vẫn còn, task này quyết phát gì tiếp. Không tự phát (đặc tả 1.4).
- **Âm lượng, EQ**: `Settings::{volume, is_muted}`, `EqSettings::effective_gains()` là mức đưa vào DSP, đã cộng bass boost và kẹp ±10 dB.
- **`Song::external(path)`** cho tệp ngoài thư viện; hiển thị qua `display_title`, `display_artist`.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N5, N6, N8 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
