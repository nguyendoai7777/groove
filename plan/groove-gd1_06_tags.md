# groove-gd1_06 — tags

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~850 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/tags.rs`, `library/fixtures.rs` |

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Đã có**: `lofty` 0.25, `id3` 1 đã khai; `tempfile` là dev-dep, fixture ở `library/fixtures.rs` chỉ dùng std hoặc `id3`, `lofty`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 06. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

| Phần việc | Vai |
| --- | --- |
| `library/tags.rs`, `library/fixtures.rs`, test | `[opus]` |
| Tương thích với player khác trên máy thật (Explorer, iTunes) | `[owner-qa]` |

## 6. Nghiệm thu

Đều `[opus-unit]` trong `tags.rs`, trừ C11, C12.

- C1. Cho tệp mẫu 6 định dạng, khi đọc, thì ra khoảng 1 giây, trường trống.
- C2. Cho mọi định dạng và chế độ, khi ghi đủ trường, thì đọc lại đúng từng trường; phần âm thanh còn nguyên; M4A `stco` trỏ đúng khung đầu.
- C3 (N2). Cho tệp có `TIMELINE`, khi ghi ở cả 3 chế độ trên 6 định dạng, thì `TIMELINE` còn. `Some(tl)` đẩy vào tệp, `Some("")` xoá. `TXXX:TIMELINE` bản cũ ghi được giữ, không tính là rác.
- C4. 6 test của bản cũ (`tags.rs:744-992`) chạy trên tệp tự sinh, không còn đường `return` âm thầm (M6).
- C5. `TDRC` đọc được và không làm năm đã xoá hiện lại; Merge chỉ sửa tổng số vẫn giữ số thứ tự; tệp chỉ có ID3v1 giữ giá trị khi `clean_keep`; `APE`, `ID3v1` bị bỏ.
- C6 (S17). `A\0B` đọc thành `A; B`, ghi v2.3 không còn `\0`, cả ở Merge; Vorbis nhiều `ARTIST` cũng nối.
- C7. Bìa `Set`, `Keep`, `Remove` đúng ở mọi định dạng và chế độ; không phải ảnh, quá trần thì từ chối; 20 MiB qua được trừ FLAC (trần 16 MiB của định dạng, báo lỗi).
- C8 (N3). Lỗi ở đâu thì tệp gốc nguyên vẹn, không sót tạm; tạm cũ do sập được thay; tệp chỉ đọc từ chối; `verify` bắt trường ghi trượt, mất timeline, lệch thời lượng.
- C9. Đang phát: handle `File::open` cũ đọc tiếp được khi tệp bị thay. Windows: app khác giữ không cho xoá thì `TagError::Replace`, tệp gốc nguyên.
- C10 (S25). Cắt cụt, lật byte ở 6 định dạng không panic ra ngoài; trần độ dài khi đọc.
- C11 `[opus-run]`. ffprobe, ffmpeg đọc và giải mã được tệp đã ghi: tệp mẫu và tệp tự mã hoá bằng ffmpeg (MP3, FLAC, WAV, M4A, OGG Vorbis, OGG Opus). Lệnh: test `dump_for_ffprobe` (`#[ignore]`, biến `GROOVE_DUMP`).
- C12 `[owner-qa]`. Mở tệp đã ghi bằng Explorer, một player khác: thấy đúng album, bìa. Chờ kiểm tay.

## 9. Bàn giao thực tế

**Kết quả**: `-p groove-library`: build, clippy `-D warnings`, 43 test xanh (1 `#[ignore]`), `cargo deny check licenses` xanh. Đã thử bằng ffprobe như C11; không dùng thư viện nhạc thật.

**API đã khoá** (`groove_library::tags`):

- `read_track(&Path) -> Result<ScannedTrack, TagError>`: một lần đọc đủ `fields`, `cover`, `duration_ms`, `stamp` (S14).
- `write_tags(&Path, &MetadataEdit, WriteMode) -> Result<FileStamp, TagError>`: ghi qua `<tên>.groove-tmp`, đọc lại, thay tệp.
- `read_raw_frames(&Path) -> Result<Vec<RawFrame>, TagError>`.
- `TagFields`, `MetadataEdit { fields, cover }`, `CoverEdit { Keep, Remove, Set(Vec<u8>) }`, `CoverArt`, `FileStamp { size, mtime_ms }`, `WriteMode`, `TagError`, `APP_FRAMES`, `TEMP_SUFFIX`, `is_temp_path`.
- `fixtures`: `AudioKind`, `sample_bytes`, `audio_bytes`, `write_sample`, `write_mp3_with_frames`, `append_id3v1`, `has_id3v1`, `TINY_PNG`.

**Opus chọn 02/11/2026** (có ở `plan/confirm.md`):

- WAV đi qua `id3`, không phải `lofty` như 5.8: thẻ chung của `lofty` không đặt được `TXXX`.
- FLAC, OGG, M4A ghi trên thẻ riêng của định dạng, không qua `TaggedFile`: `TaggedFile` lưu lại từ thẻ chung nên làm rơi `TIMELINE` và mọi khoá lạ, kể cả ở Merge. `verify` đã bắt được đúng lỗi này. M4A lưu `TIMELINE` ở `----:com.apple.iTunes:TIMELINE`.
- `CoverEdit::Keep` giữ bìa cả ở `Clean`, vì form không phải lúc nào cũng gửi lại bìa.
- Bỏ `RawFrame.name`: nhãn hiển thị do task 19 tra theo `id`, chữ đi qua i18n.
- M4A không có khoá `Publisher`, lofty âm thầm bỏ, nên ghi vào `LABEL`.

**Lệch so với plan**: 957 dòng mã `tags.rs` (ước 850, trần 900), cộng 338 dòng `fixtures.rs` là hạ tầng test. Không tách vì task chỉ sở hữu hai tệp và `lib.rs` đã khoá. Thừa nhận tại đây, ghi cả `plan/confirm.md`.

**Bẫy**:

- Tuỳ chọn toàn cục của `lofty` theo từng luồng: `guarded` đặt lại ở mỗi lần gọi, trần cấp phát 64 MiB thay cho 16 MiB.
- `TagType::Ape.remove_from_path` để sót thẻ trên MP3, nên tự cắt theo footer.
- OGG mẫu có phần setup giả: lofty đọc được, ffmpeg không mở được. Cần tệp giải mã được thì đừng dùng mẫu OGG (task 10).

**Soi cải tiến**: bảo mật: panic thành `Error`, trần độ dài và cỡ bìa, không log nội dung thẻ. RAM: `read_track` giữ bìa trong bộ nhớ đến khi caller nhả, xem bàn giao 08. UI/UX: lỗi thay tệp phân biệt được qua `TagError::Replace`, chữ cho người dùng là việc của task 18, 19. Pro: không áp.

**Việc chưa làm**: C12 chờ kiểm tay (`plan/_backlog.md`). Đổi tên tệp, DB, `note_self_write`, đọc lại lyrics từ DB khi `Clean` là của task 18.
