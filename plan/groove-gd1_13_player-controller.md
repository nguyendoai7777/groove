# groove-gd1_13 — player-controller

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
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

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Tra bài**: `Library::song(TrackRef)`, `songs_for_tracks(Vec<TrackRef>)` giữ thứ tự, bỏ bài không còn (N5). `None` với `External` nghĩa là tệp ngoài thật, dùng `Song::external`. `track_for_path(path)` cho N6, `song_text(id)` cho lyrics, timeline bài đang phát (S16).
- **Nghe**: `Library::subscribe()` trả `Stream<LibraryEvent>`; `Synced(report).removed` là `id` cần bỏ khỏi hàng đợi.
- **Mở thư viện chưa có chủ**: đường chạy thật gọi `Library::open_standard()` một lần (không chặn), giữ một bản; khi thoát `block_on(library.shutdown())`. Ai làm trước trong 13, 15, 17 thì gọi trong `app.rs`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 13. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 0. Bàn giao từ groove-gd1_12 (02/11/2026)

<!-- handoff: groove-gd1_12 -->

- **Đã có sẵn để dùng**: `groove_core::queue::{Queue, QueueOutcome}`. Mỗi thao tác đổi danh sách rồi trả `QueueOutcome`, controller đổi thành lệnh engine; hàng đợi không giữ trạng thái phát (N8).
- **Bảng đổi sang engine**: `Play(track)` nạp và phát từ đầu; `Resume` chỉ tiếp tục nếu đang dừng; `Replay` tua 0 và phát; `SeekToStart` tua 0, giữ trạng thái phát; `Stop` là `pause + seek(0)` (N8); `Nothing` không làm gì.
- **Sự kiện**: engine báo hết bài gọi `track_ended()`, không gọi `next_track()`: chỉ `track_ended` phân biệt repeat lặp một bài. Tên là `next_track`, `prev_track`, không phải `next`, `prev`.
- **Phiên**: `Queue::from_session(&session)` lúc mở, không tự phát, không xáo lại; `write_to(&mut session)` không đụng `position_sec`, controller tự giữ vị trí.
- **N5**: `retain_tracks(keep)` trả `true` khi mất bài đang phát; lúc đó `current() = None`, hàng đợi còn nguyên, `next_track()` vẫn chạy và không đẩy gì vào history. Tối đa 3 lỗi liên tiếp là việc của task này, `Queue` không đếm.
- **Thời lượng**: `timeline::parse(text, duration_sec)` cần thời lượng để chốt đoạn cuối; đổi bài xong có thời lượng thật thì parse lại.
- **Test**: `Queue::with_seed` cho kết quả lặp lại được; `Queue::new` lấy hạt giống từ OS.
- **Chi tiết**: `plan/groove-gd1_12_queue-timeline.md` mục 8, 9.

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

## 6. Nghiệm thu

Phủ bằng `#[gpui::test]` trong `player.rs` (`[opus-unit]`), case `C1`…`C21` ghi ở đầu mỗi test: N8 (C1, C2), N5 (C3–C6, C13–C15), N6 (C7–C9), repeat, sự kiện cũ, Prev (C10–C12), phiên (C16–C19, C21), âm lượng + EQ (C20).

## 9. Bàn giao thực tế

**Kiểm** (cwd worktree): `cargo build`, `clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, `cargo deny check licenses` xanh. Test player chạy lặp 5 lần vẫn xanh. Test dùng `FakeOutput`, thư mục tạm, DB tạm: không ra loa, không chạm DB, phiên thật.

**Chưa kiểm**: đường chạy thật `player::install` (`SettingsSource::App`, `default_output`) chỉ biên dịch, chưa mở app, chưa nghe (`[owner-qa]`, máy dùng thật, chưa có thanh phát trước task 14).

**API đã khoá** (`crate::player`):

- Lấy: `player(cx) -> Entity<Player>`, `install(cx)`, `Player::spawn(PlayerDeps, cx)`, `MAX_CONSECUTIVE_ERRORS`.
- Đọc: `queue()`, `current_track()`, `current_song()`, `is_playing()`, `position_sec()`, `duration_sec()`, `lyrics()`, `timeline()`, `external_cover()`.
- Hàng đợi: `play_list(track, list, cx)`, `play_track(track, cx)`, `append(tracks, cx)`, `play_paths(paths, cx)`, `next`, `prev`, `play_random`, `toggle_shuffle`, `toggle_repeat`, `clear`.
- Điều khiển: `toggle_play`, `play`, `pause`, `seek(sec, cx)`, `seek_by(delta, cx)`, `set_volume(u8, cx)`, `adjust_volume(i16, cx)`, `toggle_mute`, `update_eq(cx, |eq| ..)`, `apply_settings`, `refresh_current`.
- Sự kiện `PlayerEvent`: `TrackChanged`, `TrackFailed { track, title, kind, gave_up }`, `OutputFailed`, `DeviceChanged`.

**Quyết định** (Opus chọn 02/11/2026, ghi cả `plan/confirm.md`):

- Toast của N5 do task 14 làm từ `TrackFailed`, `OutputFailed`: chữ thuộc vùng `player`, player không có `Window`.
- `app.rs` không cho quan sát `Settings` từ ngoài, nên âm lượng, mute, EQ đi qua hàm của `Player`; ai sửa bằng `app::update_settings` thì gọi `apply_settings`.
- Lỗi tính "liên tiếp" reset khi: người dùng thao tác, `Ended`, hoặc bài đã phát được hơn 1 giây trước khi lỗi. `Loaded` không reset, vì `Corrupt` có thể tới sau `Loaded`.
- Bài của phiên hỏng lúc khôi phục: báo, không tự sang bài (đặc tả 1.4).
- Đọc phiên ở nền, không chặn UI thread (N7), có cổng `session_ready` để không ghi đè phiên cũ bằng phiên trống. Thao tác trước khi đọc xong thì bỏ phiên cũ; `play_paths` thì chờ.
- Tệp ngoài đã mất bị bỏ khỏi phiên lúc khôi phục, bằng `exists` ở nền.

**Cỡ**: mã không tính test khoảng 920 dòng `player.rs` + 60 dòng `session_sync.rs`, vượt ước lượng 650; không tách vì cùng một máy trạng thái.

**Soi cải tiến**: bảo mật: đường dẫn, tiêu đề không vào log. UI/UX: lỗi có sự kiện cho toast; Play sau lỗi thử nạp lại. RAM: tick chỉ chạy khi đang phát, bìa tệp ngoài nhả khi đổi bài, bìa bài thư viện không nạp ở đây. Pro: không áp.
