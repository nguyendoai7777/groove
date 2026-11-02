//! ui-player-bar: icon Play / Pause biến hình như `src/components/audio-controller/toggle-play.vue` của bản web.
//!
//! Bản web vẽ hai nửa bằng `clip-path: polygon(...)` bốn đỉnh trong ô vuông (toạ độ là viewBox 24 × 24 cũ tính theo %),
//! chuyển `0.25s cubic-bezier(0.4, 0, 0.2, 1)`. Đang phát: hai vạch đứng (x 6-10 và 14-18, y 4-20). Đang dừng: cùng hai
//! hình đó dồn thành hai nửa của tam giác, chia đôi ở giữa; đỉnh nhọn là một điểm lặp đôi để số đỉnh khớp nhau.
//!
//! Ở đây nội suy tuyến tính từng đỉnh theo thời gian rồi tô bằng một `Path` (hai đường con chung một path nên chỗ hai nửa
//! chạm nhau không lộ vệt khử răng cưa). Bấm giữa chừng thì chạy tiếp từ hình đang hiện, như CSS transition. OS bật giảm
//! chuyển động thì đổi hình tức thì.

use std::time::{Duration, Instant};

use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{Hsla, IntoElement, PathBuilder, Pixels, Point, canvas, point};

/// `transition: clip-path 0.25s` của web.
pub const DURATION: Duration = Duration::from_millis(250);
/// `cubic-bezier(0.4, 0, 0.2, 1)` của web.
const EASING: (f32, f32, f32, f32) = (0.4, 0., 0.2, 1.);

type Quad = [(f32, f32); 4];

// Toạ độ trong ô vuông cạnh 1, chép từ các biến `--bar-*`, `--tri-*` của `toggle-play.vue`.
const BAR_TOP: f32 = 4. / 24.;
const BAR_BOTTOM: f32 = 20. / 24.;
const TRI_LEFT: f32 = 0.25;
const TRI_MID: f32 = 0.5;
const TRI_RIGHT: f32 = 20. / 24.;
const TRI_MID_TOP: f32 = 0.309_52;
const TRI_MID_BOTTOM: f32 = 0.690_48;

/// Hai vạch pause (đang phát).
pub const PAUSE: [Quad; 2] = [
    [(TRI_LEFT, BAR_TOP), (10. / 24., BAR_TOP), (10. / 24., BAR_BOTTOM), (TRI_LEFT, BAR_BOTTOM)],
    [(14. / 24., BAR_TOP), (0.75, BAR_TOP), (0.75, BAR_BOTTOM), (14. / 24., BAR_BOTTOM)],
];

/// Tam giác play (đang dừng), hai nửa.
pub const PLAY: [Quad; 2] = [
    [(TRI_LEFT, BAR_TOP), (TRI_MID, TRI_MID_TOP), (TRI_MID, TRI_MID_BOTTOM), (TRI_LEFT, BAR_BOTTOM)],
    [(TRI_MID, TRI_MID_TOP), (TRI_RIGHT, TRI_MID), (TRI_RIGHT, TRI_MID), (TRI_MID, TRI_MID_BOTTOM)],
];

/// `cubic-bezier(x1, y1, x2, y2)` của CSS tại thời điểm `t` (0..1): tìm tham số có x = t bằng chia đôi rồi trả y.
pub fn cubic_bezier(t: f32, (x1, y1, x2, y2): (f32, f32, f32, f32)) -> f32 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let curve = |s: f32, a: f32, b: f32| {
        let inv = 1. - s;
        3. * inv * inv * s * a + 3. * inv * s * s * b + s * s * s
    };
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.;
        if curve(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve((lo + hi) / 2., y1, y2)
}

/// Hình ở mức `shape`: 0 là hai vạch pause, 1 là tam giác play.
pub fn shape_at(shape: f32) -> [Quad; 2] {
    let shape = shape.clamp(0., 1.);
    let mut out = PAUSE;
    for (quad, (from, to)) in out.iter_mut().zip(PAUSE.iter().zip(PLAY.iter())) {
        for (vertex, (a, b)) in quad.iter_mut().zip(from.iter().zip(to.iter())) {
            *vertex = (a.0 + (b.0 - a.0) * shape, a.1 + (b.1 - a.1) * shape);
        }
    }
    out
}

/// Trạng thái chuyển hình. `Copy` để chép vào closure vẽ.
#[derive(Clone, Copy, Debug)]
pub struct PlayMorph {
    from: f32,
    to: f32,
    started: Option<Instant>,
}

impl PlayMorph {
    pub fn new(playing: bool) -> PlayMorph {
        let shape = Self::target(playing);
        PlayMorph { from: shape, to: shape, started: None }
    }

    fn target(playing: bool) -> f32 {
        if playing { 0. } else { 1. }
    }

    /// Đổi trạng thái phát; trùng trạng thái đang hướng tới thì giữ nguyên. `instant` (giảm chuyển động) bỏ hiệu ứng.
    pub fn set_playing(&mut self, playing: bool, now: Instant, instant: bool) {
        let to = Self::target(playing);
        if to == self.to {
            return;
        }
        let current = self.shape(now);
        *self = if instant {
            PlayMorph { from: to, to, started: None }
        } else {
            PlayMorph { from: current, to, started: Some(now) }
        };
    }

    /// Mức hình lúc `now`: 0 vạch pause, 1 tam giác.
    pub fn shape(&self, now: Instant) -> f32 {
        let Some(started) = self.started else {
            return self.to;
        };
        let t = now.saturating_duration_since(started).as_secs_f32() / DURATION.as_secs_f32();
        if t >= 1. {
            return self.to;
        }
        self.from + (self.to - self.from) * cubic_bezier(t, EASING)
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.started.is_some_and(|started| now.saturating_duration_since(started) < DURATION)
    }

    /// Icon cạnh `size`, tô `color`. Đang chuyển thì xin khung kế (view chứa nó vẽ lại tới khi xong).
    pub fn icon(self, size: Pixels, color: Hsla) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let now = Instant::now();
                let quads = shape_at(self.shape(now));
                let at = |(x, y): (f32, f32)| -> Point<Pixels> {
                    point(bounds.origin.x + bounds.size.width * x, bounds.origin.y + bounds.size.height * y)
                };
                let mut path = PathBuilder::fill();
                for quad in quads {
                    path.move_to(at(quad[0]));
                    for &vertex in &quad[1..] {
                        path.line_to(at(vertex));
                    }
                    path.close();
                }
                match path.build() {
                    Ok(path) => window.paint_path(path, color),
                    Err(err) => log::warn!("play icon path: {err}"),
                }
                if self.is_animating(now) {
                    window.request_animation_frame();
                }
            },
        )
        .flex_none()
        .size(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn easing_matches_the_css_curve() {
        assert!(close(cubic_bezier(0., EASING), 0.));
        assert!(close(cubic_bezier(1., EASING), 1.));
        // Giá trị của trình duyệt cho cubic-bezier(0.4, 0, 0.2, 1) tại 0.5 là ~0.7743.
        assert!((cubic_bezier(0.5, EASING) - 0.7743).abs() < 2e-3, "{}", cubic_bezier(0.5, EASING));
        let mut last = 0.;
        for i in 1..=20 {
            let y = cubic_bezier(i as f32 / 20., EASING);
            assert!(y >= last, "đơn điệu");
            last = y;
        }
    }

    #[test]
    fn shapes_are_the_web_polygons() {
        assert_eq!(shape_at(0.), PAUSE);
        assert_eq!(shape_at(1.), PLAY);
        // Vạch trái x 6-10, phải 14-18 trên viewBox 24.
        assert!(close(PAUSE[0][1].0 * 24., 10.) && close(PAUSE[1][0].0 * 24., 14.) && close(PAUSE[1][1].0 * 24., 18.));
        // Tam giác: đỉnh nhọn lặp đôi ở giữa chiều cao, hai nửa chung cạnh x = 0.5.
        assert_eq!(PLAY[1][1], PLAY[1][2]);
        assert!(close(PLAY[1][1].1, 0.5));
        assert_eq!(PLAY[0][1], PLAY[1][0]);
        assert_eq!(PLAY[0][2], PLAY[1][3]);
        let half = shape_at(0.5);
        assert!(close(half[0][1].0, (10. / 24. + 0.5) / 2.));
    }

    #[test]
    fn morph_runs_250_ms_and_resumes_from_the_current_shape() {
        let t0 = Instant::now();
        let mut morph = PlayMorph::new(true);
        assert_eq!(morph.shape(t0), 0.);
        assert!(!morph.is_animating(t0));

        morph.set_playing(false, t0, false);
        assert!(morph.is_animating(t0));
        assert_eq!(morph.shape(t0), 0.);
        let mid = morph.shape(t0 + DURATION / 2);
        assert!(mid > 0.5 && mid < 1., "easing nhanh lúc đầu: {mid}");
        assert_eq!(morph.shape(t0 + DURATION), 1.);
        assert!(!morph.is_animating(t0 + DURATION));

        // Bấm lại giữa chừng: chạy ngược từ hình đang hiện, không nhảy.
        let t1 = t0 + DURATION / 2;
        morph.set_playing(true, t1, false);
        assert!(close(morph.shape(t1), mid));
        assert_eq!(morph.shape(t1 + DURATION), 0.);

        // Cùng trạng thái: không khởi động lại.
        let before = morph;
        morph.set_playing(true, t1 + DURATION / 4, false);
        assert_eq!(morph.started, before.started);
    }

    #[test]
    fn reduced_motion_switches_at_once() {
        let t0 = Instant::now();
        let mut morph = PlayMorph::new(true);
        morph.set_playing(false, t0, true);
        assert!(!morph.is_animating(t0));
        assert_eq!(morph.shape(t0), 1.);
    }
}
