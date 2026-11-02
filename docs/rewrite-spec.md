# Groove X: đặc tả viết lại bằng Rust + GPUI + materiant

Soát từ mã ngày 02/11/2026, nhánh `master` commit `ed2fe82`. Đường dẫn dẫn chứng tính từ `groove/`; `lib.rs`, `db.rs`, `tags.rs`, `watcher.rs`, `paths.rs` nằm ở `src-tauri/src/`, `tauri.conf.json` ở `src-tauri/`. Đây là đọc mã, chưa chạy thử bản cũ. Số liệu thật lấy từ bản chép của `%APPDATA%\com.mrdxd.groovex\groovex.db` trên máy Windows: 1 002 bài, 8 album, 8 thư mục, 8 gốc theo dõi, 1 playlist rỗng, 5 bài có timeline, lyrics tổng 526 KB, bìa base64 tổng 1,13 MB (lớn nhất 171 KB), `user_version = 0`, `journal_mode = delete`.

Mã lỗi `N`, `S`, `M` ở mục 3. Mục 1 ghi hành vi bản cũ; chỗ bản mới phải khác thì trỏ tới mã lỗi.

## 1. Đặc tả hành vi cần giữ

### 1.1 Khung cửa sổ

- Cửa sổ không viền, nền trong suốt, mặc định 800 × 600, tiêu đề `Groove X` (`tauri.conf.json:14-22`). Hệ thống làm mờ desktop phía sau: Windows Aero Blur tint `(18,18,18,107)` (`lib.rs:1017`), macOS vật liệu `UnderWindowBackground`, bo 10 px, luôn active (`lib.rs:1039,1045,1106`), Linux không có. Chỉ cột điều hướng trong suốt (`--nav-bg`, `src/tailwind.css:5,98,134,254-260`); vùng nội dung và thanh phát đục (`src/layouts/main-layout.vue:4`, `src/tailwind.css:65`).
- Bố cục: cột trái; bên phải là thanh trên cao 36 px + vùng cuộn; thanh phát cố định đáy cao 90 px (`src/tailwind.css:21,64`, `src/layouts/main-layout.vue:2-14`). Đóng cửa sổ là thoát app; không có khay.
- Cột trái (`src/components/side-nav/side-nav.vue`): rộng 230 px, thu còn 56 px, lưu `side-nav-collapsed` (`:4,125,141-146`). Đầu cột cao 36 px, kéo được cửa sổ, tên app + nút thu. Ba mục cao 42 px: My Music, Now Playing, Playlists; mục active có vạch trái 3 px (`:29-35`). Khi thu thì chỉ icon + tooltip. Dưới cùng là nút Settings.
- Thanh trên (`src/components/app-bar/app-bar.vue`): nút lùi theo lịch sử, mờ khi không lùi được (`:64-66`); ô tìm kiếm giữa rộng 40 %, bấm mở bảng lệnh, hiện chip `Ctrl` `K`; nút thu nhỏ, phóng to / khôi phục, đóng (hover đỏ). Vùng trống kéo được cửa sổ.
- Điều hướng: `/my-music`, `/playing`, `/playlists`; mở app về trang cuối (`lastActiveRoute`, mặc định `my-music`) nhưng không nhớ tham số xem danh sách (`src/app.route.ts:15-18,39-41`). Tiêu đề cửa sổ theo trang, ở Now Playing là `<bài> - <nghệ sĩ> | GrooveX` (`src/pages/now-playing/now-playing.vue:431-441`).
- Toast: góc trên phải, 4 000 ms, có nút đóng (`src/shared/composables/use-toast.ts:12`, `src/App.vue:19-36`).
- Giao diện: chạy tối cố định (`index.html:2`), bộ màu sáng đã có token nhưng không có chỗ đổi (`src/tailwind.css:132-192`). Màu nhấn cyan: `#06b6d4`, hover `#0891b2`, sáng `#22d3ee`; nền `#121212`, thẻ `#18181b`, ô `#27272a` (`src/tailwind.css:64-92`). Bản mới dùng theme materiant, đổi được Light / Dark / System.

### 1.2 My Music và Playlists

Hai route chung một màn với `mode` (`src/pages/my-music/my-music.vue:2`, `src/pages/playlists/playlists.vue:2`). Trạng thái: `grid` hoặc `list` của một category (`src/components/my-music/my-music.vue:276-288`).

**Lưới My Music**

- Tiêu đề, hai thẻ hành động: "Not finding everything?" gọi import; "Albums look wrong?" gọi rescan, đang chạy thì đổi chữ (`:32-65`).
- Hai tab Albums, Folders, mặc định Albums, không lưu (`:69-72,295`).
- Ô lưới 2 đến 6 cột theo bề ngang (`:77`). Đang import có thêm ô chờ "Importing..." với vòng quay (`:92-103`). Rỗng có icon + lời hướng dẫn (`:106-112,147-151`).
- Thẻ (`src/components/music-card/music-card.vue`): bìa vuông bo góc, placeholder theo loại; hover phóng bìa 1,05, phủ nền + hai nút Play all và Delete; viền + quầng màu theo accent (`:157-160`). Dòng phụ: album là nghệ sĩ hoặc `Unknown Artist`; thư mục là đường dẫn; playlist là `<n> items` (`:133-142`). Delete mở hộp xác nhận (`:72-79`).
- Bấm thẻ vào danh sách; Play all phát bài đầu với cả danh sách làm hàng đợi (`src/components/my-music/my-music.vue:555-561`), danh sách rỗng thì toast.

**Lưới Playlists**: thẻ "Create a New Playlist" mở hộp tên + màu (mặc định `#06b6d4`), tên rỗng thì toast, trùng tên thì báo lỗi SQL thô (`:601-622`). Thẻ không có ảnh, nền bằng màu playlist.

**Danh sách bài** (`src/components/music-list/music-list.vue`)

- Đầu trang dính: mở rộng thì bìa 160 px, nhãn loại, tên, dòng phụ + `<n> items`, nút Play all; cuộn quá 120 px thu còn bìa 80 px, về lại khi dưới 15 px (`:384-391`). Có trễ hai ngưỡng để không rung. Quầng màu accent phía sau cao 380 px, độ đục 0,4 (`:8-11`).
- Bảng: `#`, Title, Duration, Artist (ẩn khi hẹp, `:107-110`). Thứ tự theo filename (`db.rs:377,405,536`). Tiêu đề rỗng thì dùng filename bỏ đuôi; nghệ sĩ rỗng hiện `—`.
- Cột `#`: bài khác hiện số, hover hiện Play; bài đang phát hiện bars chạy, hover hiện Pause / Play (`:129-166`).
- Bấm dòng: bài đang phát thì đảo phát / dừng; bài khác thì phát với toàn danh sách làm hàng đợi (`:542-556`).
- Chuột phải: Thêm vào danh sách phát (menu con: các playlist có chấm màu, vạch, Tạo danh sách mới), Edit timeline, Edit lyrics, Edit metadata; trong playlist thêm Xóa khỏi danh sách phát, chữ đỏ (`:206-278`). Tạo từ menu thì tạo xong thêm luôn bài (`:669-679`).
- Sự kiện đồng bộ thư viện thì nạp lại danh sách (`:521-523`) và lưới (`src/components/my-music/my-music.vue:355-363`).

### 1.3 Thanh phát (`src/components/audio-controller/audio-controller.vue`)

- Không có bài: toàn thanh mờ, không bấm được (`:4,422-432`); chữ `No song selected`, `Unknown Artist`.
- Trái: bìa vuông 90 px, hover phủ nút phát; bấm bìa hoặc chữ sang Now Playing. Nghệ sĩ, tiêu đề; tiêu đề tràn thì chạy marquee 12 s, dừng khi rê chuột (`:35,444-457`).
- Giữa: Shuffle, Prev, Play / Pause, Next, Repeat; shuffle và repeat bật thì xanh (`:55-85`). Nút phát biến hình bars sang tam giác trong 250 ms (`src/components/audio-controller/toggle-play.vue:66-68`).
- Thanh tua chia đoạn: mỗi đoạn timeline một khúc, tỉ lệ theo thời lượng, cách 2 px; không có timeline thì một khúc (`:240-256`). Cao 4 px, hover 6 px, đoạn đang rê 8 px. Rê chuột hiện nhãn `<tên đoạn> (<đầu> - <cuối>)` khi có timeline (`:273-280`). Nhấn + kéo là tua (`:300-327`). Thời gian hai bên; số trái nhấp nháy mỗi giây khi phát (`:458-472`). Định dạng `m:ss` hoặc `h:mm:ss`, giá trị rỗng là `0:00` (`src/shared/core/utils/time.ts`).
- Phải: nút mute, icon theo mức: 0 hoặc mute, 1-35, 36-65, trên 65 (`:384-395`). Thanh âm lượng 0-100, bánh xe ±1 (`:409-415`). Nút Settings hiện chưa nối gì (`:152-156`); bản mới nối vào hộp Settings.

### 1.4 Lịch sử, hàng đợi, shuffle, repeat (`src/shared/stores/audio-player.ts`)

Ba danh sách: `originalPlaylist` (nguồn), `historyPlaylist`, `queuePlaylist`; hiển thị = history + current + queue (`:52-61`). Mỗi bài xuất hiện một lần (`:476-504`).

| Thao tác | Kết quả | Dẫn chứng |
| --- | --- | --- |
| Phát kèm danh sách mới | original = danh sách; history rỗng; queue = phần sau bài (shuffle: mọi bài còn lại) | `:212-220` |
| Phát không kèm danh sách (bấm trong hàng đợi) | bài cũ vào history; bài được chọn ra khỏi history và queue; bài khác giữ nguyên chỗ | `:222-230` |
| Phát đúng bài đang phát | chỉ tiếp tục nếu đang dừng, bỏ qua danh sách mới (S2) | `:198-201` |
| Next, queue còn | lấy đầu queue, shuffle thì lấy ngẫu nhiên; bài cũ vào history | `:265-278` |
| Next, queue hết, repeat bật | history xoá, nạp lại từ đầu original | `:280-298` |
| Next, queue hết, repeat tắt | dừng, về 0 (N8) | `:299-304` |
| Prev | history rỗng thì tua về 0; có thì lấy cuối history, bài hiện tại vào đầu queue. Không có luật "quá 3 giây thì về đầu bài" | `:308-324` |
| Hết bài | repeat bật thì phát lại đúng bài này; tắt thì Next | `:580-587` |
| Bật shuffle | tắt repeat; queue = original trừ bài đang phát và bài đã phát | `:460-465` |
| Tắt shuffle | queue = phần sau bài hiện tại trong original | `:466-472` |
| Bật repeat | tắt shuffle | `:514-517` |

Như vậy repeat là **lặp một bài khi hết tự nhiên**, cộng với quay vòng khi người dùng bấm Next ở cuối. Bản mới giữ đúng nghĩa này ở mốc đầu tiên, đặt `RepeatMode { Off, One }`.

Khôi phục khi mở app: nạp lại bài cũ ở vị trí cũ, **không tự phát** (`:24,593-615`). Âm lượng mặc định 75, seek step 5 giây, volume step 2 (`:27,32,38`).

### 1.5 Now Playing (`src/pages/now-playing/now-playing.vue`)

Ba cột cao đầy, không cuộn trang:

- Trái, rộng 25 % trong khoảng 180-300 px: bìa bo góc, tên, nghệ sĩ; có timeline thì hộp đoạn: visualizer, `<hiện tại>/<tổng>`, từng dòng thời gian + tên; dòng đang phát tô màu và tự cuộn vào giữa (`:32-74,418-428`). Bấm dòng là tua tới đầu đoạn và phát (`:61-64`).
- Giữa: tiêu đề Lyrics, ba nút Metadata, Timeline, Edit / Add Lyrics chỉ hiện khi có bài (`:86-120`). Trạng thái: không có bài; có lyrics thì các dòng canh giữa, cuộn, dòng trống cao 16 px; không có lyrics. Sửa tại chỗ với Cancel, Save (`:155-184`). Đổi bài thì huỷ chế độ sửa và đóng dialog (`:338-343`).
- Phải: bảng hàng đợi rộng 32 % trong khoảng 240-340 px (`src/components/play-list/playlist-history.vue`, `src/tailwind.css:226-228`): tiêu đề + `(<n> tracks)`; dòng đã phát mờ 25 %; dòng hiện tại nền nhấn + bars, tự cuộn vào giữa khi đổi bài trừ khi vừa bấm trong bảng (`:128-134,172-185`).
- Quầng màu theo bìa cao 450 px, độ đục 0,3, chuyển 700 ms (`:5-6`). Màu tính từ trung bình ảnh, độ sáng lớn hơn 120 thì kéo về 120 theo công thức `(r*299+g*587+b*114)/1000` (`:400-406`).

### 1.6 Timeline

Định dạng mỗi dòng `<thời gian> <tên>`; thời gian là giây, `m:ss` hoặc `h:mm:ss` (`src/components/song/timeline-dialog.vue:7-18`). Regex `^([\d:]+)\s+(.+)$`, dòng khác bỏ qua, sắp xếp theo đầu đoạn; cuối đoạn là đầu đoạn sau, đoạn cuối hết ở thời lượng (`src/pages/now-playing/now-playing.vue:253-285`). Dialog 500 px, hướng dẫn, ô mono, Cancel, Save. Lưu cả tệp và DB (`lib.rs:448-504`); xem N2.

### 1.7 Lyrics

Dialog 500 px, nạp từ thẻ tệp, 12 dòng (`src/components/song/lyrics-dialog.vue`). Lưu cả tệp và DB (`lib.rs:385-445`).

### 1.8 Metadata (`src/components/song/metadata-dialog.vue`)

Dialog 900 px, đọc **từ tệp**, không từ DB (`lib.rs:536-540`).

- Trái: bìa 200 px, nhấp chọn tệp, kéo thả, hoặc dán khi đang rê chuột trên ô (`:18-45,515-531`); nút Remove Cover. Ngoài ra tiêu đề có cảnh báo số khung rác.
- Tab Common: Filename (đuôi hiện chip riêng; gõ đuôi quen thuộc tự cắt, `:427-457`), Title, Artist, Album, Album Artist, Track # / Total, Disc # / Total. Album có nhưng Album Artist trống thì hiện cảnh báo + nút Fix grouping: gán Album Artist = Album, bật compilation, bật áp cho cả nhóm (`:365-381`).
- Tab Details: Year, Genre, Composer, Publisher, Copyright, BPM, Grouping, Comment, Compilation.
- Tab Raw Frames: mọi khung trong tệp, chỉ MP3 (`tags.rs:378-408`), khung lạ tô vàng là junk.
- Chế độ ghi: `clean_keep` (mặc định), `clean`, `merge`, có câu giải thích (`:295-311`). Hàng `Select` này hiện **không hiển thị được** ở bản cũ (S24).
- Ô "áp cho tất cả bài trong album / thư mục", chỉ hiện khi mở từ category có trên một bài: Album, Album Artist, Genre, Year, Compilation, Cover; bài khác ghi `clean_keep` hoặc `merge`, không bao giờ `clean` (`:243-251,550-559,574-590`).
- Quy ước trường: `None` là không đụng, `Some("")` là xoá (`tags.rs:70-75`). Form gửi mọi trường (`:546-548`).
- Luật thẻ: MP3 luôn ID3v2.3; bỏ `ID3v1`, `APE` khi clean; `TYER` ưu tiên, nhận `TDRC`; `TCMP` là compilation (`tags.rs:6-18,312-322,514-520,532-540`). Giữ nguyên toàn bộ, và giữ các test (`tags.rs:744-992`), đổi tệp mẫu để chạy được ở mọi máy (M6).

### 1.9 Bảng lệnh (`src/components/commands/keyboard-palette.vue`)

Dialog 600 px, phía trên màn hình. Mở từ `Ctrl+K` hoặc ô tìm. Ô nhập, chip chế độ (`Search` / `CLI Mode`), danh sách nhóm theo loại, chân trang gợi ý phím.

- Trống: ba thao tác nhanh: phát ngẫu nhiên (hiện chạy được), đi tới Thư viện, đi tới Đang phát (hiện không làm gì, S21) (`:296-302`, `src/components/commands/command.const.ts:68-90`).
- Có chữ: tìm debounce 250 ms, tối đa 50 bài trên tiêu đề, nghệ sĩ, filename, album, không dấu tiếng Việt, ưu tiên khớp tiêu đề (`:150-163`, `db.rs:430-443`). Dòng: `Bài hát • <nghệ sĩ> • <album>`.
- Bắt đầu bằng `/`: lọc lệnh; Tab điền lệnh, Enter chạy chữ đang gõ (`:250-285`). Lệnh: `/play [tên]`, `/playall`, `/queue`, `/pause`, `/next`, `/prev`, `/volume <0-100>`, `/theme`, `/help`. `/queue`, `/theme`, `/help` hiện chỉ `alert` (`:369-372`). Bản mới làm thật: `/queue` thêm vào cuối hàng đợi, `/theme` đổi theme, `/help` liệt kê lệnh.
- Chọn bài **không đóng** bảng, có chú thích cố ý (`:303,311`); chạy lệnh thì đóng (`:375`). ↑ ↓ quay vòng; rê chuột dời con trỏ. Esc đóng. Bài đang phát hiện bars.

### 1.10 Settings (`src/components/side-nav/side-nav.vue:70-108`)

Dialog 800 × 480, cột trái hai mục: Cài đặt chung, Phím tắt. Thay đổi EQ áp ngay; Cancel khôi phục giá trị lúc mở; OK lưu phím tắt, JSON sai thì nhảy về tab đó và không đóng (`:156-186`).

**Cài đặt chung** (`src/components/side-nav/setting-general.vue`):

- Seek step (≥ 1), Volume step (1-100).
- EQ 10 dải: 31, 63, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz; -10 đến +10 dB, nấc 0,5; dải đầu lowshelf, dải cuối highshelf, giữa peaking `Q = 1.414` (`src/shared/core/audio-engine.ts:2,57-74`). Giá trị trên, thanh dọc, nhãn dưới; đường cong spline nối các điểm vẽ sau thanh trượt (`:90-111,199-231`).
- Bass boost 0-10 dB, nấc 1, cộng thêm vào 4 dải đầu với hệ số `[1, 0.8, 0.5, 0.2]`, kẹp ±10 (`src/shared/stores/audio-player.ts:72-78`). Đường cong không tính phần này.
- Preset: Custom (toàn 0), Flat, Bass Strong, Bass Moderate, Pop, Rock, Electronic, Vocal, Treble; bảng số ở `:161-171`. Kéo dải bất kỳ thì thành Custom; kéo bass boost thì thành Custom trừ khi preset đang chứa `Bass` (`:178-186`).

**Phím tắt** (`src/components/side-nav/setting-keyboard-binding.vue`): ô JSON kiểu VS Code, kiểm tra mảng và đủ `key`, `command`; Đặt lại mặc định; tự đóng ngoặc, gợi ý tên lệnh sau `"command":`.

Mặc định (`src/shared/composables/global-event-listener-top.ts:11-23`):

| Phím | Lệnh | Phím | Lệnh |
| --- | --- | --- | --- |
| `ctrl+k` | `open_search` | `arrowup` | `volume_up` |
| `space` | `toggle_play` | `arrowdown` | `volume_down` |
| `arrowleft` | `seek_backward` | `alt+p` | `play_random` |
| `arrowright` | `seek_forward` | `g m` | `go_to_library` |
| `alt+arrowleft` | `prev_track` | `g p` | `go_to_now_playing` |
| `alt+arrowright` | `next_track` | | |

Luật khớp: tiền tố `ctrl`, `alt`, `shift` theo thứ tự đó, `meta` tính là `ctrl`; chuỗi tối đa 2 phím, quên sau 800 ms; khớp chuỗi trước, khớp phím đơn sau; đang gõ trong ô nhập thì bỏ qua hết (`:92-107,175-219`). `play_random`: bật shuffle, phát ngẫu nhiên trong hàng đợi hiện tại (`:160-165`).

### 1.11 Import, rescan, watcher

- **Import** (`lib.rs:194-293`): chọn thư mục, quét đệ quy `mp3`, `m4a`, `flac`, `wav`; xoá dòng dưới thư mục đó không còn trên đĩa; đọc thẻ mọi tệp và upsert; ghi gốc theo dõi (gốc cha nuốt gốc con, `db.rs:567-581`); dọn album và thư mục rỗng. Toast báo số thêm / xoá (`src/components/my-music/my-music.vue:417-425`). Thư mục trong DB là **thư mục cha trực tiếp của tệp**, không phải gốc đã chọn (`lib.rs:91-97`).
- **Bìa nhóm**: lấy từ bài đầu tiên có bìa (`lib.rs:165-188`).
- **Khoá nhóm album**: tên album; nghệ sĩ album ưu tiên `album_artist`, rồi `artist`, chỉ điền khi đang trống (`lib.rs:131-148`, `db.rs:261-266`). Xem S1.
- **Rescan** (`lib.rs:625-669`): đọc lại thẻ mọi bài, cập nhật tiêu đề, nghệ sĩ, album; đếm tệp thiếu nhưng không xoá. Toast số đã đọc và số thiếu.
- **Watcher** (`watcher.rs`): debounce 2 giây, chỉ tệp nhạc, bỏ qua tệp app vừa ghi trong 5 giây, đẩy `{added, updated, removed}` cho UI, toast tóm tắt (`:27-29,62-74,238-240`).
- **Reconcile khi mở app** (`watcher.rs:145-214`): gắn watcher trước, rồi so đĩa với DB: tệp mới, tệp mất, mtime lệch. Dòng chưa có mtime thì lấy mtime hiện tại làm mốc, không đọc lại, để lần nâng cấp đầu không báo hàng trăm bài cập nhật (`:170-178`). Tệp ngoài mọi gốc thì không đụng (`:187-196`). Gốc không tồn tại thì bỏ qua (`:119-121`).
- So sánh đường dẫn theo đoạn, không phân biệt hoa thường trên Windows và macOS, phân biệt trên Linux (`paths.rs`). Port nguyên kèm test.

### 1.12 Mở tệp, một phiên, nút taskbar

- Liên kết định dạng: `mp3`, `flac`, `wav`, `m4a`, `m4r`, `ogg` (`tauri.conf.json:37-74`).
- Mở lần đầu: `argv[1]` là tệp có đuôi trên thì phát sau 800 ms (`lib.rs:1204-1221`). Đã chạy: phiên mới chuyển `argv` + `cwd` cho phiên đang chạy, focus cửa sổ, phát (`lib.rs:1150-1158`, `src/shared/stores/audio-player.ts:627-642`). Tệp ngoài thư viện vẫn phát được, không ghi vào DB (`lib.rs:933-1009`); xem N6.
- Taskbar Windows: Prev, Play / Pause, Next; tooltip `Previous`, `Play`, `Pause`, `Next`; icon ở `src-tauri/icons/taskbar/`; bật khi có bài, nút giữa đổi theo trạng thái (`tauri.conf.json:77-99`, `src/shared/stores/audio-player.ts:410-437`).

### 1.13 Khoá localStorage

| Khoá | Giá trị | Mặc định | Bản mới |
| --- | --- | --- | --- |
| `currentSong` | JSON `Song` kèm cả `thumbnail` base64 | null | `session.json` chỉ giữ tham chiếu bài |
| `currentTime` | số giây, ghi mỗi ≥ 1 giây và khi dừng / tua | 0 | `session.json` |
| `volume`, `isMuted` | 0-100, `true` / `false` | 75, false | `settings.json` |
| `isShuffle`, `isRepeat` | `true` / `false` | false | `session.json` |
| `seekStep`, `volumeStep` | số | 5, 2 | `settings.json` |
| `eqGains`, `bassBoost`, `currentPresetName` | mảng 10 số, số, chuỗi | toàn 0, 0, `Flat` | `settings.json` |
| `originalPlaylist`, `historyPlaylist`, `queuePlaylist` | mảng `Song` đầy đủ trừ `thumbnail`, gồm cả lyrics | rỗng | `session.json`, chỉ id |
| `playlist`, `currentIndex` | bản ghép cũ, chỉ để tương thích | rỗng, -1 | bỏ |
| `lastActiveRoute` | `/my-music`… | | `settings.json` |
| `custom-keybindings` | JSON `[{key, command}]` | mặc định ở 1.10 | `settings.json` |
| `side-nav-collapsed` | `true` / `false` | false | `settings.json` |

Dẫn chứng: `src/shared/core/utils/storage.ts:1-19`, `src/shared/stores/audio-player.ts:23-54,444-455,527`, `src/shared/composables/global-event-listener-top.ts:41`, `src/components/side-nav/side-nav.vue:125`. Vị trí cửa sổ do `tauri-plugin-window-state` giữ (`lib.rs:1239`), không ở localStorage.

## 2. Mô hình dữ liệu và lệnh

### 2.1 Schema hiện tại (`db.rs:124-228`)

Không có index ngoài khoá, không dùng `user_version`. Migration là kiểm tra cột rồi `ALTER` (`:164-197`).

```
folders(id PK AI, name NN, path UNIQUE NN, thumbnail, accent_color)
albums(id PK AI, name UNIQUE NN, artist, thumbnail, accent_color)
songs(id PK AI, title, artist, album_id→albums SET NULL, folder_id NN→folders CASCADE,
      file_path UNIQUE NN, filename NN, duration NN, lyrics, timeline, file_mtime)
watched_roots(id PK AI, path UNIQUE NN)
playlists(id PK AI, name UNIQUE NN, color NN)
playlist_songs(playlist_id→playlists CASCADE, song_id→songs CASCADE, PK(playlist_id, song_id))
```

`normalize_str` là hàm SQL đăng ký theo kết nối (`db.rs:110-121`), chỉ dùng lúc truy vấn; schema không phụ thuộc nó, nên tool khác mở được DB.

### 2.2 Schema đề xuất (`user_version = 1`)

```
folders:  + cover_hash TEXT, - thumbnail, - accent_color (chuyển sang covers)
albums:   + group_key TEXT NN, + cover_hash, + cover_is_custom INT DEFAULT 0
          UNIQUE(name, group_key) thay UNIQUE(name)
songs:    + album_artist, + track_no INT, + disc_no INT, + cover_hash,
          + file_size INT, + search_key TEXT NN DEFAULT '', + tags_read_at INT
playlist_songs: + position INT NN, + added_at INT NN
covers(hash PK, accent_color NN, width INT, height INT)
INDEX songs(folder_id), songs(album_id), playlist_songs(song_id), songs(cover_hash)
PRAGMA journal_mode=WAL, synchronous=NORMAL, foreign_keys=ON, busy_timeout=5000
```

Bản 2 (`groove-gd1_09`, N1, N9): `songs + missing_since INT` (ms, chỉ mục một phần), `excluded_paths(path_key PK, path NN)`. Nâng từ bản 1 trong một transaction khi mở; DB mới đi qua đúng các bước đó.

`group_key` = `album_artist` đã chuẩn hoá khi có; không có thì `folder:<folder_id>`. Cách này vừa tách hai "Greatest Hits" của hai nghệ sĩ khác thư mục (S1), vừa không xé album nhiều nghệ sĩ cùng thư mục mà không có `TPE2`, đúng tình huống mà `lib.rs:128-130` đã né.

Bìa nằm ở tệp `<cache>/covers/<hash>_<256|512>.jpg`, không nằm trong DB (S10). Khớp với `VirtualGrid`, vốn nạp ảnh bằng `img(path).image_cache(&LruImageCache)` (`.claude/worktrees/materiant-gd5_04/src/virtual_grid.rs:24-26`).

### 2.3 Tương thích

**DB cũ: đọc được, cần chuyển đổi, không phá bản gốc.**

1. Tìm `<data_dir>/com.mrdxd.groovex/groovex.db` (`lib.rs:1163-1164`, identifier ở `tauri.conf.json:5`): Windows `%APPDATA%`, macOS `~/Library/Application Support`, Linux `~/.local/share`. Đã xác nhận đường dẫn Windows. Chạy thử từ app Claude thì AppData bị ảo hoá MSIX, nên đừng kết luận "không có DB" khi chạy trong đó.
2. Chép sang `<data_dir>/Groove X/library.db`, migrate bản chép trong một transaction. Không sửa tệp cũ, nên chạy lại bản Tauri vẫn được đến khi bỏ hẳn.
3. Giữ nguyên `id` của `songs`, `playlists`, `albums`, `folders`.
4. Giải base64 ở `folders.thumbnail`, `albums.thumbnail` ra tệp bìa; `accent_color` tính lại, vì 8 / 8 album trong DB thật đều là hằng `#06b6d4` (`lib.rs:175,185`), không phải màu từ bìa.
5. Dòng `albums` cũ: `group_key` tạm là `folder:<folder_id nhỏ nhất>`. Tác vụ nền (`tags_read_at IS NULL`) đọc lại thẻ từng bài để điền `album_artist`, `track_no`, `disc_no`, `cover_hash`, rồi gán lại album, có tiến độ, không chặn UI; 1 002 bài chạy một lần. `search_key` tính ngay từ cột sẵn có trong migration.
6. Gặp `user_version` lớn hơn bản đang hỗ trợ thì từ chối mở và báo, không ghi đè.

**localStorage: không đọc trực tiếp được đơn giản.** Windows là LevelDB của WebView2 (`%LOCALAPPDATA%\com.mrdxd.groovex\EBWebView\Default\Local Storage\leveldb`, 1,6 MB, đã thấy chuỗi khoá trong `.ldb`), macOS và Linux là kho của WebKit, định dạng khác. Hàng đợi và vị trí không đáng giữ vì dựa trên snapshot cũ. Thứ tiếc nếu mất chỉ là `custom-keybindings`, `eqGains`, `bassBoost`, `currentPresetName`, `volume`. Đề xuất: bản mới chạy với mặc định, cài lại tay vài giây. Task 28 là tuỳ chọn, chỉ Windows, đọc bản chép, lỗi thì im lặng về mặc định; định dạng khoá LevelDB còn phải xác minh trong task đó. Vị trí cửa sổ không chuyển.

**Thẻ tệp nhạc**: tương thích hai chiều, `TXXX:TIMELINE` giữ nguyên tên.

### 2.4 Lệnh và sự kiện

Tầng Tauri (`invoke`, `#[tauri::command]`, `emit`, JSON, `asset://`, `convertFileSrc`, plugin) bỏ hết: UI gọi hàm Rust trực tiếp. Tầng giữ lại về nghiệp vụ: `paths.rs` nguyên bản; `tags.rs` sửa N2, N3, S17; `db.rs` đổi schema; `watcher.rs` đổi luật N1, N4.

| Command cũ (`lib.rs:1241-1263`) | Bản mới | Ghi chú |
| --- | --- | --- |
| `get_music_categories` | `Library::categories()` | không kèm ảnh |
| `import_music_folder` | `LibraryCtl::import(path)` | chọn thư mục ở UI, chạy nền, có tiến độ |
| `get_category_songs`, `get_playlist_songs` | `Library::songs(CategoryRef)` | không kèm lyrics |
| `search_songs` | `Library::search(query, limit)` | trên `search_key` |
| `update_music_thumbnail`, `update_category_accent_color` | `LibraryCtl::set_category_cover(CategoryRef, path)` | nối vào menu thẻ, bản cũ không gọi được (S13) |
| `update_song_lyrics`, `update_song_timeline` | `MetadataOps::save_lyrics`, `save_timeline` | S5, N2 |
| `get_song_metadata` | `MetadataOps::read(TrackRef)` | đọc nền |
| `update_song_metadata` | `MetadataOps::save(TrackRef, MetadataEdit, WriteMode)` | N3 |
| `apply_metadata_to_songs`, `get_category_song_ids` | `MetadataOps::save_bulk(CategoryRef, …)` | có tiến độ |
| `rescan_songs` | `LibraryCtl::rescan()` | nền; đọc đủ trường |
| `delete_category` | `LibraryCtl::remove_category(CategoryRef)` | N9 |
| `get_playlists`, `create_playlist`, `delete_playlist`, `add_song_to_playlist`, `remove_song_from_playlist` | `Library::playlists()`, `LibraryCtl::…` | giữ thứ tự theo `position` |
| `get_song_by_path` | `TrackRef::from_path(path)` | N6 |

| Sự kiện cũ | Bản mới |
| --- | --- |
| `library-synced {added, updated, removed}` (`watcher.rs:239`) | `LibraryEvent::Synced(SyncReport)`, thêm `Moved`, `Progress` |
| `open-file {args, cwd}` (`lib.rs:1154,1214`) | `AppEvent::OpenPaths(Vec<PathBuf>)` |
| `media-prev`, `media-toggle`, `media-next` (`tauri.conf.json:80-84`) | `Command::{PrevTrack, TogglePlay, NextTrack}` qua một dispatcher chung với phím tắt và media session |

Kiểu chính: `TrackRef { Library(i64), External(PathBuf) }`; `CategoryRef { Album(i64), Folder(i64), Playlist(i64) }`; `WriteMode { CleanKeepKnown, Clean, Merge }`. `settings.json`, `session.json` khoá camelCase, ghi bằng tệp tạm + đổi tên.

## 3. Lỗi và điểm chưa tối ưu của nghiệp vụ

### Nghiêm trọng

**N1. Đọc thư mục lỗi bị coi là tệp đã xoá.**
Bằng chứng: `scan_directory` nuốt lỗi `read_dir` (`lib.rs:46`). `reconcile` lấy mọi dòng dưới gốc không có trong kết quả quét làm thay đổi (`watcher.rs:189-196`), `sync_paths` xoá khi `!path.exists()` (`watcher.rs:272-276`), FK cascade xoá khỏi playlist (`db.rs:224`). Chốt chặn duy nhất là gốc phải tồn tại (`watcher.rs:119`).
Hậu quả: gốc là điểm gắn Linux chưa mount (thư mục rỗng vẫn tồn tại), thư mục con mất quyền, ổ mạng chập chờn, hoặc thư mục con đổi tên: mở app là mất cả dải bài và mất luôn thành viên playlist, không phục hồi được. Thư viện thật của chủ dự án nằm trong OneDrive (`tags.rs:681-682`), nên càng đáng lo.
Làm đúng: quét trả `ScanOutcome { files, failed_dirs }`, dùng `walkdir` (Unlicense / MIT), không xoá dòng dưới `failed_dirs`. Không xoá dòng ngay mà đánh `missing_since`, sau 30 ngày hoặc khi người dùng xác nhận mới xoá. Một gốc quét ra 0 tệp trong khi DB còn dòng dưới gốc đó thì coi là gốc offline, không đụng gì.

**N2. Lưu metadata xoá timeline khỏi tệp; import lại xoá nốt trong DB.**
Bằng chứng: chế độ mặc định tạo `id3::Tag::new()` và chỉ ghi các khung liệt kê (`tags.rs:435-438,29-41`), `TXXX` bị coi là rác (`tags.rs:225,234-255`), nhánh `lofty` cũng bỏ mọi thẻ (`tags.rs:576-579`). Ghi xong không ghi lại `TIMELINE`. `sync_song_row` không đụng cột timeline nên DB tạm còn (`lib.rs:584`), nhưng `index_song_file` đọc từ tệp rồi `timeline = excluded.timeline` (`lib.rs:108-126`, `db.rs:300`). Ngoài ra `FullMetadata` chú thích timeline "stored in the database rather than in the file" (`lib.rs:511`), trái với `lib.rs:460`. Sửa bulk thì mất trên cả album.
Hậu quả: bấm Save metadata (hoặc Fix grouping cho cả album), rồi lần sau bấm import cùng thư mục hoặc sửa tệp bằng app khác: timeline nhập tay mất vĩnh viễn. Chế độ `clean` cũng xoá lyrics trên tệp vì form không có trường lyrics (`src/components/song/metadata-dialog.vue:334-355`), DB giữ bản cũ đến lần index sau.
Làm đúng: `tags` có danh sách bảo toàn `APP_FRAMES = ["TIMELINE"]` mà mọi chế độ ghi đều giữ, inspector không gắn nhãn rác cho nó; `clean` đọc lại lyrics từ DB. Khi index, tệp không có timeline mà DB có thì **giữ DB** (`COALESCE`), và đẩy ngược vào tệp lúc ghi kế tiếp. Có test khứ hồi cho mỗi định dạng; `lofty` với `ItemKey::Unknown` trên M4A phải được test, chưa xác minh ghi được.

**N3. Đổi tên và ghi thẻ không an toàn.**
Bằng chứng: `parent.join(trimmed_name)` rồi `std::fs::rename` (`lib.rs:709-710`), không kiểm tra đích tồn tại; `rename` thay thế tệp đích trên cả Windows lẫn Unix. Tên chứa `..\` hoặc đường dẫn tuyệt đối đi ra ngoài thư mục. Đổi tên xong `write_metadata` lỗi thì `?` thoát, DB còn đường dẫn cũ (`lib.rs:723`); `UPDATE file_path` trùng UNIQUE cũng sau khi tệp đã đổi (`lib.rs:584`). Tên rỗng cho ra `.mp3` (`src/components/song/metadata-dialog.vue:569`). Ghi thẻ gọi thẳng `write_to_path`, `save_to_path` trên tệp thật (`tags.rs:515,658`), không có bản tạm.
Hậu quả: gõ trùng tên một bài khác là **mất tệp nhạc đó**. Lỗi giữa chừng để DB trỏ tệp không còn, lần mở sau reconcile xoá + thêm dòng mới, mất playlist. Mất điện khi ghi có thể hỏng tệp nhạc.
Làm đúng: kiểm tra tên một đoạn, không ký tự cấm, không rỗng, đích chưa có (`create_new` hoặc kiểm tra + lỗi rõ). Thứ tự: chép ra `<tên>.groove-tmp` cùng thư mục, ghi thẻ trên bản tạm, đọc lại kiểm tra, `rename` thay tệp gốc, đổi tên nếu có, cập nhật DB trong transaction; lỗi ở đâu thì xoá tạm và tệp gốc nguyên vẹn. Đang phát đúng tệp đó: decoder mở sẵn handle cũ, phát tiếp được trên Unix; trên Windows phải mở tệp có `FILE_SHARE_DELETE` (mặc định của Rust `File::open`) và test thật. Watcher bỏ qua tệp `.groove-tmp`.

**N4. Đổi tên ngoài app, đổi tên thư mục làm mất dữ liệu hoặc bị lờ.**
Bằng chứng: `handle_changes` trải sự kiện thành danh sách đường dẫn, lọc theo đuôi (`watcher.rs:101-104,222`), đường dẫn cũ thành xoá, mới thành thêm với `id` mới (`watcher.rs:262-276`). Đổi tên thư mục chỉ có sự kiện thư mục, không có đuôi nhạc nên bị bỏ.
Hậu quả: đổi tên một tệp: bài rơi khỏi playlist, timeline chỉ còn nếu nằm trong thẻ. Đổi tên thư mục: bài cũ thành dòng chết đến lần mở sau, lần đó thì xoá + thêm, cùng hậu quả.
Làm đúng: dùng cặp đổi tên của `notify-debouncer-full`; trong mỗi lô, ghép dòng biến mất với tệp mới theo `(file_size, duration, tiêu đề)`, ghép được thì `UPDATE file_path` giữ `id`. Sự kiện thư mục dưới gốc: quét lại đúng cây đó, có ghép đổi tên như trên.

**N5. Tệp lỗi làm kẹt phát nhạc; hàng đợi là snapshot cũ.**
Bằng chứng: store không có listener `error` (`src/shared/stores/audio-player.ts:556-587`), `play()` lỗi chỉ `console.error` (`:142-146`). Chỉ `ended` mới sang bài. Tên mới cập nhật cho current, history, queue, **bỏ sót original** (`:335-343`); danh sách đang mở cũng không cập nhật `file_path` (`src/components/music-list/music-list.vue:617-623`). Xoá category, đổi đĩa, không đụng hàng đợi; `library-synced` store không nghe.
Hậu quả: tệp bị xoá hoặc đổi tên thì UI dừng im, không báo. Sửa tên bài xong bấm chính dòng đó trong danh sách không phát; bật repeat vòng lại cũng gặp đường dẫn chết. Hàng đợi khôi phục sau khi mở lại có thể đầy bài chết.
Làm đúng: hàng đợi chỉ giữ `TrackRef`, tra bài từ thư viện lúc phát. Engine phát `Error { generation, kind }`: toast, tự sang bài kế, tối đa 3 lỗi liên tiếp thì dừng. Nghe `LibraryEvent`: bài mất thì bỏ khỏi queue, đổi đường dẫn thì tự đúng vì tra theo `id`.

**N6. Tệp ngoài thư viện đều có `id = -1`.**
Bằng chứng: `lib.rs:998-1003`; `playSong` so `id` (`src/shared/stores/audio-player.ts:198`); Next thoát khi original rỗng (`:260`); `get_song_metadata(-1)` lỗi (`:179`, `lib.rs:532`).
Hậu quả: mở bằng "Open with" một tệp thứ hai khi tệp ngoài thứ nhất đang phát: **không có gì xảy ra**. Không hiện bìa. Hết bài UI kẹt ở trạng thái đang phát vì `ended` gọi Next rồi thoát sớm. Nút Metadata, Lyrics, Timeline vẫn hiện nhưng Save lỗi im lặng (`src/components/song/lyrics-dialog.vue:87-88`).
Làm đúng: `TrackRef::External(path)` so theo đường dẫn; hàng đợi nhận được; đọc bìa từ tệp; sửa thẻ ghi tệp, không có phần DB. Timeline của tệp ngoài chỉ ở thẻ. Mở nhiều tệp một lúc thì vào hàng đợi, `lib.rs:1206` chỉ lấy một.

**N7. Mọi IO chạy trên main thread và giữ `Mutex` suốt.**
Bằng chứng: mọi `#[tauri::command]` là `fn` đồng bộ, mà Tauri 2 chạy lệnh đồng bộ trên main thread. Hộp thoại `rfd` chặn (`lib.rs:199`). Import giữ khoá suốt quét và đọc thẻ (`lib.rs:218-287`). Lyrics, timeline, metadata, bulk giữ khoá khi ghi tệp (`lib.rs:390,453,680,773`). Rescan cả thư viện (`lib.rs:629-665`). Watcher giữ khoá khi đọc thẻ từng tệp (`watcher.rs:247,262`). Mỗi lần đổi bài đọc cả thẻ + base64 bìa trên main (`src/shared/stores/audio-player.ts:179`).
Hậu quả: import 1 002 bài đóng băng cửa sổ, ô "Importing..." khó mà vẽ được; hàng đợi không chạy; watcher đang chạy thì mọi lệnh DB chờ. Tiếng `<audio>` sống ở tiến trình WebView nên tiếng không đứt, nhưng bản mới mà giữ cấu trúc này thì GPUI đứng hình hoàn toàn.
Làm đúng: UI thread không chạm đĩa. Một `LibraryActor` thread ghi, nhận lệnh qua channel; một kết nối đọc riêng nhờ WAL. Đọc thẻ ở worker tối đa 4 luồng, **ngoài transaction**, gom 100 dòng một transaction. Chọn tệp bằng `prompt_for_paths` của GPUI (`gpui-pre/src/app.rs:2015`), không chặn.

**N8. Trạng thái phát sai ở cuối hàng đợi.**
Bằng chứng: nhánh cuối `nextTrack` đặt `isPlaying = false; audio.currentTime = 0`, **không `pause()`** (`src/shared/stores/audio-player.ts:299-304`).
Hậu quả: bấm Next ở bài cuối khi đang phát: tiếng chạy lại từ đầu, UI báo đã dừng. Space lúc đó gọi `play()`, UI mới đúng lại.
Làm đúng: trạng thái phát chỉ có một nguồn là engine; `QueueOutcome::Stop` phát lệnh `pause + seek(0)`. Viết `queue.rs` là hàm thuần, có test cho mọi dòng ở bảng 1.4.

**N9. Xoá album hoặc thư mục không có tác dụng lâu dài.**
Bằng chứng: chỉ xoá dòng (`lib.rs:844-851`), `watched_roots` không có lệnh xoá nào, reconcile thấy tệp không có trong DB thì thêm lại (`watcher.rs:177`). `get_music_categories` không báo gì.
Hậu quả: người dùng xác nhận "Xóa nhé", lần mở sau mọi thứ quay lại, kèm toast "N bài mới". Không có cách nào bỏ một thư mục khỏi thư viện. Playlist đã mất các bài đó (cascade).
Làm đúng: xoá thư mục tương ứng bỏ gốc hoặc thêm `excluded_paths(path)` khi nó nằm dưới một gốc. Xoá album: nêu rõ chỉ ẩn, ghi vào `excluded_paths` theo từng tệp hoặc đổi nhãn nút thành "Bỏ khỏi thư viện", không thao tác tệp. Settings thêm danh sách gốc (kiểu `WatchedRoot` đã khai sẵn ở `src/shared/types/audio.types.ts:82-86` mà chưa dùng).

### Nên sửa

**S1. Album gộp chỉ theo tên.** `albums.name UNIQUE`, `get_or_create_album` chỉ tra tên (`db.rs:138,251`), dù `tags.rs:6-11` nói mọi player nhóm theo `(album, album_artist)`. Hậu quả: hai "Greatest Hits" của hai nghệ sĩ gộp một, bìa + nghệ sĩ của bài vào trước. Làm đúng: `group_key` ở mục 2.2.

**S2. "Phát" im lặng khi bài đầu đang phát.** `:198-201` về sớm trước khi đổi danh sách. Hậu quả: đang nghe bài A thuộc thư mục X, bấm Play all ở album có A đứng đầu: không đổi hàng đợi, không báo gì. Làm đúng: có danh sách mới thì luôn thay hàng đợi, chỉ giữ nguyên tiếng khi cùng bài.

**S3. Chọn sai category khi trùng tên.** `handlePlay` tìm theo `type` + `title` (`src/components/my-music/my-music.vue:541`); tên thư mục không unique (`db.rs:127-128`). Hậu quả: hai thư mục cùng tên `Album` ở hai đường dẫn: Play all ở thẻ thứ hai phát thẻ thứ nhất. Làm đúng: truyền `CategoryRef`.

**S4. Shuffle làm mất phần còn lại của hàng đợi.** Bật shuffle dựng lại từ original (`src/shared/stores/audio-player.ts:463-464`), tắt thì bỏ các bài trước vị trí hiện tại chưa nghe (`:468-471`). Shuffle bốc ngẫu nhiên từng bước (`:267`), hàng đợi hiển thị không phải thứ tự sẽ phát, nên Prev rồi Next không quay về đúng bài. Làm đúng: shuffle xáo cố định phần queue một lần (Fisher-Yates, không thêm crate), hiển thị đúng thứ tự thật; tắt shuffle thì khôi phục theo original, gồm các bài chưa phát ở cả hai phía. Đây là đổi hành vi có chủ đích.

**S5. Lưu lyrics, timeline sai luật và nuốt lỗi.** Ghi `Id3v24` (`lib.rs:411,471`), trái luật v2.3 (`tags.rs:12-14`). Lỗi ghi chỉ `eprintln`, DB vẫn đổi, UI báo xong (`lib.rs:411-413,430-432`). Không `note_self_write` (so `lib.rs:721-722`), nên 2 giây sau watcher đọc lại, hiện toast "1 bài cập nhật". Lỗi lưu ở dialog chỉ `console.error` (`src/components/song/lyrics-dialog.vue:87-88`). Làm đúng: gộp vào đường ghi an toàn của N3, trả lỗi, toast.

**S6. Chống vòng lặp watcher mong manh.** Ân hạn 5 giây (`watcher.rs:27`) tính từ lúc đánh dấu, trong khi `sync_paths` có thể chờ khoá dài hơn khi bulk đang ghi; `take_self_write` gỡ ở lô đầu (`:70-74`) nên lô thứ hai sau `strip_legacy_containers` vẫn bị đọc lại. Hậu quả: bulk nhiều bài vẫn index lại từng bài, toast thừa. Làm đúng: so `(mtime, size)` sau khi ghi với giá trị đã lưu trong DB; bằng nhau thì bỏ qua. Không cần cửa sổ thời gian.

**S7. Rescan, metadata cập nhật thiếu trường.** `sync_song_row` không ghi `lyrics`, `duration`, không gieo bìa cho album mới tạo (`lib.rs:583-593`). Hậu quả: nút "Albums look wrong?" có thể tạo album không bìa; sửa lyrics ngoài app rồi rescan vẫn thấy bản cũ. Làm đúng: một hàm `apply_scanned_track` cho cả import, watcher, rescan, sửa thẻ.

**S8. Lưu metadata một bài đè bìa cả nhóm.** `update_folder_thumbnail`, `update_album_thumbnail` chạy vô điều kiện khi bài có bìa (`lib.rs:730-735`). Hậu quả: sửa năm của một bài đổi bìa cả album, thư mục. Làm đúng: chỉ đổi bìa nhóm khi người dùng chọn áp cho nhóm hoặc nhóm chưa có bìa; `cover_is_custom` thì không bao giờ tự đổi.

**S9. Không có transaction.** Import xoá, thêm, dọn đều autocommit (`lib.rs:262,269,279-280`); một lỗi giữa vòng `?` thoát để lại nửa chừng. Bulk và watcher tương tự. Chỉ baseline mtime có transaction (`watcher.rs:202`). `journal_mode = delete` trên DB thật. Hậu quả: mỗi lần `INSERT` một fsync, import chậm; tắt giữa chừng lệch trạng thái; `.ok()` giấu lỗi. Làm đúng: mục N7.

**S10. Bìa base64 trong DB và IPC.** `thumbnail` là `data:` URI (`lib.rs:347`, `tags.rs:291-295`), nhân đôi cho album và thư mục, trả về trọn trong mỗi lần nạp lưới (`db.rs:324,350`). Số thật: 1,13 MB chuỗi mỗi lần refresh, bản lớn nhất 171 KB. **Với thư viện hiện tại chưa đau**; đau khi nhiều album hoặc bìa gốc vài MB, vì không resize. Bản Rust không có IPC nhưng giữ cách này thì GPUI giữ ảnh giải mã suốt đời (`.claude/worktrees/materiant-gd5_04/src/virtual_grid.rs:24`). Làm đúng: mục 2.2; ảnh gốc chỉ đọc từ thẻ khi mở Now Playing, Metadata. Đặt `image::Limits` chống ảnh bom.

**S11. Base64 trong URL route.** `openCategoryDetails` nhét bìa vào query (`src/components/my-music/my-music.vue:465`). Hậu quả: URL, `history.state` phình; bìa rất lớn có thể làm điều hướng hỏng. Không chuyển sang bản mới: `Route::Category(CategoryRef)`.

**S12. localStorage là bản sao thừa, đồng bộ, không bắt lỗi.** Bốn watcher sâu ghi lại toàn bộ danh sách kèm lyrics của từng bài (`src/shared/stores/audio-player.ts:132-134,444-447`); `currentSong` giữ cả base64 (`:527`, `cleanPlaylist` không áp ở đây); `enforceUniqueness` stringify mỗi lần đổi (`:488,501`); `setItem` không `try` (`src/shared/core/utils/storage.ts:24-30`). Ước tính khi hàng đợi chứa cả thư viện hiện tại: riêng lyrics đã khoảng 1,6 MB chuỗi mỗi lần đổi bài (ba bản sao). Vượt quota thì mất khôi phục mà không báo. Làm đúng: `session.json` chỉ id, ghi nền có debounce, vị trí mỗi 5 giây + khi dừng + khi thoát.

**S13. Tính năng bìa tuỳ chỉnh và accent chết.** `MusicCard` khai `settings` nhưng không nơi nào phát (`src/components/music-card/music-card.vue:97`); `handleSettings` là lối gọi duy nhất tới `update_music_thumbnail` và `update_category_accent_color`. Hậu quả: thẻ luôn viền cyan hoặc màu mặc định (`src/components/my-music/my-music.vue:328,338`); màu chỉ tính lại ở trang chi tiết và Now Playing bằng canvas mỗi lần vào. Làm đúng: tách `accent` một lần khi tạo bìa, lưu `covers`; thêm "Đổi bìa…" vào menu thẻ. Canvas 1 × 1 trên main thread giờ thành `image` thu về 32 × 32 ở nền.

**S14. Đọc thẻ lặp 3-4 lần mỗi tệp.** `read_metadata`, `Probe` thời lượng, `Probe` timeline, `id3` timeline (`lib.rs:101-126`). Không `prepare_cached`, mỗi tệp thêm truy vấn bìa (`lib.rs:167-183`). Đây là N+1 thật của app: ở đường ghi; đường đọc không có N+1, `fetch_*` đều một truy vấn. Làm đúng: `read_track(path) -> ScannedTrack` một lần mở, gom mọi thứ.

**S15. Thiếu index.** `songs(folder_id)`, `songs(album_id)`, `playlist_songs(song_id)`. Xoá một bài phải quét `playlist_songs` vì PK bắt đầu bằng `playlist_id` (`db.rs:225`); `DELETE ... NOT IN` chạy sau mỗi thao tác. Làm đúng: mục 2.2.

**S16. Không ảo hoá.** Lưới, bảng bài, hàng đợi, lyrics dựng toàn bộ (`src/components/music-list/music-list.vue:114`, `src/components/play-list/playlist-history.vue:22`, `src/components/my-music/my-music.vue:78`). `setActiveRowRef` còn `console.log` mỗi dòng mỗi lần vẽ (`src/components/play-list/playlist-history.vue:141`). Danh sách bài kèm cả lyrics (`db.rs:376`). Làm đúng: `VirtualList`, `VirtualGrid`, model dòng không có lyrics và timeline; chỉ nạp khi là bài đang phát.

**S17. Thẻ nhiều giá trị bị dính.** `clean` xoá `\0` (`tags.rs:144`), mà ID3v2.4 dùng `\0` ngăn nhiều nghệ sĩ. Hậu quả: `A\0B` thành `AB`, `clean_keep` ghi vĩnh viễn vào tệp. Cần test trên tệp thật để chắc `id3` trả chuỗi thô. Làm đúng: thay `\0` bằng `; ` khi đọc; v2.3 ghi `; `.

**S18. Tìm kiếm quét toàn bảng + bốn lần gọi hàm Rust mỗi dòng.** `db.rs:437-441`; trả cả lyrics; kết quả về muộn đè kết quả mới (`src/components/commands/keyboard-palette.vue:150-160`, không có mã lượt). `normalize_string` loại mọi ký tự không chữ số, `"-"` thành `%%` khớp tất cả. Làm đúng: `search_key` đánh sẵn (không dấu + hạ chữ + giữ khoảng trắng), `instr` trên cột này, 1 002 dòng dưới 1 ms; thứ tự theo mã lượt.

**S19. Đánh số không theo track.** Sắp xếp theo filename (`db.rs:377,405`), track number không lưu cột. Thứ tự playlist cũng theo filename, không sắp được (`db.rs:536`). Lưới không `ORDER BY` (`db.rs:324,350`). Làm đúng: album theo `(disc_no, track_no, filename)`, playlist theo `position`, lưới theo tên không dấu.

**S20. Thanh tua lệch khi có timeline.** Vẽ theo `flexGrow` từng đoạn (`src/components/audio-controller/audio-controller.vue:114`), tính vị trí theo cả bề ngang (`:324-325`). Timeline không bắt đầu từ 0 thì phần trước đoạn đầu không được vẽ. Hậu quả: nơi bấm khác nơi vẽ. Làm đúng: thêm đoạn ẩn danh từ 0; hàm thuần `segments_to_bar` và `x_to_time` dùng chung; có test. Tua mỗi `mousemove` đặt `currentTime` + ghi localStorage (`:326`, `src/shared/stores/audio-player.ts:252-254`): bản mới chỉ tua khi thả.

**S21. Tính năng hứa mà không làm.** Quick action 2, 3 hiện phím nhưng không chạy (`src/components/commands/keyboard-palette.vue:296`); `/queue`, `/theme`, `/help` là `alert`; Enter ở `/n` chạy chữ gõ thay vì lệnh đang chọn (`:276`); "Tìm" chọn bài thay hẳn hàng đợi bằng một bài (`:309`), đổi thành chèn bài đó làm tiếp theo và phát; `dialogShortcuts` là `console.log` (`src/shared/composables/global-event-listener-top.ts:60-71`), bỏ.

**S22. Mở tệp theo liên kết chưa đủ.** macOS gửi tệp qua Apple Event, bản cũ chỉ đọc `argv`, không xử lý `RunEvent::Opened`, nên nhấp đúp trên Mac không phát (`lib.rs:1204`). 800 ms cứng có thể lỡ listener (`lib.rs:1213`). Lọc đuôi phân biệt hoa thường: `BÀI.MP3` bị bỏ (`src/shared/stores/audio-player.ts:632-637`). `set_focus` không khôi phục cửa sổ đang thu nhỏ (`lib.rs:1152`). Đuôi lệch nhau: liên kết có `ogg`, `m4r`, quét không có (`watcher.rs:31`, `lib.rs:53`). Làm đúng: mục 5.

**S23. Âm thanh EQ có thể rè.** Tăng tới +10 dB không có preamp (`src/shared/core/audio-engine.ts:36-46`), bass boost cộng thêm. Âm lượng tuyến tính (`:85`). Làm đúng: preamp tự động `-max(gain dương)` có nội suy, hoặc limiter nhẹ; âm lượng theo đường cong `v²`. Đổi cảm giác âm lượng, nên ghi chú chuyển đổi.

**S24. Component chưa đăng ký.** `main.ts:11-28` không đăng ký `VSelect`, `VTextarea`, `VDivider`, cũng không có plugin tự import (`vite.config.ts:24`). Hậu quả: hàng Write mode, ô Comment, vạch ngăn **không hiển thị**, người dùng không đổi được chế độ ghi; icon `DefaultCover` không có trong `sourceMap` (`src/components/song/metadata-dialog.vue:28`, `src/components/svg-sprite/svg-sprite.vue:12-46`). Bản mới làm đúng ý thiết kế.

**S25. An toàn.** `assetProtocol` scope `**` + `csp: null` (`tauri.conf.json:25-29`), feature `devtools` trong bản phát hành (`src-tauri/Cargo.toml:21`): một lỗi XSS đọc được mọi tệp của người dùng. Hiện không có `v-html`, nên chưa có đường khai thác đã biết. Tất cả biến mất khi bỏ WebView. Rủi ro còn lại ở bản Rust là **tệp độc**: thẻ và luồng âm thanh từ tệp bất kỳ. Yêu cầu: decoder chạy ở luồng riêng; workspace groove **không** đặt `panic = "abort"` như profile của materiant để `catch_unwind` được panic ở decoder và đọc thẻ, lỗi thành `Error`, không sập app (đo cỡ exe khi đổi); giới hạn kích thước ảnh, độ dài thẻ khi đọc. Ghi tệp chỉ nhận đường dẫn lấy từ DB hoặc người dùng chọn.

**S26. Quét có thể tràn stack.** `scan_directory` đệ quy theo symlink, junction (`lib.rs:49-50`). Vòng lặp là tràn stack, sập app. Làm đúng: `walkdir`, không theo symlink mặc định. Tệp OneDrive chưa tải: đọc thẻ sẽ kéo cả tệp; bản mới nên lấy `file_size`, mtime trước, không đọc thẻ khi không đổi (đã đúng ở reconcile), cần kiểm thử trên máy thật ở lần import đầu.

### Nhỏ

- **M1. `CLEAR_DB_ON_START`** (`lib.rs:19,1166-1171`): hằng `false`, nhưng đổi nhầm rồi phát hành là xoá thư viện người dùng khi mở. Bỏ. Cần DB trống khi thử thì dùng biến môi trường `GROOVE_DATA_DIR` chỉ ở bản debug.
- **M2.** Trùng dòng do khác hoa thường trên Windows: `song_exists`, `UNIQUE` so chính xác (`db.rs:655`, `watcher.rs:257`), trong khi reconcile so chuẩn hoá. Lưu thêm cột `path_key` chuẩn hoá làm UNIQUE.
- **M3.** Settings đóng bằng bấm ra ngoài: giữ thay đổi EQ, bỏ phím tắt nháp (`src/components/side-nav/side-nav.vue:70`, chỉ OK / Cancel xử lý). Bản mới `close_on_backdrop(false)`.
- **M4.** Phím toàn cục vẫn chạy khi dialog mở nếu focus không ở ô nhập (`src/shared/composables/global-event-listener-top.ts:175-183`); `Ctrl+K` không chạy khi đang ở ô nhập. Dispatcher theo ngữ cảnh GPUI giải quyết.
- **M5.** Lệnh không tồn tại trong JSON phím tắt vẫn qua kiểm tra (`src/components/side-nav/setting-keyboard-binding.vue:252-258`). Kiểm tra theo danh sách, báo phím trùng.
- **M6.** Test thẻ dựa vào tệp nhạc trên máy chủ dự án, không có thì `return` âm thầm thành xanh (`tags.rs:679-700,751-760`). Tự sinh tệp MP3, FLAC, M4A ngắn làm fixture trong repo.
- **M7.** Preset `Custom` đặt toàn 0 thay vì giữ giá trị người dùng (`src/components/side-nav/setting-general.vue:162,173-176`). Lưu `customGains` riêng.
- **M8.** Dòng log `[boot]`, `console.log` rải khắp; bản mới dùng log có mức như fastcast.

## 4. Bảng ánh xạ UI

`Toast` đặt `ToastViewport` ở view gốc. Mọi chữ qua `t!`, nhãn viết hoa chữ cái đầu (D-13).

| Bản cũ | materiant / GPUI | Ghi chú |
| --- | --- | --- |
| `v-btn`, `custom-btn` | `button::Button` (`.filled()`, `.outlined()`, `.text()`, `.loading()`) | |
| `icon-btn` | `button::IconButton` nhận `IconSource` (`materiant/src/button.rs:572`) | icon của app: `impl From<GrooveIcon> for IconSource` (`materiant/src/icon.rs:15`) |
| `svg-sprite` + `public/icons/*.svg` | `icon::Icon`, `svg()` | tách sprite thành từng tệp, 29 icon |
| `v-dialog` + `v-card`, `confirmer` | `overlay::dialog::{Dialog, DialogHandle}` | thiếu độ rộng, xem dưới |
| `v-text-field` | `field::TextField`, `NumberField` cho seek / volume step | chip đuôi tệp: một `Badge` đặt cạnh ô, `TextField` không có suffix |
| `textarea`, `contenteditable` (lyrics, timeline, phím tắt) | `field::TextArea` | Tab không chèn khoảng trắng; bỏ tự đóng ngoặc |
| `v-select` | `select::Select` | |
| `v-slider` (âm lượng, bass, EQ dọc) | `slider::Slider`, `.vertical()`, `.step(0.5)` | `on_change` áp EQ ngay; đang kéo bỏ qua `.value()` |
| `v-progress-circular`, spinner | `progress::ProgressCircular`, thêm `ProgressBar` cho tiến độ import | |
| `v-tabs`, tab Settings | `tabs::Tabs`, `.vertical()` | nội dung tab do app vẽ |
| `v-menu` chuột phải, menu con | `menu::{ContextMenuExt, Menu, MenuItem}`, `.submenu()`, `.danger(true)` | |
| `use-toast` | `toast::Toast::push` | toast có `ToastKind`, bản cũ một kiểu |
| `x-kbd` | `kbd::Kbd` | |
| `switch.vue` (không dùng ở đâu) | bỏ; có sẵn `switch::Switch` | |
| `input[type=checkbox]` | `checkbox::Checkbox` | |
| `input[type=color]` | `color_picker::ColorPicker` trong `overlay::popover::Popover` | |
| Thuộc tính `title` | `overlay::tooltip`, `.with_tooltip(id, text)` | |
| `overlay-scrollbars` | `scroll::ScrollArea` | |
| Bảng bài, hàng đợi | `list::VirtualList`, dòng cùng cao | cuộn tới giữa: `handle.scroll_to(ix, Align::…)` |
| Lưới thẻ | `virtual_grid::VirtualGrid`, `.height_per_width(1.)`, `LruImageCache` | đang ở worktree gd5_04 |
| `data-tauri-drag-region` | `window::DragRegionExt::drag_region()` | |
| Kéo thả, dán bìa | `on_drop::<ExternalPaths>` (`gpui-pre/src/interactive.rs:715`), `read_from_clipboard` (`gpui-pre/src/app.rs:1823`) | |
| `vue-router` | `enum Route`, ngăn xếp lịch sử trong app | |
| `pinia` | `Entity<Player>`, `Entity<LibraryStore>`, `cx.notify()` | |

### Component còn thiếu thật sự trong materiant

Chỉ hai chỗ, đều nhỏ, gộp thành task `materiant-gd5_05`:

1. **`Dialog` không chỉnh được bề rộng**: `MAX_WIDTH = 512 px` cứng (`materiant/src/overlay/dialog.rs:54,281`), không có setter (`:169-206`). Metadata cần 900, Settings 800, Palette 600. Thêm `Dialog::max_width(Pixels)`. Palette còn cần neo trên cao: `.align_top()` hoặc để app vẽ bằng `Popover` + lớp phủ.
2. **`MenuItem::icon`, `Button::icon` chỉ nhận `IconName`** (`materiant/src/menu.rs:238`, `materiant/src/button.rs:518`) trong khi `IconButton` nhận `IconSource`; chấm màu playlist không có chỗ đặt. Đổi sang `impl Into<IconSource>`, và thêm `IconSource` dạng chấm màu, hoặc app sinh icon SVG tròn theo màu.

Không có component thiếu nào khác: thứ còn lại là ghép trong app, không phải việc của materiant.

### Dựng trong app (`crates/groove-app/src/views/`)

| View | Dựng từ | Ghi chú |
| --- | --- | --- |
| `SeekBar` | `canvas` + `window.on_mouse_event` | `Slider` một rail không vẽ được nhiều đoạn; hành vi preview khi kéo, commit khi thả như `Slider` |
| `CategoryCard`, `CoverImage` | `img(path).image_cache(&cache)`, placeholder gradient | `linear_gradient` có sẵn (`gpui-pre/src/color.rs:865`) |
| `SongRow`, `QueueRow` | `div` theo cột | ẩn cột Artist theo bề ngang đo được |
| `SideNav` | `Button` + vạch 3 px | `Tabs::nav` không có chế độ chỉ icon |
| `CommandPalette` | `TextField` + `VirtualList` + `Kbd` | theo `materiant/examples/launcher.rs`; đầu nhóm là một dòng cùng cao |
| `EqCurve` | `canvas` + `PathBuilder` | nét dày tĩnh, vẽ lại khi đổi giá trị |
| `PlayingBars`, `Marquee`, `TogglePlayIcon` | xem dưới | |

### Hiệu ứng web không có ở GPUI

Lý do chung: `div` không có `scale`, `transform`, `backdrop-filter`, transition khai báo (`materiant/src/toast.rs:23-24`, `overlay/dialog.rs:17`).

| Web | Ở đâu | Thay bằng |
| --- | --- | --- |
| `backdrop-blur` từng phần tử | đầu danh sách, Now Playing, hover thẻ, Palette, toast (`music-list.vue:15`, `now-playing.vue:79`, `playlist-history.vue:3`, `music-card.vue:32`, `keyboard-palette.vue:5`, `App.vue:24`) | Nền alpha 0,85 (D-10) cho khối nổi; đầu danh sách dính dùng nền đặc pha accent. Chỉ giữ mờ toàn cửa sổ cho cột trái: `translucent_window_options()`, `WindowBackgroundAppearance::Blurred` |
| Marquee CSS | tiêu đề thanh phát | Phần tử cắt `overflow_hidden` + lệch `left` theo `with_animation` lặp 12 s, chuỗi nhân đôi; chỉ chạy khi tràn và không rê; đo bề ngang chữ để biết có tràn không |
| `transition` bố cục | thu đầu danh sách, thu cột trái | Đầu danh sách dùng `motion::transition` trên số px (cao, cỡ bìa) hoặc đổi ngay; cột trái nội suy bề rộng 300 ms. OS bật giảm chuyển động thì nhảy |
| `scale` khi hover | thẻ, nút | Bỏ scale; đổi nền, viền. Ảnh không có `scale` trong `div` |
| `box-shadow` màu theo accent | hover thẻ | `BoxShadow` của GPUI nhận màu, hoặc chỉ viền 2 px |
| `clip-path` morph | nút Play | Vẽ hai đa giác nội suy bằng `canvas` + `with_animation` 250 ms, không cần thì đổi icon tức thì |
| Bars keyframe | `playing-visualizer.vue` | `canvas` với 4 thanh; chỉ xin khung khi đang phát và lọt vùng hiển thị, như `materiant/src/progress.rs:17-18` |
| Quầng mờ gradient | Now Playing, danh sách | `linear_gradient` màu accent sang trong suốt, chuyển màu nội suy 700 ms bằng `motion` |
| `snap`, cuộn `smooth` | hộp đoạn timeline | Cuộn tức thì tới giữa; thêm nội suy sau nếu cần |
| Nhấp nháy số giờ | thanh phát | Bỏ: tốn khung hình liên tục, trái "RAM là tính năng". Đổi có chủ ý |
| Chữ không đục trong cửa sổ Blurred | toàn app | GPUI tắt ClearType khi nền không đục (`materiant/src/window.rs:44`), chữ cả app thành xám. Phải xem thật trên Windows, chấp nhận hoặc thêm tuỳ chọn tắt mờ |

Thanh phát cập nhật tối đa 4 lần mỗi giây và chỉ khi đang phát, để lúc nghỉ không tốn khung hình.

## 5. Phần nền tảng phải thay

Giấy phép dưới đây lấy từ `cargo info` ngày 02/11/2026 (riêng `rubato`, `windows-sys`, `image`, `walkdir`, `notify` 8 theo trường license của crate đã biết, chạy `cargo deny check licenses` ở task 01 để chốt). Mọi crate này cho phép theo D-2.

| Việc | Crate | Giấy phép |
| --- | --- | --- |
| Giải mã | `symphonia` 0.6 (features `mp3, aac, alac, isomp4, flac, wav, pcm, ogg, vorbis`) | MPL-2.0, dùng nguyên bản |
| Xuất âm | `cpal` 0.18 + `ringbuf` 0.6 | Apache-2.0; MIT OR Apache-2.0 |
| Đổi tần số mẫu | `rubato` | MIT |
| EQ | tự viết biquad RBJ, khoảng 60 dòng | — |
| Thẻ | `lofty`, `id3` giữ như cũ, nâng phiên bản có test | MIT OR Apache-2.0; MIT |
| Ảnh, màu | `image` 0.25 (đã có trong cây qua GPUI) | MIT OR Apache-2.0 |
| DB | `rusqlite` bundled | MIT |
| Theo dõi tệp | `notify` 8, `notify-debouncer-full` bản stable tương ứng; không dùng bản `-rc` | CC0-1.0; MIT OR Apache-2.0 |
| Quét | `walkdir` | chọn nhánh MIT của `Unlicense/MIT` |
| Một phiên | `interprocess` 2.4 (fastcast đang dùng) | 0BSD OR Apache-2.0 |
| Media session | `souvlaki` 0.9 | MIT |
| Taskbar | `windows-sys` (fastcast đang dùng) | MIT OR Apache-2.0 |

Không dùng: `rodio` (MIT OR Apache-2.0, dùng được, nhưng lớp `Sink` không có mã lượt, tua chính xác, xoá ring tức thì; viết `cpal` + `symphonia` trực tiếp ít mã hơn so với luồn qua nó), `rfd` (GPUI đã có `prompt_for_paths`), `color-thief` (tự tính 40 dòng).

### 5.1 Phát nhạc

Thiết kế (`crates/groove-audio`):

```
UI thread ─ AudioCmd ─▶ decoder thread ─ ringbuf f32 ─▶ cpal callback (thực thời, không khoá)
UI thread ◀─ AudioEvent ─ decoder thread                position: AtomicU64 frame đã phát
AudioCmd:   Load{generation, path, start_sec, autoplay}, Play, Pause, Seek{generation, sec},
            SetVolume(f32), SetMuted(bool), SetEq(EqParams)
AudioEvent: Loaded{generation, duration_sec}, Ended{generation}, Error{generation, kind}, DeviceChanged
```

- **Chống race đổi bài**: mọi lệnh và sự kiện có `generation`; sự kiện khác `generation` hiện tại bị bỏ. Thay cho `AbortError`, `targetSeekTime`, `isSeeking`, `isInitialLoad` của bản cũ (`src/shared/stores/audio-player.ts:81-85,143,158-169`). Next nhanh liên tục chỉ giải mã bài cuối.
- **Tua**: dừng đọc, xoá ring, `format.seek` chính xác, tăng số thứ tự frame, nối bằng fade 5 ms để khỏi lách tách. Vị trí hiển thị lấy từ `position` trừ độ trễ ring.
- **Hết bài**: `Ended` khi cả decoder xong và ring rỗng, không phải khi đọc xong; không thì mất đuôi bài.
- **DSP**: áp ở luồng decoder trước khi vào ring, không ở callback, nên đổi EQ trễ bằng độ dài ring; giữ ring khoảng 200 ms. Hệ số biquad tính theo tần số mẫu, trạng thái theo kênh. Công thức cho khớp Web Audio (lowshelf, highshelf bỏ qua Q, peaking dùng Q). Âm lượng và mute áp ở callback bằng atomic để phản hồi ngay, nội suy tránh click như `setTargetAtTime(…, 0.01)` (`src/shared/core/audio-engine.ts:89`).
- **RAM**: không mở stream khi chưa phát; dừng 30 giây thì đóng stream, giữ vị trí.

Rủi ro theo OS:

| OS | Rủi ro | Xử lý |
| --- | --- | --- |
| Windows | WASAPI shared ép tần số thiết bị | `rubato` đổi về `default_output_config`; rút tai nghe: stream lỗi ⇒ dựng lại trên thiết bị mặc định, giữ vị trí |
| macOS | CoreAudio đổi thiết bị khi đang phát | Như trên; test trên Intel và Apple Silicon |
| Linux | `cpal` đi qua ALSA ⇒ `alsa-sys` link động `libasound.so` (**LGPL-2.1**) | **Phải hỏi chủ dự án** (luật LGPL ở `CLAUDE.md`, D-2; D-9 không thay luật này). `cargo deny` không thấy thư viện hệ thống này. Link động, không kèm vào bản cài. Trên Ubuntu chạy qua lớp ALSA của PipeWire; cần test |
| Mọi OS | Symphonia chưa giải HE-AAC, không có Opus | Tệp `m4a` HE-AAC vào đường `Error` (N5). Cần soát tỉ lệ tệp này trong thư viện thật; `<audio>` của WebView2 phát được, nên có thể là chỗ hụt so với bản cũ |

### 5.2 Tách màu ảnh bìa

Giải mã `image` với `Limits`, thu về 32 × 32 `Triangle`, tính trung bình, kẹp sáng ≤ 120 như 1.5, lưu `covers.accent_color`, chạy ở worker. Không rủi ro theo OS. Giữ công thức trung bình để cảm giác như cũ; nếu thấy màu đục thì đổi sang histogram.

### 5.3 Taskbar (chỉ Windows)

`ITaskbarList3::ThumbBarAddButtons` qua `windows-sys`, HWND lấy từ `Window` (`impl HasWindowHandle`, `gpui-pre/src/window.rs:7616`). **Rủi ro chính**: GPUI giữ WndProc, nên phải `SetWindowSubclass` để bắt `WM_COMMAND` + `THBN_CLICKED` và message `TaskbarButtonCreated`; phải gửi về UI thread. Icon `.ico` từ `src-tauri/icons/taskbar/`, nhúng và nạp `HICON` theo DPI. macOS, Linux: không làm, như bản cũ.

### 5.4 Media session (tính năng mới, chưa có ở `src/`)

`souvlaki`: Windows SMTC cần HWND, macOS `MPNowPlayingInfoCenter` cần vòng lặp main thread (GPUI có), Linux MPRIS qua D-Bus. Dùng backend `zbus` thuần Rust trên Linux, không dùng `libdbus`; phải kiểm tên feature và giấy phép cây phụ thuộc ở task. Ảnh bìa truyền đường dẫn tệp bìa cache, phù hợp 2.2. Phím media vào dispatcher chung. Rủi ro: SMTC cần cửa sổ đã tạo; macOS test cả khi cửa sổ thu nhỏ.

### 5.5 Một phiên

`interprocess` local socket, theo mẫu `fastcast/crates/core`: tên theo người dùng, DACL trên Windows. Phiên sau gửi một dòng JSON `{"paths": […], "cwd": "…"}` rồi thoát. Đường dẫn tương đối ghép với `cwd`. Windows: phiên sau gọi `AllowSetForegroundWindow` trước khi gửi, phiên chính `ShowWindow(SW_RESTORE)` rồi focus (S22). Linux: socket chết sau khi crash phải dọn; Wayland không cho tự focus nếu không có token kích hoạt, chấp nhận chỉ nhấp nháy taskbar.

### 5.6 Liên kết định dạng

Đuôi: `mp3, flac, wav, m4a, m4r, ogg`, và quét thư viện dùng cùng danh sách trong một hằng (S22).

| OS | Khai báo | Nhận tệp |
| --- | --- | --- |
| Windows | Bộ cài NSIS ghi ProgID, `OpenWithProgids`; Windows không cho tự đặt mặc định, người dùng tự chọn | `argv` |
| macOS | `CFBundleDocumentTypes` trong `Info.plist` | `App::on_open_urls` (`gpui-pre/src/app.rs:325`, `gpui-pre-macos/src/platform.rs:136`), đăng ký **trước** `run`, có thể tới trước khi có cửa sổ, nên xếp vào hàng chờ. Chưa xác minh GPUI có chuyển `application:openFiles:`, phải test |
| Linux | `.desktop` `MimeType=…`, `Exec=groove %F`, cập nhật mime | `argv` |

Mở app lần đầu: nhận hết rồi mới áp sau khi player sẵn sàng, không hẹn 800 ms; ưu tiên hơn khôi phục session.

### 5.7 Vị trí cửa sổ

Lưu `window.window_bounds()` (`gpui-pre/src/window.rs:2536`) vào `settings.json` khi đóng (`on_window_should_close`, `:6914`) và debounce khi đổi; kèm id màn. Mở: màn không còn hoặc ngoài vùng ⇒ về giữa màn chính. Wayland không cho đặt vị trí: chỉ giữ cỡ. Khi thử máy chủ dự án, chỉ mở ở màn DELL theo cách của `materiant/tools/gallery-run.ps1`.

Nền mờ: `plan/_backlog.md` mục groove đã ghi việc soát trên macOS. Kéo cửa sổ acrylic trên Windows 10 có thể trễ một nhịp (`materiant/src/window.rs:42`), máy chủ dự án đang là Windows 10, bản cũ đã chọn Aero Blur vì cùng lý do (`lib.rs:1066`). Phải xem thật; không ổn thì nền đục làm mặc định.

### 5.8 Đọc ghi thẻ

Giữ `id3` cho MP3 (đang đạt mục tiêu v2.3), `lofty` cho còn lại; đổi `thumbnail: Option<String>` sang `CoverEdit { Keep, Remove, Set(Vec<u8>) }`, bỏ base64 và `decode_data_uri` dò chuỗi mime (`tags.rs:188-199`). Rủi ro: `lofty` 0.21 → 0.25 đổi API, `ItemKey::Unknown` trên `ilst` M4A chưa rõ; WAV, M4R, OGG ghi thẻ phải test từng loại. Windows: tệp đang mở bởi app khác làm `rename` thất bại, phải báo lỗi rõ, không nuốt. Thay cả tệp làm đổi `file id`, watcher thấy là thay đổi, xem S6.

## 6. Đề xuất chia task

Tệp task đặt `plan/groove-gd1_<NN>_<slug>.md`. Repo `groove`, trừ dòng ghi rõ. Mã mới ở `crates/`, cùng cấp `src-tauri/` (không nằm trong workspace mới), đến task 29 mới xoá Tauri + Vue. Mỗi task ≤ 900 dòng mã mới, ≤ 7 tệp.

Luật song song: task 01 khai sẵn **mọi** phụ thuộc trong `Cargo.toml` của từng crate và mọi `mod` với tệp rỗng, các task sau không sửa `Cargo.*`, `lib.rs`, `main.rs`; tệp rỗng không tính vào trần. i18n chia theo vùng `i18n/{vi,en}/<area>.toml`, mỗi task sở hữu một vùng, có test hai ngôn ngữ cùng bộ khoá. `views/mod.rs` khai sẵn; task 03 dựng slot trống cho từng view.

Mọi task nghiệp vụ có test đơn vị; phân loại theo D-14. `[sonnet]` nhận API đã khoá từ task phụ thuộc.

| # | Slug | Loại | Dòng | Tệp sở hữu | Phụ thuộc |
| --- | --- | --- | --- | --- | --- |
| 00 | `materiant-gd5_05_dialog-width-menu-icon` (repo materiant) | `[sonnet]` | 120 | `overlay/dialog.rs`, `menu.rs`, `button.rs`, `icon.rs`, trang gallery | gd5_04 |
| 01 | `workspace` | `[opus]` | 350 | `Cargo.toml`, `deny.toml`, `crates/*/Cargo.toml`, `core/{paths,i18n}.rs`, `app/main.rs`, `i18n/*/common.toml` | — |
| 02 | `settings-session` | `[opus]` | 400 | `core/{settings,session,model}.rs` | 01 |
| 03 | `app-shell` | `[sonnet]` | 700 | `app/{app,route,icons}.rs`, `views/{side_nav,app_bar}.rs`, `assets/icons/*`, `i18n/*/shell.toml` | 00, 01, 02 |
| 04 | `db-migration` | `[opus]` | 650 | `library/db/{schema,migrate}.rs`, test fixture DB cũ | 01 |
| 05 | `library-store` | `[opus]` | 750 | `library/{actor,store}.rs`, `library/db/queries.rs` | 04 |
| 06 | `tags` | `[opus]` | 850 | `library/tags.rs`, `library/fixtures.rs` | 01 |
| 07 | `covers` | `[opus]` | 350 | `library/covers.rs` | 04 |
| 08 | `indexer-import` | `[opus]` | 650 | `library/{scanner,indexer}.rs` | 05, 06, 07 |
| 09 | `watcher-reconcile` | `[opus]` | 700 | `library/{watcher,reconcile}.rs` (N1, N4, S6) | 08 |
| 10 | `audio-engine` | `[opus]` | 850 | `audio/{engine,decoder,output}.rs` | 01 |
| 11 | `audio-dsp` | `[opus]` | 300 | `audio/dsp.rs` | 01 |
| 12 | `queue-timeline` | `[opus]` | 550 | `core/{queue,timeline}.rs` (bảng 1.4, S4, S20) | 02 |
| 13 | `player-controller` | `[opus]` | 650 | `app/player.rs`, `app/session_sync.rs` (N5, N6, N8) | 05, 10, 11, 12 |
| 14 | `player-bar` | `[sonnet]` | 800 | `views/{player_bar,seek_bar,marquee,playing_bars}.rs`, `i18n/*/player.toml` | 03, 13 |
| 15 | `library-grid` | `[sonnet]` | 650 | `views/{library_page,category_card,cover_image}.rs`, `i18n/*/library.toml` | 03, 05, gd5_04 |
| 16 | `song-list` | `[sonnet]` | 700 | `views/{song_list,song_row}.rs`, `i18n/*/song_list.toml` | 03, 13 |
| 17 | `library-actions` | `[opus]` | 750 | `app/library_ctl.rs`, `views/{song_menu,playlist_dialog,card_menu}.rs`, `i18n/*/actions.toml` (N9, S2, S3) | 08, 15, 16 |
| 18 | `metadata-ops` | `[opus]` | 600 | `library/metadata_ops.rs` (N2, N3, S5, S7, S8) | 05, 06 |
| 19 | `metadata-dialog` | `[sonnet]` | 850 | `views/metadata/{dialog,fields,cover_pane,raw_frames}.rs`, `i18n/*/metadata.toml` | 00, 18, 13 |
| 20 | `lyrics-timeline-dialogs` | `[sonnet]` | 300 | `views/{lyrics_dialog,timeline_dialog}.rs`, `i18n/*/song_dialogs.toml` | 18, 13 |
| 21 | `now-playing` | `[sonnet]` | 800 | `views/{now_playing,timeline_box,lyrics_panel,queue_panel}.rs`, `i18n/*/now_playing.toml` | 14, 20 |
| 22 | `commands-keymap` | `[opus]` | 650 | `core/{keymap,cli}.rs`, `app/commands.rs` (M4, M5, S21) | 02, 13 |
| 23 | `command-palette` | `[sonnet]` | 550 | `views/command_palette.rs`, `i18n/*/palette.toml` | 00, 22, 05 |
| 24 | `settings-dialog` | `[sonnet]` | 850 | `views/settings/{dialog,general,eq_panel,eq_curve,keybindings,roots}.rs`, `i18n/*/settings.toml` | 00, 11, 17, 22 |
| 25 | `single-instance-open` | `[opus]` | 400 | `app/instance.rs`, `app/open_paths.rs` (N6, S22) | 13 |
| 26 | `media-session-taskbar` | `[opus]` | 550 | `app/platform/{media_session,taskbar_windows}.rs`, `i18n/*/platform.toml` | 22 |
| 27 | `window-state-packaging` | `[opus]` | 500 | `app/window_state.rs`, `packaging/*`; ở repo gốc: `build.ps1`, `build.sh`, `init.*`, `start.*` (nhánh groove) | 25 |
| 28 | `localstorage-import` (tuỳ chọn) | `[opus]` | 200 | `library/legacy_ls.rs` | 02 |
| 29 | `acceptance-remove-tauri` | `[opus]` | 100 | Bảng so bản cũ, số đo RAM, gỡ `src/`, `src-tauri/`, Node; viết `groove/CLAUDE.md`, `groove/ROADMAP.md` | tất cả |

Tên viết tắt: `core` = `crates/groove-core/src`, `library` = `crates/groove-library/src`, `audio` = `crates/groove-audio/src`, `app`, `views` = `crates/groove-app/src`, `crates/groove-app/src/views`.

Tổng **30 task**: 20 `[opus]` (gồm task 28 tuỳ chọn), 10 `[sonnet]`. Lát dọc chạy được: sau 09 có thư viện chạy nền không UI; sau 13-14 nghe được nhạc; sau 15-17 đủ luồng chính.

Làn song song sau 01:

```
Làn nền:   04 → 05 → 08 → 09      06 → 18      07 (vào 08)
Làn tiếng: 10, 11 song song; 12 → 13 (cần 05, 10, 11)
Làn UI:    00 → 03 → 14, 15, 16 → 17 → 21, 19, 20, 23, 24
Làn OS:    25 → 27; 26
```

Quyết định chủ dự án cần chốt, ghi `docs/decisions.md` trước task 10: chấp nhận `libasound` LGPL link động trên Linux (5.1). Việc cần chạy thật mới biết, nghiệm thu ở task tương ứng: Symphonia với HE-AAC trong thư viện thật, chữ trong cửa sổ Blurred, kéo cửa sổ acrylic trên Windows 10, `lofty` ghi `TIMELINE` vào M4A, thẻ `id3` đa giá trị.
