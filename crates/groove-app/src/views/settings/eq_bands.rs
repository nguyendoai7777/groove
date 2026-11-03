//! groove-gd1_24: mười cột dọc và thanh bass của bảng EQ, vẽ và nhận chuột bằng `canvas` (dáng của `setting-general.vue`).
//!
//! Cột: cùng dáng thanh bass dựng đứng — rãnh 2 px mờ suốt chiều cao, phần tô màu nhấn cùng bề dày từ đáy lên tới núm, núm
//! tròn 10 px, vầng sáng quanh núm khi rê, kéo hay có focus bàn phím. Không vẽ đường nối các núm (chủ dự án bỏ, 03/11/2026). Bấm chỗ nào
//! trong cột cũng đặt giá trị tại đó rồi kéo tiếp được (như bấm rãnh của thanh trượt); đang kéo thì chuột ra ngoài khung
//! vẫn theo. Mọi đổi giá trị đi qua `EqPanel::set_band` / `set_bass`, nên áp ngay tới player.
//!
//! Hình học (chuột ↔ giá trị) là hàm thuần để test, không cần cửa sổ.

use groove_core::settings::{BASS_BOOST_MAX_DB, EQ_BAND_COUNT, EQ_GAIN_LIMIT_DB};
use materiant::gpui_kit::{
    Bounds, DispatchPhase, Entity, HitboxBehavior, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Styled, Window, canvas, fill, point, px, size,
};
use materiant::gpui_kit::{CursorStyle, Hsla};
use materiant::theme::ActiveTheme;

use super::eq_panel::{BAND_STEP, BASS_STEP, EqPanel};

/// Núm 10 px, rãnh (và phần tô) 2 px, vầng 22 px — chung cho cột dải và thanh bass (web: `thumb-size 10`, `track-size 2`).
const THUMB_SIZE: f32 = 10.;
const RAIL_WIDTH: f32 = 2.;
const HALO_SIZE: f32 = 22.;
/// Vùng bấm cao của thanh bass.
pub const BASS_HIT_HEIGHT: f32 = 24.;
/// Nửa núm: núm không bao giờ ra khỏi cột, nên chừa chừng đó ở đỉnh và đáy.
const INSET: f32 = THUMB_SIZE / 2.;

/// Thứ đang được kéo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    Band(usize),
    Bass,
}

/// Cột dưới chuột theo hoành độ `x` trong khung rộng `width` (kẹp vào khoảng hợp lệ).
pub fn band_at(x: f32, width: f32) -> usize {
    if width <= 0. {
        return 0;
    }
    ((x / width * EQ_BAND_COUNT as f32).floor().max(0.) as usize).min(EQ_BAND_COUNT - 1)
}

/// Mức dB của tung độ `y` trong khung cao `height`: đỉnh (trừ `INSET`) là +10, đáy là -10, làm tròn theo nấc.
pub fn gain_at(y: f32, height: f32) -> f32 {
    let travel = (height - 2. * INSET).max(1.);
    let ratio = 1. - ((y - INSET) / travel).clamp(0., 1.);
    snap(-EQ_GAIN_LIMIT_DB + ratio * 2. * EQ_GAIN_LIMIT_DB, -EQ_GAIN_LIMIT_DB, BAND_STEP)
}

/// Mức bass boost của hoành độ `x` trong thanh rộng `width`.
pub fn bass_at(x: f32, width: f32) -> f32 {
    let travel = (width - 2. * INSET).max(1.);
    let ratio = ((x - INSET) / travel).clamp(0., 1.);
    snap(ratio * BASS_BOOST_MAX_DB, 0., BASS_STEP)
}

fn snap(value: f32, min: f32, step: f32) -> f32 {
    min + ((value - min) / step).round() * step
}

/// Tung độ núm của mức `gain` trong khung cao `height` (nghịch đảo của `gain_at`).
fn thumb_y(gain: f32, height: f32) -> f32 {
    let ratio = (gain.clamp(-EQ_GAIN_LIMIT_DB, EQ_GAIN_LIMIT_DB) + EQ_GAIN_LIMIT_DB) / (2. * EQ_GAIN_LIMIT_DB);
    INSET + (1. - ratio) * (height - 2. * INSET).max(0.)
}

fn circle(window: &mut Window, center_x: Pixels, center_y: Pixels, diameter: f32, color: Hsla) {
    let half = px(diameter / 2.);
    let frame = Bounds::new(point(center_x - half, center_y - half), size(px(diameter), px(diameter)));
    window.paint_quad(fill(frame, color).corner_radii(half));
}

/// Chung cho cả hai canvas: gắn ba bộ nghe chuột (nhấn, di, thả). `pick` đổi vị trí chuột (so với góc khung) thành thứ được
/// kéo, hoặc `None` nếu không bấm trúng gì.
/// Vùng của một canvas: hitbox để biết chuột có đang trên nó, khung để đổi toạ độ, focus của bảng.
struct Area {
    hitbox: materiant::gpui_kit::Hitbox,
    bounds: Bounds<Pixels>,
    focus: materiant::gpui_kit::FocusHandle,
}

fn listen(
    window: &mut Window,
    view: &Entity<EqPanel>,
    area: Area,
    pick: impl Fn(f32, f32) -> Drag + Clone + 'static,
    apply: impl Fn(&mut EqPanel, Drag, f32, f32, &mut materiant::gpui_kit::Context<EqPanel>) + Clone + 'static,
    hover: impl Fn(&mut EqPanel, Option<Drag>, &mut materiant::gpui_kit::Context<EqPanel>) + Clone + 'static,
    owns: impl Fn(Drag) -> bool + Clone + 'static,
) {
    let Area { hitbox, bounds, focus } = area;
    let local = move |position: materiant::gpui_kit::Point<Pixels>| {
        (f32::from(position.x - bounds.origin.x), f32::from(position.y - bounds.origin.y))
    };
    window.on_mouse_event({
        let (view, hitbox, pick, apply) = (view.clone(), hitbox.clone(), pick.clone(), apply.clone());
        move |event: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left && hitbox.is_hovered(window) {
                window.focus(&focus, cx);
                let (x, y) = local(event.position);
                let target = pick(x, y);
                view.update(cx, |this, cx| {
                    this.set_drag(Some(target), cx);
                    apply(this, target, x, y, cx);
                });
            }
        }
    });
    window.on_mouse_event({
        let (view, hitbox, pick, apply) = (view.clone(), hitbox.clone(), pick, apply);
        let owns_move = owns.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let (x, y) = local(event.position);
            match view.read(cx).drag().filter(|drag| owns_move(*drag)) {
                Some(Drag::Band(band)) => view.update(cx, |this, cx| apply(this, Drag::Band(band), x, y, cx)),
                Some(Drag::Bass) => view.update(cx, |this, cx| apply(this, Drag::Bass, x, y, cx)),
                None if view.read(cx).drag().is_none() => {
                    let over = hitbox.is_hovered(window).then(|| pick(x, y));
                    view.update(cx, |this, cx| hover(this, over, cx));
                }
                None => {}
            }
        }
    });
    window.on_mouse_event({
        let view = view.clone();
        move |event: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                view.update(cx, |this, cx| {
                    if this.drag().is_some_and(&owns) {
                        this.set_drag(None, cx);
                    }
                });
            }
        }
    });
}

/// Khung mười cột (đặt `absolute` đúng vùng thanh, giữa hai hàng nhãn).
pub fn bands_canvas(view: Entity<EqPanel>) -> impl IntoElement {
    canvas(
        |bounds: Bounds<Pixels>, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox, window, cx| {
            let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let c = cx.theme().colors().clone();
            let (gains, drag, hover, key_band, focus) = {
                let panel = view.read(cx);
                (*panel.gains(), panel.drag(), panel.hover(), panel.key_band(), panel.focus().clone())
            };
            let keyboard_focus = materiant::style::focus_visible(&focus, window, cx);

            let column = width / EQ_BAND_COUNT as f32;
            let bottom = bounds.origin.y + px(height - INSET);
            for (band, gain) in gains.iter().enumerate() {
                let x = bounds.origin.x + px((band as f32 + 0.5) * column);
                let y = bounds.origin.y + px(thumb_y(*gain, height));
                let rail = Bounds::new(
                    point(x - px(RAIL_WIDTH / 2.), bounds.origin.y + px(INSET)),
                    size(px(RAIL_WIDTH), px(height - 2. * INSET)),
                );
                window.paint_quad(fill(rail, c.text.opacity(0.15)).corner_radii(px(RAIL_WIDTH / 2.)));
                let bar = Bounds::new(point(x - px(RAIL_WIDTH / 2.), y), size(px(RAIL_WIDTH), bottom - y));
                window.paint_quad(fill(bar, c.primary).corner_radii(px(RAIL_WIDTH / 2.)));
                let active = drag == Some(Drag::Band(band))
                    || (drag.is_none() && hover == Some(band))
                    || (keyboard_focus && key_band == band);
                if active {
                    circle(window, x, y, HALO_SIZE, c.primary.opacity(0.2));
                }
                circle(window, x, y, THUMB_SIZE, c.primary);
            }
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
            listen(
                window,
                &view,
                Area { hitbox, bounds, focus },
                move |x, _| Drag::Band(band_at(x, width)),
                move |this, target, _, y, cx| {
                    if let Drag::Band(band) = target {
                        this.set_key_band(band, cx);
                        this.set_band(band, gain_at(y, height), cx);
                    }
                },
                |this, over, cx| {
                    let band = over.and_then(|drag| if let Drag::Band(band) = drag { Some(band) } else { None });
                    this.set_hover(band, this.bass_hover(), cx);
                },
                |drag| matches!(drag, Drag::Band(_)),
            );
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Thanh bass ngang, lấp đầy ô cha (cao `BASS_HIT_HEIGHT`).
pub fn bass_canvas(view: Entity<EqPanel>) -> impl IntoElement {
    canvas(
        |bounds: Bounds<Pixels>, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox, window, cx| {
            let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let c = cx.theme().colors().clone();
            let (value, drag, hover, focus) = {
                let panel = view.read(cx);
                (panel.bass_boost(), panel.drag(), panel.bass_hover(), panel.focus().clone())
            };
            let travel = (width - 2. * INSET).max(0.);
            let ratio = (value / BASS_BOOST_MAX_DB).clamp(0., 1.);
            let center_y = bounds.origin.y + px(height / 2.);
            let thumb_x = bounds.origin.x + px(INSET + ratio * travel);
            let rail = Bounds::new(
                point(bounds.origin.x + px(INSET), center_y - px(RAIL_WIDTH / 2.)),
                size(px(travel), px(RAIL_WIDTH)),
            );
            window.paint_quad(fill(rail, c.text.opacity(0.15)).corner_radii(px(RAIL_WIDTH / 2.)));
            let done = Bounds::new(rail.origin, size(thumb_x - rail.origin.x, px(RAIL_WIDTH)));
            window.paint_quad(fill(done, c.primary).corner_radii(px(RAIL_WIDTH / 2.)));
            if drag == Some(Drag::Bass) || (drag.is_none() && hover) {
                circle(window, thumb_x, center_y, HALO_SIZE, c.primary.opacity(0.2));
            }
            circle(window, thumb_x, center_y, THUMB_SIZE, c.primary);
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
            listen(
                window,
                &view,
                Area { hitbox, bounds, focus },
                |_, _| Drag::Bass,
                move |this, _, x, _, cx| this.set_bass(bass_at(x, width), cx),
                |this, over, cx| this.set_hover(this.hover(), over.is_some(), cx),
                |drag| drag == Drag::Bass,
            );
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mười cột chia đều bề ngang; ngoài khung thì kẹp về cột đầu / cuối.
    #[test]
    fn band_at_splits_the_width_evenly_and_clamps() {
        assert_eq!(band_at(0., 500.), 0);
        assert_eq!(band_at(49., 500.), 0);
        assert_eq!(band_at(50., 500.), 1);
        assert_eq!(band_at(499., 500.), 9);
        assert_eq!(band_at(-30., 500.), 0);
        assert_eq!(band_at(900., 500.), 9);
        assert_eq!(band_at(10., 0.), 0);
    }

    /// Đỉnh vùng núm là +10, đáy là -10, giữa là 0, làm tròn về nấc 0,5; ngoài khung kẹp lại.
    #[test]
    fn gain_at_maps_height_to_decibels() {
        assert_eq!(gain_at(INSET, 240.), 10.);
        assert_eq!(gain_at(240. - INSET, 240.), -10.);
        assert_eq!(gain_at(120., 240.), 0.);
        assert_eq!(gain_at(-50., 240.), 10.);
        assert_eq!(gain_at(999., 240.), -10.);
        let value = gain_at(100., 240.);
        assert_eq!(value * 2., (value * 2.).round(), "{value} lệch nấc 0,5");
    }

    /// Núm vẽ và chỗ bấm dùng cùng một phép ánh xạ: bấm đúng tâm núm trả lại đúng mức.
    #[test]
    fn thumb_position_round_trips_through_the_pointer() {
        for gain in [-10., -7.5, -0.5, 0., 3., 9.5, 10.] {
            assert_eq!(gain_at(thumb_y(gain, 240.), 240.), gain, "{gain}");
        }
    }

    #[test]
    fn bass_at_runs_from_zero_to_the_limit_in_whole_steps() {
        assert_eq!(bass_at(0., 140.), 0.);
        assert_eq!(bass_at(70., 140.), 5.);
        assert_eq!(bass_at(140., 140.), BASS_BOOST_MAX_DB);
        assert_eq!(bass_at(-20., 140.), 0.);
        assert_eq!(bass_at(500., 140.), BASS_BOOST_MAX_DB);
    }
}
