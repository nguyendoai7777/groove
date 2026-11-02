# groove-gd1_25 — single-instance-open

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~400 dòng |
| Phụ thuộc | groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/instance.rs`, `app/open_paths.rs` (N6, S22) |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Mở tệp**: `play_paths(Vec<PathBuf>, cx)`: tệp đầu phát, tệp sau vào hàng đợi, tự đổi sang `TrackRef::Library` nếu có trong thư viện. Gọi trước khi phiên đọc xong thì tự chờ và thắng bài của phiên (đặc tả 5.6), không cần hẹn giờ. Lọc đuôi (S22) là việc của bên gọi, player không lọc. Trùng bài đang phát thì chỉ tiếp tục (N6 đã đúng theo đường dẫn).
- **Thứ tự**: `player::install` ở `app::run` sau `library_page::install`; lời gọi của task này đặt sau nó.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 25. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N6, S22 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`. Kiểm tay `[owner-qa]`: ba mục `groove-gd1_25` ở `plan/_backlog.md`.

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | Có phiên chính, phiên sau là tiến trình thật / gửi tệp tương đối / phiên sau thoát, tệp ghép với `cwd` của nó | `second_process_forwards_and_exits` |
| C2 | 8 bên đua cùng lúc / thì đúng một phiên chính, 7 yêu cầu tới đủ | `racing_processes_elect_one_primary_and_lose_nothing` |
| C3 | Rác, dòng quá 1 MiB, `.exe`, thiết bị Windows, NUL / thì bị bỏ, không focus, server vẫn phục vụ | `server_filters_input_and_survives_junk`, `oversized_and_junk_lines_are_rejected`, `resolve_refuses_devices_and_drive_relative_paths`, `nul_in_a_path_is_refused` |
| C4 | Pipe đã có chủ / giành lần hai thì `AddrInUse`; DACL đúng một SID; so thư mục exe của server | `second_claim_is_refused`, `descriptor_grants_only_the_current_user`, `same_directory_ignores_case_and_file_name` |
| C5 | N6: ba tệp ngoài, ba yêu cầu sát nhau / tệp đầu phát, hai tệp sau vào hàng đợi | `requests_close_together_become_one_queue` |
| C6 | N6: tệp ngoài thứ hai mở khi tệp ngoài thứ nhất đang phát / thì phát | `second_external_open_later_takes_over` |
| C7 | S22: `BÀI.MP3`, đủ sáu đuôi | `resolve_keeps_only_audio` |
| C8 | S22: `argv` đứng trước yêu cầu của phiên khác, không hẹn giờ | `initial_paths_come_first` |
| C9 | Mở lại không kèm tệp / chỉ focus | `empty_open_only_activates`, `empty_request_only_focuses` |
| C10 | Unix: socket chết sau crash được dọn, 0600 trong 0700 | `stale_socket_is_cleaned_up`, `socket_is_private`: máy này chỉ biên dịch, chưa chạy |
| C11 | Cửa sổ thu nhỏ thật lên trước, Finder, Wayland | `[owner-qa]` |

## 8. Nợ chuyển task khác

- Task 29: `main.rs` dùng `std::env::args()`, panic nếu `argv` không phải Unicode, nên phiên đầu mở tệp tên hỏng là sập. Task này không được sửa `main.rs`; đã ghi vào task 29.
- Task 27: NSIS, `Info.plist`, `.desktop` (`%F`) của đặc tả 5.6. Đã ghi bàn giao.

## 9. Bàn giao thực tế

**Kiểm** (cwd worktree, `CARGO_TARGET_DIR` riêng): `cargo build`, `clippy --workspace --all-targets -D warnings`, `cargo deny check licenses` xanh. `cargo test -p groove-app` chạy lặp 8 lần xanh. Test của task: endpoint `for_test`, `FakeOutput`, không thư viện, không phiên: không ra loa, không chạm DB thật, không đụng liên kết định dạng, không mở cửa sổ.

**Lỗi tìm được khi chạy song song**: `second_process_forwards_and_exits` đỏ 2 / 16 lần với 8 bản test cùng chạy, `no request`: phiên sau báo đã chuyển mà phiên chính không nhận được, tức mất tệp. Chưa tách được nguyên nhân giữa hai khả năng: hết hạn gửi 3 giây, hoặc client đóng trước khi server nhận kết nối. Đã sửa cả hai: phiên chính trả `ok`, phiên sau chờ rồi mới thoát; hạn gửi 10 giây, hạn giành 5 giây. Sau đó 24 lần chạy song song không còn đỏ. Đây là khác biệt có chủ ý so với "gửi rồi thoát" của đặc tả 5.5.

**Test chập chờn khi máy bận, không thuộc task này, chưa sửa**: `groove-audio engine::tests::idle_closes_stream_and_keeps_position` và `groove-app player::tests::volume_and_mute_reach_the_engine` đỏ lúc các làn khác đang build, chạy lại thì xanh; cùng nhóm còn `position_excludes_audio_still_in_the_ring`, `device_loss_reopens_on_new_rate_and_keeps_position`, `no_stream_until_played_and_restore_position_without_autoplay`, `plays_every_frame_and_ends_only_after_the_ring_is_empty` và `player::tests::natural_end_of_the_last_track_stops`. Là test của task khác, làn này không được sửa tệp của chúng; nguyên nhân chưa điều tra, "do máy bận" mới là suy đoán.

**Chưa kiểm**: chưa mở app thật: máy một màn, và kiểm thật phải giành focus, ra loa (`plan/_backlog.md`). Vì vậy `run_app`, `bring_to_front` với cửa sổ thật, `forward_to_primary` với endpoint thật chưa chạy lần nào, chỉ biên dịch. Mã Unix: `cargo check --all-targets` cho `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` xanh trên `instance.rs` qua crate tạm (cả app không biên dịch chéo được vì `wayland-backend`, `libsqlite3-sys`), chưa chạy. `open_paths.rs` trên Unix chưa biên dịch chéo.

**API đã khoá**:

- `instance::forward_to_primary() -> bool`, `instance::take_incoming()`, `ForwardMessage { paths, cwd }`.
- `open_paths::run_app(application, on_ready)`, `open_paths::resolve(paths, cwd)`, `open_paths::positional_args(args)`, `OpenRequest { paths, activate }`.
- `run_app` bọc `Application::run`: bơm yêu cầu cài sau toàn bộ closure của `app::run`, nên thứ tự các lời gọi trong closure không ảnh hưởng. Task khác vẫn thêm dòng vào closure như cũ.

**Quyết định** (Opus chọn 02/11/2026): các dòng `groove-gd1_25` ở `plan/confirm.md`.

**Đã sửa ngoài phạm vi**: `plan/_backlog.md` trên `master` còn dấu xung đột và dòng trùng, đã gỡ.

**Soi cải tiến**: bảo mật: pipe một SID + instance đầu, phiên sau kiểm exe server trước khi lộ đường dẫn, Unix 0600 / 0700, dòng có trần 1 MiB, không log đường dẫn. UI/UX: focus ngay, phát sau 200 ms. RAM: thêm một luồng accept, luồng đọc ngắn, không phụ thuộc mới; chưa đo. Pro: không áp.
