//! groove-gd1_12: phân tích timeline, segments_to_bar, x_to_time (S20).
//!
//! Vẽ thanh tua, rê chuột và tua đều đi qua cùng một `Vec<BarSegment>`, nên nơi bấm luôn là nơi vẽ. Giây là `f64`, pixel là `f32`
//! theo GPUI.

/// Một đoạn timeline. `end` là đầu đoạn sau, đoạn cuối hết ở thời lượng (đặc tả 1.6).
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub title: String,
}

/// Phân tích văn bản timeline. Mỗi dòng `<thời gian> <tên>`, dòng khác bỏ qua, sắp xếp theo đầu đoạn (giữ thứ tự viết khi trùng).
pub fn parse(text: &str, duration_sec: f64) -> Vec<Segment> {
    let mut lines: Vec<(f64, &str)> = text.trim_start_matches('\u{feff}').lines().filter_map(parse_line).collect();
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));

    let duration = sane_duration(duration_sec);
    let starts: Vec<f64> = lines.iter().map(|line| line.0).collect();
    lines
        .into_iter()
        .enumerate()
        .map(|(ix, (start, title))| {
            let next_start = starts.get(ix + 1).copied().unwrap_or(duration);
            Segment { start, end: next_start.max(start), title: title.to_owned() }
        })
        .collect()
}

// Tương đương regex `^([\d:]+)\s+(.+)$` của bản cũ trên dòng đã trim.
fn parse_line(line: &str) -> Option<(f64, &str)> {
    let line = line.trim();
    let (time, title) = line.split_once(char::is_whitespace)?;
    let title = title.trim_start();
    if title.is_empty() {
        return None;
    }
    Some((parse_time(time)?, title))
}

/// Giây, `m:ss` hoặc `h:mm:ss`. Chỉ chữ số ASCII; không chặn phút, giây quá 59 như bản cũ (`1:75` là 135 giây).
pub fn parse_time(text: &str) -> Option<f64> {
    let mut total = 0u64;
    let mut parts = 0;
    for part in text.split(':') {
        parts += 1;
        if parts > 3 || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        total = total.checked_mul(60)?.checked_add(part.parse().ok()?)?;
    }
    Some(total as f64)
}

/// `m:ss` hoặc `h:mm:ss`, cắt phần lẻ; giá trị rỗng, âm, không hữu hạn là `0:00` (đặc tả 1.3).
pub fn format_time(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return "0:00".to_owned();
    }
    let total = secs as u64;
    let (hours, minutes, seconds) = (total / 3600, total % 3600 / 60, total % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

/// Chỉ số đoạn chứa thời điểm: đầu đoạn tính, cuối đoạn không, trừ đoạn cuối tính cả. Trước đoạn đầu là `None`.
pub fn active_index(segments: &[Segment], time_sec: f64) -> Option<usize> {
    let last = segments.len().checked_sub(1)?;
    segments.iter().enumerate().position(|(ix, seg)| {
        time_sec >= seg.start && if ix == last { time_sec <= seg.end } else { time_sec < seg.end }
    })
}

/// Một khúc trên thanh tua.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarSegment {
    /// Mép trái, tính từ đầu thanh.
    pub x: f32,
    pub width: f32,
    pub start: f64,
    pub end: f64,
    /// Chỉ số trong danh sách `Segment`. `None` là khúc không có tên: đoạn ẩn danh từ 0 (S20), hoặc cả thanh khi không có timeline.
    pub segment: Option<usize>,
}

impl BarSegment {
    /// Phần đã phát của khúc, `0.0..=1.0`, để tô.
    pub fn fill(&self, time_sec: f64) -> f32 {
        if time_sec <= self.start {
            return 0.0;
        }
        if time_sec >= self.end {
            return 1.0;
        }
        ((time_sec - self.start) / (self.end - self.start)) as f32
    }
}

/// Chia thanh tua rộng `width` thành từng khúc, tỉ lệ theo thời lượng, cách nhau `gap`. Timeline không bắt đầu từ 0 thì thêm
/// khúc ẩn danh phía trước (S20). Đoạn rỗng hoặc nằm ngoài thời lượng không có khúc. Không có timeline thì một khúc.
///
/// Thời lượng chưa biết vẫn ra một khúc để vẽ, `x_to_time` khi đó luôn là 0. Rộng không dương thì danh sách rỗng.
pub fn segments_to_bar(segments: &[Segment], duration_sec: f64, width: f32, gap: f32) -> Vec<BarSegment> {
    if !width.is_finite() || width <= 0.0 {
        return Vec::new();
    }
    let duration = sane_duration(duration_sec);

    // (start, end, segment) đã cắt theo thời lượng, phủ kín 0..duration trừ chỗ timeline tự để hở.
    let mut spans: Vec<(f64, f64, Option<usize>)> = Vec::with_capacity(segments.len() + 1);
    for (ix, seg) in segments.iter().enumerate() {
        let (start, end) = (seg.start.max(0.0), seg.end.min(duration));
        if end > start {
            spans.push((start, end, Some(ix)));
        }
    }
    let first_start = spans.first().map_or(duration, |span| span.0);
    if first_start > 0.0 {
        spans.insert(0, (0.0, first_start, None));
    }
    if spans.is_empty() {
        return vec![BarSegment { x: 0.0, width, start: 0.0, end: 0.0, segment: None }];
    }

    // Quá chật thì bỏ khoảng cách, không để khúc có bề rộng âm.
    let gaps = (spans.len() - 1) as f32 * gap.max(0.0);
    let (gap, usable) = if gaps < width { (gap.max(0.0), width - gaps) } else { (0.0, width) };

    let mut x = 0.0f32;
    let mut bar: Vec<BarSegment> = spans
        .into_iter()
        .map(|(start, end, segment)| {
            let seg_width = ((end - start) / duration) as f32 * usable;
            let item = BarSegment { x, width: seg_width, start, end, segment };
            x += seg_width + gap;
            item
        })
        .collect();
    // Làm tròn f32 không được để mép phải lệch khỏi thanh.
    if let Some(last) = bar.last_mut() {
        last.width = (width - last.x).max(0.0);
    }
    bar
}

/// Thời điểm tại `x`, dùng chung cho tua và nhãn khi rê chuột. Rơi vào khe giữa hai khúc thì lấy đầu khúc sau; ngoài thanh thì
/// kẹp về hai đầu.
pub fn x_to_time(bar: &[BarSegment], x: f32) -> f64 {
    for item in bar {
        if x < item.x {
            return item.start;
        }
        if x <= item.x + item.width {
            if item.width <= 0.0 {
                return item.start;
            }
            let ratio = f64::from((x - item.x) / item.width);
            return item.start + ratio * (item.end - item.start);
        }
    }
    bar.last().map_or(0.0, |item| item.end)
}

/// Ngược lại của `x_to_time`, để đặt vị trí con trỏ. Thời điểm rơi vào khoảng timeline để hở thì lấy mép trái khúc sau.
pub fn time_to_x(bar: &[BarSegment], time_sec: f64) -> f32 {
    for item in bar {
        if time_sec <= item.start {
            return item.x;
        }
        if time_sec < item.end {
            return item.x + item.fill(time_sec) * item.width;
        }
    }
    bar.last().map_or(0.0, |item| if item.end > item.start { item.x + item.width } else { item.x })
}

fn sane_duration(duration_sec: f64) -> f64 {
    if duration_sec.is_finite() && duration_sec > 0.0 { duration_sec } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: f32 = 2.0;

    fn seg(start: f64, end: f64, title: &str) -> Segment {
        Segment { start, end, title: title.to_owned() }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn parses_every_time_format_and_ends_at_the_next_start() {
        let segments = parse("0 Mở đầu\n1:05 Đoạn hai\n1:00:00 Đoạn ba\n", 3700.0);
        assert_eq!(segments, [seg(0.0, 65.0, "Mở đầu"), seg(65.0, 3600.0, "Đoạn hai"), seg(3600.0, 3700.0, "Đoạn ba")]);
    }

    #[test]
    fn sorts_by_start_and_ignores_other_lines() {
        let text = "\u{feff}2:00 Sau\r\n\r\nkhông phải dòng hợp lệ\n1:00\n  0:30   Trước  \n1:2:3:4 Bốn phần\n:30 Thiếu\n1a Sai\n";
        let segments = parse(text, 180.0);
        assert_eq!(segments, [seg(30.0, 120.0, "Trước"), seg(120.0, 180.0, "Sau")]);
    }

    #[test]
    fn title_keeps_inner_spaces_and_tab_separates_too() {
        assert_eq!(parse("0:10\tA  B", 20.0), [seg(10.0, 20.0, "A  B")]);
    }

    #[test]
    fn empty_text_has_no_segments() {
        assert!(parse("", 100.0).is_empty());
        assert!(parse(" \n\n", 100.0).is_empty());
    }

    #[test]
    fn unknown_duration_does_not_give_negative_or_nan_ends() {
        for duration in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            assert_eq!(parse("1:00 A", duration), [seg(60.0, 60.0, "A")]);
        }
    }

    #[test]
    fn start_past_the_duration_keeps_end_not_before_start() {
        assert_eq!(parse("5:00 A", 60.0), [seg(300.0, 300.0, "A")]);
    }

    #[test]
    fn parse_time_rejects_what_is_not_a_time() {
        assert_eq!(parse_time("75"), Some(75.0));
        assert_eq!(parse_time("1:75"), Some(135.0));
        assert_eq!(parse_time("01:02:03"), Some(3723.0));
        for bad in ["", ":", "1:", ":1", "1::2", "a", "1.5", "-1", "1:2:3:4", "٣", "99999999999999999999"] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn format_time_matches_the_old_app() {
        for (secs, text) in [
            (0.0, "0:00"),
            (-1.0, "0:00"),
            (f64::NAN, "0:00"),
            (f64::INFINITY, "0:00"),
            (5.9, "0:05"),
            (65.0, "1:05"),
            (3599.9, "59:59"),
            (3600.0, "1:00:00"),
            (3725.0, "1:02:05"),
        ] {
            assert_eq!(format_time(secs), text);
        }
    }

    #[test]
    fn active_index_follows_the_old_boundaries() {
        let segments = parse("0:10 A\n0:20 B", 30.0);
        assert_eq!(active_index(&segments, 5.0), None);
        assert_eq!(active_index(&segments, 10.0), Some(0));
        assert_eq!(active_index(&segments, 19.9), Some(0));
        assert_eq!(active_index(&segments, 20.0), Some(1));
        assert_eq!(active_index(&segments, 30.0), Some(1), "đoạn cuối tính cả mép phải");
        assert_eq!(active_index(&segments, 31.0), None);
        assert_eq!(active_index(&[], 1.0), None);
    }

    #[test]
    fn no_timeline_is_one_whole_bar() {
        let bar = segments_to_bar(&[], 100.0, 500.0, GAP);
        assert_eq!(bar, [BarSegment { x: 0.0, width: 500.0, start: 0.0, end: 100.0, segment: None }]);
        assert!(near(x_to_time(&bar, 250.0), 50.0));
    }

    #[test]
    fn s20_timeline_not_starting_at_zero_gets_an_anonymous_lead_segment() {
        // Bản cũ không vẽ phần trước đoạn đầu mà tính vị trí theo cả bề ngang: nơi bấm khác nơi vẽ.
        let segments = parse("0:30 A\n1:00 B", 100.0);
        let bar = segments_to_bar(&segments, 100.0, 1000.0, 0.0);
        assert_eq!(bar.len(), 3);
        assert_eq!(bar[0].segment, None);
        assert_eq!((bar[0].start, bar[0].end), (0.0, 30.0));
        assert_eq!(bar[1].segment, Some(0));
        assert_eq!(bar[2].segment, Some(1));
        assert!(near(f64::from(bar[1].x), 300.0), "đoạn A vẽ đúng ở 30 % thanh");
        assert!(near(x_to_time(&bar, 300.0), 30.0), "bấm ở mép đoạn A ra đúng đầu đoạn A");
        assert!(near(x_to_time(&bar, 150.0), 15.0));
    }

    #[test]
    fn s20_clicking_where_a_segment_is_drawn_lands_in_that_segment_despite_gaps() {
        // Bốn khúc, khe 2 px: công thức x / width của bản cũ lệch dần về cuối thanh.
        let segments = parse("0:00 A\n0:10 B\n0:50 C\n0:55 D", 100.0);
        let bar = segments_to_bar(&segments, 100.0, 400.0, GAP);
        for item in &bar {
            let middle = item.x + item.width / 2.0;
            let time = x_to_time(&bar, middle);
            assert!(near(time, (item.start + item.end) / 2.0), "{item:?} -> {time}");
            assert_eq!(active_index(&segments, time), item.segment);
        }
    }

    #[test]
    fn bar_fills_the_exact_width_with_the_requested_gaps() {
        let segments = parse("0:00 A\n0:33 B\n1:07 C", 100.0);
        let bar = segments_to_bar(&segments, 100.0, 777.0, GAP);
        assert_eq!(bar.len(), 3);
        assert_eq!(bar[0].x, 0.0);
        for pair in bar.windows(2) {
            assert!((pair[1].x - (pair[0].x + pair[0].width) - GAP).abs() < 0.001);
        }
        let last = bar[2];
        assert_eq!(last.x + last.width, 777.0);
        let widths: f32 = bar.iter().map(|item| item.width).sum();
        assert!((widths - (777.0 - 2.0 * GAP)).abs() < 0.01);
        // Tỉ lệ theo thời lượng.
        assert!((bar[0].width / bar[1].width - 33.0 / 34.0).abs() < 0.001);
    }

    #[test]
    fn x_in_a_gap_snaps_to_the_next_segment_and_outside_clamps() {
        let segments = parse("0:00 A\n0:50 B", 100.0);
        let bar = segments_to_bar(&segments, 100.0, 402.0, GAP);
        assert_eq!(x_to_time(&bar, bar[1].x - 1.0), 50.0);
        assert_eq!(x_to_time(&bar, -10.0), 0.0);
        assert_eq!(x_to_time(&bar, 1000.0), 100.0);
    }

    #[test]
    fn x_to_time_never_goes_backwards_along_the_bar() {
        let segments = parse("0:07 A\n0:07 Trùng\n0:40 B\n0:41 C", 90.0);
        let bar = segments_to_bar(&segments, 90.0, 300.0, GAP);
        let mut last = 0.0;
        for step in 0..=3000 {
            let time = x_to_time(&bar, step as f32 / 10.0);
            assert!(time >= last, "x = {}", step as f32 / 10.0);
            assert!((0.0..=90.0).contains(&time));
            last = time;
        }
        assert_eq!(last, 90.0);
    }

    #[test]
    fn time_to_x_round_trips_with_x_to_time() {
        let segments = parse("0:20 A\n0:50 B", 100.0);
        let bar = segments_to_bar(&segments, 100.0, 500.0, GAP);
        for time in [0.0, 10.0, 20.0, 35.0, 50.0, 75.0, 100.0] {
            assert!(near(x_to_time(&bar, time_to_x(&bar, time)), time), "{time}");
        }
        assert_eq!(time_to_x(&bar, -1.0), 0.0);
        assert_eq!(time_to_x(&bar, 500.0), 500.0);
    }

    #[test]
    fn duplicate_starts_and_segments_past_the_duration_get_no_bar_segment() {
        let segments = parse("0:00 A\n0:00 Trùng\n1:00 B\n9:00 Ngoài", 120.0);
        let bar = segments_to_bar(&segments, 120.0, 300.0, GAP);
        assert_eq!(bar.iter().map(|item| item.segment).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(bar[1].end, 120.0, "đoạn B bị cắt ở thời lượng");
    }

    #[test]
    fn timeline_entirely_past_the_duration_is_one_anonymous_segment() {
        let segments = parse("5:00 A", 60.0);
        let bar = segments_to_bar(&segments, 60.0, 300.0, GAP);
        assert_eq!(bar, [BarSegment { x: 0.0, width: 300.0, start: 0.0, end: 60.0, segment: None }]);
    }

    #[test]
    fn unknown_duration_is_one_segment_and_time_stays_zero() {
        let segments = parse("0:10 A", 0.0);
        for duration in [0.0, f64::NAN, -1.0] {
            let bar = segments_to_bar(&segments, duration, 300.0, GAP);
            assert_eq!(bar.len(), 1);
            assert_eq!(x_to_time(&bar, 150.0), 0.0);
            assert_eq!(time_to_x(&bar, 5.0), 0.0);
        }
    }

    #[test]
    fn degenerate_width_is_safe() {
        let segments = parse("0:00 A\n0:50 B", 100.0);
        for width in [0.0, -5.0, f32::NAN, f32::INFINITY] {
            let bar = segments_to_bar(&segments, 100.0, width, GAP);
            assert!(bar.is_empty());
            assert_eq!(x_to_time(&bar, 10.0), 0.0);
            assert_eq!(time_to_x(&bar, 10.0), 0.0);
        }
    }

    #[test]
    fn bar_narrower_than_its_gaps_drops_them() {
        let text: String = (0..100).map(|i| format!("{i} Đoạn\n")).collect();
        let segments = parse(&text, 100.0);
        let bar = segments_to_bar(&segments, 100.0, 50.0, GAP);
        assert_eq!(bar.len(), 100);
        assert!(bar.iter().all(|item| item.width > 0.0));
        let last = bar[99];
        assert!((last.x + last.width - 50.0).abs() < 0.001);
    }

    #[test]
    fn fill_is_clamped_and_handles_an_empty_span() {
        let item = BarSegment { x: 0.0, width: 10.0, start: 10.0, end: 20.0, segment: Some(0) };
        assert_eq!(item.fill(5.0), 0.0);
        assert_eq!(item.fill(15.0), 0.5);
        assert_eq!(item.fill(25.0), 1.0);
        let empty = BarSegment { x: 0.0, width: 10.0, start: 0.0, end: 0.0, segment: None };
        assert_eq!(empty.fill(0.0), 0.0, "chưa biết thời lượng thì thanh không đầy");
    }
}
