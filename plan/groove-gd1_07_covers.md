# groove-gd1_07 — covers

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~350 dòng |
| Phụ thuộc | groove-gd1_04 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/covers.rs` |

## 0. Bàn giao từ groove-gd1_04 (02/11/2026)

<!-- handoff: groove-gd1_04 -->

- **Phải làm**: `impl db::migrate::CoverSink`: `store(&mut self, image: &[u8]) -> anyhow::Result<CoverRecord>`, ghi `<hash>_<256|512>.jpg`, trả `CoverRecord { hash, accent_color, width, height }`. Migration tự chèn dòng `covers` và gán `cover_hash`, sink không chạm DB.
- `image` là byte ảnh gốc đã giải base64, có thể hỏng: trả `Err`. Migration bắt cả lỗi và panic, chỉ mất bìa dòng đó.
- **Bảng đã có**: `covers(hash PK, accent_color NN, width, height)`, `cover_hash` ở `songs`, `albums`, `folders`, `albums.cover_is_custom`.
- **Soát lại khi làm**: sink chạy trong transaction của migration nên đừng mở kết nối DB khác; `covers_dir()` có thể chưa tồn tại, tự tạo. Test mẫu: `crates/groove-library/tests/migrate_legacy.rs`.
Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 07. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`, không giao subagent. Tệp: `library/covers.rs`, test mới `crates/groove-library/tests/covers_{migrate,memory}.rs`.

## 6. Nghiệm thu

Case C1-C10 `[opus-unit]` trong `covers.rs`, C11, C12 test tích hợp.

- C1. Cho ảnh 800 × 800. Khi `save`. Thì có `<sha256>_256.jpg`, `<sha256>_512.jpg` đúng cỡ, là JPEG, bản ghi mang cỡ gốc.
- C2. Cho cùng ảnh lưu hai lần. Thì bản ghi giống nhau, tệp không ghi lại. Xoá một tệp rồi lưu lại thì có lại.
- C3. Ảnh 1000 × 500 ra 512 × 256 và 256 × 128; 100 × 50 không phóng to; 3000 × 1 không ra cạnh 0.
- C4. Accent: `#285078` giữ nguyên, sáng đúng 120 không kẹp, trắng thành `#787878`, ví dụ tính tay `#c16325`; trung bình hai nửa màu; điểm trong suốt không tính; trong suốt hoàn toàn thành `#000000`.
- C5. Rỗng, rác, PNG cụt, JPEG cụt thành `Err`, không tệp nào; 200 lần hỏng ngẫu nhiên trên 5 định dạng không panic, không sót `.tmp`.
- C6. PNG khai 9000 × 9000 và cạnh 16 385 thành `ImageError::Limits`; tệp vào quá 64 MiB thành `TooLarge`.
- C7. Đọc được PNG, JPEG, GIF, BMP, WebP, 16 bit, xám.
- C8. Thư mục chưa có thì tạo; không tạo được thì `Write`.
- C9. EXIF xoay 90 độ: bản ghi và tệp đều đổi chiều.
- C10. 8 luồng lưu cùng ảnh đều thành công.
- C11. `open_or_migrate` với sink thật: thư mục và album chung ảnh thành một dòng `covers`, accent thật thay `#06b6d4`, ảnh hỏng chỉ mất bìa dòng đó.
- C12. Ảnh 4000 × 4000: đỉnh cấp phát không quá giải mã + 16 MiB, nhả hết sau khi xong.

## 8. Nợ chuyển task khác

- `groove-gd1_08`: nối `CoverStore` vào luồng đọc thẻ, dòng `covers` do indexer chèn.
- `groove-gd1_05`: truyền `CoverStore::default_location()?` cho `open_default` ở đường chạy thật.
- `groove-gd1_18`: `CoverEdit::Set`, `set_category_cover` đi qua `CoverStore::save`.

## 9. Bàn giao thực tế

**API đã khoá** (`groove_library::covers`):

- `CoverStore::new(dir)`, `default_location() -> io::Result<Self>`, `dir()`, `path(hash, CoverSize) -> PathBuf`, `has_files(hash) -> bool`, `save(&self, &[u8]) -> Result<CoverRecord, CoverError>`; `impl CoverSink`. `Clone + Send + Sync`, không giữ trạng thái.
- `CoverSize::{Small, Large}`, `.pixels()`; `cover_hash(&[u8]) -> String`; `CoverError::{Empty, TooLarge, Image, Panicked, Write}`; hằng `MAX_INPUT_BYTES`, `MAX_DECODED_BYTES`, `MAX_DIMENSION`.

**Soát**: `cargo build`, `clippy --all-targets -D warnings`, `test` với `-p groove-library` xanh (lib 25, covers_memory 1, covers_migrate 1, migrate_legacy 20), `cargo fmt --check`, `cargo deny check licenses` ok. Không mở DB thật.

**Đo**: ảnh JPEG 4000 × 4000 (giải mã 45 MiB), đỉnh cấp phát 48 MiB, còn lại 72 byte sau `save` (C12, bản debug). Bản gốc nhả ngay sau bước thu về 1024 px, nên đỉnh chỉ cao hơn cỡ giải mã 3 MiB. Số này là cấp phát qua allocator, không phải RSS.

**Opus chọn 02/11/2026** (đã ghi `plan/confirm.md`):

- `hash` là SHA-256 của byte ảnh gốc, không phải của điểm ảnh: tính được trước khi giải mã nên caller tránh được giải mã lặp.
- Giữ tỉ lệ, không cắt vuông, không phóng to: `_512.jpg` có thể nhỏ hơn 512. Cắt vuông là việc của lớp hiển thị.
- Trần: tệp vào 64 MiB, giải mã 192 MiB (RGB 8000 × 8000), cạnh 16 384.
- Accent tính theo alpha, đúng với canvas cũ (đọc từ `now-playing.vue:393-406`), đặc tả không nói. Tệp JPEG phủ alpha lên nền đen.

**Lệch nhỏ so với đặc tả 5.2**: mẫu 32 × 32 lấy từ bản đã thu về 1024 px, không phải bản gốc, để không giữ bản gốc lâu hơn cần; ảnh màu khối cho đúng cùng giá trị (C4). Lọc `Triangle` trên ảnh alpha làm màu ẩn lem vài đơn vị ở mép, chấp nhận (C4 ghi rõ khoảng).

**Bẫy**:

- `ImageReader::into_decoder` không soát `max_alloc`, chỉ `decode` mới soát. Cần đọc EXIF nên phải qua `into_decoder`, vì vậy tự gọi `limits.reserve(decoder.total_bytes())`.
- PNG chỉ có `IHDR` báo `UnexpectedEof` chứ không báo vượt trần, test ảnh bom phải có thêm `IDAT`.
- `save` đã có `catch_unwind` riêng, nên caller ngoài migration (indexer) không phải bọc lại. Panic hook mặc định vẫn in ra stderr.

**Lăng kính**: bảo mật: `Limits` và trần tệp vào chặn ảnh bom, log không chứa dữ liệu ảnh. UI/UX: không áp. RAM: số đo trên; `save` song song cộng dồn đỉnh giải mã, xem mục 10. Pro: không áp.

**Chưa làm**: không có việc chờ kiểm tay. Chưa nối vào đường chạy thật (task 05, 08), chưa đối chiếu màu với 8 album thật vì không được mở DB thật; màu đục thì đặc tả 5.2 cho đổi sang histogram.

## 10. Đề xuất cải tiến

- [ram] Nhiều worker gọi `save` cùng lúc thì đỉnh giải mã cộng dồn, trần là 192 MiB mỗi ảnh — `covers.rs` `decode_limited` — task 08 giới hạn một luồng giải mã bìa và soát `has_files` + `covers` trước khi `save` — tránh đỉnh RAM khi quét — S — P2. Ghi vào bàn giao task 08.
- [ram] Tệp `.tmp` có thể sót nếu app chết giữa lúc ghi — `write_atomic` — quét dọn lúc mở app — chỉ tốn vài trăm KB trong thư mục cache — S — P3, chưa làm.
- [ram] Không có dọn tệp bìa mồ côi khi bìa bị thay hoặc bài bị xoá — chưa có caller nào xoá — task 18 hoặc 09 cần quyết định lúc đổi bìa — S — P3, chưa làm.