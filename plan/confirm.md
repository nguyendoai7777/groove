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
- [groove-gd1_17] Hộp chọn thư mục, ảnh dùng `cx.prompt_for_paths` của GPUI theo đặc tả mục 5; bản đầu thêm `rfd` đã gỡ. Đổi lại: hộp không lọc được đuôi tệp, chọn nhầm tệp không phải ảnh thì báo toast lỗi khi đặt bìa.
- [groove-gd1_17] Không gọi `reconcile` riêng khi mở app — gọi như lời giao — `LibraryWatcher::start` đã tự so, gọi thêm là quét hai lần.
- [groove-gd1_17] Bỏ album ghi từng tệp vào `excluded_paths`, nút đổi nhãn "Bỏ khỏi thư viện" — chỉ đổi nhãn, không loại trừ — đặc tả N9 cho cả hai cách; không loại trừ thì lần quét sau bài quay lại.
- [groove-gd1_17] Bỏ thư mục kéo theo thư mục con — chỉ bỏ đúng thư mục đó — `excluded_paths` loại cả cây, để dòng con lại thì chúng còn hiện mà không còn được quét.
- [groove-gd1_17] Tên playlist rỗng báo toast, trùng tên báo dưới ô — cùng một kiểu — đặc tả ghi toast cho tên rỗng, trùng tên cần giữ hộp mở để sửa.
- [groove-gd1_17] Menu thẻ thêm "Phát tất cả" và "Sửa thông tin" cạnh "Đổi ảnh bìa" — chỉ đổi bìa và xoá — dùng được bằng chuột phải khi lớp phủ khó bấm.
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
- [groove-gd1_24] Hộp Settings có ba tab (Cài đặt chung, Phím tắt, Thư viện) — hai tab theo đặc tả 1.10 — danh sách tệp của task có `roots.rs` và bàn giao task 17 giao `forget_root`, cần một chỗ đặt.
- [groove-gd1_24] Thêm `Slot::Settings`, một dòng vẽ trong `Shell::render` và một lời gọi `install` trong `app::run` (tệp của task 03), cộng hai hàm `#[cfg(test)]` `install_settings_for_test`, `set_shell_for_test` — dựng cửa sổ phụ cho hộp — `Dialog` của materiant chỉ vẽ được trong cây view; làm như bảng lệnh.
- [groove-gd1_24] Bước tua, bước âm lượng cũng áp ngay và Huỷ trả về giá trị lúc mở — chỉ lưu khi bấm OK — đặc tả chỉ nói EQ áp ngay; cùng một luật thì Huỷ nhất quán.
- [groove-gd1_24] Esc và nút ✕ coi như Huỷ (trả EQ, hai bước về lúc mở) — giữ thay đổi như bấm nền của bản cũ — M3 chỉ nói bấm nền; đóng mà không bấm OK thì không nên để lại thay đổi âm thanh âm thầm. Bấm nền không đóng (M3).
- [groove-gd1_24] Không có ô ngôn ngữ, theme trong hộp — thêm hai `Select` — đặc tả 1.10 không liệt kê; `/theme` đã đổi theme, ngôn ngữ theo OS.
- [groove-gd1_24] Ô JSON phím tắt bỏ tự đóng ngoặc và gợi ý tên lệnh, thay bằng danh sách lệnh dưới ô — làm gợi ý — `TextArea` của materiant chưa có chỗ chèn gợi ý; đặc tả mục 4 cũng ghi bỏ tự đóng ngoặc.
- [groove-gd1_24] OK ghi `None` khi danh sách bằng mặc định và không ghi gì khi không đổi — luôn ghi `Some(list)` — phím mới của bản sau tới được người chưa tuỳ biến.
- [groove-gd1_24] Bỏ gốc thư mục hỏi lại bằng nút đổi chữ (bấm lần hai mới bỏ), không mở hộp xác nhận — hộp trong hộp — dialog lồng cần thêm `Slot`; thao tác không xoá tệp.
- [groove-gd1_24] Bước tua có trần 3600 giây — không trần — chỉ để ô không nhận số vô lý; `Settings::sanitize` vẫn chỉ chặn dưới.
- [groove-gd1_24] Không sửa `library_ctl.rs`: danh sách gốc đọc lại theo `LibraryCtlEvent::Notice`, bỏ gốc thì gỡ khỏi danh sách ngay — thêm sự kiện "gốc đổi" vào `LibraryCtl` — task 17 đã xong, ngoài danh sách tệp, và không cần.
- [groove-gd1_24] Vượt trần 900 dòng code (khoảng 1.060 kể cả chú thích), sáu tệp — tách task — một lát dọc chạy được; tách ra thì mỗi mảnh vẫn dính `dialog.rs`.
- [flaky-audio] `set_language` chạy theo từng luồng khi bật feature `test-support` của `groove-core` (dev-dep của `groove-app`), thêm một dòng `Cargo.toml` mỗi crate — thêm API đặt ngôn ngữ riêng cho test, sửa từng test — một chỗ sửa mọi test, bản phát hành vẫn dùng atomic toàn tiến trình.
- [groove-gd1_27] Mở lại theo hình học, id màn chỉ để ưu tiên — coi id không thấy là màn đã mất như đặc tả 5.7 — `DisplayId` trên Windows là `HMONITOR`, đổi sau khởi động lại nên cửa sổ sẽ về giữa màn sau mỗi lần bật máy; hệ quả: rút màn mà màn khác chiếm đúng toạ độ đó thì cửa sổ mở ở đó thay vì giữa màn chính.
- [groove-gd1_27] `restore` thêm trước `open_window`, đăng ký lưu bằng `observe_new::<Shell>` — lời gọi sau `open_window` rồi dời cửa sổ — GPUI không có lệnh dời, và luật chỉ cho một lời gọi trong `app.rs`.
- [groove-gd1_27] `--display` thắng và không lưu vị trí — vẫn lưu — chạy thử không được đổi vị trí của người dùng.
- [groove-gd1_27] Vùng kéo còn tối thiểu 100 × 16 px trên màn thì giữ chỗ — đòi cả cửa sổ trong màn — đặc tả không ghi ngưỡng, cửa sổ thò ra một phần là việc người dùng tự làm.
- [groove-gd1_27] Một ProgID `GrooveX.AudioFile` cho sáu đuôi — mỗi đuôi một ProgID — cùng lệnh mở, cùng icon. Thêm `Applications\groove.exe` để hiện đúng tên trong "Mở bằng".
- [groove-gd1_27] Bản cài hỏi đóng phiên đang chạy rồi thử lại, im lặng thì mã 2 — tự đóng app — app không có lệnh thoát từ ngoài, không giết theo tên vì máy dev có thể chạy bản khác.
- [groove-gd1_27] Gói Debian tên `groove-x`, lệnh `groove`, Maintainer `noreply@groove-x.invalid` — tên khác — chủ dự án đổi địa chỉ khi phát hành thật.
- [groove-gd1_27] `Depends: libasound2 | libasound2t64` — chỉ tên do `dpkg-shlibdeps` ghi — Ubuntu 24.04 đổi tên gói, chỉ một tên thì gói dựng ở bản này không cài được trên bản kia. Chưa chạy thử.
- [groove-gd1_27] macOS khai `CFBundleTypeExtensions`, không khai UTI — `LSItemContentTypes` — bản Tauri cũng khai theo đuôi, M4R chưa rõ UTI công khai. Finder có nhận hay không phải kiểm trên Mac.
- [groove-gd1_27] Không soát chuỗi `GROOVE_DATA_DIR` trong exe release — soát như `fastcast` — `instance.rs:112` đọc tên biến ở mọi bản, `dev_tag` bỏ qua ở release, nên chuỗi còn trong exe mà hành vi vẫn đúng.
- [groove-gd1_27] Repo gốc: `build.* --project groove` chỉ uỷ quyền cho `packaging/*`, build cho máy đang chạy, không chọn chip; `start.* groove` mở app với dữ liệu thật, không ép `--display` vì lệnh này cho chủ dự án xem tay.

## groove-gd1_29

- [groove-gd1_29] Màu nhấn cyan (`#06b6d4` nền tối, `#0e7490` nền sáng) — giữ hồng mặc định của materiant — đặc tả 1.1 ghi cyan là màu của app; nền sáng đậm hơn để chữ trên trắng đủ tương phản.
- [groove-gd1_29] Now Playing: cột trái tối thiểu 150 px, cột phải 190 px — 180 / 240 px như đặc tả 1.5 — ở cửa sổ 800 px mặc định cột lời chỉ còn ~86 px, ba nút bị cắt.
- [groove-gd1_29] Toast khi đĩa đổi ngoài app là Info, qua sự kiện mới `LibraryEvent::ExternalChange` — toast mọi `Synced` — import, quét lại, xoá đã có toast riêng; phân biệt bằng thời điểm thì chạy đua.
- [groove-gd1_29] Không làm UI xoá bài mất trước hạn 30 ngày (`reconcile::forget_missing` có test, không có nút) — thêm danh sách bài mất ở Settings — N1 ghi "sau 30 ngày hoặc khi người dùng xác nhận", 30 ngày đã có; giao diện là thiết kế mới.
- [groove-gd1_29] Toast giữ 5 s của materiant — 4 s như bản cũ — một hằng chung của bộ component, khác biệt nhỏ.
- [groove-gd1_29] Đo RAM bằng desktop Win32 ẩn và exe release có debug assertions — ghi chưa đo — không hiện cửa sổ ở màn nào, không chạm dữ liệu thật; bản phát hành thật bỏ qua `GROOVE_DATA_DIR` nên không đo.
- [groove-gd1_29] Gỡ thêm `public/`, `scripts/`, `.husky/`, `.vscode/extensions.json`, `.oxfmtrc.json`, `app.json`, `index.html`, `tsconfig*` cùng nhánh gỡ Tauri — chỉ gỡ `src/`, `src-tauri/`, `package.json` — đều chỉ phục vụ bản Vue / Tauri, không gì trong `crates/`, `packaging/`, `tools/`, script gốc tham chiếu tới.

- [groove-gd2_01] Mỗi đuôi một ProgID `GrooveX.<đuôi>` (tên loại "Audio file (MP3)" / "Tệp nhạc (MP3)"), bỏ ProgID chung `GrooveX.AudioFile` của gd1_27 (cài đè tự dọn) — giữ một ProgID chung — Explorer, Default apps hiện đúng loại tệp như WMP, VLC.
- [groove-gd2_01] `Capabilities` đặt ở `HKCU\Software\Clients\Media\Groove X` — `Software\GrooveX\Capabilities` — đúng chỗ của trình phát nhạc (WMP, VLC), cả hai đều được `RegisteredApplications` trỏ tới.
- [groove-gd2_01] Một URI `ms-settings:defaultapps?registeredAppUser=Groove%20X` cho cả Windows 10 và 11 — Windows 10 gọi `IApplicationAssociationRegistrationUI` — Windows 11 vào thẳng trang của app; Windows 10 bỏ qua tham số, mở trang Default apps chung có ô Trình phát nhạc, ngắn hơn trang "theo app" của Windows 10.
- [groove-gd2_01] Ô "Đặt Groove X làm trình phát mặc định" ở trang cuối bộ cài tích sẵn — để trống — chỉ mở trang hệ thống, người dùng vẫn tự chọn; đúng mục tiêu "dùng làm trình phát mặc định".
- [groove-gd2_01] Lối tắt Desktop mặc định không tích (trang chọn thành phần), cài đè giữ lựa chọn cũ — tích sẵn như VLC — Start Menu đủ để tìm, Desktop là tuỳ chọn theo task.
- [groove-gd2_01] Gỡ cài hỏi xoá thư viện + cài đặt (`%APPDATA%\Groove X`, `%LOCALAPPDATA%\Groove X`), mặc định No, gỡ im lặng thì giữ — không hỏi — đúng mục 1 của task; không đụng thư mục `com.mrdxd.groovex` của bản Tauri.
- [groove-gd2_01] Không làm mục chuột phải "Phát bằng Groove X" cho thư mục, không verb "Thêm vào hàng đợi" — làm luôn — `open_paths::resolve` chỉ nhận tệp nhạc, app chưa có cờ dòng lệnh xếp hàng; ghi mục 8.
- [groove-gd2_01] Mục Settings "Trình phát mặc định" nằm ở tab Cài đặt chung, giữa bước tua và EQ; bản không qua bộ cài thì khoá nút và nói lý do — ẩn hẳn mục — người dùng bản dev vẫn hiểu vì sao không đặt được.
- [groove-gd2_01] Version info của exe sinh `.rc` trong `OUT_DIR` từ `Cargo.toml`, bỏ `assets/app/groove.rc` — giữ `.rc` tĩnh — phiên bản không lệch khi `tag.sh` nâng.
- [groove-ui-settings] Vẽ mười cột EQ và thanh bass bằng `canvas` riêng (`eq_bands.rs`) thay `materiant::slider` — bản web có cột dày 7 px, núm 14 px, vầng sáng khi kéo; `Slider` của materiant cố định rail 4 px, núm 16 px, vầng 34 px và không đổi được bằng style — không sửa materiant theo yêu cầu của một app; bàn phím vẫn đủ (Tab vào bảng, ← → chọn dải, ↑ ↓ 0,5 dB, PageUp / PageDown 5 dB, Home / End).
- [groove-ui-settings] Bảng EQ đứng đầu tab Cài đặt chung, bước tua / bước âm lượng / trình phát mặc định xuống dưới có đường kẻ ngăn — ảnh mẫu của chủ dự án mở tab ở Equalizer, trong khi `setting-general.vue` hiện tại đặt bước tua trên đầu — theo ảnh vì đó là cái chủ dự án muốn thấy — EQ là phần dùng nhiều nhất, bước tua ít đổi.
- [groove-ui-settings] Cột tab của hộp Settings tự vẽ (mục chọn có nền màu nhấn nhạt, chữ màu nhấn) thay `materiant::tabs` (vạch ở mép trái) — ảnh mẫu dùng nền mục chọn — giữ bàn phím ↑ ↓ Home End.
- [groove-ui-settings] Không dùng phông đơn cách cho nhãn dB — web dùng `font-mono` — app chưa khai phông đơn cách nào và máy mỗi OS một tên phông — nhãn dB dùng phông chung của app.
- [groove-ui-settings] Bỏ dòng ghi chú dưới bảng EQ (khoá `settings.eq.note`) và khoá `band_label` — bản web không có, dòng đó chiếm chỗ của bảng — bỏ ở cả hai ngôn ngữ; thêm `settings.eq.bass_boost_hint` cho dòng mô tả của thẻ bass; ô chọn mẫu bỏ nhãn phía trên, `settings.eq.preset` thành tiền tố "Mẫu cài sẵn:" cạnh ô chọn. Nhãn thẻ bass giữ "Tăng bass" (bản web viết hoa từng chữ, D-13).

- [groove-ui-nav] Side nav tự dựng hàng điều hướng ở groove, không dùng `materiant::tabs::Tabs::nav` — dùng Tabs.nav — Tabs.nav có vạch trượt 2 px, icon chỉ nhận `IconName` của materiant, một màu cho cả icon và chữ; bản web cần vạch 3 px đứng, icon accent-light + chữ trắng khi chọn, hover đổi riêng icon / chữ, kèm chế độ thu chỉ icon.
- [groove-ui-nav] Màu hàng nav: icon + vạch `#22d3ee` (accent-light), chữ trắng đậm khi chọn; muted `#a1a1aa`, hover `#d4d4d8` — `primary_ink` của materiant — `primary_ink` ở theme tối là cyan pha trắng, lệch màu web; theme sáng vẫn dùng `primary`.
- [groove-ui-nav] Thanh nhảy chạy một `with_animation` lặp 2 713 200 ms (bội chung nhỏ nhất của bốn chu kỳ 600 / 760 / 560 / 680 ms), mỗi thanh tính pha riêng — chu kỳ 1200 ms sin như cũ — web có bốn thời lượng không chung bội nhỏ, nhịp tổng hợp khớp thật; trần 30 khung/s.
- [groove-ui-nav] Pha thanh nhảy giữ ở `Phase` toàn app (global, đồng hồ của executor): dừng thì cộng dồn thời gian đã chạy và đứng đúng khung đó, phát lại chạy tiếp — để GPUI tự giữ pha — GPUI không giữ pha qua lần vẽ lại; một bản chung nên mọi nơi dùng thanh nhảy cùng pha, và dừng lúc không có thanh nào đang hiện thì khung đứng là khung ở lần vẽ kế.
- [groove-ui-nav] Bỏ `PlayingBars::color` và màu `primary_ink` ở ba nơi gọi — giữ tuỳ chỉnh màu — web luôn dùng `--deco-visualizer-color` mặc định cyan-400.
## groove-ui-card

- [groove-ui-card] Thẻ lưới: đệm 12 px, viền 2 px, thẻ rounded_xl, bìa tự bo rounded_lg và cắt góc, chữ cách bìa 12 px, dòng phụ cách 4 px — giữ bố cục cũ viền sát bìa — đúng p-3 rounded-xl border-2 và mt-3, mt-1 của music-card.vue; nền thẻ 40%, hover 60% theo bg-theme-bg-card.
- [groove-ui-card] Viền hover vẫn 2 px, màu accent theo bìa (không có thì cyan) — mảnh hơn 2 px — web cũng border-2 và item.accentColor; ảnh mẫu cyan vì thẻ đó không có accent.
- [groove-ui-card] Không làm chuyển 300 ms, phóng bìa 1,05, quầng bóng, làm mờ nền (backdrop-blur) — GPUI div không có — giữ nhịp bằng đổi tức thì.

- [groove-ui-card-2] Hoạt ảnh hover bằng `with_animation` (thẻ, lớp phủ 300 ms, phóng bìa 1,05 trong 500 ms, đường `cubic-bezier(0.4,0,0.2,1)`), trạng thái rê giữ bằng `window.use_keyed_state` + `on_hover` thay `group_hover` — chỉ giữ `group_hover` — để chạy được hoạt ảnh; chỉ dựng hoạt ảnh sau lần rê đầu, đứng yên thì vẽ tĩnh.
- [groove-ui-card-2] Quầng bóng accent 25% nhưng blur 12 px — 20 px như web — ô `VirtualGrid` cắt mọi thứ ngoài ô (`virtual_grid.rs` dòng `overflow_hidden`, đệm 6 px), blur 20 để lộ khung chữ nhật bị cắt; muốn đúng 20 thì materiant cần tuỳ chọn không cắt ô.
- [groove-ui-card-2] Không làm đổi màu tên sang trắng khi rê (150 ms) và `backdrop-blur` của lớp phủ — GPUI không có làm mờ nền; chữ tên đã gần trắng ở nền tối.

- [groove-ui-nav] Hàng side nav dựng trên `gpui_kit::base::Button` (focus, Tab, Enter / Space, vai trò nút, nhãn trợ năng, `selected` cho mục đang chọn) với vòng focus 2 px tự vẽ trong hàng — IconButton / Button của materiant — vòng focus của materiant vẽ tràn ra ngoài, bị cột `overflow_hidden` cắt.

## ui-player-bar

- [ui-player-bar-2] Nút Play vùng bấm tròn 60 px, icon 36 px như `toggle-play.vue` (thay 48 px của lượt trước) — giữ 48 px — chụp thử thanh 90 px vẫn gọn: hàng nút 60 px + `mb-1` + hàng tua nằm trọn trong thanh.
- [ui-player-bar] Nút Play không nền, không đổi màu khi rê (đúng web), chỉ thêm vòng focus khi Tab tới — nền cyan tròn của bản Rust cũ — chủ dự án muốn khớp web.
- [ui-player-bar-2] Thêm ở materiant `.ink` / `.hover_ink` (Button, IconButton) và `IconButton::icon_size`, icon đổi màu theo group hover của nút — vẽ nút riêng ở groove — `icon-btn.vue` cần chữ phụ rê thành chữ chính, hộp 36 icon 28; materiant cũ cố định màu icon lúc nghỉ và cỡ icon 62,5 % hộp.
- [ui-player-bar-2] Nút phụ: hộp 36 px icon 28 px (Settings 24 / 20), cách nhau 16 px (`gap-4`), bên phải 12 px (`gap-3`); tắt dùng `muted` rê thành `text` của theme, bật xanh `#22c55e` cả lúc rê; nền rê là màu chữ (bật: xanh) 9 % — overlay `currentColor` của Vuetify — màu muted của theme materiant (#a3a3a3 tối) lệch 1 bậc so với `#a1a1aa` của web, giữ theme để sáng / tối cùng đúng.

## lyrics-lag

- [lyrics-lag] Hộp lyrics / timeline: bài đang phát điền sẵn chữ panel đang hiện (`Player::loaded_text`), sửa được ngay, không đọc gì; bài khác đọc `MetadataOps::read_text` — bài thư viện lấy DB, chỉ tệp ngoài thư viện mới đọc thẻ — đặc tả 1.6 "lấy từ tệp, tệp không có thì DB" như cũ — 765 / 1769 tệp nhạc của chủ dự án là bản OneDrive chỉ trên mạng, mở tệp là phải tải cả bài về; DB đã khớp thẻ nhờ indexer, watcher, lần lưu của app (N2), và là đúng chữ panel đang hiện. Lưu vẫn ghi tệp (phải tải về nếu tệp chỉ trên mạng).
- [lyrics-lag] Hộp Metadata giữ đọc tệp lúc mở (hiện ngay, chữ "Đang đọc thẻ từ tệp...") — điền sẵn từ DB — đặc tả 1.8 cần đủ trường, bìa, khung thô của tệp; tệp chỉ trên mạng thì chờ OneDrive tải về.

## ui-round2

- [ui-round2] Màu nhấn deeppink lấy từ theme (G-4): gỡ `ACCENT`, `DEFAULT_PLAYLIST_COLOR`, cyan cứng ở side nav, thanh nhảy; playlist không có màu dùng `primary`, hộp tạo playlist mặc định màu nhấn lúc mở — giữ `#06b6d4` làm màu playlist mặc định — G-4 cấm mã màu nhấn cứng; playlist cũ đã lưu màu cyan trong DB giữ màu của nó (dữ liệu người dùng). Xanh `#22c55e` của Shuffle / Repeat bật và xám `#d4d4d8` hover side nav không phải màu nhấn, giữ.
- [ui-round2] Chuẩn hộp thoại ở materiant: `Dialog::divided(true)` (header / body / footer, kẻ 1 px `border`, đệm ngang 24 px, dọc 16 px) và `flush_body(true)` (body không đệm, cho khung cuộn chạm mép, D-16) — chỉ làm ở groove — mọi app dùng được; để tuỳ chọn, không đổi mặc định nên fastcast giữ nguyên bố cục cũ. groove bật `divided` ở mọi hộp có tiêu đề / nút: Settings, metadata, lời bài / timeline, tạo playlist, xác nhận xoá; bảng lệnh `divided` + `flush_body` (không header / footer) để danh sách chạm mép.
- [ui-round2] Bỏ nút ✕ ở mọi hộp của groove (Settings, metadata, lời bài / timeline) cho đồng bộ một chuẩn — chỉ bỏ ở Settings — Huỷ, Esc (và bấm nền nơi có) đã đóng. Bấm nền đóng (coi như Huỷ, G-5) chỉ đổi ở Settings; metadata giữ `close_on_backdrop(false)` vì form dài, bấm hụt mất cả bản sửa.
- [ui-round2] Body Settings: cột tab cách mép trái 12 px, khe 8 px, nội dung cuộn đệm 16 px đều hai bên, trên dưới 12 px (bản web: tab 13 px, nội dung cách mép phải ~17 px). Tiêu đề hộp giữ 16 px đậm vừa của materiant — web to hơn (~22 px đậm) — chủ dự án không nêu; đổi thì đổi chung ở materiant.
- [ui-round2] Thẻ lưới lấp kín ô: materiant thêm `VirtualGrid::cell_padding` / `cell_radius` / `clip_cells`; groove đặt `cell_padding(0)`, `cell_radius(12)` (= `rounded-xl` của thẻ, tính tới mép ngoài viền), `clip_cells(false)` — nền rê / chọn / ripple của ô trùng khít viền thẻ, quầng bóng `0 0 20px <accent>40` như web tỏa vào khe; khe lưới 24 px (`gap-6` của web) thay 16 + 2 × 6 đệm ô; ô tối thiểu 128 px để cửa sổ nhỏ nhất vẫn 2 cột. Quầng của hàng đầu bị mép trên khung cuộn cắt (lưới không có đệm dọc).
- [ui-round2] D-16 đã soát: lưới thư viện (đúng sẵn, `padding_x` 24), danh sách bài (sửa: bỏ `px` ngoài, `padding_x(24)`, dòng tiêu đề cột tự lùi 24), hàng đợi và timeline (sửa: `.p_1()` ngoài thành `py_1`, `padding_x(TRACK_THICKNESS)`, nội dung dòng lùi để thẳng tiêu đề), lời bài (đúng sẵn), Settings (sửa), metadata (sửa: `flush_body`, đệm trong vùng cuộn), bảng lệnh (sửa), hộp lời bài / timeline / playlist / xác nhận (không có khung cuộn ngoài ô nhập; ô nhiều dòng tự cuộn trong khung viền của materiant).
- [ui-round2] Chữ đậm về thường (chủ dự án 02/11/2026): tên bài và tiêu đề cột ở danh sách bài, tên bài hàng đợi, dòng đang phát ở timeline, mục chọn side nav, tab Settings, tiêu đề các mục trong Settings (Equalizer, Tăng bass và số dB, bước, phím tắt, thư mục, trình phát mặc định), tên thẻ, chữ thanh phát, nhãn nhóm bảng lệnh, nhãn loại / tên nhỏ ở đầu danh sách bài, thông báo trống, tab Albums / Folders và tab metadata (`Tabs::font_weight(NORMAL)`, materiant thêm tuỳ chọn, mặc định vẫn `MEDIUM`). Giữ đậm: tiêu đề "Lyrics", "Playing queue", tên bài lớn ở Now Playing, tên lớn ở đầu danh sách bài (36 / 20 px), tên app "Groove X" đầu side nav, tiêu đề hộp thoại (materiant). Còn đậm vừa do mặc định materiant, chưa đổi: nhãn ô nhập (`Name`, bước tua…), `Badge` ở bảng lệnh, mục chọn trong `Select`.
- [ui-round2] Không chụp được thẻ khi rê trên desktop ẩn: thư viện tạm trống, `--keys` không gõ được chữ nên không tạo được playlist; soát bằng test (`cards_fill_their_cells`, `cell_padding_insets_the_content`, `clip_cells_controls_overflow`). Cần chủ dự án xem tay.

## audio-webaudio (G-7, 02/11/2026)

- [audio-webaudio] Gỡ preamp tự động của EQ (gạch `[groove-gd1_11]` preamp ở trên hết hiệu lực) — giữ preamp, hoặc limiter / soft clip — G-7: Web Audio của bản cũ không hạ mức; preamp làm bass boost +10 dB hạ cả bài 9,87 dB (3,1 lần) và 40 Hz còn -3,87 dB so với tắt EQ. Đổi lại: EQ dương trên bài master sát 0 dBFS có thể vượt trần, bị cắt cứng ở callback (như đường ra float của trình duyệt), y như bản web.
- [audio-webaudio] Âm lượng tuyến tính `volume / 100` (gạch `[groove-gd1_10]` `v²` ở trên hết hiệu lực) — giữ `v²` — G-7: `GainNode.gain = volume / 100` của bản cũ; `v²` làm thanh 75 % nhỏ hơn web 2,5 dB, 50 % nhỏ 6 dB, 25 % nhỏ 12 dB (4 lần).
- [audio-webaudio] Test hộp Settings `c1b_preset_reaches_the_engine` đổi mẫu `Bass strong` sang `Vocal` — sửa test khác — hết preamp thì `Bass strong` gần như không đổi tiếng 440 Hz của test (đúng như web), `Vocal` có +3 dB ở 500 Hz.

## dialog-font

- [dialog-font] Đệm dọc hộp chia ba phần: header 10 px (tiêu đề không đặt line-height), footer 8 px (hàng nút có chiều cao riêng, chữ trong nút line-height tự động), body vẫn 16 px — yêu cầu "line-height tự động 8 px, không đặt line-height 10 px" không nói rõ phần nào là phần nào — theo ảnh Settings: tiêu đề chỉ có chữ trần (không đặt line-height) nên 10, hàng nút Huỷ / OK là phần tử có chiều cao riêng nên 8. Đổi hằng `HEADER_PADDING_Y` / `FOOTER_PADDING_Y` ở materiant `overlay/dialog.rs` nếu hiểu ngược.
- [dialog-font] Ô tìm bảng lệnh: `TextField::flat(true)` mới ở materiant (không viền / nền / bo / vòng focus, icon đầu), hàng cao 56 px (như `HEADER_HEIGHT` của launcher fastcast) có gạch `border_b` dưới; bỏ badge "Search" (khoá `palette.mode_search` xoá khỏi hai catalog), giữ badge "CLI" khi gõ `/` vì đó là chỉ báo chế độ, không phải nút Search — user chỉ nói bỏ "Search".
- [dialog-font] G-6: materiant thêm tuỳ chọn độ đậm (mặc định giữ nguyên, fastcast không đổi): `Dialog::title_weight`, `Badge::font_weight`, `TextField` / `TextArea` / `NumberField::label_weight`, `Select::label_weight` / `selected_weight`, `ToastSpec::title_weight`; groove truyền `NORMAL` ở mọi chỗ dùng. `Menu::label` (nhãn nhóm SEMIBOLD) chưa thêm tuỳ chọn vì groove không dùng. Test `crates/groove-app/tests/font_weight.rs` quét `src/**` không còn `FontWeight::` / `.font_*()` khác NORMAL.
- [dialog-font] `side_nav.rs` (tệp của làn khung cửa sổ + side nav) còn `SEMIBOLD` lúc gộp nên sửa nốt về NORMAL ở nhánh này để test quét xanh.
## player-polish

- [player-polish] Bấm Mute đưa thanh âm lượng về 0 và nhớ mức cũ trong settings (`volumeBeforeMute`, ghi qua `update_settings`, sống qua lần mở lại app); bỏ Mute trả về mức đó. Kéo thanh / bánh xe lên khỏi 0 khi đang tắt tiếng thì bỏ tắt tiếng (`Player::set_volume`). Chỉ khi không có mức nhớ (settings cũ có `isMuted` mà `volume` 0): bỏ Mute về 50 (`UNMUTE_FALLBACK_VOLUME`); tắt tiếng kiểu cũ (`isMuted` với âm lượng > 0) bỏ Mute giữ mức đang đặt — chủ dự án: mở lại app đang mute không được mất mức. Sửa tối thiểu ở `player.rs` (`set_volume`, `toggle_mute`, hàm thuần `mute_toggle`) ngoài phạm vi player_bar.
- [player-polish] Repeat one: dùng sprite `LoopBadge` của bản web (vòng có chỗ trống phía trên bên phải) và vẽ số "1" vào chỗ trống ngay trong `loop-badge.svg` — bản web chỉ chừa chỗ chứ chưa vẽ số, nên toạ độ chữ số là ước lượng — một icon, không chồng phần tử, đổi màu theo nút.
- [player-polish] Ripple nút Play: tâm giữa, `RippleTrigger` giữ trong `PlayerBar` (hai nút: giữa thanh và trên bìa) vì `play_button` chỉ có `&Window`; Space / Enter phát ripple khi `ClickEvent::Keyboard`. Nền nút vẫn không có, chỉ có vòng loang.
- [player-polish] Cỡ thanh: materiant thêm `Slider::scale` (mặc định 1, cỡ vùng bấm không đổi); groove đặt 0,85 cho thanh âm lượng (rail 3,4 / 5,1 px, chấm 13,6 px). Thanh tua của groove là canvas riêng, không có chấm tròn: chỉ rãnh 4 / 6 / 8 px thành 3,4 / 5,1 / 6,8 (`seek_bar::SLIDER_SCALE`).
- [player-polish] Gỡ nút Settings khỏi thanh phát và khoá i18n `player.bar.settings`; ô chữ thanh phát đặt `font_weight(NORMAL)` ở gốc thanh để mọi chữ con (kể cả Marquee) thường.

## chrome-nav

- [chrome-nav] Nút cửa sổ là vùng điều khiển của OS (`WindowControlArea::Min / Max / Close` ⇒ `HTMINBUTTON`…), không `on_click` trên Windows — sửa trong groove bằng lời gọi khôi phục riêng — `zoom_window` của gpui-pre-windows chỉ phóng to, không khôi phục; đường của OS có cả khôi phục, Snap Layouts của Windows 11, giống nút tiêu đề thật. macOS / Linux vẫn `on_click` gọi API GPUI. Nút cửa sổ không còn nằm trong vòng Tab (như Windows).
- [chrome-nav] Màu nút cửa sổ: Đóng rê `#c42b1c` icon trắng, nhấn đỏ 90 %; thu nhỏ / phóng to rê lớp màu chữ 9 %, nhấn 6 % (Windows 11: nhấn nhạt hơn rê); icon nét 1 px 10 px tự vẽ (`window-*.svg`) — giữ icon 24 px cũ — ảnh 27 của chủ dự án.
- [chrome-nav] Thu nhỏ: ở desktop ẩn, bản cũ bấm Minimize vẫn thu được (`IsIconic` true), chỉ Restore là hỏng thật; không tái hiện được lỗi thu nhỏ ngoài màn thật. Bản mới đi đường OS nên hết phụ thuộc chuyện này — cần chủ dự án bấm thử trên màn thật.
- [chrome-nav] Nút thu cột trái: một nút tròn 28 px icon khung có vách + mũi tên (`sidebar-collapse` / `sidebar-expand`) ở đầu cột, cạnh tên app; thu lại thì đứng giữa đầu cột 56 px — bỏ nút lùi của thanh trên — "nút `<` thứ hai" ở ảnh 19 là nút Lùi theo lịch sử (đặc tả 1.1), không phải nút thu; giữ nó, đổi icon nút thu cho khỏi trông trùng. Bề rộng vẫn nội suy 300 ms như trước. Tên app đầu cột về độ đậm normal (G-6).
- [chrome-nav] Vạch chỉ báo side nav: `materiant::tabs::InkBar` (tách từ `Tabs`, cùng 250 ms `cubic-bezier(0.35, 0, 0.25, 1)`), dày 3 px màu nhấn — dùng thẳng `Tabs::nav()` — `Tabs` chỉ một điểm dừng Tab + phím mũi tên, side nav cần giữ Tab qua từng hàng, Enter / Space.
- [chrome-nav] Chữ chạy: canvas tự vẽ hai bản chữ theo đồng hồ, xin khung theo nhịp màn hình chỉ khi đang chạy; rê chuột dừng tại chỗ (như web), trước đây quay về đầu dòng. Khoảng giữa hai bản giữ 48 px (web 32 px). Chạy liên tục là vẽ lại cả cửa sổ ~60 lần / s khi thanh phát có tên dài (GPUI không vẽ lại từng phần) — trước là ~20 lần / s.
- [chrome-nav] Tên bài lớn ở Now Playing vẫn `FontWeight::BOLD` (`now_playing.rs`) — thuộc làn độ đậm chữ của agent khác, không sửa ở đây để tránh xung đột.

## ui-audit

- [ui-audit] Chữ chạy (`Marquee`) và thanh nhảy (`PlayingBars`) chỉ chạy khi cửa sổ active; cửa sổ nền / thu nhỏ đứng tại chỗ, active lại chạy tiếp — chạy mọi lúc như web — đo desktop ẩn, release: tên dài chạy ~37–46 % một nhân (cả cửa sổ vẽ lại mỗi nhịp màn hình), đứng yên 0,9–1,4 %; cửa sổ nền sau khi sửa 1,1–1,4 %. RAM không đổi (working set 63–67 MB, private ~71 MB). Lúc active vẫn ~35–45 %: không hạ nhịp (chủ dự án coi trần khung là lỗi), 1/4 px không giúp vì 54 px/s cần >60 vị trí/s.
- [ui-audit] Thử `.cached()` cho view trang trong `Shell` (khung vẽ lại vì chữ chạy dùng lại cây của trang): 37–46 % → 34–39 % ở thư viện trống, rồi bỏ — view cached chỉ dựng lại khi chính nó `notify`, trang đọc entity khác trong `render` sẽ hiện dữ liệu cũ; lợi ít so với rủi ro.
- [ui-audit] Sửa chữ chạy không khởi động: bài khôi phục khi mở app (chưa phát, chuột không động) đứng cắt `…` mãi vì số đo trong `prepaint` gọi `notify` giữa lúc vẽ không xin khung mới; giờ `request_animation_frame`.
- [ui-audit] Thanh nhảy bỏ `with_max_fps(30)` và layout `div` (làm tròn pixel) — vẽ `paint_quad` toạ độ lẻ mỗi nhịp màn hình — chủ dự án coi trần khung là lỗi; giá CPU như chữ chạy lúc cửa sổ active và danh sách đang hiện bài phát.
- [ui-audit] Ảnh bìa vẽ bằng `paint_image(khung, vùng cover × zoom, bo góc)` trong `CoverImage` (`.radius`, `.zoom`, `.embedded`): bìa thẻ phóng 1,05 không còn lòi góc vuông, bìa Now Playing bo 16 px như khung (trước 8 trong khung 16), bìa nhúng ở thanh phát bo 8 (trước vuông), lớp phủ rê trên bìa thẻ / thanh phát bo cùng bán kính, ảnh trong ô bìa hộp metadata bo 6 (8 trừ viền 2).
- [ui-audit] Nút Lùi ở thanh trên thành nút icon tròn 32 px cách mép 2 px — giữ ô chữ nhật cao bằng thanh kiểu Windows — luật nút icon tròn; ba nút cửa sổ bên phải vẫn kiểu Windows.
- [ui-audit] Thêm ripple cho thẻ hành động đầu trang thư viện, tab dọc của Settings, hàng side nav — để nguyên — mọi mục bấm được có ripple như component materiant.
- [ui-audit] Độ đậm normal đặt một lần ở theme (`app::groove_theme`: `label_weight` / `title_weight` 400); các `*_weight(NORMAL)` cũ ở view giữ nguyên (vô hại).
- [ui-audit] Vòng focus của nút Play, hàng side nav, bảng EQ đi qua `materiant::style::focus_visible`; lưới thư viện hết hiện vòng khi bấm chuột rồi Space.
- [ui-audit] Màu cảnh báo materiant thành cam `#c2410c` chữ trắng: toast cảnh báo (nhập thiếu bài, quét lại có lỗi) đổi theo; dòng cảnh báo trong hộp metadata (nhóm album, lỗi từng tệp khi sửa lô) dùng `Accent::Warning.tokens(cx).ink` thay `c.warning` trần — chữ cam trên nền tối chỉ ~3,3 : 1.

## cpu

- [cpu] Đo bằng bộ lấy mẫu stack tự viết (C# qua `Add-Type`: treo luồng, `StackWalk64`, trọng số theo `QueryThreadCycleTime`, ký hiệu từ PDB; chỉ ở scratchpad, không vào repo) — `samply` / WPR cần quyền admin trên Windows, máy không chạy admin. Kết quả bản cũ, chữ chạy, cửa sổ active: luồng chính ~99 % CPU của app, trong đó `Window::draw` 84 %: bố cục (`request_layout` + taffy) ~36 %, `paint` ~29 %, `prepaint` ~10 %, `present` DirectX ~7 %, dọn arena ~5 %; `render` của view chỉ ~3 %, shape chữ ~2 % — tiền nằm ở việc cả cây cửa sổ đi qua bố cục / vẽ mỗi khung, không phải ở chữ chạy.
- [cpu] Sửa bằng view `cached` của GPUI theo đường hoạt ảnh (`views::island`), không hạ nhịp khung — chủ dự án coi trần khung là lỗi — chữ chạy, thanh nhảy là entity riêng; Shell nhúng `cached` cột trái, thanh trên, trang thư viện; thanh phát giữ bìa / cụm điều khiển / âm lượng `cached`; Now Playing giữ dòng thời gian / lời bài / hàng đợi `cached`.
- [cpu] Bỏ phương án vẽ hoạt ảnh trên một lớp phủ cấp Shell (đăng ký toạ độ từ view chủ) — lớp phủ đè lên hộp thoại, dữ liệu vị trí cũ khi view chủ dùng lại cache, khó đúng — cách `cached` theo đường tổ tiên là cách GPUI định sẵn.
- [cpu] Trang thư viện nhúng `cached` (bản ui-audit bỏ vì sợ dữ liệu cũ): soát `LibraryPage` — mọi setter đều `notify`, danh sách bài nghe `Player`, ảnh bìa / trạng thái khoá tự `notify` view đang vẽ; theme, ngôn ngữ, cỡ cửa sổ, rê, focus thì GPUI làm mới cả cửa sổ. Có test chứng minh trang vẫn cập nhật khi nó `notify` và không dựng lại theo khung của hoạt ảnh.
- [cpu] Bảng lệnh và hộp Settings vẫn nhúng thường (dựng lại theo khung của chữ chạy, ~0,5 % một nhân theo profiler) — để nguyên — chúng đọc nhiều entity, đóng thì gần như rỗng; cache sau khi soát kỹ nếu cần.
- [cpu] Thanh nhảy trong danh sách bài (trang thư viện) vẫn dựng lại cả danh sách mỗi khung khi đang phát: hàng của danh sách ảo dựng theo closure, không cache từng hàng được mà không trễ một khung — để nguyên, chỉ còn trang đó (không còn cả cửa sổ). Hàng đợi ở Now Playing tương tự (đo ~9 % cùng chữ chạy).
- [cpu] Số đo (03/11/2026, desktop ẩn, release + debug-assertions, cửa sổ active 816 × 608, 60 khung / s, hai bản đo trong cùng lượt): thư viện trống chữ chạy 17 % → 7–8 %; 300 bài, My Music, chữ chạy 31 % → 7–9 %; Now Playing (hai dòng chữ chạy) 19 % → 8–9 %; Now Playing chữ chạy + thanh nhảy hàng đợi (ép chạy bằng bản vá tạm) 19–21 % → 9–10 %; cửa sổ nền 0,4–0,8 % như cũ. RAM không đổi (working set 63–66 MB, private 67–72 MB). Lượt đo đầu buổi bản cũ ra 44–48 % (thư viện trống): cùng exe khác lượt lệch 2–3 lần, nên chỉ so trong cùng lượt.
- [cpu] Phần còn lại ~7 % là chi phí cố định của GPUI mỗi khung: phát lại cả cảnh (`Scene::replay`, dùng lại nét vẽ của view `cached`), dựng bộ đệm DirectX và `present` toàn cửa sổ, luồng driver GPU. Giảm tiếp phải sửa GPUI (vẽ lại / present theo vùng bẩn, `Present1` với dirty rect), không vá ở groove — ghi `_backlog` của materiant nếu chủ dự án muốn.

## nowplaying-ui

- [nowplaying-ui] Placeholder bìa album (và playlist) dùng icon đĩa `album.svg` (= symbol `#Album` của `cli.svg`, đã tách sẵn từ trước) thay `default-cover`. Dáng theo chỗ đặt như web (`CoverImage::look`): thẻ nền N200 icon 64 px; bìa lớn Now Playing gradient N300 → N100 icon 64 px; bìa nhỏ thanh phát không nền, icon lấp khung trừ đệm 8 px. Màu `text-theme-text-disabled` của web lấy bậc N500 của dải trung tính theme (không mã màu cứng).
- [nowplaying-ui] Bìa nhỏ thanh phát vuông, bỏ bo góc cả lớp phủ hover (chủ dự án yêu cầu; web `audio-controller.vue` cũng không bo).
- [nowplaying-ui] Trang thư viện / danh sách bài: icon trạng thái rỗng / lỗi đổi sang đĩa như các trạng thái rỗng của web (`my-music.vue`). Hộp Metadata giữ `default-cover` vì web cũng dùng `DefaultCover` ở đó.
- [nowplaying-ui] Ba nút khung Lyrics: `Button` tonal, `Size::Sm`, bo tròn viên thuốc, icon trái (Album / Loop / Edit) — không sửa materiant. Màn Now Playing không còn nút chữ nào khác (hàng đợi, dòng thời gian không có `Button`).

## window-buttons

- [window-buttons] Nút thu nhỏ / phóng to / đóng thành nút bấm thường (`on_click` → `platform::window_controls`), bỏ vùng `WindowControlArea` cho OS hit-test — sửa gpui-pre-windows, hay giữ đường OS — chủ dự án bấm trên màn thật không ăn; nguyên nhân tìm ra sau: thanh trên nhúng `cached`, GPUI không chép vùng `WindowControlArea` khi dùng lại khung (mục kéo cửa sổ dưới) — giữ nút client vì chắc ăn và đi đúng đường mọi nút khác của app đang chạy. Mất Snap Layouts khi rê nút phóng to trên Windows 11 (Windows 10 không có). Luật "Nút cửa sổ" ở `materiant/CLAUDE.md` (dùng `window_control_area` trên Windows) chưa sửa — repo materiant, chờ chủ dự án xem bản này trên màn thật.
- [window-buttons] Windows phóng to / khôi phục bằng `ShowWindowAsync` (`IsZoomed` ⇒ `SW_RESTORE`, không ⇒ `SW_MAXIMIZE`), đóng bằng `WM_CLOSE` (qua `on_window_should_close` như nút tiêu đề thật) — `zoom_window` của GPUI không khôi phục. Kiểm trong desktop ẩn bằng `measure-hidden.ps1 -Clicks`: phóng to → khôi phục → thu nhỏ → đóng đổi `IsZoomed` / `IsIconic` thật, cả khi cửa sổ active lẫn nền, cả khi rê 1,5 s cho tooltip hiện rồi mới bấm.
- [window-buttons] Nút như `icon-btn` của web: ô 36 px (trước 46 px), icon 20 px `Minus`, `ArrowsExpand` / `ArrowsCompress`, `Close` (tách sẵn từ `arrow.svg`, `action.svg`); bỏ bốn icon `window-*` tự vẽ. Rê Đóng đỏ `red-600` `#dc2626` của web (trước `#c42b1c` của Windows 11), icon trắng. Rê thu nhỏ / phóng to giữ lớp xám vuông cao bằng thanh, không bo 4 px như `v-btn`.
- [window-buttons] Kéo cửa sổ như `data-tauri-drag-region` của web: cả thanh trên và header cột trái (chữ Groove X), trừ nút Lùi, ô tìm, nút cửa sổ, nút thu cột; nhấp đúp phóng to / khôi phục (OS lo qua `HTCAPTION`). Vùng kéo cũ đã khai nhưng chết khi chạy thật: nằm trong view `cached` (Island), GPUI không chép vùng `WindowControlArea` khi dùng lại khung. Sửa bằng khung bọc ở `Shell` nằm sau phần `cached` — sửa gpui-pre (chép thêm `window_control_hitboxes` trong `reuse_prepaint`) hay bỏ `cached` của thanh trên / cột trái (tốn CPU, task cpu) — không vá crate ngoài, giữ số đo CPU. Hover nền header cột trái của web (`hover:bg-side-nav-header-hover`) chưa làm.
- [nowplaying-ui] (lượt 2) Ba nút khung Lyrics bỏ tonal (chủ dự án: nền đậm quá): `Button` dáng text màu `c.text` (nền rê / nhấn trung tính, ripple + vòng focus giữ nguyên), chữ / icon `primary_ink`, đè nền `layer(N200)` alpha 0,5, viền `border` alpha 0,3, cao 28, đệm ngang 12, chữ 12 px như `now-playing.vue`. Lúc rê viền về trong suốt (dáng text của materiant), không sửa materiant cho một chỗ.
- [nowplaying-ui] Nút thu / mở cột trái: icon 16 → 18 px, nét 1,75 → 1,5 (chủ dự án: to hơn, mảnh hơn).
- [nowplaying-ui] Bảng lệnh: bỏ tiêu đề nhóm "Quick actions" khi ô tìm trống (chủ dự án), xoá khoá `palette.group.quick`; nhóm Commands / Songs giữ tiêu đề.
- [window-buttons] Vẽ lại `minus.svg`, `arrows-expand.svg`, `arrows-compress.svg` (chỉ nút cửa sổ dùng) cho cùng cỡ quang học với X (chủ dự án yêu cầu): nét `stroke` 1,5 đơn vị đầu tròn như X, cùng khung 12 × 12 đơn vị (X đi 6 → 18 trong viewBox 24; gạch cũ 4,25 → 19,75, mũi tên cũ 2,25 → 21,75), mũi tên giữ dáng web thu 2/3 quanh tâm. Đoạn thẳng đặt khớp lưới điểm ảnh ở cỡ 20 px (lệch ≤ 0,4 px) để gạch không nhoè hai hàng, nhạt hơn X. Không đổi `close.svg`.
- [nowplaying-ui] (lượt 3) Nút thu / mở cột trái: SVG vẽ lại trên lưới 18 đơn vị, nét 1 px canh nửa điểm ảnh (vẽ 18 px ⇒ sắc, không nhoè 2 px), màu như icon mục nav chưa chọn (`muted`, rê sáng lên) thay trắng đặc — chủ dự án thấy vẫn dày ở bản 1,5.
- [nowplaying-ui] Placeholder bìa lớn Now Playing: đĩa to gấp 3 (64 → 192 px, kẹp 70 % khung khi cửa sổ hẹp), dùng `album-thin.svg` — cùng hình `album.svg` vẽ lại bằng nét 0,6 / 46 (≈ 2,5 px ở 192 px) để nét không phình như bản tô kín. Thẻ, thanh phát giữ `album.svg`.

## linux-chrome (03/11/2026)

- [linux-chrome] Linux không có mờ (ngoài Wayland + KWin) thì cột trái alpha 0,94 như khối nổi Linux (D-10), cửa sổ vẫn `Blurred` (= trong suốt trơn); Wayland + KWin giữ 0,88 + mờ (`platform::backdrop`). Chủ dự án chốt 03/11/2026 sau khi thử bản đục: "không làm được thì đưa về trong suốt 0,94". GNOME 50 trên máy công ty không quảng bá giao thức mờ nào (đã liệt kê global Wayland), nên không app nào xin mờ được ở đó; muốn mờ thật trên GNOME phải nhờ extension (Blur my Shell).
- [linux-chrome] Nhấp đúp vùng kéo ở macOS / Linux sửa ở `materiant::window::DragRegionExt::drag_region` (dùng chung mọi app) chứ không riêng groove: lần nhấn thứ hai (`click_count >= 2`) gọi `titlebar_double_click` (macOS, theo cài đặt hệ thống) / `zoom_window` (Linux, chỉ cửa sổ đổi cỡ được) thay vì `start_window_move`. Windows không đổi (OS lo qua `HTCAPTION`).

## card-hover (03/11/2026)

- [card-hover] "Nút hover hơi xám, opacity 0,85" hiểu là: nền nút lúc rê nhích một bậc trung tính (`neutral(N200)` → `neutral(N300)`) với độ đục 0,85 (lúc nghỉ giữ 0,8 như bản web), nền rê 9 % của `IconButton` tắt (màu gốc trong suốt, `.ink` giữ màu icon) — trước đó hai lớp chồng thành mảng xám đặc — hoặc làm mờ cả nút 0,85 (icon cũng mờ, khó đọc trên bìa sáng). Chưa nhìn trên màn thật.
- [card-hover] Nút 36 px (32 + 4), icon 22 px, nét icon 2 → 1,5: Play thêm biến thể `PlayThin` (Play dùng ở danh sách bài, thanh phát), Delete chỉ thẻ dùng nên làm mảnh ngay tệp `delete.svg` — hoặc tạo `DeleteThin` riêng.
- [card-hover] Bỏ phóng bìa 1,05 khi rê, xoá luôn `CoverImage::zoom` (không chỗ nào khác dùng, Now Playing không phóng).

## settings-eq-nav (03/11/2026)

- [settings-eq-nav] Thanh dải EQ giữ vẽ bằng `canvas` của groove (`eq_bands`), đổi sang đúng dáng thanh Bass boost dựng đứng: rãnh và phần tô cùng 2 px, núm 10 px, vầng 22 px (dùng chung hằng số với thanh bass) — hoặc thêm chiều dọc cho `materiant::slider` (rail 4 px, núm 16 px cố định, sẽ dày hơn thanh bass mà chủ dự án lấy làm mẫu, và phải viết lại bàn phím một điểm dừng Tab cho mười dải). Bỏ hẳn đường cong nối các núm, xoá `eq_curve.rs`. Chưa nhìn trên màn thật.
- [settings-eq-nav] Cột tab của hộp Settings dùng lại `side_nav::NavInk` (mở `pub`) + `materiant::tabs::InkBar` dọc 3 px; hàng cao 40 px (cột trái chính 42 px), không icon, không bo góc, không khe giữa hàng — giữ mỗi hàng một điểm dừng Tab như trước (↑ ↓ Home End), thêm vòng focus như hàng của cột trái.

## ui-font (03/11/2026)

- [ui-font] Groove dùng Mona Sans (chủ dự án hỏi Mona Sans rồi đặt tệp vào materiant), Be Vietnam Pro nhúng ở materiant nhưng groove không bật — hoặc Be Vietnam Pro (thiết kế cho tiếng Việt). Đổi: `groove_theme` một dòng `font_family` + feature trong `Cargo.toml`.
- [ui-font] Nguyên nhân lệch dấu chưa chứng minh bằng mắt: dữ liệu tên bài là Unicode dựng sẵn (`ọ` = U+1ECD), font hệ thống Ubuntu Sans là font biến thiên; đã đo GPUI Linux nhận đúng họ Mona Sans và chọn đúng tệp theo độ đậm, hết cảnh báo `.SystemUIFont`. Dấu có về đúng chỗ không: chờ chủ dự án xem.
- [ui-font] Ô chọn font là `Select` ở Settings → Chung, đầu khối dưới bảng EQ, áp ngay khi chọn, Huỷ / Esc trả về như bước tua — hoặc radio, hoặc áp khi bấm Lưu. Tên font hiện nguyên văn ở cả hai ngôn ngữ (tên riêng), mục chưa vẽ bằng chính font của nó (`Select` của materiant chưa nhận mục tự vẽ). Khoá `font` trong `settings.json`: `mona_sans` (mặc định) / `be_vietnam_pro`.
- [marquee-pause] Nhạc dừng thì tên bài chạy (thanh phát, Now Playing) dừng tại chỗ, phát tiếp chạy tiếp từ chỗ đó — chạy cả lúc dừng như web — lúc đứng yên không xin khung nào (số đo cpu: chạy ~7–9 % một nhân, đứng yên ~1 %).

## settings-theme (03/11/2026)

- [settings-theme] Bảng màu ở Settings → Chung dùng `materiant::theme::picker::ThemePicker` (thẻ xem trước nền, cột bên, chữ, nút màu nhấn — component chung, gallery trang theme cũng dùng thay hàng nút chữ cũ) — hoặc giữ nút chữ như gallery cũ. Bốn preset: Mặc định (deeppink, G-4), Đen hồng, Hồng đỏ, Tím; khoá `themePreset` trong `settings.json` (`default`, `black-pink`, `rose-red`, `rebecca-purple`), khoá lạ ⇒ mặc định. Chưa nhìn trên màn thật (đã chụp ở GNOME Shell headless: tối, sáng, đổi preset).
- [settings-theme] Nền, bảng màu áp ngay khi chọn và lưu (debounce) như font; Huỷ / Esc / bấm lớp phủ trả cả hai về lúc mở hộp.
- [settings-theme] Nền và bảng màu tách hẳn (chủ dự án, ảnh mẫu popover hai mục): ở materiant preset không còn ghim nền (Black Pink từng ghim tối), `set_palette` chỉ thay màu, giữ nền / font / cỡ chữ; groove lưu `theme` và `themePreset` riêng, Nền `Hệ thống` đổi sáng / tối theo OS với bảng màu đã chọn (mọi preset đủ cặp sáng / tối, có test).
- [settings-theme] Mục Nền là nút segmented Sáng / Tối / Hệ thống (`materiant::button::segmented::SegmentedButtons`, mới) như ảnh mẫu. Mục Bảng màu GIỮ thẻ xem trước (chủ dự án vừa chỉnh dấu chọn của chính các thẻ này thành badge góc trên phải) thay vì danh sách chấm màu của ảnh mẫu; dáng danh sách có sẵn (`ThemePicker::list()`, chấm chia chéo: nửa màu nhấn, nửa nền của preset), đổi một dòng ở `general.rs` nếu muốn.
- [settings-theme] Bỏ dòng "Tự pha một bảng màu →": chưa có trình tự chỉnh màu (cần chọn màu nhấn + sinh cặp sáng / tối + lưu `MateriantTheme` tuỳ chỉnh), `set_palette` đã nhận bảng màu bất kỳ khi làm.
- [settings-theme] Màu nhấn mặc định đổi sang xanh Material `#005cbb` (chủ dự án), nhánh tối `#abc7ff` (azure tông 80 của Material 3, chữ trên nút lật đen) — hoặc giữ `#005cbb` cả hai nền (~3:1 trên nền tối, khó đọc). Áp cho cả fastcast (preset Default của nó). Preset Black Pink đổi khoá thành `deeppink`, nhãn "Deeppink"; khoá cũ `black-pink` trong `settings.json` tự đọc thành `deeppink`. Nhãn preset hai ngôn ngữ theo ảnh mẫu: Rose & Red, Rebecca Purple (tên riêng).
- [settings-theme] Preset Deeppink chỉ đổi màu nhấn (`#c71585` / `#ff1493`), nền, chữ, viền, màu trạng thái như Mặc định (chủ dự án: "chỉ khác đúng màu chủ đạo") — trước đó mang cả nền đen tuyền `#0a0a0a` và trung tính zinc của Black Pink cũ. Rose & Red, Rebecca Purple vẫn có trung tính ám màu riêng.

## wrap

Chủ dự án (2026-11-03): không muốn chữ bị cắt "…" ở đâu trong Groove X, xuống dòng như Apple Music.

- [wrap] Danh sách ảo dòng khác cao (`VirtualList::measured_rows`, materiant) cho hàng đợi Now Playing, hộp timeline, danh sách bài; bảng lệnh dùng `fit_content` (cao theo dòng đã đo, tối đa 352 px) — giữ dòng cao đều + cắt — chữ phải xuống dòng; vẫn chỉ dựng dòng trong khung (D-17).
- [wrap] Tên thẻ trong lưới thư viện (`category_card`) xuống tối đa 2 dòng rồi mới cắt "…", dòng phụ (số bài / nghệ sĩ) vẫn một dòng cắt "…" — cho xuống dòng hết — `VirtualGrid` cần ô cao đều để dựng ảo (D-17); thẻ cao thêm 20 px cho dòng thứ hai.
- [wrap] Tên bài ở thanh phát và Now Playing vẫn chạy chữ (`Marquee`, đã có); tên nghệ sĩ ở thanh phát giờ cũng chạy chữ; Now Playing cho tên nghệ sĩ xuống dòng — xuống dòng ở thanh phát — thanh cao cố định, xuống dòng là đẩy vỡ thanh.
- [wrap] Còn cắt một dòng, chữ là của app (i18n, ngắn), không phải dữ liệu người dùng: tên app + nhãn mục ở cột trái (`side_nav`: cột co giãn 56 ↔ 230 px có hoạt ảnh, vạch chọn đặt theo `ix × ITEM_HEIGHT`), nhãn tab Settings (vạch chọn theo `ix × TAB_HEIGHT`), chữ gợi ý ô tìm (ô nhập một dòng), ô "Đang nhập…" trong lưới (ô cao đều) — đổi sang xuống dòng — thấy "…" ở đây nghĩa là bản dịch quá dài, sửa bản dịch.
- [wrap] Đường dẫn thư mục nhạc trong Settings xuống dòng (ngắt giữa chữ khi không có chỗ ngắt) — cắt "…" giữa đường dẫn — người dùng cần thấy đủ đường dẫn.

## perf-notify

- [perf-notify] Nhịp vị trí 250 ms tách khỏi `Player` thành entity `PlaybackClock` (`Player::clock`) — giữ `notify` Player rồi lọc ở từng view — chỉ thanh tua, nhãn giờ / dòng đang phát của TimelineBox và media session nghe nhịp; `Player` chỉ `notify` khi trạng thái đổi (nhịp vẫn `notify` Player đúng lúc `engine.is_playing()` đổi, để nút Play / Pause theo kịp luồng âm thanh).
- [perf-notify] Cửa sổ nền (không active, vẫn nhìn thấy) thì thanh tua, nhãn giờ vẽ lại mỗi giây một lần (`PlaybackClock::due`) — bỏ hẳn — đứng im trông như treo; nhãn giờ vốn chỉ đổi theo giây. Media session vẫn nhận mọi nhịp.
- [perf-notify] View nghe `Player` chỉ `notify` khi đúng thứ nó vẽ đổi: thanh phát so `BarState` (bài, đang phát, shuffle, repeat, âm lượng / mute), lời bài so `Body`, hàng đợi so danh sách / dòng hiện tại / đang phát, bảng lệnh chỉ khi đang mở, TimelineBox theo giây hiện ở nhãn và dòng đang phát.
- [perf-notify] Thanh tua thành view `cached` riêng trong thanh phát, ra khỏi `Island` cụm nút: nhịp chỉ dựng lại thanh tua.
- [perf-notify] Không nhúng `cached` thanh phát / bảng lệnh / Settings ở Shell như dự tính — `cached` mà bẩn thì GPUI dựng lại CẢ cây con (`refreshing`), chữ chạy trong thanh phát làm nó bẩn mỗi khung, nhúng `cached` còn tốn hơn; bảng lệnh / Settings đóng thì gần như rỗng.
- [perf-notify] Thanh nhảy của bài đang phát trong danh sách bài (trang thư viện), hàng đợi và header dòng thời gian (Now Playing) vẽ trên lớp phủ cấp nội dung của Shell (`views::bars_overlay`) — giữ dựng lại cả trang `cached` mỗi khung (mục `cpu` để nguyên) — làm lại phương án từng bị loại, gỡ từng lý do: lớp phủ nằm trong vùng nội dung nên hộp thoại / menu / bảng lệnh / thanh phát vẫn đè lên; anchor ghi khung + vùng cắt lúc paint, view dùng lại cache chỉ khi bố cục y nguyên nên vị trí vẫn đúng; view chứa anchor `forget` đầu `render`, Shell `forget_all` khi đổi view nội dung; rê hàng thì phần ẩn không paint nên anchor không ghi. Bảng lệnh vẫn dùng `PlayingBars` (không nằm trong view `cached`).
- [perf-notify] Bìa nhúng của tệp ngoài giải mã một lần ở luồng nền (`covers::decode_preview`, cạnh dài 512 px, có giới hạn + `catch_unwind` như bìa thư viện), `Player` giữ một `Arc<RenderImage>` dùng chung cho thanh phát và Now Playing, đổi bài thì `drop_image` (hoãn bằng `defer`) — giữ `Image::from_bytes` + `use_render_image` (ảnh gốc vào bộ đệm tài nguyên của GPUI, không bao giờ nhả, luồng UI chép + băm cả ảnh mỗi lần đổi bài).
- [perf-notify] Hộp metadata (`CoverPane`) nhả thêm texture trong atlas GPU (`get_render_image` lúc vẽ, `drop_image` khi đổi / đóng) — trước chỉ `remove_asset`.
- [perf-notify] Trần bộ đệm bìa của lưới thư viện 192 → 96 (~48 → ~24 MB bìa 256 px): khoảng 2,5 khung ở cửa sổ lớn; cuộn xa hơn thì đọc lại JPEG nhỏ từ đĩa.
- [perf-notify] Watcher: tick của debouncer `debounce / 2` (1 s, trước 500 ms mặc định): báo trễ nhất 3 s thay vì 2,5 s; hẹn giờ thử gắn lại gốc vắng (30 s) chỉ khi có gốc đang vắng.
- [perf-notify] Luồng âm thanh hỏi 100 ms một lần trong 30 s sau khi dừng (`groove-audio` `engine.rs`) — để nguyên — chỉ 30 s rồi ngủ hẳn, sửa phải đổi vòng lặp engine.

## pip

Trình phát thu nhỏ (`views::pip`): cửa sổ 420 × 140, mở từ nút cạnh thanh âm lượng, menu chuột phải của thanh phát, menu dock.

- [pip] Mở PiP thì để nguyên cửa sổ chính như trình duyệt — thu nhỏ / ẩn cửa sổ chính — người dùng tự chọn; nút "Quay lại cửa sổ chính" đóng PiP rồi đưa cửa sổ chính lên. Đóng cửa sổ chính vẫn thoát app (đóng luôn PiP) — giữ app sống khi còn PiP — chưa có khay hệ thống để mở lại cửa sổ chính.
- [pip] Cỡ cố định 420 × 140, không đổi cỡ, không thu nhỏ; chỉ nhớ vị trí (`pipWindow` trong settings) — cho kéo giãn — bố cục cố định bìa vuông + một dòng tên + thanh tua + hàng nút; nháy đúp vùng kéo không làm gì.
- [pip] Mở không lấy focus — lấy focus — giống PiP trình duyệt, không cướp phím của app đang gõ. Hệ quả: chữ chạy chỉ chạy khi người dùng bấm vào PiP (luật CPU: chỉ hoạt ảnh ở cửa sổ active + đang phát).
- [pip] Luôn trên cùng: Windows `SetWindowPos(HWND_TOPMOST)`, macOS `NSFloatingWindowLevel` + theo mọi Space / cạnh app toàn màn hình; Linux không làm (Wayland không có giao thức, người dùng tự chọn Alt+Space → "Always on Top") — `WindowKind::PopUp` / `Floating` của GPUI — PopUp trên Windows mất khung + nút taskbar, trên macOS là `NSPanel` tự ẩn khi app mất focus, trên X11 là override-redirect.
- [pip] Lần đầu mở ở góc dưới phải màn có cửa sổ chính, cách mép 32 px; Wayland không cho app đặt vị trí nên compositor tự đặt và không nhớ được vị trí.
- [pip] Menu dock: Linux `Actions=pip` trong `.desktop` (cả bản dev của `start.sh`) chạy `groove --pip`, đi qua kênh một phiên `instance.rs`; Windows jump list `--dock-action 0` (tiến trình thứ hai chuyển cho phiên đang chạy); macOS menu dock gọi action trong tiến trình — đi qua MPRIS — MPRIS không có lệnh mở cửa sổ riêng, kênh `instance.rs` đã có sẵn và chạy cả khi app chưa mở (mở app rồi mở PiP).
- [queue-scroll-top] Mở màn Now Playing (lần đầu và mỗi lần quay lại từ màn khác) thì bài đang phát lên sát mép trên hàng đợi; đổi bài khi đang ở màn vẫn cuộn vào giữa như bản web — chủ dự án chỉ yêu cầu lúc mở màn — đổi bài cũng đưa lên đỉnh.

## pip-ui

- Thanh tua PiP bỏ hai nhãn giờ để giãn hết bề ngang cột; nhãn khi rê / kéo hiện giờ tại con trỏ (khúc có tên: `giờ · tên (đầu - cuối)`).
  Không còn thấy thời lượng bài khi không rê — đổi lấy thanh dài như chủ dự án yêu cầu.
- Lỗi "rê là thanh chạy theo": cả PiP là vùng kéo, nhấn trên thanh tua thành `HTCAPTION` → GPUI bắt đầu kéo nhưng Windows nuốt
  lần thả chuột. Sửa: khung thanh tua `occlude` (ưu tiên slider: kéo thanh là tua, không dời cửa sổ), thanh tua dừng lan
  MouseDown. Thanh tua, thanh âm lượng ở cửa sổ chính không dính (không nằm trong vùng kéo) — đã soát trong desktop ẩn.
- Cột phải: tên bài 28 px, thanh tua 16 px, hàng nút 40 px, khe 8 px, cửa sổ đệm 8 px bốn phía, bìa 100 px cách cột
  phải 8 px (chủ dự án chốt 03/11/2026); cửa sổ 420 × 116. Bìa bo 4 px; đĩa giữ chỗ lấp cả ô bìa để mép đĩa cách cột 8 px.

## drag-lag

- Kéo cửa sổ giật có ba nguồn: (1) gpui-pre 0.3.10 `refresh()` cả cây (bỏ cache `cached`) mỗi `WM_MOVE`; (2) `window_state` lưu vị
  trí bằng `update_settings` ⇒ `Shell` `notify` mỗi nhịp; (3) nền acrylic của `Blurred` trên Windows 10.
- (1) subclass `platform::move_loop` giữ `WM_MOVE` trong vòng dời, chuyển một lần lúc thả (đo desktop ẩn, bản debug, 120 nhịp
  16 ms: 52–57 % CPU → 0–3 %; vị trí vẫn lưu đúng). Không vá GPUI (crate ngoài). (2) `app::store_settings_quietly`.
- (3) build < 22621 đổi accent sang `ACCENT_ENABLE_BLURBEHIND` không tô màu (bản Tauri dùng `window_vibrancy::apply_blur`, tô
  `(18,18,18,107)`; ở đây cột trái tự tô alpha 0,88 nên bỏ màu tô). Mất hạt nhiễu acrylic. Desktop ẩn không có nền mờ: chủ dự
  án kiểm tay trên màn DELL (độ mượt khi kéo, độ mờ nền theme sáng / tối). Bản release tệ hơn dev có lẽ vì vẽ nhanh hơn nên
  trình ra nhiều khung hơn trong vòng dời — chưa đo trên màn thật.

## caption-ripple

- Ba nút cửa sổ có ripple tâm (`RippleConfig::centered()`), màu theo icon lúc rê: xám nhạt trên nền xám, trắng trên nền đỏ của
  Đóng. Vòng tròn đường kính bằng bề ngang nút như nút icon của materiant, không loang tới góc ô vuông. PiP không có nút cửa
  sổ riêng (nút quay lại là nút icon đã có ripple).
- [volume-compact] Thanh âm lượng dùng `Slider::compact()` (rãnh 2 px, núm 10 px như thanh EQ / bass) — `.rail(3.4).thumb(10.)` cho cùng dày với thanh tua — vì sao: chủ dự án lấy tỉ lệ thanh EQ làm mẫu; thanh tua và PiP là canvas không có núm nên không đổi.

## layout-style

Settings → Chung → "Kiểu chia vùng": thẻ bo tròn (mặc định) / chia lưới (`Settings::layout_style`, `views::layout`).

- [layout-style] Kiểu lưới chỉ đổi **vùng** (cột trái, thanh trên, ba cột Now Playing, đầu trang thư viện / danh sách bài, tab Settings): sát nhau, đường kẻ 1 px màu `border`. Thứ là **nội dung** (bìa, thẻ album, thẻ hành động, hộp dòng thời gian, bảng EQ, nút, hộp thoại) giữ bo góc ở cả hai kiểu — làm phẳng hết — app Claude desktop cũng giữ bo cho ô nhập, nút; bìa vuông góc nhìn như lỗi cắt.
- [layout-style] Cột trái giữ nền mờ (D-10) ở kiểu lưới — đổi về nền đặc như nội dung cho "liền một khối" — mờ là tính năng riêng, đường kẻ dọc đã tách vùng rõ.
- [layout-style] Đường dưới header cột trái nối liền đường dưới thanh trên (cùng 36 px) thành một vạch ngang suốt cửa sổ — chỉ kẻ dưới thanh trên — giống ảnh app Claude chủ dự án gửi.
- [layout-style] PiP không theo kiểu này — áp cả PiP — PiP là một khối nhỏ, không có vùng nào để chia.
- [layout-style] Đổi kiểu gọi `refresh_windows` (như đổi ngôn ngữ) thay vì cho từng view `cached` nghe cài đặt — `observe` → `notify` từng view — một chỗ, không sót view `cached` mới thêm sau; đổi cài đặt khác vẫn không dựng lại view `cached` (test `app::tests::layout_style_change_rebuilds_the_cached_page`).
