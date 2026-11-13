//! groove-gd1_03: GrooveIcon.
//!
//! Icon riêng của app, tách từ năm sprite `public/icons/*.svg` của bản Tauri thành từng tệp ở `assets/icons/`, nhúng vào
//! binary. [`Assets`] là `AssetSource` của app: icon của app trước, rồi icon của materiant (`materiant::icon::Assets`).
//!
//! ```ignore
//! gpui_kit::application().with_assets(icons::Assets).run(|cx| { … });
//! IconButton::new("back", GrooveIcon::AngleLeft)        // `From<GrooveIcon> for IconSource`
//! Icon::new(GrooveIcon::Song).size(px(16.))
//! ```
//!
//! Thêm icon = thêm tệp `.svg` vào `assets/icons/` + một dòng trong `groove_icons!`; test `every_file_is_declared` bắt
//! tệp mồ côi.

use std::borrow::Cow;

use materiant::gpui_kit::{AssetSource, SharedString};
use materiant::icon::IconSource;

macro_rules! groove_icons {
    ($($(#[$doc:meta])* $name:ident => $file:literal,)*) => {
        /// Icon của app. Đường dẫn asset: `groove/<tệp>.svg`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum GrooveIcon {
            $($(#[$doc])* $name,)*
        }

        impl GrooveIcon {
            pub const ALL: &'static [GrooveIcon] = &[$(GrooveIcon::$name,)*];

            /// Đường dẫn trong [`Assets`].
            pub fn path(self) -> &'static str {
                match self {
                    $(GrooveIcon::$name => concat!("groove/", $file, ".svg"),)*
                }
            }

            /// Tên tệp, không đuôi.
            pub fn file_name(self) -> &'static str {
                match self {
                    $(GrooveIcon::$name => $file,)*
                }
            }
        }

        fn embedded(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("groove/", $file, ".svg") => Some(include_bytes!(concat!("../assets/icons/", $file, ".svg"))),)*
                _ => None,
            }
        }
    };
}

groove_icons! {
    // Thanh trên, cột điều hướng.
    Close => "close",
    Minus => "minus",
    AngleLeft => "angle-left",
    AngleRight => "angle-right",
    /// Nút thu / mở cột trái: khung có vách trái, mũi tên trong khung.
    SidebarCollapse => "sidebar-collapse",
    SidebarExpand => "sidebar-expand",
    ArrowsExpand => "arrows-expand",
    ArrowsCompress => "arrows-compress",
    Search => "search",
    Settings => "settings",
    /// My Music.
    Song => "song",
    /// Now Playing.
    NowPlaying => "now-playing",
    /// Playlists.
    Album => "album",
    /// Đĩa vẽ bằng nét mảnh (cùng hình `album`), cho placeholder bìa lớn Now Playing: phóng to không phình nét.
    AlbumThin => "album-thin",
    // Thanh phát.
    Shuffle => "shuffle",
    Loop => "loop",
    LoopBadge => "loop-badge",
    Prev => "prev",
    Next => "next",
    Play => "play",
    /// Như `play`, nét 1,5: nút Play all trên lớp phủ của thẻ thư viện (chủ dự án 03/11/2026: icon mảnh hơn).
    PlayThin => "play-thin",
    /// groove-gd1_14: nút Phát khi đang phát.
    Pause => "pause",
    VolumeMute => "volume-mute",
    VolumeMin => "volume-min",
    VolumeMedium => "volume-medium",
    VolumeMax => "volume-max",
    /// Trình phát thu nhỏ (PiP): khung có ô nhỏ ở góc dưới phải.
    Pip => "pip",
    /// Nút của cửa sổ PiP: quay lại cửa sổ chính (mũi tên ra khỏi khung).
    PipExit => "pip-exit",
    // Thẻ, menu, hộp thoại, bảng lệnh.
    Folder => "folder",
    /// Nét 1,5 (mảnh hơn icon khác): hiện chỉ nút Delete trên lớp phủ của thẻ dùng.
    Delete => "delete",
    Edit => "edit",
    LyricsBubble => "lyrics-bubble",
    HelpCircle => "help-circle",
    Artist => "artist",
    Action => "action",
    // groove-gd2_05: nút cập nhật trên thanh trên (Lucide `circle-arrow-down`).
    Update => "update",
}

impl From<GrooveIcon> for IconSource {
    fn from(icon: GrooveIcon) -> Self {
        IconSource::Asset(icon.path().into())
    }
}

/// `AssetSource` của app.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> materiant::gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match embedded(path) {
            Some(bytes) => Ok(Some(Cow::Borrowed(bytes))),
            None => materiant::icon::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> materiant::gpui_kit::Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> =
            GrooveIcon::ALL.iter().map(|icon| icon.path()).filter(|p| p.starts_with(path)).map(Into::into).collect();
        out.extend(materiant::icon::Assets.list(path)?);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C7 — mọi icon có bytes, là SVG hợp lệ có viewBox, không trùng đường dẫn.
    #[test]
    fn every_icon_is_embedded() {
        let mut seen = std::collections::HashSet::new();
        for icon in GrooveIcon::ALL {
            assert!(seen.insert(icon.path()), "trùng {}", icon.path());
            let bytes = Assets.load(icon.path()).unwrap().unwrap_or_else(|| panic!("thiếu {}", icon.path()));
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(
                text.starts_with("<svg ") && text.contains("viewBox=\""),
                "{} không phải SVG có viewBox",
                icon.path()
            );
            assert!(text.trim_end().ends_with("</svg>"), "{} không đóng thẻ", icon.path());
        }
        assert!(Assets.load("groove/khong-co.svg").unwrap().is_none());
    }

    #[test]
    fn falls_back_to_materiant_icons() {
        let check = materiant::icon::IconName::Check.path();
        assert!(Assets.load(check).unwrap().is_some());
        let listed = Assets.list("").unwrap();
        assert_eq!(listed.len(), GrooveIcon::ALL.len() + materiant::icon::IconName::ALL.len());
    }

    /// Tệp trong `assets/icons/` không khai trong `groove_icons!` là tệp mồ côi (mất công nhúng nhưng không dùng được).
    #[test]
    fn every_file_is_declared() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let declared: std::collections::HashSet<_> = GrooveIcon::ALL.iter().map(|i| i.file_name()).collect();
        let mut files = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let stem = name.strip_suffix(".svg").unwrap_or_else(|| panic!("{name} không phải .svg"));
            assert!(declared.contains(stem), "{name} chưa khai trong groove_icons!");
            files += 1;
        }
        assert_eq!(files, declared.len());
    }

    #[test]
    fn converts_to_asset_source() {
        assert_eq!(IconSource::from(GrooveIcon::Song), IconSource::Asset("groove/song.svg".into()));
    }
}
