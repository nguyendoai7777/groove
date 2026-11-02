//! groove-gd1_24: đường cong EQ (đặc tả 1.10, bảng "Dựng trong app").
//!
//! Vẽ sau các núm (`eq_bands` gọi `paint_curve` trước khi vẽ cột): spline nối tâm thanh của mười dải, theo `eq.gains` (không tính bass boost, không tính preamp,
//! như bản cũ). Phần hình học là hàm thuần để test; chỗ vẽ là `paint_curve`, không nhận chuột.

use groove_core::settings::{EQ_BAND_COUNT, EQ_GAIN_LIMIT_DB};
use materiant::gpui_kit::{Bounds, Hsla, PathBuilder, Pixels, Window, point, px};

/// Đường kính núm của thanh dải (web: `thumb-size 12`; ảnh mẫu to hơn chút).
pub const THUMB_SIZE: f32 = 14.;
/// Bề dày nét chính và nét quầng sáng dưới nó (web: 1.2 và 3 px).
const STROKE: f32 = 1.5;
const GLOW_STROKE: f32 = 4.;
/// Số điểm lấy mẫu giữa hai dải kề nhau.
const STEPS: usize = 10;

/// Nửa núm thanh trượt: núm không bao giờ ra khỏi cột, nên đường cong cũng chừa chừng đó ở đỉnh và đáy.
pub const INSET: f32 = THUMB_SIZE / 2.;

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

/// Vẽ đường cong trong `bounds`: quầng sáng mờ rồi nét chính (như hai `path` của bản web).
pub fn paint_curve(window: &mut Window, bounds: Bounds<Pixels>, gains: &[f32; EQ_BAND_COUNT], color: Hsla) {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let line = spline(&band_points(gains, width, height), height);
    let points: Vec<_> = line.into_iter().map(|(x, y)| bounds.origin + point(px(x), px(y))).collect();
    for (stroke, color) in [(GLOW_STROKE, color.opacity(0.25)), (STROKE, color)] {
        let mut builder = PathBuilder::stroke(px(stroke));
        builder.add_polygon(&points, false);
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
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
