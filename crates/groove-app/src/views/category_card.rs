//! groove-gd1_15: thẻ album / thư mục / playlist của lưới (đặc tả 1.2, bản cũ `music-card.vue`).
//!
//! Bìa vuông bo góc; rê chuột phủ nền + hai nút Play all, Delete; viền theo màu accent. Dòng phụ: album là nghệ sĩ hoặc
//! "chưa rõ nghệ sĩ", thư mục là đường dẫn, playlist là số bài. Thẻ chỉ báo lên: mở danh sách, Play all, Delete — nghiệp
//! vụ (phát, hộp xác nhận, xoá) là của task 17.
//!
//! Hover như `music-card.vue`: viền accent 2 px, nền thẻ 40% lên 60%, quầng bóng accent `0 0 20px <accent>40` tỏa ra ngoài
//! viền, lớp phủ hiện dần 300 ms (`with_animation`, chỉ chạy lúc đổi trạng thái). Bỏ phóng bìa 1,05 của bản web (chủ dự án
//! 03/11/2026). Số đo: `p-3 rounded-xl border-2`, bìa `rounded-lg`, chữ `mt-3`, dòng phụ
//! `mt-1`. Không làm được: `backdrop-blur` của lớp phủ.
//!
//! Thẻ lấp kín ô của lưới (`cell_padding(0)`, `cell_radius(CARD_RADIUS)`): nền rê / chọn và ripple của ô trùng khít mép ngoài
//! viền thẻ, cùng bán kính (chủ dự án 02/11/2026: nền rê lệch viền). Ô không cắt phần vẽ tràn (`clip_cells(false)`) để quầng
//! bóng tỏa vào khe 24 px giữa các thẻ; mép khung cuộn vẫn cắt.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use groove_core::model::CategoryRef;
use groove_core::t;
use groove_library::covers::{CoverSize, CoverStore};
use groove_library::db::queries::CategorySummary;
use materiant::button::IconButton;
// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ClickEvent, Entity, Hsla, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, point, px,
};
use materiant::menu::ContextMenuExt;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::{ActiveTheme, Neutral};
use materiant::virtual_grid::LruImageCache;

use crate::icons::GrooveIcon;
use crate::views::cover_image::{self, CoverImage, CoverKind, parse_color};

/// Số hàng chữ của tên thẻ: tên dài xuống tới hai hàng rồi mới cắt (chủ dự án 03/11/2026 không muốn cắt "…"; lưới ô cùng cỡ
/// của `VirtualGrid` không cho thẻ cao riêng, xem `plan/confirm.md` mục `wrap`).
const TITLE_LINES: usize = 2;
/// Cao một hàng chữ tên thẻ.
const TITLE_LINE_HEIGHT: f32 = 20.;
/// Chiều cao cố định của khối tên và dòng phụ: thẻ cao đúng `COVER + TEXT_BLOCK`, lưới tính được cao ô.
const TITLE_HEIGHT: f32 = TITLE_LINE_HEIGHT * TITLE_LINES as f32;
const SUBTITLE_HEIGHT: f32 = 16.;
const TEXT_GAP: f32 = 12.;
/// Khe giữa dòng tên và dòng phụ (`mt-1` của bản cũ).
const SUBTITLE_GAP: f32 = 4.;
/// `border-2` và `p-3` của bản cũ.
const BORDER: f32 = 2.;
/// `hover:shadow-[0_0_20px_<accent>40]` của bản web: blur 20 px, alpha 0x40 = 25 %.
const GLOW_BLUR: f32 = 20.;
const GLOW_ALPHA: f32 = 0.25;
/// `rounded-xl` của thẻ, tính tới mép ngoài viền; lưới bo ô đúng bằng số này.
pub const CARD_RADIUS: f32 = 12.;
const PADDING: f32 = 12.;
/// Phần cao của ô ngoài bìa vuông: hai viền + khoảng cách + hai dòng chữ + dư 2 px cho sai số làm tròn pixel.
/// `VirtualGrid::new(.., cell_height = CARD_EXTRA_HEIGHT)` + `.height_per_width(1.)`. Thẻ lấp kín ô, bìa hẹp
/// hơn ô 2 × (`BORDER` + `PADDING`) = 28 px; cao ô vì thế dư vài px so với viền + đệm + bìa + chữ, phần dư nằm cuối thẻ.
pub const CARD_EXTRA_HEIGHT: f32 = 2. * BORDER + TEXT_GAP + SUBTITLE_GAP + TITLE_HEIGHT + SUBTITLE_HEIGHT + 2. + 4.;

/// Chuyển của `transition-all duration-300` (thẻ, lớp phủ) bản cũ.
const FADE: Duration = Duration::from_millis(300);
/// Nút tròn trên lớp phủ: cạnh nút (32 của `Size::Md` + 4 theo chủ dự án 03/11/2026), icon giữ tỉ lệ ~62 % như `IconButton`.
const ACTION_EDGE: f32 = 36.;
const ACTION_ICON: f32 = 22.;
/// Độ đục nền nút lúc nghỉ (`bg-theme-bg-placeholder/80` bản cũ) và lúc rê (chủ dự án: hơi xám, 0,85).
const ACTION_REST_ALPHA: f32 = 0.8;
const ACTION_HOVER_ALPHA: f32 = 0.85;

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

/// Màu accent của thẻ: playlist theo màu của nó, album và thư mục theo bìa; chưa có thì `None` (nơi vẽ dùng màu nhấn của theme).
pub fn accent_of(summary: &CategorySummary) -> Option<Hsla> {
    let text = match summary.reference {
        CategoryRef::Playlist(_) => summary.color.as_deref(),
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

/// Trạng thái rê của một thẻ, giữ qua các khung bằng `window.use_keyed_state`. `seq` tăng mỗi lần đổi: là khoá của hoạt ảnh,
/// đổi khoá thì hoạt ảnh chạy lại. `seq == 0` = chưa từng rê, vẽ tĩnh, không dựng hoạt ảnh.
#[derive(Default, Clone, Copy)]
struct HoverState {
    hovered: bool,
    seq: usize,
}

/// `cubic-bezier(0.4, 0, 0.2, 1)`, đường chuyển mặc định của Tailwind (`transition-all`): tìm t để x(t) = `x` rồi lấy y(t).
fn ease(x: f32) -> f32 {
    let (mut lo, mut hi) = (0_f32, 1_f32);
    for _ in 0..24 {
        let t = (lo + hi) / 2.;
        let bx = 3. * (1. - t).powi(2) * t * 0.4 + 3. * (1. - t) * t * t * 0.2 + t.powi(3);
        if bx < x { lo = t } else { hi = t }
    }
    let t = (lo + hi) / 2.;
    3. * (1. - t) * t * t + t.powi(3)
}

/// Chạy `style(phần_tử, tiến_độ)` với tiến độ 0..1 (1 = đang rê). Chưa từng rê thì vẽ tĩnh ở 0; đã rê thì nội suy
/// `duration` khi `seq` đổi, xong thì đứng yên, không vẽ lại. Tắt chuyển động của OS: `with_animation` tự nhảy tới đích.
fn animated<E: IntoElement + 'static>(
    element: E,
    id: impl Into<gpui::ElementId>,
    state: HoverState,
    duration: Duration,
    style: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if state.seq == 0 {
        return style(element, 0.).into_any_element();
    }
    let hovered = state.hovered;
    element
        .with_animation(id, Animation::new(duration).with_easing(ease), move |el, t| {
            style(el, if hovered { t } else { 1. - t })
        })
        .into_any_element()
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let reference = self.summary.reference;
        let (tag, id) = kind_tag(reference);
        let group: SharedString = format!("card-{tag}-{id}").into();
        let accent = accent_of(&self.summary);
        // Bản cũ: `accentColor || '#06b6d4'`; không có màu thì viền theo màu nhấn của theme (G-4).
        let border_hover = accent.unwrap_or(c.primary);

        let cover = CoverImage::new(
            cover_path(&self.summary, self.covers.as_ref()),
            CoverKind::of(reference),
            accent,
            &self.cache,
        );

        let state_entity = window.use_keyed_state(group.clone(), cx, |_, _| HoverState::default());
        let state = *state_entity.read(cx);
        let surface = c.surface;
        let (on_open, on_play, on_delete) = (self.on_open, self.on_play, self.on_delete);
        // Nút trong lớp phủ không được lọt nhấp lên thẻ (sẽ mở danh sách): `stop_propagation` trong `on_click`. Không dùng
        // `occlude()` ở đây: nó chặn hover của thẻ, `on_hover` tắt và lớp phủ biến mất ngay dưới con trỏ.
        let action =
            |button_id: (&'static str, i64), icon: GrooveIcon, label: &'static str, danger: bool, run: Handler| {
                let ink = if danger { c.danger } else { c.text };
                // `bg-theme-bg-placeholder/80` + viền `border-theme-border/50` của bản cũ, tròn. Rê: nền nhích một bậc xám, đục
                // 0,85 — khung bọc tự đổi nền; nền rê của `IconButton` tắt (màu gốc trong suốt, `.ink` giữ màu icon) để hai
                // lớp không chồng thành mảng xám đặc.
                div()
                    .rounded_full()
                    .bg(c.neutral(Neutral::N200).opacity(ACTION_REST_ALPHA))
                    .hover(|s| s.bg(c.neutral(Neutral::N300).opacity(ACTION_HOVER_ALPHA)))
                    .border_1()
                    .border_color(c.border.opacity(0.5))
                    .debug_selector(move || format!("{}-{}", button_id.0, button_id.1))
                    .child(
                        IconButton::new((button_id.0, button_id.1 as usize), icon)
                            .accessibility_label(label)
                            .color(gpui::transparent_black())
                            .ink(ink)
                            .w(px(ACTION_EDGE))
                            .h(px(ACTION_EDGE))
                            .icon_size(px(ACTION_ICON))
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
            // Bo đúng góc của bìa: GPUI không cắt con theo bo góc của khung.
            .rounded(cover_image::RADIUS)
            .flex()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(c.surface.opacity(0.75))
            .child(action(("card-play", id), GrooveIcon::PlayThin, t!("library.card.play_all"), false, on_play))
            .child(action(("card-delete", id), GrooveIcon::Delete, t!("library.card.delete"), true, on_delete));

        let overlay =
            animated(overlay, SharedString::from(format!("{group}-overlay-{}", state.seq)), state, FADE, |el, p| {
                if p <= 0. { el.invisible() } else { el.opacity(p) }
            });

        let card = div()
            .id((tag, id as usize))
            .relative()
            .context_menu(move |menu, _, cx| crate::views::card_menu::build(menu, reference, cx))
            .on_hover({
                let state_entity = state_entity.clone();
                move |hovered: &bool, _, cx| {
                    state_entity.update(cx, |s, cx| {
                        if s.hovered != *hovered {
                            s.hovered = *hovered;
                            s.seq += 1;
                            cx.notify();
                        }
                    });
                }
            })
            .size_full()
            .flex()
            .flex_col()
            .p(px(PADDING))
            .border(px(BORDER))
            .rounded(px(CARD_RADIUS))
            .cursor_pointer()
            .debug_selector(move || format!("card-{tag}-{id}"))
            .on_click(move |_: &ClickEvent, window, cx| on_open(reference, window, cx))
            .child(div().relative().w_full().aspect_square().child(cover).child(overlay))
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .mt(px(TEXT_GAP))
                    .child(
                        div()
                            // Tên một dòng thì dòng phụ nằm sát dưới; chỗ thừa của ô dồn xuống đáy.
                            .max_h(px(TITLE_HEIGHT))
                            .line_height(px(TITLE_LINE_HEIGHT))
                            .line_clamp(TITLE_LINES)
                            .text_ellipsis()
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
            );
        animated(card, SharedString::from(format!("{group}-fx-{}", state.seq)), state, FADE, move |el, p| {
            let glow = if p > 0. {
                vec![BoxShadow {
                    color: border_hover.opacity(GLOW_ALPHA * p),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(GLOW_BLUR),
                    spread_radius: px(0.),
                    inset: false,
                }]
            } else {
                Vec::new()
            };
            el.border_color(border_hover.opacity(p)).bg(surface.opacity(0.4 + 0.2 * p)).shadow(glow)
        })
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
        assert_eq!(accent_of(&playlist), None, "playlist không màu: nơi vẽ dùng màu nhấn của theme");
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
