//! groove-gd1_15: ảnh bìa vuông + placeholder theo loại (đặc tả 1.2, 4).
//!
//! Ảnh nạp qua `LruImageCache` dùng chung của màn (`img(path).image_cache(&cache)`): `img` trần giữ ảnh đã giải mã suốt đời
//! app, xem `materiant/plan/completed/gd5/materiant-gd5_04_virtual-grid.md` mục 9. Không có tệp (chưa có bìa, thư mục cache
//! bị xoá, tệp hỏng) thì vẽ placeholder: album là nền trung tính + icon bìa mặc định, thư mục là gradient màu chủ đạo +
//! icon thư mục, playlist là màu của playlist + icon album.

use std::path::PathBuf;

use groove_core::model::CategoryRef;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::{
    AnyElement, Entity, Hsla, ImageSource, IntoElement, ObjectFit, ParentElement, RenderOnce, Rgba, Styled,
    StyledImage, Window, div, img, linear_color_stop, linear_gradient, px,
};
use materiant::icon::Icon;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::virtual_grid::LruImageCache;

use crate::icons::GrooveIcon;

/// Loại thẻ, quyết định placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverKind {
    Album,
    Folder,
    Playlist,
}

impl CoverKind {
    pub fn of(reference: CategoryRef) -> CoverKind {
        match reference {
            CategoryRef::Album(_) => CoverKind::Album,
            CategoryRef::Folder(_) => CoverKind::Folder,
            CategoryRef::Playlist(_) => CoverKind::Playlist,
        }
    }

    fn icon(self) -> GrooveIcon {
        match self {
            CoverKind::Album => GrooveIcon::DefaultCover,
            CoverKind::Folder => GrooveIcon::Folder,
            CoverKind::Playlist => GrooveIcon::Album,
        }
    }
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` thành màu; chuỗi hỏng thì `None` (màu trong DB là chuỗi người dùng / bản cũ ghi).
pub fn parse_color(text: &str) -> Option<Hsla> {
    Rgba::try_from(text.trim()).ok().map(Hsla::from)
}

/// Ảnh bìa lấp đầy khung cha (cha lo cỡ vuông). `path` là tệp trong thư mục bìa, `None` = chưa có bìa.
#[derive(IntoElement)]
pub struct CoverImage {
    path: Option<PathBuf>,
    kind: CoverKind,
    /// Màu chủ đạo của bìa (cột `accent_color`) hoặc màu playlist.
    accent: Option<Hsla>,
    cache: Entity<LruImageCache>,
}

impl CoverImage {
    pub fn new(
        path: Option<PathBuf>,
        kind: CoverKind,
        accent: Option<Hsla>,
        cache: &Entity<LruImageCache>,
    ) -> CoverImage {
        CoverImage { path, kind, accent, cache: cache.clone() }
    }
}

/// Khung placeholder, dựng lại được từ giá trị `Copy` để dùng trong `with_fallback`.
fn placeholder(kind: CoverKind, accent: Option<Hsla>, c: &materiant::theme::ThemeColors) -> AnyElement {
    let (background, ink) = match kind {
        CoverKind::Album => (div().bg(c.neutral(Neutral::N200)), c.muted),
        CoverKind::Folder => {
            let base = accent.unwrap_or(c.primary);
            (
                div().bg(linear_gradient(135., linear_color_stop(base, 0.), linear_color_stop(base.opacity(0.55), 1.))),
                c.on_primary,
            )
        }
        CoverKind::Playlist => {
            // Playlist chưa ghi màu theo màu nhấn của theme (G-4).
            let base = accent.unwrap_or(c.primary);
            (div().bg(base), materiant::theme::color::on_color(base.into()).into())
        }
    };
    background
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(kind.icon()).size(px(56.)).color(ink.opacity(0.9)))
        .into_any_element()
}

impl RenderOnce for CoverImage {
    fn render(self, _: &mut Window, cx: &mut materiant::gpui_kit::App) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let (kind, accent) = (self.kind, self.accent);
        let fallback = {
            let colors = colors.clone();
            move || placeholder(kind, accent, &colors)
        };
        let frame = div().size_full().overflow_hidden().rounded_lg();
        match self.path {
            Some(path) => frame.child(
                img(ImageSource::from(path))
                    .image_cache(&self.cache)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(fallback.clone())
                    .with_fallback(fallback),
            ),
            None => frame.child(placeholder(kind, accent, &colors)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors_and_rejects_garbage() {
        assert!(parse_color("#06b6d4").is_some());
        assert!(parse_color(" #fff ").is_some());
        assert!(parse_color("#06b6d4ff").is_some());
        assert!(parse_color("06b6d4").is_none(), "thiếu #");
        assert!(parse_color("#12").is_none());
        assert!(parse_color("").is_none());
        assert!(parse_color("#éééééé").is_none(), "không hoảng với unicode");
    }

    #[test]
    fn kind_follows_the_reference() {
        assert_eq!(CoverKind::of(CategoryRef::Album(1)), CoverKind::Album);
        assert_eq!(CoverKind::of(CategoryRef::Folder(1)), CoverKind::Folder);
        assert_eq!(CoverKind::of(CategoryRef::Playlist(1)), CoverKind::Playlist);
    }
}
