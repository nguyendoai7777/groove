# groove-gd1_10 — audio-engine

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~850 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `audio/{engine,decoder,output}.rs`; MỚI `crates/groove-audio/tests/realtime_no_alloc.rs`, `crates/groove-audio/examples/audio_check.rs`; SỬA `crates/groove-audio/Cargo.toml` (G-2) |

## 0. Bàn giao từ groove-gd1_11 (02/11/2026)

<!-- handoff: groove-gd1_11 -->

- **Đã có sẵn để dùng**: `crate::dsp::{EqParams, Equalizer}`. `AudioCmd::SetEq(EqParams)` dùng đúng kiểu này. `Equalizer::new(sample_rate, channels)`, `set_params(&EqParams)`, `process(&mut [f32])` xen kẽ, không cấp phát, gọi ở luồng decoder trước khi vào ring.
- **Phải gọi**: `reset()` khi tua và đổi bài; `set_format(rate, channels)` khi định dạng đổi. Áp sau khi đổi tần số mẫu bằng `rubato` thì truyền tần số sau đổi.
- **Vẫn ở task này**: âm lượng đường cong `v²` (S23) ở callback.

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Nợ chuyển sang đây**: mã đụng `cpal` đặt sau `#[cfg(any(not(target_os = "linux"), feature = "linux-alsa"))]`, nhánh còn lại trả `Error`; `libasound` chưa được chấp nhận, không ghi là đã chấp nhận (đặc tả 5.1).

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 10. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Linux: quyết định `libasound` (LGPL, link động) đang chờ chủ dự án. Làm và kiểm trên Windows, code Linux biên dịch sau `cfg`, chưa coi Linux là đạt.
- Thử HE-AAC với thư viện thật, ghi tỉ lệ tệp không phát được ở mục 9.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`; nghe thật và Linux, macOS `[owner-qa]`.

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | 200 Load liên tiếp, đường dẫn giữa không tồn tại ⇒ chỉ `Loaded` của bài cuối, không `Error` nào | `[opus-unit]` `rapid_next_only_opens_the_last_track` |
| C2 | `Ended` cũ nằm trong kênh khi Load mới ⇒ không tới UI; Seek, Load sai `generation` bị bỏ | `[opus-unit]` `stale_*` |
| C3 | Tua ⇒ âm cũ không quá 5 ms fade, rơi đúng frame đích, vị trí nhảy ngay | `[opus-unit]` `seek_lands_exactly_*`; codec thật `[opus-run]` `audio_check verify` |
| C4 | Decoder xong, ring còn ⇒ chưa `Ended`; lấy hết ⇒ `Ended`, đủ mọi frame | `[opus-unit]` `plays_every_frame_*` |
| C5 | Stream lỗi, đổi 48 kHz sang 44,1 kHz ⇒ mở lại, giữ vị trí, đủ phần còn lại; không thiết bị ⇒ thử lại rồi `Error { Output }`; callback đứng ⇒ coi như mất stream | `[opus-unit]` `device_loss_*`, `rerouted_*`, `stalled_*` |
| C6 | Callback không cấp phát (đếm thật), không khoá (chỉ atomic + ring SPSC) | `[opus-unit]` `tests/realtime_no_alloc.rs` |
| C7 | Chưa Play không mở stream; dừng quá hạn thì đóng, Play tiếp đúng chỗ | `[opus-unit]` `no_stream_until_played_*`, `idle_closes_*` |
| C8 | Tệp mất, tệp rác, decoder panic ⇒ `Error`, engine chạy tiếp | `[opus-unit]` `bad_files_*`, `panic_in_decoder_*` |
| C9 | 2000 lệnh dồn dập khi callback đang chạy ⇒ không kẹt, chỉ sự kiện của bài cuối | `[opus-unit]` `hammering_*` |
| C10 | Nghe thật: tua, rút tai nghe, đổi thiết bị mặc định, âm lượng | `[owner-qa]`, xem mục 9 |

## 8. Nợ chuyển task khác

- `groove-gd1_13`: dựng engine, tự sang bài theo `Error` (N5), xem HE-AAC ở mục 9.
- `groove/CLAUDE.md` mục `libasound` và đặc tả 5.1 còn ghi chưa được chấp nhận, không thuộc tệp của task này: người điều phối sửa theo G-2.

## 9. Bàn giao thực tế

**Kiểm** (cwd worktree, `-p groove-audio`): build, `clippy --all-targets -D warnings`, test 51 + 1 xanh, chạy lặp 5 lần phần engine vẫn xanh; `cargo deny check licenses` ok. Test không chạm thiết bị (`FakeOutput`). `cargo check` với `aarch64-apple-darwin` qua; `x86_64-unknown-linux-gnu --no-default-features` qua; Linux bật `linux-alsa` không kiểm được từ Windows (`alsa-sys` cần `pkg-config` của đích).

**Linux (G-2)**: `linux-alsa` đã vào feature mặc định của `groove-audio`, chỉ link động `libasound`, không link tĩnh, không kèm vào bản cài. `Cargo.lock` không đổi. **Linux chưa đạt** cho tới khi chạy thử trên Ubuntu `[owner-qa]`.

**API đã khoá** (`groove_audio::engine`): `AudioEngine::spawn(Box<dyn AudioOutput>, EngineOptions) -> io::Result<(AudioEngine, AudioEvents)>`; `next_generation`, `send(AudioCmd)`, `position_sec`, `is_playing`. `AudioEvents`: `Stream` (`next().await`) và `try_recv`, đã lọc `generation` cũ. `AudioCmd`, `AudioEvent` đúng đặc tả 5.1, `kind` là `AudioErrorKind { NotFound, Io, Unsupported, Corrupt, Output }`. Bản phát hành truyền `output::default_output()`; test dùng `output::FakeOutput`.

**Định dạng**, đo bằng tệp tự sinh qua ffmpeg (`audio_check verify`), lệch sau tua không quá 1 frame:

| Phát được | Không |
| --- | --- |
| MP3 (có, không Xing), FLAC (cả 5.1), WAV, OGG Vorbis, M4A AAC-LC, M4A ALAC, M4R, ADTS `.aac` | Opus (`Unsupported`) |

HE-AAC báo tường minh trong ASC: `make_audio_decoder` từ chối (đọc mã `symphonia-codec-aac` `aac/mod.rs:91`), vào `Error`; không có tệp mẫu để chạy.

**Thư viện thật**, đọc `groovex.db` chế độ chỉ đọc (`immutable=1`) và đọc tệp, không sửa gì: 1002 bài, gồm 979 MP3, 20 M4A, 3 FLAC. Cả 1002 mở và giải mã được gói đầu. **18/1002 (1,8%) là HE-AAC theo `ffprobe`** (18 trong 20 M4A), báo ngầm nên **không vào `Error`**: symphonia chỉ giải lõi LC ở 22 050 Hz, đúng độ dài (đo trên cả 18 tệp) nhưng **mất dải cao trên khoảng 11 kHz** (suy từ tần số mẫu, chưa nghe). Khác đặc tả 5.1, xem `plan/confirm.md`. Phần lớn là tệp dài (tới 80 phút).

**Lỗi thư viện đã né**:

- MP4 của symphonia 0.6.1: tua sau khi đọc tới cuối tệp thì mọi gói báo `no atom pending read`, nên phát lại sau `Ended` sẽ hỏng. `TrackDecoder::seek` mở lại tệp trong trường hợp này.
- Một MP3 thật có hai thẻ ID3 liền nhau làm probe thất bại: thử lại sau khi bỏ các thẻ đầu tệp (`SkipPrefix`).

**Quyết định** (Opus chọn 02/11/2026):

- `SetVolume` áp `v²` (S23) ở callback.
- `position_sec` đếm mẫu đã rời ring nên đã trừ độ trễ ring; bộ đệm trong thiết bị không trừ.
- Thiết bị đổi: WASAPI của cpal 0.18 tự báo `StreamInvalidated` (đọc mã, chưa chạy thật), không cần dò định kỳ. Thêm canh callback đứng 3 giây.
- Hết bài rồi Play không tua: báo `Ended` lại, không kẹt ở trạng thái đang phát.
- Thêm `is_playing`, `next_generation`, `EngineOptions`, ngoài danh sách đặc tả, cần cho test và cho N8.

**Cỡ**: mã không tính test khoảng 1 600 dòng sau `cargo fmt`, vượt ước lượng 850; không tách vì ba tệp cùng một đường đi.

**Chờ kiểm tay `[owner-qa]`** (có phát ra loa):

```bash
cargo run -p groove-audio --example audio_check -- play "<tệp>"   # gõ: p, s <giây>, v <0-1>, q
```

Mong đợi: tua không lách tách, hết bài không cụt đuôi, rút tai nghe vẫn phát tiếp đúng chỗ trên loa, đổi thiết bị mặc định theo sang (có dòng `DeviceChanged`), âm lượng không nhảy. Nghe thử một bài HE-AAC so với bản cũ. Làm lại trên macOS (Intel, Apple Silicon) và Ubuntu; riêng Ubuntu cần `libasound2-dev`.

Soi cải tiến: bảo mật không áp ngoài S25 (đã bọc `catch_unwind`), đường dẫn và tên tệp không vào log. UI/UX không áp (chưa có UI). RAM: chưa đo tiến trình thật; ring 200 ms khoảng 77 KB ở 48 kHz stereo, không stream khi chưa phát, đóng sau 30 giây. Pro: không áp.

## 10. Đề xuất cải tiến

- `[ux]` HE-AAC mất dải cao — 18/1002 bài thật, mục 9 — cần bộ giải có SBR hoặc báo trên UI — giữ chất lượng như bản cũ — M — P2, chờ chủ dự án.
