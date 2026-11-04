# Ảnh chụp Groovex cho cửa hàng / trang chủ

Ghi nợ tới khi phát hành (D-21): chủ dự án chụp, đặt tệp PNG vào thư mục này. Sau khi đưa lên host công khai (vd
`https://sandtechs.com/groovex/screenshots/`), thêm khối `<screenshots>` vào `packaging/linux/groovex.metainfo.xml` (AppStream
chỉ nhận URL, không nhận tệp trong gói) — mẫu ở cuối tệp này.

## Thông số

| Mục | Giá trị |
| --- | --- |
| Định dạng | PNG, không nén mất dữ liệu, không trong suốt |
| Cỡ | **1600 × 900 px** (16:9), đúng cỡ cửa sổ, tỉ lệ màn 1× (màn 2× thì đặt cửa sổ 800 × 450 logic hoặc thu ảnh về 1600 × 900) |
| Nội dung | Chỉ cửa sổ app, không viền OS / bóng / hình nền, không con trỏ chuột, không thông báo hệ thống |
| Ngôn ngữ | Tiếng Anh (AppStream lấy ảnh chung cho mọi ngôn ngữ) |
| Dữ liệu mẫu | Thư viện thử có bài và bìa mình có quyền dùng (tự làm / miễn phí bản quyền); không bìa album thương mại, không tên tệp / đường dẫn cá nhân |
| Số ảnh | 5 (tối thiểu 3, ảnh đầu là ảnh đại diện) |

## Danh sách

| Tệp | Màn | Theme | Chú thích (en / vi) |
| --- | --- | --- | --- |
| `01-library-dark.png` | My Music, tab Albums, lưới có bìa | Tối | Your music library at a glance / Thư viện nhạc trong một màn |
| `02-now-playing-dark.png` | Now Playing đang phát, có lời bài và hàng đợi | Tối | Lyrics and the playing queue / Lời bài hát và hàng đợi |
| `03-songs-light.png` | My Music, danh sách bài của một album | Sáng | Albums, folders and playlists / Album, thư mục, danh sách phát |
| `04-equalizer-dark.png` | Settings → EQ | Tối | Ten-band equalizer with presets / Bộ chỉnh âm 10 dải có sẵn mẫu |
| `05-mini-player-dark.png` | Trình phát thu nhỏ (PiP) đặt trên cửa sổ chính mờ phía sau | Tối | Mini player that stays on top / Trình phát thu nhỏ luôn trên cùng |

Đổi danh sách thì sửa bảng này trước, rồi mới chụp.

## Mẫu khối metainfo (khi đã có URL)

```xml
<screenshots>
  <screenshot type="default">
    <caption>Your music library at a glance</caption>
    <caption xml:lang="vi">Thư viện nhạc trong một màn</caption>
    <image type="source" width="1600" height="900">https://sandtechs.com/groovex/screenshots/01-library-dark.png</image>
  </screenshot>
  <!-- … mỗi ảnh một <screenshot> -->
</screenshots>
```
