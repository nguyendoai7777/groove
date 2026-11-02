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
- [groove-gd1_09] Bài có `missing_since` ẩn khỏi mọi danh sách, kể cả playlist, về thì hiện đúng chỗ — hiện dòng xám trong playlist — `Song` thuộc `groove-core` không có cờ mất, phát bài chết chỉ ra lỗi.
- [groove-gd1_09] `SyncReport.removed` gồm cả bài vừa đánh dấu mất — thêm trường riêng — `actor.rs` ngoài phạm vi; với hàng đợi thì đều không phát được.
- [groove-gd1_09] Đổi tên vào tên của dòng đang mất thì xoá dòng đó, mất chỗ trong playlist của nó — từ chối đổi tên — tệp không còn trên đĩa, đường dẫn là danh tính của dòng; `metadata_ops` đã coi tên đó là trống.
- [groove-gd1_09] Hai bản sao giống hệt đổi cả tên thì không ghép — ghép đại — gán nhầm playlist khó phát hiện hơn là một dòng mất.
- [groove-gd1_09] Dọn sau 30 ngày chỉ khi gốc quét được và tệp vẫn `NotFound` lúc đó — dọn theo mốc thời gian — ổ tháo lâu không được mất playlist.
- [groove-gd1_09] Import đúng chỗ đang bị `excluded_paths` che thì gỡ che, import gốc cha thì không — luôn tôn trọng — chọn thẳng là ý muốn rõ ràng.
- [groove-gd1_09] `excluded_paths` có thêm cột `path_key` ngoài `path` của đặc tả — chỉ `path` — so đường dẫn kiểu M2.
- [groove-gd1_22] Phím tắt qua `intercept_keystrokes` + ngữ cảnh GPUI — `KeyBinding` của GPUI — chuỗi GPUI cố định 1 giây (đặc tả 800 ms), đổi phím lúc chạy phải xoá cả phím materiant.
- [groove-gd1_22] Dialog mở thì `Ctrl+K` cũng tắt — vẫn mở bảng lệnh — chồng dialog lên dialog; M4 chỉ đòi `Ctrl+K` chạy ở ô nhập thường.
- [groove-gd1_22] Space trên nút đến bằng Tab là bấm nút, đến bằng chuột là phát / dừng — luôn phát / dừng như bản cũ — giữ dùng được bằng bàn phím mà bấm Next xong Space không Next tiếp.
- [groove-gd1_22] `/playall` phát lại danh sách hiện tại từ đầu, trống thì báo — phát cả thư viện — thư viện không có khái niệm tất cả bài; giữ như bản cũ.
- [groove-gd1_22] `/theme` không đối số đổi qua lại sáng, tối; `/queue <tên>` thêm bài khớp đầu — bắt buộc đối số; thêm mọi bài khớp — ít bất ngờ nhất.
- [groove-gd1_14] Nút Play đổi icon Play / Pause tức thì — biến hình hai đa giác 250 ms bằng `canvas` — đặc tả mục 4 cho phép khi không cần; thêm icon `Pause` vào `icons.rs`.
- [groove-gd1_14] Marquee đứng ở đầu dòng khi rê chuột, chạy lại từ đầu khi rời — tạm dừng giữ nguyên chỗ — `with_animation` của GPUI không tạm dừng giữa chừng.
- [groove-gd1_14] Không bài thì chặn cả nút Settings của thanh phát — chừa riêng Settings — đặc tả 1.3 "không bấm được"; Settings còn ở cột trái.
- [groove-gd1_14] Âm lượng, tắt tiếng đọc từ `app::settings` — thêm hàm đọc vào `Player` — `player.rs` thuộc task 13 đã ✅; ghi mục 8.
- [groove-gd1_25] Lời gọi duy nhất trong `app::run` là `open_paths::run_app(application, |cx| ..)` bọc `Application::run` — thêm một dòng `install(cx)` trong closure — `on_open_urls` phải đăng ký trước `run`, macOS gửi tệp lần đầu trước khi closure chạy.
- [groove-gd1_25] Gộp các yêu cầu về cách nhau dưới 200 ms, trần khoảng 1 giây — mỗi yêu cầu một lần `play_paths` — Explorer có thể bật mỗi tệp một tiến trình, tệp sau sẽ cướp bài của tệp trước; đổi lại mở một tệp trễ thêm 200 ms, focus thì ngay.
- [groove-gd1_25] Không gọi `ShowWindow(SW_RESTORE)` riêng — gọi như đặc tả 5.5 — `activate_window` của GPUI đã `IsIconic` + `ShowWindowAsync(SW_RESTORE)`, gọi thêm từ luồng UI dễ lồng wndproc.
- [groove-gd1_25] Không giành được endpoint trong 5 giây thì vẫn mở app, không có một phiên — thoát im lặng — tiến trình lạ giữ tên pipe không được làm người dùng không mở nổi app. Phiên chính treo quá 10 giây thì phiên sau thoát, không mở phiên hai giành DB.
- [groove-gd1_25] Phiên chính trả `ok` một dòng, phiên sau chờ rồi mới thoát — gửi rồi thoát ngay như đặc tả 5.5 — test hai tiến trình tái hiện được mất tệp khi máy bận: phiên sau thoát trước khi tệp tới phiên chính.
- [groove-gd1_25] Tên không phải Unicode không chuyển qua IPC được, bỏ khi gửi — mã hoá riêng — đặc tả chốt dòng JSON; hiếm gặp.
- [groove-gd1_25] Bản debug dùng endpoint riêng (`-dev`, có `GROOVE_DATA_DIR` thì theo băm thư mục) — chung một tên — chạy thử không được đánh thức phiên thật của chủ dự án. Mã Windows, Unix đặt trong `mod sys` của `instance.rs`, không ở `app/platform` — task này chỉ sở hữu tệp của mình, `platform/mod.rs` không được sửa.
- [groove-gd1_20] Hộp lyrics, timeline nhận MetadataOps qua tham số, không có Global — thêm Global ở app.rs — tệp thuộc task 03 / chưa rõ chủ; để Opus chọn chỗ đặt.
- [groove-gd1_20] Esc và bấm nền đóng hộp lyrics, timeline kể cả khi có chữ chưa lưu — chặn Esc + hỏi xác nhận — như bản Tauri; đổi là quyết định UX.

## groove-gd1_16

- [groove-gd1_16] Bấm dòng tách chuột với phím bằng `window.last_input_was_keyboard()`; nhấp đúp từ chuột không làm gì thêm — phát ở `on_activate` — lần nhấp đầu đã phát, lần hai sẽ đảo thành dừng.
- [groove-gd1_16] Đầu danh sách là khối cao cố định đứng ngoài danh sách, thu / mở tức thì, chỉ thu khi sau khi thu còn cuộn > 15 px — nội suy như bản cũ (`transition` 300 ms) — danh sách ngắn thu xong bị clamp về đỉnh rồi mở lại gây nhấp nháy; nội suy thêm sau được (đặc tả cho phép nhảy ngay).
- [groove-gd1_16] Playlist không hiện dòng phụ, chỉ "n bài" — hiện cả hai như dòng phụ của thẻ — tránh nói số bài hai lần.
- [groove-gd1_16] Cột nghệ sĩ ẩn khi cửa sổ < 768 px — theo bề ngang vùng nội dung — đặc tả không nêu ngưỡng, `md` của bản cũ là theo viewport.
- [groove-gd1_16] Bìa đầu danh sách dùng bản 256 px cho cả hai cỡ — bản 512 px — không đảm bảo tệp lớn tồn tại; nhoè nhẹ ở màn DPI cao.

## groove-gd1_19

- [groove-gd1_19] Áp cho nhóm chỉ gửi trường đã đổi so với lúc mở — gửi đủ sáu trường như bản cũ — `GroupEdit` ghi đè mọi bài trong nhóm, gửi trường không đụng sẽ xoá thể loại / năm riêng của bài khác.
- [groove-gd1_19] Nhãn chế độ `merge` là "Gộp vào thẻ hiện có" — "Chỉ sửa trường đã đổi" — form gửi mọi trường theo đặc tả, nên nhãn cũ sai nghĩa.
- [groove-gd1_19] Ctrl+V chỉ dán ảnh khi chuột đang trên ô bìa và clipboard có ảnh / tệp ảnh — luôn chặn Ctrl+V trong dialog — để ô nhập vẫn dán chữ được.
- [groove-gd1_19] Bài `not_started` của báo cáo lô chỉ hiện số lượng — kèm tên — `BulkReport.not_started` chỉ có id, tra tên cần DB đang đóng.
- [groove-gd1_19] Nền dialog bấm không đóng, Esc bị chặn khi đang lưu — theo mặc định của materiant — tránh mất bản sửa dài.
- [groove-gd1_26] Nút giữa taskbar luôn bật, Prev, Next tắt khi chưa có bài — tắt cả ba — bản cũ chỉ tắt hai nút điều hướng, Play khi chưa có bài tự lấy bài kế.
- [groove-gd1_26] Bài không bìa trên Windows hiện logo app — giữ nguyên ảnh cũ — souvlaki không xoá được ảnh, ảnh bài trước sẽ dính sang bài sau.
- [groove-gd1_26] Bỏ qua `SetVolume`, `Quit`, `OpenUri` từ OS, nhận `Raise` — làm đủ — ngoài đặc tả 5.4; `OpenUri` từ tiến trình khác còn là cửa mở tệp tuỳ ý.
- [groove-gd1_26] Thêm `assets/taskbar`, `assets/media/no_cover.png` chép từ `src-tauri/icons` — trỏ thẳng `src-tauri` — task 29 gỡ thư mục đó.
- [groove-gd1_23] Thêm `Slot::CommandPalette` và một dòng vẽ trong `Shell::render` của `app.rs` (tệp của task 03) — treo `Dialog` ở slot sẵn có hay dựng cửa sổ phụ — `Dialog` của materiant chỉ vẽ được khi nằm trong cây view, chưa có chỗ chung cho dialog.
- [groove-gd1_23] Nhấp một dòng của bảng lệnh chạy ngay khi dòng đó đang chọn (rê chuột đã chọn nó), nhấp đúp không chạy lần hai — chạy ngay mọi lần nhấp — tránh chạy hai lần khi nhấp đúp; đặc tả chỉ ghi "rê chuột dời con trỏ".
- [groove-gd1_21] Quầng accent đổi tức thì, không chuyển 700 ms — transition khai báo — GPUI `div` không có transition (bảng thay thế đặc tả mục 4).
- [groove-gd1_21] Bài là tệp ngoài không có quầng accent — tính trung bình ảnh ở UI — UI không chạm đĩa / không có crate ảnh trong app; accent chỉ đọc từ `covers.accent_color` ở luồng nền.
- [groove-gd1_21] Nút Metadata mở `MetadataDialog` (task 19), Sửa / Thêm lời và Timeline mở `TextDialog` (task 20) thay cho sửa tại chỗ ở đặc tả 1.5 — sửa tại chỗ — theo bàn giao task 20.
- [groove-gd1_21] Tiêu đề cửa sổ bỏ phần `- <nghệ sĩ>` khi bài không có nghệ sĩ — hiện "Unknown artist" — tiêu đề gọn, không bịa chữ.
- [groove-gd1_21] Phím mũi tên trong hàng đợi chỉ dời con trỏ, Enter / bấm mới phát — phát ngay khi dời — tránh đổi bài theo từng lần nhấn.
