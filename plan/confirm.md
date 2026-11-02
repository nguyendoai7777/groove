# Chờ chủ dự án xác nhận

Việc **đã làm** trong lúc chủ dự án vắng (ngủ / bận) mà Claude không chắc chủ dự án đồng ý — đổi API, lệch bản web có
chủ ý, đánh đổi RAM / CPU, đổi luật, chọn phương án thay chủ dự án. Mỗi gạch: `- [<task>] <việc đã chọn> — <phương án
khác> — <vì sao chọn>`. Chủ dự án đọc: đồng ý ⇒ xoá gạch (quyết định lớn thì chép vào `docs/decisions.md`); không đồng
ý ⇒ ghi lại yêu cầu ngay dưới gạch để session sau làm lại.

Session thêm gạch bằng `Edit` chèn vào mục của task mình (không `Write` cả tệp — nhiều session cùng ghi).

## groove-gd1_01

- [groove-gd1_01] `cpal` optional trên Linux sau feature `linux-alsa`, tắt mặc định: bản Linux chưa có tiếng tới khi chủ dự án quyết `libasound` (LGPL) — khai thẳng cho mọi OS — chưa được phép link LGPL khi chưa hỏi. Cần chủ dự án quyết trước task 10.
- [groove-gd1_01] Khai thêm phụ thuộc ngoài mục 5 của đặc tả (`base64`, `sha2`, `unicode-normalization`, `crossbeam-channel`, `futures`, `anyhow`, `thiserror`, `rusty-leveldb`, `raw-window-handle`, `objc2*`, `embed-resource`) — để task sau tự thêm — Luật song song cấm task sau sửa `Cargo.*`.
- [groove-gd1_01] `app/app.rs` là ngoại lệ của Luật song song: task sau thêm đúng một lời gọi khởi tạo — mỗi task một hook rỗng khai sẵn — chưa có API thật để khai, xung đột một dòng dễ giải.

## groove-gd1_02

- [groove-gd1_02] Theme mặc định `dark` — `system` như materiant — bản cũ chạy tối cố định, giữ cảm giác khi chuyển sang.
- [groove-gd1_02] `customKeybindings = null` là dùng bảng mặc định — ghi cả bảng vào tệp — phím mặc định mới của bản sau tới được người chưa từng sửa.
- [groove-gd1_02] Tệp cấu hình sai kiểu một khoá thì chỉ mất khoá đó, tệp hỏng hẳn giữ lại thành `.bad` — về mặc định và ghi đè — không mất cấu hình âm thầm. Thêm so với đặc tả.
- [groove-gd1_02] Thêm `settings::JsonSaver` (luồng nền, debounce) ở core — để từng task app tự viết — task 03, 13, 24 đều cần và N7, S12 yêu cầu ghi nền.

## groove-gd1_03

- [groove-gd1_03] Cỡ cửa sổ nhỏ nhất 560 × 360 — không đặt trần như bản Tauri (800 × 600 mặc định, không ghi cỡ nhỏ nhất) — cột trái mở 230 px + ô tìm + ba nút cửa sổ cần chỗ, không có trần thì kéo nhỏ là vỡ bố cục.
- [groove-gd1_03] Nền cột trái dùng `translucent_surface` (alpha 0,88 tông tối, tông sáng đục) — alpha 0,4 như `--nav-bg` bản cũ — materiant đã kiểm tương phản chữ ≥ 4,5:1 trên nền trắng xấu nhất ở 0,88; 0,4 trượt. Đổi thì sửa một dòng ở `views/side_nav.rs`.
- [groove-gd1_03] Tên mục: Thư viện, Đang phát, Danh sách phát, Cài đặt (en: My Music, Now Playing, Playlists, Settings) — dịch sát từng chữ "Nhạc của tôi" — khớp tên lệnh `go_to_library` ở đặc tả 1.9 và ngắn hơn trong cột 56 / 230 px.
- [groove-gd1_03] Tiêu đề cửa sổ `<trang> | Groove X` — `GrooveX` liền như dòng Now Playing của bản cũ — đặc tả 1.1 ghi "theo trang"; tên app lấy từ `common.app_name` để một chỗ.
- [groove-gd1_03] Một `Slot::Library` cho My Music, Playlists và danh sách bài của category — slot riêng cho từng route — đặc tả 1.2: hai route chung một màn với `mode`; view nghe `RouteChanged` để chọn lưới hay danh sách.
- [groove-gd1_03] Nút đóng bằng `window.remove_window()` + `cx.on_window_closed` thoát app — hit-test `WindowControlArea::Close` của OS — bản này chạy cùng một mã trên ba OS và click trong GPUI test được; mất Snap Layouts của nút phóng to trên Windows 11.

- [groove-gd1_11] Preamp theo đỉnh đáp ứng thật của chuỗi 10 dải — `-max(gain dương)` như S23 — toàn +10 dB đo được đỉnh +15,06 dB, công thức S23 vẫn rè; một dải thì hai cách như nhau.

## groove-gd1_04

- [groove-gd1_04] Thêm `songs.path_key` UNIQUE, bỏ UNIQUE trên `file_path` — giữ cả hai — M2 cần, cột kia thừa. Đặc tả 2.2 chưa ghi cột này.
- [groove-gd1_04] DB cũ trùng `path_key` thì gộp về `id` nhỏ nhất, lấp lyrics, timeline — dừng chuyển đổi — trùng do khác hoa thường là chính lỗi M2, dừng thì người dùng kẹt. Ngoại lệ duy nhất của "giữ nguyên `id`".
- [groove-gd1_04] Chụp bằng `VACUUM INTO` chỉ đọc — chép tệp như đặc tả ghi — vẫn nhất quán khi bản Tauri đang mở DB.
- [groove-gd1_04] `position` playlist cũ theo tên tệp — theo thứ tự thêm — bản cũ hiện theo tên tệp, không lưu thứ tự thêm.
- [groove-gd1_04] DB cũ hỏng trả lỗi cho caller, không tự tạo thư viện trống — tự tạo rồi im lặng — tạo rồi thì không bao giờ thử chuyển lại.
- [groove-gd1_04] `search_key` giữ dấu câu theo S18 — bỏ như bản cũ — `AC/DC` không còn khớp `acdc`, đổi lại `-` không khớp tất cả.

## groove-gd1_10

- [groove-gd1_10] HE-AAC báo ngầm (18/1002 bài, 1,8%) vẫn phát, nhưng chỉ lõi LC, mất dải cao trên khoảng 11 kHz — đẩy vào `Error` như đặc tả 5.1, hoặc thêm bộ giải có SBR — thà nghe được còn hơn bỏ bài, và symphonia không báo SBR khi mở tệp.
- [groove-gd1_10] Âm lượng `v²` áp ở callback nên cảm giác âm lượng khác bản cũ (S23) — giữ tuyến tính — đặc tả S23 yêu cầu.

## groove-gd1_12

- [groove-gd1_12] Tắt shuffle: queue = phần sau bài hiện tại, rồi vòng về các bài phía trước chưa nghe, cuối cùng là bài ngoài original — xếp đúng thứ tự original từ đầu — Next vẫn ra bài kế như bảng 1.4, mà S4 không mất bài phía trước.
- [groove-gd1_12] Bật shuffle: bài chưa phát của original cộng bài đang chờ ngoài original — chỉ xáo phần queue hiện có như câu đầu S4 — bảng 1.4 và dòng "phát kèm danh sách" đều lấy mọi bài còn lại, đồng thời không mất bài thêm tay.
- [groove-gd1_12] `append` không đụng original, vòng lặp repeat không gồm bài thêm tay — thêm cả vào original — bảng 1.4 ghi nạp lại từ original, và original là danh sách nguồn.
- [groove-gd1_12] Repeat ở cuối khi original rỗng (toàn tệp ngoài): vòng qua history + bài hiện tại — xoá history, chỉ lặp bài hiện tại — bảng 1.4 không có ca này, bản cũ thoát sớm (N6), xoá thì mất bài đã nghe.
- [groove-gd1_12] Dòng timeline bốn phần thời gian trở lên hoặc phần rỗng (`1:2:3:4 x`, `:30 x`) bị bỏ qua — coi là đoạn ở giây 0 hoặc giây 30 như bản cũ — đặc tả 1.6 ghi dòng khác bỏ qua.

## groove-gd1_06

- [groove-gd1_06] WAV ghi thẻ qua `id3` — `lofty` như đặc tả 5.8 — thẻ chung của `lofty` 0.25 không đặt được `TXXX:TIMELINE`, mà WAV dùng ID3v2.
- [groove-gd1_06] FLAC, OGG, M4A ghi trên thẻ riêng từng định dạng — `TaggedFile` như bản cũ — `TaggedFile` làm rơi `TIMELINE` và mọi khoá lạ kể cả ở Merge.
- [groove-gd1_06] M4A: `TIMELINE` ở `----:com.apple.iTunes:TIMELINE`, publisher ở `LABEL` — bỏ như bản cũ — lofty không có khoá `Publisher` cho MP4 và âm thầm bỏ.
- [groove-gd1_06] `tags.rs` 957 dòng mã, vượt trần 900 — tách tệp — task chỉ sở hữu hai tệp, `lib.rs` đã khoá; dòng thừa chủ yếu là nhánh theo định dạng.

## groove-gd1_07

- [groove-gd1_07] `hash` là SHA-256 byte ảnh gốc — băm điểm ảnh sau giải mã — tính được trước khi giải mã nên tránh giải mã lặp; cùng ảnh khác mã hoá sẽ thành hai bộ tệp, chấp nhận.
- [groove-gd1_07] Tệp bìa giữ tỉ lệ, không phóng to, `_512.jpg` có thể nhỏ hơn 512 — cắt vuông hoặc phóng to cho đúng cỡ — cắt là việc của lớp hiển thị, phóng to chỉ tốn đĩa.
- [groove-gd1_07] Trần tệp vào 64 MiB, giải mã 192 MiB (RGB 8000 × 8000), cạnh 16 384 — trần thấp hơn để bớt RAM — bìa quá trần sẽ mất, đổi trần chỉ sửa một hằng.
- [groove-gd1_07] Accent tính theo alpha như canvas cũ, JPEG phủ alpha lên nền đen — coi trong suốt là đen khi tính màu — logo nền trong suốt vẫn ra đúng màu logo.

## groove-gd1_18

- [groove-gd1_18] `save` thêm tham số tên mới — đổi `MetadataEdit` — `MetadataEdit` thuộc `tags`, không chứa tên tệp.
- [groove-gd1_18] Lưu trong app phát `Progress` kind `MetadataBulk`, không phát `Synced` — thêm sự kiện mới — `actor.rs` đã khoá, `Synced` kéo theo toast (S5).
- [groove-gd1_18] Cấm đổi đuôi tệp; cấm ký tự, tên dành riêng của Windows ở mọi OS — chỉ cấm theo OS đang chạy — thư viện có thể nằm trên ổ dùng chung, đổi đuôi làm bài rơi khỏi thư viện.
- [groove-gd1_18] `Clean` lấy lyrics DB trước, DB trống mới lấy tệp — tệp trước — theo đặc tả N2; lyrics vừa sửa ngoài app mà watcher chưa kịp đọc có thể bị bản DB đè.
- [groove-gd1_18] Áp bìa cho nhóm đánh dấu `cover_is_custom`, kể cả đè bìa custom cũ; bìa custom theo bài khi album bị dọn — giữ bìa custom cũ, để mất theo album — đây là lựa chọn rõ của người dùng.
- [groove-gd1_18] `metadata_ops.rs` 691 dòng mã, vượt ước 600 — tách tệp — task chỉ sở hữu một tệp, `lib.rs` đã khoá.

## groove-gd1_05

- [groove-gd1_05] `LegacyFailed` để thư viện ở trạng thái lỗi, UI cho chọn thử lại hoặc bắt đầu trống — tự `open_fresh` — tạo `library.db` rồi thì không bao giờ chuyển DB cũ lại nữa.
- [groove-gd1_05] Hạt giống `watched_roots` từ `folders` chỉ một lần sau chuyển đổi — mỗi lần đọc như bản cũ — để bỏ hết gốc không bị gốc quay lại (N9).
- [groove-gd1_05] Chưa thêm `missing_since`, `excluded_paths` vào schema — thêm ngay — `schema.rs` thuộc task 04 đã đóng, đặc tả 2.2 không ghi; đẩy sang 09, 17, cần quyết cách nâng `user_version`.

## groove-gd1_15

- [groove-gd1_15] Đầu màn (tiêu đề, thẻ hành động, tab) đứng yên, chỉ lưới cuộn — cuộn cả trang như bản cũ — `VirtualGrid` không chứa phần đầu.
- [groove-gd1_15] Trần 6 cột bằng `min_cell_width` tính theo bề ngang cửa sổ (ước theo cột trái thu), ô tối thiểu 130 px — cố định số cột theo breakpoint — `VirtualGrid` chỉ nhận bề ngang ô tối thiểu; cột trái mở thì ít cột hơn mức tối đa.
- [groove-gd1_15] Bấm thẻ vào danh sách ngay, Enter / nhấp đúp cũng vào — nhấp chọn, nhấp đúp mới vào — đặc tả ghi "Bấm thẻ vào danh sách".
- [groove-gd1_15] `Route::Category` vẽ chữ giữ chỗ tới khi `SongList` (16) có — chờ 16 — 16 không sở hữu `library_page.rs`; task 17 thay nhánh này.
- [groove-gd1_15] `LibraryPageEvent` phát lên cho task 17 nghe, không nối nghiệp vụ nhập / quét lại / tạo / xoá / phát — gọi thẳng `Library` — nghiệp vụ và hộp thoại là của 17.

## groove-gd1_15

- [groove-gd1_15] Đầu màn (tiêu đề, thẻ hành động, tab) đứng yên, chỉ lưới cuộn — cuộn cả trang như bản cũ — `VirtualGrid` không chứa phần đầu.
- [groove-gd1_15] Trần 6 cột bằng `min_cell_width` tính theo bề ngang cửa sổ (ước theo cột trái thu), ô tối thiểu 130 px — cố định số cột theo breakpoint — `VirtualGrid` chỉ nhận bề ngang ô tối thiểu; cột trái mở thì ít cột hơn mức tối đa.
- [groove-gd1_15] Bấm thẻ vào danh sách ngay, Enter / nhấp đúp cũng vào — nhấp chọn, nhấp đúp mới vào — đặc tả ghi "Bấm thẻ vào danh sách".
- [groove-gd1_15] `Route::Category` vẽ chữ giữ chỗ tới khi `SongList` (16) có — chờ 16 — 16 không sở hữu `library_page.rs`; task 17 thay nhánh này.
- [groove-gd1_15] `LibraryPageEvent` phát lên cho task 17 nghe, không nối nghiệp vụ nhập / quét lại / tạo / xoá / phát — gọi thẳng `Library` — nghiệp vụ và hộp thoại là của 17.

## groove-gd1_08

- [groove-gd1_08] Import bỏ qua tệp `(mtime, size)` khớp và đã đọc thẻ — đọc thẻ mọi tệp như 1.11 — S26 không muốn kéo tệp OneDrive khi không đổi; đọc hết thì dùng `rescan`.
- [groove-gd1_08] `file_mtime` mới tính bằng ms — giữ giây như DB cũ — khớp `FileStamp`, so chính xác hơn; dòng cũ đổi khi đọc lại thẻ, task 09 phải tính tới dòng `tags_read_at IS NULL`.
- [groove-gd1_08] Import xoá thật dòng đã xác nhận mất — đợi `missing_since` — 1.11 yêu cầu xoá, `missing_since` thuộc task 09; đã chặn đủ các lỗi đọc.
- [groove-gd1_08] Giữ dòng nằm sau liên kết thư mục không đi theo — xoá như tệp mất — bản cũ có theo liên kết, DB cũ có thể chứa bài ở đó.
- [groove-gd1_08] Đọc theo lô, chờ đủ lô rồi ghi — luồng đọc liên tục có bộ đệm sắp xếp — đơn giản, RAM có trần; tệp cloud chậm giữ cả lô.

## groove-gd1_13

- [groove-gd1_13] Toast N5 ở task 14, player chỉ phát sự kiện — player tự toast — chữ thuộc vùng `player`, player không có `Window`.
- [groove-gd1_13] Âm lượng, mute, EQ đi qua hàm của `Player` — quan sát `Settings` — `app.rs` không mở entity ra ngoài; sửa bằng `update_settings` thì phải gọi `apply_settings`.
- [groove-gd1_13] Bài của phiên hỏng lúc mở app chỉ báo, không sang bài — tự sang bài như lúc đang nghe — đặc tả 1.4 không tự phát.
- [groove-gd1_13] Tệp ngoài đã mất bị bỏ khỏi phiên lúc khôi phục — giữ tới lúc phát rồi lỗi — ổ rời chưa cắm thì mất bài khỏi hàng đợi, đổi lại không đầy bài chết (N5).
