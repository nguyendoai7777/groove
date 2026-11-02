//! groove-gd1_14: tiêu đề chạy ngang khi tràn (đặc tả 1.3, bảng "Hiệu ứng web không có ở GPUI").
//!
//! Chỉ chạy khi chữ rộng hơn khung; ngược lại là một dòng cắt `…`, không xin khung hình nào. Đo chữ ở `prepaint` của một
//! `canvas` phủ khung (lúc đó kiểu chữ của cha đã áp), nên nhịp vẽ đầu tiên còn là dòng tĩnh. OS bật giảm chuyển động thì
//! đứng yên, cắt `…`. Rê chuột lên thì dừng tại chỗ, rời ra chạy tiếp (web: `animation-play-state: paused`).
//!
//! Chạy: một `canvas` tự vẽ dòng chữ đã shape hai lần, cách nhau [`GAP`], trong mặt nạ đúng khung, lệch theo đồng hồ
//! [`CYCLE`] (12 s một vòng như web) và xin khung kế theo nhịp màn hình. chrome-nav (02/11/2026): bản trước dời `margin-left`
//! bằng `with_animation` + `with_max_fps(30)` — layout GPUI làm tròn theo pixel thiết bị nên chữ nhảy từng pixel nguyên, hẹn
//! giờ 33 ms còn chờ thêm tới một nhịp vsync: đo ở desktop ẩn chữ chỉ đổi chỗ 19–20 lần / s, trông lờ đờ. Vẽ thẳng bằng
//! `ShapedLine::paint` thì glyph đặt theo 1/4 px, mỗi nhịp màn hình một lần (đo ~57–62 lần / s mỗi dòng).
//!
//! ui-audit (02/11/2026): chạy liên tục là vẽ lại CẢ cửa sổ mỗi nhịp màn hình (GPUI không vẽ lại từng phần). Đo ở desktop
//! ẩn, bản release: ~40 % một nhân lúc tên chạy, ~1 % lúc đứng yên. Nên tên chỉ chạy khi cửa sổ đang active (đang được nhìn,
//! có focus); cửa sổ nền, thu nhỏ thì dừng tại chỗ như khi rê chuột, active lại chạy tiếp từ đó.
//!
//! Kiểu chữ (cỡ, đậm, màu) do phần tử cha đặt.

use std::time::{Duration, Instant};

// `#[derive(IntoElement)]` trỏ crate `gpui`; bản không test chỉ có `materiant::gpui_kit`.
use materiant::gpui_kit as gpui;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ContentMask, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, TextAlign, Window,
    canvas, div, point, px, transparent_black,
};

/// Một vòng chạy hết (đặc tả: 12 s).
pub const CYCLE: Duration = Duration::from_secs(12);
/// Khoảng trống giữa hai bản chữ.
pub const GAP: f32 = 48.;

/// Chữ rộng hơn khung thì chạy. Khung chưa đo (0) thì chưa chạy.
pub fn overflows(text_width: f32, box_width: f32) -> bool {
    box_width > 0. && text_width > box_width + 0.5
}

/// Độ lệch trái (px, âm) ở tiến độ `progress` của một vòng: cuối vòng đúng một bản chữ + khoảng trống, nối liền đầu vòng.
pub fn offset(progress: f32, text_width: f32) -> f32 {
    -progress.clamp(0., 1.) * (text_width + GAP)
}

/// Tiến độ trong vòng (0 ≤ p < 1) sau `elapsed` đã chạy.
pub fn progress_at(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f64() / CYCLE.as_secs_f64()).fract() as f32
}

/// Đồng hồ chạy / dừng được: thời gian đã chạy = phần đã cộng dồn + từ lúc chạy lại.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    banked: Duration,
    since: Option<Instant>,
}

impl Clock {
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.banked + self.since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }

    pub fn run(&mut self, now: Instant) {
        self.since.get_or_insert(now);
    }

    pub fn pause(&mut self, now: Instant) {
        if let Some(since) = self.since.take() {
            self.banked += now.saturating_duration_since(since);
        }
    }

    pub fn reset(&mut self) {
        *self = Clock::default();
    }
}

#[derive(Default)]
struct State {
    box_width: f32,
    text_width: f32,
    hovered: bool,
    text: SharedString,
    clock: Clock,
}

#[derive(IntoElement)]
pub struct Marquee {
    id: ElementId,
    text: SharedString,
}

impl Marquee {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Marquee {
        Marquee { id: id.into(), text: text.into() }
    }
}

impl RenderOnce for Marquee {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let now = cx.background_executor().now();
        let reduce_motion = cx.reduce_motion();
        let window_active = window.is_window_active();
        let (animate, running) = state.update(cx, |s, _| {
            if s.text != self.text {
                // Bài mới chạy lại từ đầu dòng.
                s.text = self.text.clone();
                s.clock.reset();
            }
            let animate = overflows(s.text_width, s.box_width) && !reduce_motion;
            let running = animate && !s.hovered && window_active;
            if running {
                s.clock.run(now);
            } else if animate {
                s.clock.pause(now);
            } else {
                s.clock.reset();
            }
            (animate, running)
        });
        if running {
            // Khung kế theo nhịp màn hình; xin lúc dựng (view đang dựng là view chứa dòng chữ).
            window.request_animation_frame();
        }

        let measure = {
            let (state, text) = (state.clone(), self.text.clone());
            canvas(
                move |bounds, window, cx| {
                    let style = window.text_style();
                    let size = style.font_size.to_pixels(window.rem_size());
                    let run = style.to_run(text.len());
                    let line = window.text_system().shape_line(text, size, &[run], None);
                    let (box_width, text_width) = (f32::from(bounds.size.width), f32::from(line.width));
                    let changed = {
                        let s = state.read(cx);
                        (s.box_width - box_width).abs() > 0.5 || (s.text_width - text_width).abs() > 0.5
                    };
                    if changed {
                        state.update(cx, |s, _| {
                            s.box_width = box_width;
                            s.text_width = text_width;
                        });
                        // `notify` giữa lúc vẽ không xin khung mới: bài vừa khôi phục (chưa phát, chuột không động) thì tên cứ
                        // đứng yên cắt `…` tới lần vẽ lại kế (ui-audit đo ở desktop ẩn). Xin thẳng khung kế cho view này.
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
        };

        // Đang chạy: canvas vẽ hai bản chữ; dòng tĩnh bên dưới chỉ giữ chiều cao, chữ trong suốt.
        let painter = animate.then(|| {
            let (state, text) = (state.clone(), self.text.clone());
            canvas(
                move |_, window, _| {
                    // Kiểu chữ lấy ở `prepaint`, như phần đo: cùng cỡ, đậm, màu với dòng tĩnh.
                    let style = window.text_style();
                    let size = style.font_size.to_pixels(window.rem_size());
                    let run = style.to_run(text.len());
                    (window.text_system().shape_line(text, size, &[run], None), window.line_height())
                },
                move |bounds, (line, line_height), window, cx| {
                    let elapsed = state.read(cx).clock.elapsed(cx.background_executor().now());
                    let text_width = f32::from(line.width);
                    let shift = offset(progress_at(elapsed), text_width);
                    let y = bounds.origin.y + (bounds.size.height - line_height) / 2.;
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        for copy in [0., 1.] {
                            let x = bounds.origin.x + px(shift + copy * (text_width + GAP));
                            if let Err(err) = line.paint(point(x, y), line_height, TextAlign::Left, None, window, cx) {
                                log::warn!("marquee paint: {err:#}");
                            }
                        }
                    });
                },
            )
            // Không đặt mép thì phần tử `absolute` đứng ở chỗ nó sẽ nằm trong luồng — dưới dòng tĩnh, ngoài khung cắt.
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        });

        div()
            .id(self.id)
            .relative()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .on_hover(move |hovered: &bool, _, cx| {
                state.update(cx, |s, cx| {
                    if s.hovered != *hovered {
                        s.hovered = *hovered;
                        cx.notify();
                    }
                });
            })
            .child(measure)
            .child(div().truncate().when(animate, |d| d.text_color(transparent_black())).child(self.text))
            .children(painter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_only_when_text_is_wider_than_the_box() {
        assert!(overflows(300., 200.));
        assert!(!overflows(150., 200.));
        assert!(!overflows(200.2, 200.), "lệch nửa px không tính là tràn");
        assert!(!overflows(300., 0.), "khung chưa đo");
    }

    #[test]
    fn offset_loops_seamlessly() {
        assert_eq!(offset(0., 300.), 0.);
        // Cuối vòng đúng một bản chữ cộng khoảng trống: bản thứ hai nằm chỗ bản thứ nhất lúc đầu.
        assert_eq!(offset(1., 300.), -(300. + GAP));
        assert_eq!(offset(2., 300.), offset(1., 300.), "kẹp ở cuối vòng");
        assert_eq!(offset(-1., 300.), 0.);
        assert!(offset(0.5, 300.) < 0.);
    }

    #[test]
    fn progress_wraps_every_cycle() {
        assert_eq!(progress_at(Duration::ZERO), 0.);
        assert!((progress_at(CYCLE / 4) - 0.25).abs() < 1e-6);
        assert!(progress_at(CYCLE) < 1e-6, "hết vòng quay về đầu");
        assert!((progress_at(CYCLE * 3 + CYCLE / 2) - 0.5).abs() < 1e-6);
    }

    /// Rê chuột dừng tại chỗ, rời ra chạy tiếp từ đó; đổi bài thì về đầu.
    #[test]
    fn clock_pauses_and_resumes() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut clock = Clock::default();
        assert_eq!(clock.elapsed(at(500)), Duration::ZERO, "chưa chạy");
        clock.run(at(0));
        clock.run(at(100));
        assert_eq!(clock.elapsed(at(300)), Duration::from_millis(300), "chạy lại khi đang chạy không dời mốc");
        clock.pause(at(300));
        assert_eq!(clock.elapsed(at(5_000)), Duration::from_millis(300), "đứng yên khi dừng");
        clock.run(at(5_000));
        assert_eq!(clock.elapsed(at(5_200)), Duration::from_millis(500));
        clock.reset();
        assert_eq!(clock, Clock::default());
    }
}
