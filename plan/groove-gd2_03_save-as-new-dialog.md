# groove-gd2_03 — save-as-new-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | 🔄 |
| Repo | groove |
| Model | opus |
| Cỡ | M — ~650 dòng (hộp thoại, mục menu, menu hàng đợi, i18n) + test |
| Phụ thuộc | groove-gd2_02 |
| Quyết định áp dụng | D-9, D-10, D-13, G-4, G-6, N7 |
| Phụ thuộc mới | — |
| File đụng tới | MỚI `crates/groove-app/src/views/save_as_dialog.rs`, `i18n/{vi,en}/save_as.toml`; SỬA `crates/groove-app/src/views/song_menu.rs`, `crates/groove-app/src/views/queue_panel.rs`, `crates/groove-app/src/library_ctl.rs`, `crates/groove-app/src/views/mod.rs`, `crates/groove-core/src/i18n.rs` (vùng mới) |

## 1. Làm gì

- Menu chuột phải của hàng bài (mọi danh sách bài dùng `song_menu`) có thêm **Save as new**.
- Hộp thoại Save as new: điểm đầu / điểm cuối (thanh trượt + ô `mm:ss`), thanh đoạn đã chọn, định dạng (`current`, `mp3`,
  `m4r` — lựa chọn không dùng được bị khoá kèm lý do), báo khi `m4r` dài hơn 40 s, chỗ lưu + tên (mặc định cạnh tệp gốc,
  tên `<gốc> (cut).<đuôi>`, không trùng tệp có sẵn), "Thêm vào thư viện sau khi lưu", tiến độ + nút huỷ.
- Lưu xong: toast, tuỳ chọn thêm bài mới vào thư viện (`LibraryCtl::import_path`).
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

## 9. Bàn giao thực tế

## 10. Đề xuất cải tiến
