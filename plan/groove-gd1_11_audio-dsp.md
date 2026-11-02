# groove-gd1_11 — audio-dsp

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~300 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `audio/dsp.rs` |

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Đã có**: `audio/dsp.rs` trống, không phụ thuộc `cpal`.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 11. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 6. Nghiệm thu — case test

Mọi case là `[opus-unit]` trong `audio/dsp.rs`, không phát tiếng (`cargo test -p groove-audio`).

| Case | Cho / Khi / Thì | Test |
| --- | --- | --- |
| C1 | 9 cấu hình dải, gain, tần số mẫu; đáp ứng tại 8 tần số lệch dưới 0,001 dB so với `getFrequencyResponse` của Chromium 152 | `matches_web_audio_reference` |
| C2 | Dải ở từ Nyquist trở lên thành đơn vị, đúng như Chromium đo ở 16 kHz, 22,05 kHz, 32 kHz | `band_at_or_above_nyquist_is_identity_like_web_audio` |
| C3 | Hệ số theo tần số mẫu, đỉnh luôn đúng tâm dải từ 22,05 đến 192 kHz; ổn định từ 8 đến 384 kHz | `coefficients_depend_on_sample_rate`, `filters_are_stable_for_every_rate_and_gain` |
| C4 | Sin qua `process` đo RMS lệch dưới 0,02 dB so với công thức | `processed_sine_matches_computed_response` |
| C5 | Trạng thái riêng từng kênh: kênh phải im lặng vẫn đúng 0, kênh trái trùng bản mono từng bit | `channels_keep_separate_state` |
| C6 | Đổi -10 → +10 dB không lách tách, nhảy gain thẳng tệ hơn trên 10 lần; hằng thời gian 10 ms | `gain_change_does_not_click`, `gain_follows_ten_millisecond_time_constant` |
| C7 | Kẹp ±10, NaN và vô cùng về 0; bass boost cộng đúng một lần | `gains_are_clamped_and_sanitized`, `gains_from_settings_include_bass_boost_once` |
| C8 | Preamp: một dải bằng `-gain`; mọi đường cong có đỉnh chuỗi dưới 0,1 dB; toàn âm thì 0 | `preamp_equals_band_gain_for_single_band`, `preamp_keeps_whole_chain_at_or_below_unity` |
| C9 | Phẳng trả nguyên từng bit; bật, tắt EQ giữa bài không nhảy mẫu; về lại đi tắt | `flat_is_bit_exact_and_allows_odd_block_sizes`, `leaving_and_entering_bypass_is_seamless` |
| C10 | `reset` quên âm cũ; `set_format` tính lại; 0 Hz, 0 kênh không sập; mẫu hỏng không làm câm; im lặng về đúng 0 | `reset_forgets_previous_audio`, `set_format_recomputes_for_new_stream`, `bad_samples_do_not_poison_state`, `silence_decays_to_exact_zero` |

## 8. Nợ chuyển task khác

- `groove-gd1_10`: gắn `Equalizer` vào luồng decoder, `AudioCmd::SetEq(EqParams)`, `reset` khi tua, `set_format` khi đổi bài. Âm lượng đường cong `v²` (S23) áp ở callback, không thuộc `dsp.rs`.
- `groove-gd1_13`: gửi `SetEq(EqParams::from(&settings.eq))` lúc mở app và mỗi khi EQ đổi.

## 9. Bàn giao thực tế

- **API**: `groove_audio::dsp::{EqParams, Equalizer}`. `EqParams { gains_db: [f32; 10] }`, `From<&EqSettings>`. `Equalizer::{new(sample_rate: u32, channels: usize), set_params(&EqParams), set_format(sample_rate, channels), reset(), process(&mut [f32])}`. Số dải, tần số, biên dùng lại `groove_core::settings`.
- **Opus chọn 02/11/2026: preamp theo đỉnh đáp ứng thật** thay vì `-max(gain dương)` của S23 — vì các dải chồng lấn cộng dồn: toàn +10 dB đo được đỉnh +15,06 dB, công thức cũ còn dư 5 dB, vẫn rè. Một dải thì hai cách như nhau. Không thêm limiter. Hệ quả cần biết: kéo một dải lên thì cả bài nhỏ đi đúng ngần ấy, đúng như S23 cảnh báo.
- **Opus chọn 02/11/2026: bass boost nằm ở `EqSettings::effective_gains` (task 02)**, DSP nhận gain đã cộng, không lặp luật.
- **Đối chiếu Web Audio bằng số đo thật**: chạy `OfflineAudioContext` + `getFrequencyResponse` trong Chromium 152 ngày 02/11/2026, ghi vào test.
- **Đo (release, Windows)**: 1 giây stereo 48 kHz, 10 dải: 6,2 ms (0,6% một lõi). Tính preamp mỗi lần `set_params`: 59 µs. `process`, `set_params` không cấp phát.
- **Soát**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` với `-p groove-audio` (18 test xanh), `cargo deny check licenses` xanh, sau khi gộp `master`.
- **Soi cải tiến**: bảo mật không áp. UI/UX: không áp, chưa có UI. RAM / hiệu năng: số đo trên, phẳng thì đi tắt, không tốn gì cho người không dùng EQ. Pro: không áp.
- **Giới hạn đã biết**: cú gãy độ dốc lúc preamp bắt đầu trượt (khoảng 14 lần độ cong của sin 100 Hz, -52 dB) là bản chất của `setTargetAtTime`, bản cũ cũng có; không phải nhảy mẫu. Chưa nghe tai thật, chờ task 10 nối luồng.
