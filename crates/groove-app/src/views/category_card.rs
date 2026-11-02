//! groove-gd1_15: thẻ album / thư mục / playlist của lưới (đặc tả 1.2, bản cũ `music-card.vue`).
//!
//! Bìa vuông bo góc; rê chuột phủ nền + hai nút Play all, Delete; viền theo màu accent. Dòng phụ: album là nghệ sĩ hoặc
//! "chưa rõ nghệ sĩ", thư mục là đường dẫn, playlist là số bài. Thẻ chỉ báo lên: mở danh sách, Play all, Delete — nghiệp
//! vụ (phát, hộp xác nhận, xoá) là của task 17.
//!
//! Bản cũ phóng bìa 1,05 khi rê và có quầng bóng accent, chuyển 300 ms; GPUI `div` không có `scale`, bóng mờ, chuyển
//! tiếp nên giữ viền accent 2 px bo `rounded_xl`, đệm 12 px, bìa tự bo `rounded_lg` và cắt góc (đặc tả mục 4). Số đo lấy
//! từ `music-card.vue`: `p-3 rounded-xl border-2`, bìa `rounded-lg`, chữ `mt-3`, dòng phụ `mt-1`.

use std::path::PathBuf;
use std::rc::Rc;

use groove_core::model::CategoryRef;
use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use groove_library::db::queries::CategorySummary;
use materiant::button::IconButton;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, Entity, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, px, transparent_black,
};
use materiant::menu::ContextMenuExt;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::virtual_grid::LruImageCache;

use crate::icons::GrooveIcon;
use crate::views::cover_image::{CoverImage, CoverKind, DEFAULT_PLAYLIST_COLOR, parse_color};

/// Chiều cao cố định của dòng tên và dòng phụ: thẻ cao đúng `COVER + TEXT_BLOCK`, lưới tính được cao ô.
const TITLE_HEIGHT: f32 = 20.;
const SUBTITLE_HEIGHT: f32 = 16.;
const TEXT_GAP: f32 = 12.;
/// Khe giữa dòng tên và dòng phụ (`mt-1` của bản cũ).
const SUBTITLE_GAP: f32 = 4.;
/// `border-2` và `p-3` của bản cũ.
const BORDER: f32 = 2.;
const PADDING: f32 = 12.;
/// Phần cao của ô ngoài bìa vuông: hai viền + khoảng cách + hai dòng chữ + dư 2 px cho sai số làm tròn pixel.
/// `VirtualGrid::new(.., cell_height = CARD_EXTRA_HEIGHT)` + `.height_per_width(1.)`; lưới chừa thêm 12 px quanh ô
/// (padding của ô) và bìa hẹp hơn ô 12 + 2 × `BORDER` px, hai phần này triệt tiêu nhau ở bề ngang.
pub const CARD_EXTRA_HEIGHT: f32 = 2. * BORDER + TEXT_GAP + SUBTITLE_GAP + TITLE_HEIGHT + SUBTITLE_HEIGHT + 2. + 4.;

type Handler = Rc<dyn Fn(CategoryRef, &mut Window, &mut App)>;

/// Dòng phụ của thẻ.
pub fn subtitle(summary: &CategorySummary) -> SharedString {
    match summary.reference {
        CategoryRef::Album(_) => match summary.artist.as_deref().map(str::trim) {
            Some(artist) if !artist.is_empty() => artist.to_owned().into(),
            _ => t!("library.card.unknown_artist").into(),
        },
        CategoryRef::Folder(_) => summary.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default().into(),
        CategoryRef::Playlist(_) => t!("library.card.items", count = summary.song_count).into(),
    }
}

/// Màu accent của thẻ: playlist theo màu của nó (mặc định cyan), album và thư mục theo bìa, chưa có thì `None`.
pub fn accent_of(summary: &CategorySummary) -> Option<Hsla> {
    let text = match summary.reference {
        CategoryRef::Playlist(_) => Some(summary.color.as_deref().unwrap_or(DEFAULT_PLAYLIST_COLOR)),
        _ => summary.accent_color.as_deref(),
    };
    text.and_then(parse_color)
}

/// Tệp bìa cỡ nhỏ (256 px) của thẻ; playlist không có ảnh.
pub fn cover_path(summary: &CategorySummary, covers: Option<&CoverStore>) -> Option<PathBuf> {
    if matches!(summary.reference, CategoryRef::Playlist(_)) {
        return None;
    }
    let (hash, store) = (summary.cover_hash.as_deref()?, covers?);
    // `hash` đến từ DB nhưng ghép thẳng vào đường dẫn: chỉ nhận hex (SHA-256), không để `..` hay dấu `/` thoát thư mục bìa.
    if hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(store.path(hash, CoverSize::Small))
}

#[derive(IntoElement)]
pub struct CategoryCard {
    summary: CategorySummary,
    covers: Option<CoverStore>,
    cache: Entity<LruImageCache>,
    on_open: Handler,
    on_play: Handler,
    on_delete: Handler,
}

impl CategoryCard {
    pub fn new(
        summary: &CategorySummary,
        covers: Option<&CoverStore>,
        cache: &Entity<LruImageCache>,
        on_open: impl Fn(CategoryRef, &mut Window, &mut App) + 'static,
        on_play: impl Fn(CategoryRef, &mut Window, &mut App) + 'static,
        on_delete: impl Fn(CategoryRef, &mut Window, &mut App) + 'static,
    ) -> CategoryCard {
        CategoryCard {
            summary: summary.clone(),
            covers: covers.cloned(),
            cache: cache.clone(),
            on_open: Rc::new(on_open),
            on_play: Rc::new(on_play),
            on_delete: Rc::new(on_delete),
        }
    }
}

/// Mã chỉ số của loại, để đặt id phần tử không đụng nhau giữa album 1, thư mục 1, playlist 1.
fn kind_tag(reference: CategoryRef) -> (&'static str, i64) {
    match reference {
        CategoryRef::Album(id) => ("album", id),
        CategoryRef::Folder(id) => ("folder", id),
        CategoryRef::Playlist(id) => ("playlist", id),
    }
}

impl RenderOnce for CategoryCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let reference = self.summary.reference;
        let (tag, id) = kind_tag(reference);
        let group: SharedString = format!("card-{tag}-{id}").into();
        let accent = accent_of(&self.summary);
        // Bản cũ: `accentColor || '#06b6d4'`, không có màu thì viền cyan.
        let border_hover = accent.or_else(|| parse_color(DEFAULT_PLAYLIST_COLOR)).unwrap_or(c.primary_ink);

        let cover = CoverImage::new(
            cover_path(&self.summary, self.covers.as_ref()),
            CoverKind::of(reference),
            accent,
            &self.cache,
        );

        let card_hover = c.surface.opacity(0.6);
        let (on_open, on_play, on_delete) = (self.on_open, self.on_play, self.on_delete);
        // Nút trong lớp phủ không được lọt nhấp lên thẻ (sẽ mở danh sách): `stop_propagation` trong `on_click`. Không dùng
        // `occlude()` ở đây: nó chặn hover của thẻ, `group_hover` tắt và lớp phủ biến mất ngay dưới con trỏ.
        let action =
            |button_id: (&'static str, i64), icon: GrooveIcon, label: &'static str, danger: bool, run: Handler| {
                let ink = if danger { c.danger } else { c.text };
                // `bg-theme-bg-placeholder/80` + viền `border-theme-border/50` của bản cũ, tròn.
                div()
                    .rounded_full()
                    .bg(c.neutral(Neutral::N200).opacity(0.8))
                    .border_1()
                    .border_color(c.border.opacity(0.5))
                    .debug_selector(move || format!("{}-{}", button_id.0, button_id.1))
                    .child(
                        IconButton::new((button_id.0, button_id.1 as usize), icon)
                            .accessibility_label(label)
                            .color(ink)
                            .rounded_full()
                            .on_click(move |_: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                run(reference, window, cx);
                            }),
                    )
                    .with_tooltip((button_id.0, button_id.1 as usize + 1_000_000), label)
            };
        let overlay = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(c.surface.opacity(0.75))
            .invisible()
            .group_hover(group.clone(), |s| s.visible())
            .child(action(("card-play", id), GrooveIcon::Play, t!("library.card.play_all"), false, on_play))
            .child(action(("card-delete", id), GrooveIcon::Delete, t!("library.card.delete"), true, on_delete));

        div()
            .id((tag, id as usize))
            .relative()
            .context_menu(move |menu, _, cx| crate::views::card_menu::build(menu, reference, cx))
            .group(group.clone())
            .size_full()
            .flex()
            .flex_col()
            .p(px(PADDING))
            .border(px(BORDER))
            .border_color(transparent_black())
            .rounded_xl()
            .bg(c.surface.opacity(0.4))
            .cursor_pointer()
            .debug_selector(move || format!("card-{tag}-{id}"))
            .group_hover(group, move |s| s.border_color(border_hover).bg(card_hover))
            .on_click(move |_: &ClickEvent, window, cx| on_open(reference, window, cx))
            .child(div().relative().w_full().aspect_square().rounded_lg().overflow_hidden().child(cover).child(overlay))
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .mt(px(TEXT_GAP))
                    .child(
                        div()
                            .h(px(TITLE_HEIGHT))
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text)
                            .child(SharedString::from(self.summary.name.clone())),
                    )
                    .child(
                        div()
                            .mt(px(SUBTITLE_GAP))
                            .h(px(SUBTITLE_HEIGHT))
                            .truncate()
                            .text_size(cx.theme().caption_size())
                            .text_color(c.muted)
                            .child(subtitle(&self.summary)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(reference: CategoryRef) -> CategorySummary {
        CategorySummary {
            reference,
            name: "Name".into(),
            artist: None,
            path: None,
            color: None,
            song_count: 3,
            cover_hash: None,
            accent_color: None,
        }
    }

    #[test]
    fn subtitle_depends_on_kind() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let mut album = summary(CategoryRef::Album(1));
        assert_eq!(subtitle(&album), t!("library.card.unknown_artist"));
        album.artist = Some("   ".into());
        assert_eq!(subtitle(&album), t!("library.card.unknown_artist"), "nghệ sĩ toàn khoảng trắng coi như trống");
        album.artist = Some("Adele".into());
        assert_eq!(subtitle(&album), "Adele");

        let mut folder = summary(CategoryRef::Folder(1));
        folder.path = Some(PathBuf::from("D:/Music/Rock"));
        assert!(subtitle(&folder).contains("Rock"));
        assert_eq!(subtitle(&summary(CategoryRef::Folder(2))), "", "thư mục chưa có đường dẫn: dòng trống");

        assert_eq!(subtitle(&summary(CategoryRef::Playlist(1))), t!("library.card.items", count = 3));
    }

    #[test]
    fn accent_follows_kind() {
        let mut playlist = summary(CategoryRef::Playlist(1));
        assert_eq!(accent_of(&playlist), parse_color(DEFAULT_PLAYLIST_COLOR), "playlist không màu: cyan");
        playlist.color = Some("#ff0000".into());
        assert_eq!(accent_of(&playlist), parse_color("#ff0000"));
        playlist.color = Some("garbage".into());
        assert_eq!(accent_of(&playlist), None, "màu hỏng: bỏ, không đoán");

        let mut album = summary(CategoryRef::Album(1));
        assert_eq!(accent_of(&album), None);
        album.accent_color = Some("#102030".into());
        assert_eq!(accent_of(&album), parse_color("#102030"));
        album.color = Some("#ff0000".into());
        assert_eq!(accent_of(&album), parse_color("#102030"), "album bỏ qua `color` của playlist");
    }

    #[test]
    fn cover_path_uses_the_small_file_and_skips_playlists() {
        let store = CoverStore::new("C:/cache/covers");
        let mut album = summary(CategoryRef::Album(1));
        assert_eq!(cover_path(&album, Some(&store)), None, "chưa có hash");
        album.cover_hash = Some("abc".into());
        for bad in ["", "../../secret", "a/b", r"a\b", "zz"] {
            album.cover_hash = Some(bad.into());
            assert_eq!(cover_path(&album, Some(&store)), None, "hash `{bad}` không phải hex");
        }
        album.cover_hash = Some("abc".into());
        assert_eq!(cover_path(&album, None), None, "không có thư mục bìa");
        assert_eq!(cover_path(&album, Some(&store)), Some(store.path("abc", CoverSize::Small)));

        let mut playlist = summary(CategoryRef::Playlist(1));
        playlist.cover_hash = Some("abc".into());
        assert_eq!(cover_path(&playlist, Some(&store)), None, "playlist không có ảnh");
    }
}
