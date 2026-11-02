//! groove-gd1_15: ảnh bìa vuông + placeholder theo loại (đặc tả 1.2, 4).
//!
//! Ảnh nạp qua `LruImageCache` dùng chung của màn (`cache.load(Resource::Path)`): `img` trần giữ ảnh đã giải mã suốt đời
//! app, xem `materiant/plan/completed/gd5/materiant-gd5_04_virtual-grid.md` mục 9. Không có tệp (chưa có bìa, thư mục cache
//! bị xoá, tệp hỏng) thì vẽ placeholder: album là nền trung tính + icon bìa mặc định, thư mục là gradient màu chủ đạo +
//! icon thư mục, playlist là màu của playlist + icon album.
//!
//! ui-audit (02/11/2026): GPUI không cắt con theo bo góc (`overflow_hidden` chỉ cắt hình chữ nhật), nên ảnh trong khung bo
//! góc phải tự bo đúng bán kính của khung, và ảnh phóng to (hover thẻ) phải cắt ngay lúc vẽ: ảnh vẽ bằng
//! `Window::paint_image(khung, vùng ảnh, bo góc)`, vùng ảnh là `object-fit: cover` nhân [`CoverImage::zoom`] quanh tâm, phần
//! tràn bị cắt theo khung nên bốn góc luôn tròn. Placeholder (đang nạp, lỗi, không bìa) cũng bo cùng bán kính.

use std::path::PathBuf;
use std::sync::Arc;

use groove_core::model::CategoryRef;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::{
    AnyElement, App, Bounds, Corners, Entity, Hsla, Image, ImageCache, IntoElement, ParentElement, Pixels, RenderImage,
    RenderOnce, Resource, Rgba, Styled, Window, canvas, div, linear_color_stop, linear_gradient, px,
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

/// Nguồn ảnh: tệp trong thư mục bìa (qua `LruImageCache` của màn), hoặc ảnh nhúng trong tệp nhạc ngoài thư viện.
#[derive(Clone)]
pub enum CoverSource {
    Path(PathBuf),
    Embedded(Arc<Image>),
}

/// Bo góc mặc định của bìa (`rounded-lg`).
pub const RADIUS: Pixels = px(8.);

/// Ảnh bìa lấp đầy khung cha (cha lo cỡ vuông), bo góc [`RADIUS`] hoặc [`CoverImage::radius`]. `None` = chưa có bìa.
#[derive(IntoElement)]
pub struct CoverImage {
    source: Option<CoverSource>,
    kind: CoverKind,
    /// Màu chủ đạo của bìa (cột `accent_color`) hoặc màu playlist.
    accent: Option<Hsla>,
    cache: Entity<LruImageCache>,
    radius: Pixels,
    zoom: f32,
}

impl CoverImage {
    pub fn new(
        path: Option<PathBuf>,
        kind: CoverKind,
        accent: Option<Hsla>,
        cache: &Entity<LruImageCache>,
    ) -> CoverImage {
        CoverImage { source: path.map(CoverSource::Path), kind, accent, cache: cache.clone(), radius: RADIUS, zoom: 1. }
    }

    /// Ảnh nhúng trong tệp nhạc ngoài thư viện; không có thì như [`CoverImage::new`] với `path`.
    pub fn embedded(mut self, image: Option<Arc<Image>>) -> CoverImage {
        if let Some(image) = image {
            self.source = Some(CoverSource::Embedded(image));
        }
        self
    }

    /// Bo góc — đặt bằng bo góc của khung chứa, vì GPUI không cắt con theo góc bo của cha.
    pub fn radius(mut self, radius: Pixels) -> CoverImage {
        self.radius = radius;
        self
    }

    /// Phóng ảnh quanh tâm (1 = vừa khung); phần tràn bị cắt theo khung, góc vẫn bo.
    pub fn zoom(mut self, zoom: f32) -> CoverImage {
        self.zoom = zoom.max(1.);
        self
    }
}

/// Khung placeholder, dựng lại được từ giá trị `Copy`.
fn placeholder(kind: CoverKind, accent: Option<Hsla>, radius: Pixels, c: &materiant::theme::ThemeColors) -> AnyElement {
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
        .rounded(radius)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(kind.icon()).size(px(56.)).color(ink.opacity(0.9)))
        .into_any_element()
}

/// Vùng vẽ ảnh: `object-fit: cover` trong `frame` rồi phóng `zoom` quanh tâm khung.
pub fn zoomed_cover(frame: Bounds<Pixels>, image: materiant::gpui_kit::Size<Pixels>, zoom: f32) -> Bounds<Pixels> {
    let (fw, fh) = (f32::from(frame.size.width), f32::from(frame.size.height));
    let (iw, ih) = (f32::from(image.width).max(1.), f32::from(image.height).max(1.));
    let scale = (fw / iw).max(fh / ih) * zoom.max(1.);
    Bounds::centered_at(frame.center(), materiant::gpui_kit::size(px(iw * scale), px(ih * scale)))
}

/// Ảnh đã giải mã, nếu xong.
fn loaded(
    source: &CoverSource,
    cache: &Entity<LruImageCache>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    match source {
        CoverSource::Path(path) => {
            let resource = Resource::Path(path.as_path().into());
            cache.update(cx, |cache, cx| cache.load(&resource, window, cx)).and_then(Result::ok)
        }
        CoverSource::Embedded(image) => image.clone().use_render_image(window, cx),
    }
}

impl RenderOnce for CoverImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let (radius, zoom) = (self.radius, self.zoom);
        let frame = div().size_full().rounded(radius);
        let data = self.source.as_ref().and_then(|source| loaded(source, &self.cache, window, cx));
        match data {
            Some(data) => frame.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let size = data.size(0);
                        let image = materiant::gpui_kit::size(px(size.width.0 as f32), px(size.height.0 as f32));
                        let target = zoomed_cover(bounds, image, zoom);
                        if let Err(err) =
                            window.paint_image(bounds, target, Corners::all(radius), data.clone(), 0, false)
                        {
                            log::warn!("cover paint: {err:#}");
                        }
                    },
                )
                .size_full(),
            ),
            // Đang nạp, lỗi, chưa có bìa: placeholder cùng bo góc. Nạp xong cache tự xin vẽ lại.
            None => frame.child(placeholder(self.kind, self.accent, radius, &colors)),
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

    /// Ảnh dọc 100 x 200 trong khung 50 x 50: cover phủ kín khung theo cạnh ngắn, phóng 1,05 quanh tâm khung; mép khung luôn
    /// nằm trong ảnh nên góc bo của khung không lộ nền.
    #[test]
    fn zoomed_cover_covers_the_frame_around_its_center() {
        use materiant::gpui_kit::{point, size};
        let frame = Bounds::new(point(px(10.), px(20.)), size(px(50.), px(50.)));
        let fit = zoomed_cover(frame, size(px(100.), px(200.)), 1.);
        assert_eq!(fit.size, size(px(50.), px(100.)));
        assert_eq!(fit.center(), frame.center());
        let zoomed = zoomed_cover(frame, size(px(100.), px(200.)), 1.05);
        assert!((f32::from(zoomed.size.width) - 52.5).abs() < 1e-3);
        assert_eq!(zoomed.center(), frame.center());
        assert!(zoomed.origin.x <= frame.origin.x && zoomed.bottom_right().x >= frame.bottom_right().x);
        assert_eq!(zoomed_cover(frame, size(px(100.), px(200.)), 0.5), fit, "không thu nhỏ dưới 1");
    }

    #[test]
    fn kind_follows_the_reference() {
        assert_eq!(CoverKind::of(CategoryRef::Album(1)), CoverKind::Album);
        assert_eq!(CoverKind::of(CategoryRef::Folder(1)), CoverKind::Folder);
        assert_eq!(CoverKind::of(CategoryRef::Playlist(1)), CoverKind::Playlist);
    }
}
