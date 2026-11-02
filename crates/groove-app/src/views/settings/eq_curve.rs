//! groove-gd1_24: đường cong EQ (đặc tả 1.10, bảng "Dựng trong app").
//!
//! Vẽ đè lên hàng thanh trượt: spline nối tâm thanh của mười dải, theo `eq.gains` (không tính bass boost, không tính preamp,
//! như bản cũ). Phần hình học là hàm thuần để test; chỗ vẽ là một `canvas` không nhận chuột nên không chặn thanh trượt.

use groove_core::settings::{EQ_BAND_COUNT, EQ_GAIN_LIMIT_DB};
use materiant::gpui_kit::{
    App, Bounds, Hsla, IntoElement, PathBuilder, Pixels, RenderOnce, Styled, Window, canvas, point, px,
};
use materiant::slider::SLIDER_THUMB_SIZE;

/// Bề dày nét.
const STROKE: f32 = 2.5;
/// Số điểm lấy mẫu giữa hai dải kề nhau.
const STEPS: usize = 10;

/// Nửa núm thanh trượt: núm không bao giờ ra khỏi cột, nên đường cong cũng chừa chừng đó ở đỉnh và đáy.
pub const INSET: f32 = SLIDER_THUMB_SIZE / 2.;

/// Điểm nối của từng dải trong ô `width × height`: dải `i` ở tâm cột `i`, mức `+10` dB ở đỉnh (trừ `INSET`).
pub fn band_points(gains: &[f32; EQ_BAND_COUNT], width: f32, height: f32) -> Vec<(f32, f32)> {
    let travel = (height - 2. * INSET).max(0.);
    gains
        .iter()
        .enumerate()
        .map(|(band, gain)| {
            let ratio = (gain.clamp(-EQ_GAIN_LIMIT_DB, EQ_GAIN_LIMIT_DB) + EQ_GAIN_LIMIT_DB) / (2. * EQ_GAIN_LIMIT_DB);
            let x = (band as f32 + 0.5) * width / EQ_BAND_COUNT as f32;
            (x, INSET + (1. - ratio) * travel)
        })
        .collect()
}

/// Spline Catmull-Rom đi qua mọi điểm. Y bị kẹp trong `[INSET, height - INSET]` vì spline có thể vọt quá điểm nối.
pub fn spline(points: &[(f32, f32)], height: f32) -> Vec<(f32, f32)> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let (low, high) = (INSET.min(height / 2.), (height - INSET).max(height / 2.));
    let mut out = Vec::with_capacity((points.len() - 1) * STEPS + 1);
    for segment in 0..points.len() - 1 {
        let p0 = points[segment.saturating_sub(1)];
        let (p1, p2) = (points[segment], points[segment + 1]);
        let p3 = points[(segment + 2).min(points.len() - 1)];
        for step in 0..STEPS {
            let t = step as f32 / STEPS as f32;
            let (t2, t3) = (t * t, t * t * t);
            let along = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2. * b + (c - a) * t + (2. * a - 5. * b + 4. * c - d) * t2 + (3. * b - a - 3. * c + d) * t3)
            };
            out.push((along(p0.0, p1.0, p2.0, p3.0), along(p0.1, p1.1, p2.1, p3.1).clamp(low, high)));
        }
    }
    out.push(*points.last().expect("checked above"));
    out
}

/// Phần tử vẽ: đặt `absolute` đè lên hàng thanh trượt.
#[derive(IntoElement)]
pub struct EqCurve {
    gains: [f32; EQ_BAND_COUNT],
    color: Hsla,
}

impl EqCurve {
    pub fn new(gains: [f32; EQ_BAND_COUNT], color: Hsla) -> EqCurve {
        EqCurve { gains, color }
    }
}

impl RenderOnce for EqCurve {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (gains, color) = (self.gains, self.color);
        canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, _, window, _| {
                let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let line = spline(&band_points(&gains, width, height), height);
                let points: Vec<_> = line.into_iter().map(|(x, y)| bounds.origin + point(px(x), px(y))).collect();
                let mut builder = PathBuilder::stroke(px(STROKE));
                builder.add_polygon(&points, false);
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mức giữa là chính giữa ô, +10 ở đỉnh (trừ lề), -10 ở đáy; dải đầu, cuối cách mép nửa cột.
    #[test]
    fn band_points_map_gain_to_height() {
        let mut gains = [0.; EQ_BAND_COUNT];
        gains[0] = 10.;
        gains[9] = -10.;
        let points = band_points(&gains, 500., 200.);
        assert_eq!(points.len(), EQ_BAND_COUNT);
        assert_eq!(points[0], (25., INSET));
        assert_eq!(points[9], (475., 200. - INSET));
        assert_eq!(points[4].1, 100.);
        assert!(points.windows(2).all(|pair| pair[0].0 < pair[1].0), "x tăng dần");
    }

    /// Giá trị ngoài khoảng hay NaN không đẩy điểm ra khỏi ô.
    #[test]
    fn band_points_stay_inside_the_box() {
        let mut gains = [0.; EQ_BAND_COUNT];
        gains[0] = 99.;
        gains[1] = -99.;
        for (_, y) in band_points(&gains, 500., 200.) {
            assert!((INSET..=200. - INSET).contains(&y), "{y}");
        }
    }

    /// Spline đi qua đúng các điểm nối, và không vọt khỏi ô dù hai dải kề nhau chênh tối đa.
    #[test]
    fn spline_passes_through_points_and_is_clamped() {
        let mut gains = [0.; EQ_BAND_COUNT];
        gains[4] = 10.;
        gains[5] = -10.;
        let points = band_points(&gains, 500., 200.);
        let line = spline(&points, 200.);
        assert_eq!(line.len(), (EQ_BAND_COUNT - 1) * STEPS + 1);
        for (band, point) in points.iter().enumerate() {
            assert_eq!(line[band * STEPS].0, point.0);
            assert!((line[band * STEPS].1 - point.1).abs() < 1e-3);
        }
        assert!(line.iter().all(|(_, y)| (INSET..=200. - INSET).contains(y)));
    }

    #[test]
    fn spline_of_flat_gains_is_a_straight_line() {
        let line = spline(&band_points(&[0.; EQ_BAND_COUNT], 500., 200.), 200.);
        assert!(line.iter().all(|(_, y)| (y - 100.).abs() < 1e-3));
        assert_eq!(spline(&[], 200.), vec![]);
        assert_eq!(spline(&[(1., 2.)], 200.), vec![(1., 2.)]);
    }
}
