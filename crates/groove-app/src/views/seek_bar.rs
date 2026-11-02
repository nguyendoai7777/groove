//! groove-gd1_14: thanh tua chia đoạn (đặc tả 1.3, S20).
//!
//! Một hàng: giờ hiện tại, thanh, thời lượng. Thanh là một `canvas` vẽ từng khúc theo `timeline::segments_to_bar` (tỉ lệ theo
//! thời lượng, cách [`GAP`]; timeline không bắt đầu từ 0 thì có khúc ẩn danh phía trước) và đổi chuột sang thời gian bằng
//! `timeline::x_to_time` trên đúng danh sách khúc đã vẽ, nên nơi bấm khớp nơi vẽ. Cao [`REST`] px, rê chuột [`HOT`], khúc đang
//! rê [`ACTIVE`]; rê lên khúc có tên thì hiện nhãn `<tên> (<đầu> - <cuối>)`.
//!
//! Nhấn rồi kéo chỉ đổi vị trí xem trước ([`Scrub`]); `Player::seek` gọi đúng một lần khi thả chuột (S20): kéo không đặt lại
//! `currentTime` mỗi `mousemove`, không ghi phiên liên tục. Giờ bên trái hiện vị trí xem trước lúc đang kéo; không nhấp nháy
//! mỗi giây như bản web (tốn khung hình liên tục, đặc tả mục 4).
//!
//! Không có thời lượng (chưa có bài, tệp chưa biết độ dài) thì thanh chỉ để xem, bấm không làm gì.

use groove_core::timeline::{self, BarSegment, Segment};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    Bounds, Context, CursorStyle, DispatchPhase, Entity, HitboxBehavior, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString, Styled, Subscription, Window, canvas,
    deferred, div, fill, point, px, size,
};
use materiant::theme::{ActiveTheme, Neutral};

use crate::player::{self, Player};

/// Khe giữa hai khúc.
pub const GAP: f32 = 2.;
/// Tỉ lệ cỡ của thanh tua và thanh âm lượng so với mặc định của materiant (4 / 6 px): mảnh hơn để nhìn thanh thoát, hai
/// thanh cùng một kiểu. Thanh âm lượng truyền số này vào `Slider::scale`.
pub const SLIDER_SCALE: f32 = 0.85;
/// Bề dày thanh lúc nghỉ / khi rê vào thanh / khúc đang rê.
pub const REST: f32 = 4. * SLIDER_SCALE;
pub const HOT: f32 = 6. * SLIDER_SCALE;
pub const ACTIVE: f32 = 8. * SLIDER_SCALE;
/// Vùng bấm cao hơn thanh để dễ trúng.
pub const HIT_HEIGHT: f32 = 16.;
/// Bề ngang chừa cho mỗi nhãn giờ.
const TIME_WIDTH: f32 = 48.;

/// Trạng thái kéo: vị trí xem trước, `None` khi không kéo. Hàm thuần để test không cần cửa sổ.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Scrub {
    time: Option<f64>,
}

impl Scrub {
    pub fn preview(&self) -> Option<f64> {
        self.time
    }

    pub fn is_dragging(&self) -> bool {
        self.time.is_some()
    }

    /// Nhấn chuột trái tại `x`: bắt đầu kéo, chưa tua.
    pub fn press(&mut self, bar: &[BarSegment], x: f32) {
        self.time = Some(timeline::x_to_time(bar, x));
    }

    /// Di chuột trong lúc kéo: chỉ đổi xem trước.
    pub fn drag(&mut self, bar: &[BarSegment], x: f32) {
        if self.time.is_some() {
            self.time = Some(timeline::x_to_time(bar, x));
        }
    }

    /// Thả chuột: kết thúc kéo, trả thời điểm cần tua (một lần). Không kéo thì `None`.
    pub fn release(&mut self, bar: &[BarSegment], x: f32) -> Option<f64> {
        self.time.take().map(|_| timeline::x_to_time(bar, x))
    }
}

/// Khúc dưới `x`. Khe giữa hai khúc tính cho khúc bên trái; ngoài hai đầu thì khúc gần nhất.
pub fn segment_at(bar: &[BarSegment], x: f32) -> Option<usize> {
    let last = bar.len().checked_sub(1)?;
    Some(bar.iter().position(|item| x <= item.x + item.width + GAP / 2.).unwrap_or(last))
}

/// Nhãn khi rê: `<tên> (<đầu> - <cuối>)`. Khúc ẩn danh (phần trước đoạn đầu, hoặc không có timeline) không có nhãn.
pub fn segment_label(segments: &[Segment], item: &BarSegment) -> Option<String> {
    let segment = segments.get(item.segment?)?;
    Some(format!("{} ({} - {})", segment.title, timeline::format_time(item.start), timeline::format_time(item.end)))
}

/// Bề dày của khúc `ix` ở trạng thái rê hiện tại.
pub fn thickness(ix: usize, hovered: Option<usize>, bar_hot: bool) -> f32 {
    if hovered == Some(ix) {
        ACTIVE
    } else if bar_hot {
        HOT
    } else {
        REST
    }
}

pub struct SeekBar {
    player: Entity<Player>,
    scrub: Scrub,
    /// Vị trí chuột trong thanh khi rê hoặc kéo.
    cursor_x: Option<f32>,
    /// Bề ngang thanh ở lần vẽ gần nhất, để đặt nhãn.
    width: f32,
    _observe: Subscription,
}

impl SeekBar {
    pub fn new(cx: &mut Context<Self>) -> SeekBar {
        let player = player::player(cx);
        let observe = cx.observe(&player, |_, _, cx| cx.notify());
        SeekBar { player, scrub: Scrub::default(), cursor_x: None, width: 0., _observe: observe }
    }

    #[cfg(test)]
    /// Đang kéo (cho test).
    pub fn is_dragging(&self) -> bool {
        self.scrub.is_dragging()
    }

    fn set_cursor(&mut self, x: Option<f32>, cx: &mut Context<Self>) {
        if self.cursor_x != x {
            self.cursor_x = x;
            cx.notify();
        }
    }

    fn press(&mut self, bar: &[BarSegment], x: f32, cx: &mut Context<Self>) {
        self.scrub.press(bar, x);
        self.cursor_x = Some(x);
        cx.notify();
    }

    fn drag(&mut self, bar: &[BarSegment], x: f32, cx: &mut Context<Self>) {
        self.scrub.drag(bar, x);
        self.cursor_x = Some(x);
        cx.notify();
    }

    fn release(&mut self, bar: &[BarSegment], x: f32, hovered: bool, cx: &mut Context<Self>) {
        if let Some(time) = self.scrub.release(bar, x) {
            self.player.update(cx, |player, cx| player.seek(time, cx));
        }
        if !hovered {
            self.cursor_x = None;
        }
        cx.notify();
    }
}

fn time_label(text: String, color: materiant::gpui_kit::Hsla, right: bool) -> impl IntoElement {
    let label = div().w(px(TIME_WIDTH)).flex_none().text_xs().text_color(color).child(text);
    if right { label.text_left() } else { label.text_right() }
}

impl Render for SeekBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let (duration, position, segments) = {
            let player = self.player.read(cx);
            (player.duration_sec(), player.position_sec(), player.timeline().to_vec())
        };
        let enabled = duration > 0.;
        let shown = self.scrub.preview().unwrap_or(position).clamp(0., duration.max(0.));
        let dragging = self.scrub.is_dragging();
        let view = cx.entity();

        // Nhãn: dựng từ bề ngang lần vẽ trước, đủ chính xác vì chỉ để đọc.
        let label = self.cursor_x.filter(|_| enabled && self.width > 0.).and_then(|x| {
            let bar = timeline::segments_to_bar(&segments, duration, self.width, GAP);
            let item = bar.get(segment_at(&bar, x)?)?;
            Some((x, segment_label(&segments, item)?))
        });

        let track = canvas(
            {
                let (segments, view) = (segments.clone(), view.clone());
                move |bounds: Bounds<Pixels>, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    let width = f32::from(bounds.size.width);
                    view.update(cx, |this, _| this.width = width);
                    let layout = timeline::segments_to_bar(&segments, duration, width, GAP);
                    (hitbox, layout)
                }
            },
            {
                let (view, track_color, fill_color) = (view, colors.layer(Neutral::N300), colors.primary);
                move |bounds, (hitbox, layout), window, cx| {
                    let (cursor_x, scrubbing) = {
                        let this = view.read(cx);
                        (this.cursor_x, this.scrub.is_dragging())
                    };
                    let bar_hot = cursor_x.is_some();
                    let hovered = cursor_x.and_then(|x| segment_at(&layout, x));
                    let (track_color, fill_color) =
                        if enabled { (track_color, fill_color) } else { (track_color.opacity(0.6), fill_color) };
                    for (ix, item) in layout.iter().enumerate() {
                        let height = px(thickness(ix, hovered, bar_hot));
                        let top = bounds.origin.y + (bounds.size.height - height) / 2.;
                        let frame = Bounds::new(point(bounds.origin.x + px(item.x), top), size(px(item.width), height));
                        window.paint_quad(fill(frame, track_color).corner_radii(height / 2.));
                        let done = if enabled { item.fill(shown) } else { 0. };
                        if done > 0. {
                            let part = Bounds::new(frame.origin, size(px(item.width * done), height));
                            window.paint_quad(fill(part, fill_color).corner_radii(height / 2.));
                        }
                    }
                    if !enabled {
                        return;
                    }
                    window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
                    let layout = std::rc::Rc::new(layout);
                    let local =
                        move |position: materiant::gpui_kit::Point<Pixels>| f32::from(position.x - bounds.origin.x);

                    window.on_mouse_event({
                        let (view, layout, hitbox) = (view.clone(), layout.clone(), hitbox.clone());
                        move |event: &MouseDownEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble
                                && event.button == MouseButton::Left
                                && hitbox.is_hovered(window)
                            {
                                view.update(cx, |this, cx| this.press(&layout, local(event.position), cx));
                            }
                        }
                    });
                    window.on_mouse_event({
                        let (view, layout, hitbox) = (view.clone(), layout.clone(), hitbox.clone());
                        move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let x = local(event.position);
                            if scrubbing || view.read(cx).scrub.is_dragging() {
                                view.update(cx, |this, cx| this.drag(&layout, x, cx));
                            } else if hitbox.is_hovered(window) {
                                view.update(cx, |this, cx| this.set_cursor(Some(x), cx));
                            } else {
                                view.update(cx, |this, cx| this.set_cursor(None, cx));
                            }
                        }
                    });
                    window.on_mouse_event({
                        let (view, layout, hitbox) = (view.clone(), layout.clone(), hitbox.clone());
                        move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                                let (x, hovered) = (local(event.position), hitbox.is_hovered(window));
                                view.update(cx, |this, cx| this.release(&layout, x, hovered, cx));
                            }
                        }
                    });
                }
            },
        )
        .absolute()
        .size_full();

        let bar = div()
            .id("seek-bar-track")
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(HIT_HEIGHT))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && !this.scrub.is_dragging() {
                    this.set_cursor(None, cx);
                }
            }))
            .child(track)
            .children(label.map(|(x, text)| {
                let right_half = x > self.width / 2.;
                let anchor = div().absolute().bottom(px(HIT_HEIGHT));
                let anchor =
                    if right_half { anchor.right(px((self.width - x).max(0.))) } else { anchor.left(px(x.max(0.))) };
                deferred(
                    anchor
                        .max_w(px(280.))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.surface)
                        .text_xs()
                        .text_color(colors.text)
                        .whitespace_nowrap()
                        .child(SharedString::from(text)),
                )
                .with_priority(1)
            }));

        div()
            .id("seek-bar")
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .child(time_label(timeline::format_time(shown), colors.muted, false))
            .child(bar)
            .child(time_label(timeline::format_time(duration), colors.muted, true))
            .when(dragging, |row| row.cursor_pointer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, end: f64, title: &str) -> Segment {
        Segment { start, end, title: title.into() }
    }

    #[test]
    fn scrub_seeks_only_on_release() {
        let bar = timeline::segments_to_bar(&[], 100., 400., GAP);
        let mut scrub = Scrub::default();
        assert_eq!(scrub.release(&bar, 50.), None, "thả khi chưa nhấn thì không tua");
        scrub.press(&bar, 100.);
        assert_eq!(scrub.preview(), Some(25.));
        scrub.drag(&bar, 200.);
        assert_eq!(scrub.preview(), Some(50.));
        scrub.drag(&bar, 9999.);
        assert_eq!(scrub.preview(), Some(100.), "kéo ra ngoài thì kẹp ở cuối");
        assert_eq!(scrub.release(&bar, 300.), Some(75.));
        assert!(!scrub.is_dragging());
        scrub.drag(&bar, 10.);
        assert_eq!(scrub.preview(), None, "di chuột không nhấn không bắt đầu kéo");
    }

    /// S20: timeline bắt đầu ở 30 s: khúc ẩn danh phủ 0..30, nơi bấm khớp nơi vẽ.
    #[test]
    fn click_matches_what_is_drawn_when_timeline_starts_late() {
        let segments = [seg(30., 60., "Verse"), seg(60., 100., "Chorus")];
        let bar = timeline::segments_to_bar(&segments, 100., 1000., 0.);
        assert_eq!(bar.len(), 3);
        assert_eq!(bar[0].segment, None);
        assert_eq!(timeline::x_to_time(&bar, 150.), 15., "x = 150 nằm trong khúc ẩn danh 0..300");
        assert_eq!(timeline::x_to_time(&bar, 300.), 30.);
        assert_eq!(timeline::x_to_time(&bar, 750.), 75.);
        assert_eq!(timeline::time_to_x(&bar, 75.), 750.);
    }

    #[test]
    fn label_names_the_segment_under_the_cursor() {
        let segments = [seg(0., 65., "Intro"), seg(65., 3725., "Main")];
        let bar = timeline::segments_to_bar(&segments, 3725., 800., GAP);
        let first = segment_at(&bar, 1.).unwrap();
        assert_eq!(segment_label(&segments, &bar[first]).as_deref(), Some("Intro (0:00 - 1:05)"));
        let last = segment_at(&bar, 799.).unwrap();
        assert_eq!(segment_label(&segments, &bar[last]).as_deref(), Some("Main (1:05 - 1:02:05)"));
        // Ngoài hai đầu: khúc gần nhất.
        assert_eq!(segment_at(&bar, -5.), Some(0));
        assert_eq!(segment_at(&bar, 5000.), Some(1));
    }

    #[test]
    fn anonymous_bars_have_no_label() {
        let whole = timeline::segments_to_bar(&[], 100., 400., GAP);
        assert_eq!(segment_label(&[], &whole[0]), None, "không có timeline");
        let segments = [seg(50., 100., "B")];
        let bar = timeline::segments_to_bar(&segments, 100., 400., GAP);
        assert_eq!(segment_label(&segments, &bar[0]), None, "khúc ẩn danh phía trước");
        assert_eq!(segment_at(&[], 3.), None);
    }

    #[test]
    fn rails_are_thinner_than_the_materiant_default() {
        assert!((REST - 3.4).abs() < 1e-5 && (HOT - 5.1).abs() < 1e-5 && (ACTIVE - 6.8).abs() < 1e-5);
    }

    #[test]
    fn thickness_follows_hover() {
        assert_eq!(thickness(0, None, false), REST);
        assert_eq!(thickness(0, None, true), HOT);
        assert_eq!(thickness(0, Some(1), true), HOT);
        assert_eq!(thickness(1, Some(1), true), ACTIVE);
    }
}
