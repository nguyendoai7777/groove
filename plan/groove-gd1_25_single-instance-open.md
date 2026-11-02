# groove-gd1_25 — single-instance-open

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | 🔄 |
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
