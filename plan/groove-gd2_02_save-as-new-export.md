# groove-gd2_02 — save-as-new-export

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (08/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | M — ~800 dòng (module `export` của groove-audio: cắt MP3 theo frame, WAV, FLAC, Media Foundation trên Windows) + test |
| Phụ thuộc | groove-gd1_10 (decoder) |
| Quyết định áp dụng | D-2, D-9, D-11, N7, S25 |
| Phụ thuộc mới | `flacenc` 0.5 (Apache-2.0, thuần Rust); `windows` 0.62 (MIT / Apache-2.0, đã có trong `Cargo.lock` qua GPUI, nay là phụ thuộc trực tiếp của groove-audio trên Windows) |
| File đụng tới | MỚI `crates/groove-audio/src/export/{mod,mp3_copy,pcm,mf}.rs`; SỬA `crates/groove-audio/src/lib.rs`, `crates/groove-audio/src/decoder.rs` (độ sâu bit), `crates/groove-audio/Cargo.toml` |

Chủ dự án (08/11/2026): "click chuột phải vào item song row là để cắt 1 bài nhạc - Lưu dạng mới: Save as new, cho phép cắt nhanh
theo duration và export nhiều loại định dạng như: current, mp3, m4r." Task này là phần âm thanh; hộp thoại và menu ở
`groove-gd2_03`.

## 1. Làm gì

- `groove_audio::export`: cắt đoạn `[start, end)` của một tệp, ghi tệp mới; chạy ở luồng riêng (N7), có tiến độ 0..=1 và huỷ.
- Ba định dạng: `current` (giữ định dạng gốc), `mp3`, `m4r` (AAC trong MP4, nhạc chuông iPhone, báo khi đoạn > 40 s).
- Cách làm theo nguồn × đích, ưu tiên không mã hoá lại:
  - MP3 → `current` / `mp3`: chép nguyên frame MPEG (thuần Rust, mọi OS), giữ thẻ ID3v2 gốc, bỏ frame Xing / Info / VBRI.
  - WAV → `current`: ghi WAV PCM cùng độ sâu bit (16 / 24), mọi OS.
  - FLAC → `current`: giải mã rồi mã FLAC (`flacenc`), cùng độ sâu bit, mọi OS.
  - M4A / M4R → `current` / `m4r`: Windows chép nguyên mẫu AAC qua Media Foundation (Source Reader → Sink Writer); không chép
    được thì mã lại AAC.
  - Còn lại → `mp3` / `m4r`: giải mã (symphonia) → PCM 16 bit → bộ mã hoá MP3 / AAC của Media Foundation (Windows).
- Không bao giờ ghi đè tệp gốc; ghi ra `<đích>.part` rồi đổi tên; huỷ / lỗi thì xoá tệp tạm.
- Lựa chọn không có đường hợp lệ trên OS / nguồn đó ⇒ `FormatSupport` trả lý do, hộp thoại khoá lựa chọn.

## 2. Không làm trong task này

- Hộp thoại, menu, thêm vào thư viện (`groove-gd2_03`).
- Bộ mã hoá macOS (AudioToolbox) và Linux (GStreamer): chưa có máy để chạy (mục 8).
- Mã hoá Vorbis (OGG): không có bộ mã hoá thuần Rust; thư viện C (libvorbis) cần thêm toolchain, để sau (mục 8).

## 2b. Phân vai

Toàn bộ `[opus]` (FFI Media Foundation, đa luồng, định dạng nhị phân). Không giao Sonnet.

## 3. Kiểu dữ liệu

```rust
pub enum ExportFormat { Current, Mp3, M4r }           // serde snake_case
pub struct ExportRequest { source, dest: PathBuf, start_sec, end_sec: f64, format: ExportFormat }
pub enum Method { Mp3FrameCopy, Wav, Flac, AacCopy, EncodeMp3, EncodeAac }
pub enum Unavailable { NoEncoderOnThisOs, NoEncoderForSource }
pub fn plan(source: &Path, format) -> Result<Method, Unavailable>
pub fn extension(source, format) -> &str              // đuôi tệp đích
pub struct ExportJob { … }  // spawn(req) -> ExportJob; progress() -> f32; cancel(); try_finish() -> Option<Result<PathBuf, ExportError>>
pub const RINGTONE_MAX_SEC: f64 = 40.0;
```

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | MP3 fixture 3 s, cắt 1–2 s ⇒ tệp ra giải mã được, dài ~1 s (± 1 frame), byte frame trùng tệp gốc | `[opus-unit]` |
| C2 | Bảng `plan`: MP3 → copy, WAV → Wav, FLAC → Flac, OGG `current` ⇒ `NoEncoderForSource`; `mp3`/`m4r` trên OS không có MF ⇒ `NoEncoderOnThisOs` | `[opus-unit]` |
| C3 | WAV 16 bit, FLAC: cắt ⇒ giải mã lại đúng số mẫu, cùng tần số | `[opus-unit]` |
| C4 | Đích trùng tệp gốc ⇒ lỗi, tệp gốc không đổi; huỷ ⇒ không còn `.part`, không có đích | `[opus-unit]` |
| C5 | Windows: WAV → `mp3`, WAV → `m4r`, M4A → `m4r` (chép AAC) ⇒ tệp ra giải mã được bằng symphonia, độ dài khớp ± 0,1 s | `[opus-unit]` (Windows) |
| C6 | Tiến độ tăng dần tới 1, `try_finish` trả đúng một lần | `[opus-unit]` |

## 8. Nợ chuyển task khác

- **macOS**: AAC / M4R qua AudioToolbox (`ExtAudioFile`, kAudioFileM4AType) — có sẵn trong OS, hợp lệ D-2; chưa làm vì không có
  máy Mac để chạy test. MP3 trên macOS: AudioToolbox không có bộ mã hoá MP3 ⇒ cần thư viện ngoài (LAME là LGPL — phải hỏi chủ
  dự án). Ghi ở `plan/_backlog.md`.
- **Linux**: MP3 / AAC qua GStreamer nạp lúc chạy (G-3 chấp nhận cho quay video của fastcast; dùng cho groove cần chủ dự án
  xác nhận mở rộng G-3). Ghi ở `plan/_backlog.md`.
- OGG Vorbis giữ định dạng gốc: không có bộ mã hoá thuần Rust; `vorbis_rs` (BSD, build libvorbis bằng C) hợp lệ nhưng thêm
  toolchain C — chưa làm, hộp thoại khoá `current` cho OGG kèm lý do.

## 9. Bàn giao thực tế

- Module `crates/groove-audio/src/export/`: `mod.rs` (API, `plan`, `run`, `ExportJob`), `mp3_copy.rs`, `pcm.rs` (`ClipReader`, WAV,
  FLAC), `mf.rs` (Windows), `tests.rs`. `TrackDecoder::bits_per_sample` mới trong `decoder.rs`.
- **Giấy phép (D-2)**: `flacenc` 0.5 Apache-2.0 (tắt feature mặc định; kéo `crc`, `md-5`, `heapless`, `num-traits` — MIT /
  Apache); `windows` 0.62 MIT / Apache-2.0, đã có trong `Cargo.lock`. `cargo deny check licenses`: ok. Không LAME, không ffmpeg,
  không LGPL mới. Bộ mã hoá MP3 / AAC của Media Foundation là thành phần của Windows (8+), không phân phối kèm.
- **Tự quyết (D-9)**:
  - MP3 đích từ MP3 nguồn = chép frame (như `current`): không mã lại khi không cần.
  - Mã lại luôn ra **stereo**: bộ mã hoá MP3 của Microsoft trả `MF_E_INVALIDMEDIATYPE` (0xC00D36B4) với mono ở
    `SetInputMediaType` (đo trong test). MP3 320 kbps, AAC 192 kbps; tần số giữ 44,1 / 48 kHz (MP3 nhận cả 32 kHz), khác thì đổi
    bằng `decoder::Converter` (rubato) có sẵn.
  - Cắt MP3: giữ nguyên thẻ ID3v2 gốc (tên, nghệ sĩ, bìa — kể cả `TXXX:TIMELINE` / lời của bài gốc, nay lệch mốc: mục 10), bỏ
    frame Xing / Info / VBRI, bỏ ID3v1 / APE. Chỉ Layer III. Frame đầu có thể hụt ~26 ms do bit reservoir (mọi công cụ cắt
    không mã lại đều vậy).
  - WAV / FLAC giữ độ sâu bit: 24 nếu gốc > 16, còn lại 16 (float 32 bit ⇒ 24).
  - M4A có ALAC: Source Reader trả kiểu không phải AAC ⇒ mã lại AAC (mất tính lossless của ALAC; mục 10).
  - Mã lại không ghi thẻ (tên lấy theo tên tệp).
- **Bẫy đã gặp**: `flacenc` ghi `min_block_size` = cỡ khối cuối (ngắn), symphonia đọc STREAMINFO đó báo `UnexpectedEof` ngay lúc
  probe ⇒ đặt lại `set_block_sizes(block, block)` (đúng đặc tả FLAC: khối cuối không tính).
- Đường AAC chép nguyên đã chạy thật trên Windows trong test (`windows_cuts_aac_into_m4r`: mã WAV → M4A rồi cắt M4A → M4R, độ
  dài khớp ± 0,12 s).
- Soát xanh: `cargo test -p groove-audio` 66 test (12 test export), clippy `-D warnings`, `cargo deny check licenses`.

## 10. Đề xuất cải tiến

- Cắt MP3 / M4A: bỏ `TXXX:TIMELINE`, lời đồng bộ của bài gốc trong thẻ chép sang (mốc lệch sau khi cắt), hoặc dời mốc theo
  điểm đầu. Ghi tiêu đề "<gốc> (cut)" vào thẻ của bài mới.
- Mã lại: chép thẻ cơ bản (tên, nghệ sĩ, album, bìa) sang tệp mới qua `groove_library::tags`.
- ALAC: giữ ALAC (Sink Writer chép nguyên mẫu ALAC) thay vì mã AAC.
- Cắt MP3 chính xác mẫu: ghi khung Info/LAME mới có encoder delay / padding để trình phát gapless cắt đúng mép.
