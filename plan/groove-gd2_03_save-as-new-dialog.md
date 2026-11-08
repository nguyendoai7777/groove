# groove-gd2_03 — save-as-new-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (08/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | M — ~650 dòng (hộp thoại, mục menu, menu hàng đợi, i18n) + test |
| Phụ thuộc | groove-gd2_02 |
| Quyết định áp dụng | D-9, D-10, D-13, G-4, G-6, N7 |
| Phụ thuộc mới | — |
| File đụng tới | MỚI `crates/groove-app/src/views/save_as_dialog.rs`, `i18n/{vi,en}/save_as.toml`; SỬA `crates/groove-app/src/views/song_menu.rs`, `crates/groove-app/src/views/queue_panel.rs`, `crates/groove-app/src/views/now_playing.rs`, `crates/groove-app/src/library_ctl.rs`, `crates/groove-app/src/views/mod.rs`, `crates/groove-core/src/i18n.rs` (vùng mới) |

## 0. Bàn giao từ groove-gd2_02 (08/11/2026)

<!-- handoff: groove-gd2_02 -->

- **Đã có sẵn để dùng**: `groove_audio::export::{plan, extension, ExportJob, ExportRequest, ExportFormat, ExportError, Method,
  Unavailable, RINGTONE_MAX_SEC, MIN_CLIP_SEC}`; `ExportJob::spawn` chạy luồng riêng, `progress()` / `cancel()` / `try_finish()`.
- **Đổi so với spec**: `ExportRequest` có thêm `overwrite` (đích người dùng đã đồng ý ghi đè; tệp gốc thì không bao giờ).
- **Bẫy**: `plan` khoá `mp3` / `m4r` ngoài Windows và `current` của OGG — hộp thoại phải hiện lý do, không ẩn.

## 1. Làm gì

- Menu chuột phải của hàng bài (mọi danh sách bài dùng `song_menu`) có thêm **Save as new**.
- Hộp thoại Save as new: điểm đầu / điểm cuối (thanh trượt + ô `mm:ss`), thanh đoạn đã chọn, định dạng (`current`, `mp3`,
  `m4r` — lựa chọn không dùng được bị khoá kèm lý do), báo khi `m4r` dài hơn 40 s, chỗ lưu + tên (mặc định cạnh tệp gốc,
  tên `<gốc> (cut).<đuôi>`, không trùng tệp có sẵn), "Thêm vào thư viện sau khi lưu", tiến độ + nút huỷ.
- Lưu xong: toast, tuỳ chọn thêm bài mới vào thư viện (viết plan ghi `LibraryCtl::import_path` — thêm cả thư mục làm gốc; làm thật bằng `LibraryCtl::add_file`, mục 9).
- Hàng trong hàng đợi (`views/queue_panel.rs`) có menu chuột phải: Edit / Add lyrics, Timeline, Metadata, Save as new — mở
  đúng các hộp thoại sẵn có (`open_text_dialog`, `open_metadata`).
- Chữ qua i18n vùng `save_as`; một độ đậm; màu nhấn từ theme.

## 2. Không làm trong task này

- Nghe thử đoạn chọn (cần một engine phát thứ hai hoặc chiếm hàng đợi — mục 10).
- Thanh hai núm trong materiant (dùng hai `Slider`).

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | `parse_time("1:05")` = 65; `"65"` = 65; `"1:5x"` ⇒ lỗi; `format_time(65.4)` = `1:05` | `[opus-unit]` |
| C2 | Tên mặc định cạnh tệp gốc, không trùng tệp có sẵn, không bao giờ là tệp gốc | `[opus-unit]` |
| C3 | Đầu ≥ cuối ⇒ nút Lưu khoá; `m4r` > 40 s ⇒ cảnh báo | `[opus-unit]` |
| C4 | Menu bài có mục Save as new; chuột phải hàng đợi ⇒ menu 4 mục | `[opus-unit]` (App ảo) |
| C5 | Lưu thật trong App ảo với fixture ⇒ tệp ra, toast | `[opus-unit]` |
| C6 | Hộp thoại sáng / tối ở desktop ẩn | `[opus-run]` |
| C7 | Chuột phải hàng đợi trên màn thật, cắt một bài thật ra `mp3`, `m4r`, mở trong trình phát khác | `[owner-qa]` |

## 8. Nợ chuyển task khác

- Bộ mã hoá macOS / Linux: mục 8 của `groove-gd2_02`, `plan/_backlog.md`.

## 9. Bàn giao thực tế

- Hộp `views/save_as_dialog.rs` (`SaveAsDialog`): hai entity — một của `LibraryCtl` (`open_save_as`, vẽ ở lớp phủ trang thư viện,
  như hộp lyrics), một của `NowPlaying` (menu hàng đợi). Lưu xong: `LibraryCtl::add_file` (`Indexer::index_paths` một tệp, nạp lại
  danh sách, toast "Đã lưu … và thêm vào thư viện"); bỏ tích thì chỉ toast "Đã lưu …".
- Menu bài (`song_menu`): mục "Lưu dạng mới / Save as new" ngay sau "Sửa thông tin bài hát"; trong playlist "Xoá khỏi danh sách
  phát" vẫn là mục cuối (test cũ dùng `end enter` vẫn xanh).
- Hàng đợi (`queue_panel`): chuột phải / Shift+F10 mở menu Sửa lời, Sửa timeline, Sửa thông tin, —, Lưu dạng mới. Panel phát
  `QueuePanelEvent(QueueMenuAction, TrackRef)`; `NowPlaying::open_editor` mở đúng các hộp mà ba nút ở khung Lyrics mở
  (`panel_event` nay cũng đi qua `open_editor`). Hàng có `debug_selector` `np-queue-row-<ix>`.
- **Tự quyết (D-9)**:
  - Nhãn lời bài trong menu hàng đợi dùng chung "Sửa lời bài hát" của menu bài (chủ dự án viết "Edit/Add lyrics"; dòng hàng đợi
    không biết bài đã có lời chưa, tra thêm thì phải đọc DB mỗi lần mở menu).
  - Chọn đoạn bằng hai `Slider` + ô `m:ss` (materiant chưa có slider hai núm) và một thanh đoạn đã chọn màu nhấn bên dưới.
  - Không có nút nghe thử: phát thử cần engine thứ hai (thiết bị âm thanh mở hai lần) hoặc chiếm hàng đợi đang phát — mục 10.
  - Định dạng khoá vẫn hiện, kèm lý do từng dòng ("MP3: Chưa có trên hệ điều hành này."), không ẩn.
  - Tên mặc định `<gốc> (cut)`, `(cut 2)`… không trùng tệp có sẵn với bất kỳ đuôi nào của ba định dạng (kiểm ở nền lúc mở hộp, N7).
    "Chọn…" mở hộp lưu của OS (`prompt_for_new_path`, OS tự hỏi ghi đè); đích trùng tệp có sẵn mà người dùng chưa đồng ý ở hộp OS
    ⇒ báo dưới ô tên.
  - Đang lưu: Esc / bấm nền không đóng hộp, nút Huỷ huỷ lượt xuất (tệp `.part` bị xoá).
  - Cửa sổ thấp: thân hộp cuộn (`flush_body` + `ScrollArea`, như hộp metadata), hàng nút luôn thấy.
- **Bẫy**: `MenuItem::on_select` chạy lúc cửa sổ đang bị mượn ⇒ `cx.emit` thẳng thì `subscribe_in` của `NowPlaying` không chạy;
  phải `cx.defer`. Test App ảo: vòng đọc tiến độ chờ `timer(100 ms)` ⇒ test phải `advance_clock`. Menu mở bằng chuột chưa tô mục
  nào: phím `home` trước `enter`.
- Test: `save_as_dialog::tests` (5, hàm thuần), `library_ctl::tests::save_as_new_from_the_song_menu_writes_and_adds_a_file` (C4, C5:
  chuột phải → mục cuối → lưu WAV thật → tệp mới cạnh gốc, gốc không đổi, thư viện +1, toast),
  `now_playing::view_tests::queue_row_menu_opens_editors_for_that_row` (C4 hàng đợi: menu của dòng 2 mở lời bài và Save as new cho
  đúng bài dòng 2). Toàn bộ `cargo test` xanh, clippy `-D warnings`, `cargo deny check licenses` ok.
- C6 `[opus-run]`: chụp ở desktop ẩn (`measure-hidden.ps1 -Seed … -Songs 6`, mở album, Shift+F10, End, Enter) sáng / tối ở 800 × 600 —
  bố cục đúng, chữ một độ đậm, màu nhấn từ theme. Dòng `ERROR … RefCell already borrowed` lúc khởi động có cả ở bản master 04/11, không
  do task này.

## 10. Đề xuất cải tiến

- Nút nghe thử đoạn chọn: engine phát thứ hai dùng chung thiết bị, hoặc tạm dừng bài đang phát rồi trả lại vị trí cũ.
- Slider hai núm trong materiant (`RangeSlider`) thay hai thanh trượt.
- Dạng sóng (waveform) dưới thanh đoạn để chọn mép theo âm lượng.
- Nhớ định dạng / "thêm vào thư viện" lần trước trong `settings.json`.
