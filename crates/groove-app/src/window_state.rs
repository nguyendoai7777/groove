//! groove-gd1_27: lưu vị trí cửa sổ (5.7).
//!
//! `app.rs` chỉ gọi một lần [`restore`], trước `open_window`: nó sửa `WindowOptions` theo `Settings::window` và đăng ký lưu
//! lại khi cửa sổ đổi. Không đặt vị trí sau khi mở vì GPUI không có lệnh dời cửa sổ, và mở rồi mới dời thì thấy cửa sổ nhảy.
//!
//! - **Lưu**: mỗi lần đổi vị trí, cỡ, phóng to thì sửa `Settings::window` trong bộ nhớ; `JsonSaver` gom lần ghi đĩa và xả khi
//!   thoát. Không chỉ lưu lúc đóng: nút đóng trên thanh trên ở nền khác Windows đi `remove_window`, không qua
//!   `on_window_should_close`.
//! - **Mở**: hình học quyết định, id màn chỉ để ưu tiên. Trên Windows `DisplayId` là `HMONITOR`, đổi sau khi khởi động lại,
//!   nên id không thấy chưa có nghĩa là màn đã mất. Vùng kéo trên cùng còn nằm trên một màn thì giữ, không thì về giữa màn chính.
//! - **Wayland** không cho đặt vị trí: chỉ giữ cỡ.
//! - `--display` của bản chạy thử thắng cài đặt và không ghi đè cài đặt: cửa sổ thử phải ở đúng màn được chỉ.

use groove_core::settings::WindowPlacement;
use materiant::gpui_kit::{
    App, Bounds, Context, DisplayId, Pixels, Size, Window, WindowBounds, WindowOptions, point, px, size,
};

use crate::app::{self, Shell};

/// Bản phát hành: trùng tên `packaging/linux/groove.desktop`, dock Linux ghép cửa sổ với icon theo `app_id`.
pub const RELEASE_APP_ID: &str = "groove";
/// Bản debug (dev-app-id, 03/11/2026): mã riêng để dock, ô tìm, Mission Center coi là app khác — mục `groove.dev.desktop` do
/// `start.sh` ghi không đè mục của gói đã cài. Dấu chấm, không gạch ngang: Mission Center tách tên scope
/// `app-<launcher>-<id>-<số>.scope` theo `-`.
pub const DEV_APP_ID: &str = "groove.dev";
pub const APP_ID: &str = if cfg!(debug_assertions) { DEV_APP_ID } else { RELEASE_APP_ID };
/// Cao của vùng kéo cửa sổ (thanh trên, đặc tả 1.1).
const DRAG_STRIP_HEIGHT: f32 = 36.;
/// Phần vùng kéo tối thiểu phải nằm trên màn để người dùng còn nắm được cửa sổ.
const MIN_VISIBLE_WIDTH: f32 = 100.;
const MIN_VISIBLE_HEIGHT: f32 = 16.;
/// Cỡ nhỏ nhất khi `WindowOptions` không khai.
const FALLBACK_MIN_SIZE: (f32, f32) = (1., 1.);

/// Áp vị trí đã lưu vào `options` và đăng ký lưu lại. Gọi một lần, trước `open_window`.
pub fn restore(mut options: WindowOptions, cx: &mut App) -> WindowOptions {
    options.app_id = Some(APP_ID.into());
    if options.display_id.is_some() {
        return options;
    }
    if let Some(placement) = app::settings(cx).window.clone() {
        let displays: Vec<_> = cx.displays().iter().map(|d| (u64::from(d.id()), d.bounds())).collect();
        let primary = cx.primary_display().map(|d| u64::from(d.id()));
        let min = options.window_min_size.unwrap_or(size(px(FALLBACK_MIN_SIZE.0), px(FALLBACK_MIN_SIZE.1)));
        if let Some(planned) = plan(&placement, &displays, primary, can_position(), min) {
            let display = planned.display.map(DisplayId::from);
            let bounds = match planned.origin {
                Some(origin) => Bounds::new(origin, planned.size),
                None => Bounds::centered(display, planned.size, cx),
            };
            options.display_id = display;
            options.window_bounds =
                Some(if planned.maximized { WindowBounds::Maximized(bounds) } else { WindowBounds::Windowed(bounds) });
        }
    }
    track::<Shell>(cx, |placement, cx| {
        if app::settings(cx).window.as_ref() != Some(&placement) {
            app::update_settings(cx, |s| s.window = Some(placement));
        }
    });
    options
}

/// Theo dõi cửa sổ chứa view `V` đầu tiên được tạo. Tách khỏi [`restore`] để test không cần cài đặt thật.
fn track<V: 'static>(cx: &mut App, save: fn(WindowPlacement, &mut App)) {
    cx.observe_new::<V>(move |_, window, cx: &mut Context<V>| {
        let Some(window) = window else { return };
        cx.observe_window_bounds(window, move |_, window, cx| save(current(window, cx), cx)).detach();
        // Alt+F4, đóng từ taskbar. Lần đổi gần nhất đã được lưu ở trên; đây là lưới an toàn cho lần đổi chưa kịp báo.
        window.on_window_should_close(cx, move |window, cx| {
            save(current(window, cx), cx);
            true
        });
    })
    .detach();
}

/// Chỗ mở lại cửa sổ cỡ cố định `size` (trình phát thu nhỏ) theo vị trí đã lưu: luật như cửa sổ chính (vùng kéo trên cùng còn
/// trên một màn). `None` khi chưa lưu, số vô nghĩa, vị trí không còn trên màn nào, hoặc nền không đặt được vị trí (Wayland):
/// người gọi tự chọn chỗ.
pub fn restore_fixed(
    saved: Option<&WindowPlacement>,
    size: Size<Pixels>,
    cx: &App,
) -> Option<(Bounds<Pixels>, Option<DisplayId>)> {
    let saved = WindowPlacement { width: size.width.as_f32(), height: size.height.as_f32(), ..saved?.clone() };
    let displays: Vec<_> = cx.displays().iter().map(|d| (u64::from(d.id()), d.bounds())).collect();
    let planned = plan_fixed(&saved, &displays, can_position(), size)?;
    Some((Bounds::new(planned.origin?, planned.size), planned.display.map(DisplayId::from)))
}

fn plan_fixed(
    saved: &WindowPlacement,
    displays: &[(u64, Bounds<Pixels>)],
    positionable: bool,
    size: Size<Pixels>,
) -> Option<Planned> {
    if !positionable {
        return None;
    }
    // Không có màn chính dự phòng: vị trí không giữ được thì `origin` là `None`, người gọi đặt chỗ mặc định của nó.
    plan(saved, displays, None, true, size).filter(|planned| planned.origin.is_some())
}

/// Vị trí hiện tại của `window` để lưu (bounds khi không phóng to, màn đang chứa).
pub fn current(window: &Window, cx: &App) -> WindowPlacement {
    let display_id = window.display(cx).map(|d| u64::from(d.id()));
    placement_of(window.window_bounds(), display_id)
}

/// Bounds lưu luôn là bounds khi không phóng to, để bỏ phóng to ở lần mở sau về đúng chỗ cũ.
fn placement_of(bounds: WindowBounds, display_id: Option<u64>) -> WindowPlacement {
    let (rect, maximized) = match bounds {
        WindowBounds::Windowed(rect) | WindowBounds::Fullscreen(rect) => (rect, false),
        WindowBounds::Maximized(rect) => (rect, true),
    };
    WindowPlacement {
        x: rect.origin.x.as_f32(),
        y: rect.origin.y.as_f32(),
        width: rect.size.width.as_f32(),
        height: rect.size.height.as_f32(),
        maximized,
        display_id,
    }
}

/// Wayland: GPUI chọn Wayland theo đúng biến này (`gpui-pre/src/platform.rs`).
fn can_position() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("WAYLAND_DISPLAY").is_none_or(|v| v.is_empty())
}

#[derive(Debug, PartialEq)]
struct Planned {
    /// `None`: để giữa `display`.
    origin: Option<materiant::gpui_kit::Point<Pixels>>,
    size: Size<Pixels>,
    /// `None`: màn mặc định.
    display: Option<u64>,
    maximized: bool,
}

/// Quyết định mở ở đâu. `None` khi số đã lưu vô nghĩa (tệp bị sửa tay): dùng mặc định của app.
fn plan(
    saved: &WindowPlacement,
    displays: &[(u64, Bounds<Pixels>)],
    primary: Option<u64>,
    positionable: bool,
    min: Size<Pixels>,
) -> Option<Planned> {
    let numbers = [saved.x, saved.y, saved.width, saved.height];
    if numbers.iter().any(|n| !n.is_finite()) || saved.width <= 0. || saved.height <= 0. {
        return None;
    }
    let wanted = size(px(saved.width), px(saved.height)).max(&min);
    let fit = |display: Option<u64>| {
        let area = displays.iter().find(|(id, _)| Some(*id) == display).map(|(_, b)| b.size);
        // Màn nhỏ hơn cỡ nhỏ nhất thì cỡ nhỏ nhất thắng: GPUI cũng không cho nhỏ hơn.
        area.map_or(wanted, |area| wanted.min(&area).max(&min))
    };
    let host = positionable.then(|| host_display(saved, wanted, displays)).flatten();
    Some(match host {
        Some(id) => Planned {
            origin: Some(point(px(saved.x), px(saved.y))),
            size: fit(Some(id)),
            display: Some(id),
            maximized: saved.maximized,
        },
        None if positionable => {
            Planned { origin: None, size: fit(primary), display: primary, maximized: saved.maximized }
        }
        None => Planned { origin: None, size: wanted, display: None, maximized: saved.maximized },
    })
}

/// Màn chứa vùng kéo của cửa sổ: màn đã lưu nếu nó còn giữ được, không thì màn chứa nhiều nhất.
fn host_display(saved: &WindowPlacement, wanted: Size<Pixels>, displays: &[(u64, Bounds<Pixels>)]) -> Option<u64> {
    let strip = Bounds::new(point(px(saved.x), px(saved.y)), size(wanted.width, px(DRAG_STRIP_HEIGHT)));
    let visible = |area: &Bounds<Pixels>| {
        let overlap = strip.intersect(area).size;
        (overlap.width >= px(MIN_VISIBLE_WIDTH) && overlap.height >= px(MIN_VISIBLE_HEIGHT))
            .then_some(overlap.width.as_f32() * overlap.height.as_f32())
    };
    let same_id = displays.iter().find(|(id, area)| Some(*id) == saved.display_id && visible(area).is_some());
    same_id.map(|(id, _)| *id).or_else(|| {
        displays
            .iter()
            .filter_map(|(id, area)| visible(area).map(|shown| (*id, shown)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use gpui::TestAppContext;
    use materiant::gpui_kit::{self, AppContext, IntoElement, Render, div};

    use super::*;

    const MIN: (f32, f32) = (560., 360.);

    fn min() -> Size<Pixels> {
        size(px(MIN.0), px(MIN.1))
    }

    fn area(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn saved(x: f32, y: f32, width: f32, height: f32, display_id: Option<u64>) -> WindowPlacement {
        WindowPlacement { x, y, width, height, maximized: false, display_id }
    }

    /// Màn 1 chính 1920 × 1080, màn 2 bên phải 2560 × 1440.
    fn two_displays() -> Vec<(u64, Bounds<Pixels>)> {
        vec![(1, area(0., 0., 1920., 1080.)), (2, area(1920., 0., 2560., 1440.))]
    }

    fn plan_on_two(placement: &WindowPlacement) -> Planned {
        plan(placement, &two_displays(), Some(1), true, min()).unwrap()
    }

    /// pip — cửa sổ cỡ cố định: về đúng chỗ đã lưu; ra ngoài mọi màn hay Wayland thì
    /// để người gọi tự chọn chỗ.
    #[test]
    fn fixed_window_returns_to_its_place_or_lets_the_caller_choose() {
        let pip = size(px(420.), px(140.));
        let planned = plan_fixed(&saved(2500., 1200., 420., 140., Some(2)), &two_displays(), true, pip).unwrap();
        assert_eq!(planned.origin, Some(point(px(2500.), px(1200.))));
        assert_eq!(planned.display, Some(2));
        assert_eq!(planned.size, pip);
        assert!(plan_fixed(&saved(9000., 9000., 420., 140., Some(2)), &two_displays(), true, pip).is_none());
        assert!(plan_fixed(&saved(10., 10., 420., 140., Some(1)), &two_displays(), false, pip).is_none(), "Wayland");
    }

    /// C1 — màn còn, vùng kéo trong màn: về đúng chỗ, đúng màn.
    #[test]
    fn keeps_position_on_saved_display() {
        let planned = plan_on_two(&saved(2000., 100., 900., 700., Some(2)));
        assert_eq!(planned.origin, Some(point(px(2000.), px(100.))));
        assert_eq!(planned.size, size(px(900.), px(700.)));
        assert_eq!(planned.display, Some(2));
    }

    /// C2 — id đổi (HMONITOR sau khởi động lại) mà vị trí vẫn nằm trên màn: giữ chỗ cũ, lấy id mới.
    #[test]
    fn stale_display_id_keeps_position_when_it_is_still_on_screen() {
        let planned = plan_on_two(&saved(2000., 100., 900., 700., Some(99)));
        assert_eq!(planned.origin, Some(point(px(2000.), px(100.))));
        assert_eq!(planned.display, Some(2));
    }

    /// C3 — màn không còn: rút màn 2, cửa sổ đã lưu ở đó về giữa màn chính, giữ cỡ.
    #[test]
    fn missing_display_falls_back_to_center_of_primary() {
        let displays = &two_displays()[..1];
        let planned = plan(&saved(2000., 100., 900., 700., Some(2)), displays, Some(1), true, min()).unwrap();
        assert_eq!(planned.origin, None, "None là để giữa màn");
        assert_eq!(planned.display, Some(1));
        assert_eq!(planned.size, size(px(900.), px(700.)));
    }

    /// C4 — ngoài vùng: thanh kéo trên mép trên, hoặc chỉ lòi vài px ở mép dưới, phải.
    #[test]
    fn off_screen_falls_back_to_center() {
        for (x, y) in [(100., -200.), (100., 1075.), (4470., 100.), (-895., 100.), (-5000., -5000.), (1e30, 0.)] {
            let planned = plan_on_two(&saved(x, y, 900., 700., Some(1)));
            assert_eq!(planned.origin, None, "({x}, {y})");
            assert_eq!(planned.display, Some(1));
        }
    }

    /// C4 — sát biên vẫn nắm được: còn đúng 100 px ngang, 16 px dọc của vùng kéo thì giữ, thiếu 1 px thì không.
    #[test]
    fn visible_strip_boundary() {
        assert!(plan_on_two(&saved(-800., 100., 900., 700., None)).origin.is_some());
        assert!(plan_on_two(&saved(-801., 100., 900., 700., None)).origin.is_none());
        assert!(plan_on_two(&saved(100., -20., 900., 700., None)).origin.is_some());
        assert!(plan_on_two(&saved(100., -21., 900., 700., None)).origin.is_none());
    }

    /// C5 — cửa sổ vắt ngang hai màn: chọn màn đã lưu, id lạ thì màn chứa nhiều hơn.
    #[test]
    fn straddling_window_prefers_saved_then_largest() {
        let at = saved(1400., 100., 900., 700., Some(2));
        assert_eq!(plan_on_two(&at).display, Some(2));
        assert_eq!(plan_on_two(&WindowPlacement { display_id: None, ..at.clone() }).display, Some(1));
    }

    /// C6 — Wayland: chỉ giữ cỡ, không đặt vị trí, không chọn màn.
    #[test]
    fn wayland_keeps_only_size() {
        let planned = plan(&saved(2000., 100., 900., 700., Some(2)), &two_displays(), Some(1), false, min()).unwrap();
        assert_eq!(planned, Planned { origin: None, size: size(px(900.), px(700.)), display: None, maximized: false });
    }

    /// C7 — cỡ nhỏ hơn cỡ nhỏ nhất, lớn hơn màn, số vô nghĩa trong tệp cài đặt.
    #[test]
    fn sizes_are_clamped_and_garbage_is_ignored() {
        assert_eq!(plan_on_two(&saved(10., 10., 100., 100., Some(1))).size, min());
        assert_eq!(plan_on_two(&saved(10., 10., 9000., 9000., Some(1))).size, size(px(1920.), px(1080.)));
        for bad in [
            saved(f32::NAN, 0., 800., 600., None),
            saved(0., f32::INFINITY, 800., 600., None),
            saved(0., 0., 0., 600., None),
            saved(0., 0., 800., -1., None),
        ] {
            assert_eq!(plan(&bad, &two_displays(), Some(1), true, min()), None, "{bad:?}");
        }
    }

    /// C7 — không có màn nào (headless, Wayland lúc đầu): vẫn mở, không panic.
    #[test]
    fn no_display_still_plans() {
        let planned = plan(&saved(10., 10., 900., 700., None), &[], None, true, min()).unwrap();
        assert_eq!(planned.display, None);
        assert_eq!(planned.origin, None);
        assert_eq!(planned.size, size(px(900.), px(700.)));
    }

    /// C8 — phóng to, toàn màn: lưu bounds lúc chưa phóng to; lần sau mở lại phóng to.
    #[test]
    fn placement_keeps_restore_bounds() {
        let rect = area(10., 20., 900., 700.);
        let normal = placement_of(WindowBounds::Windowed(rect), Some(3));
        assert_eq!(normal, WindowPlacement { maximized: false, ..saved(10., 20., 900., 700., Some(3)) });
        let zoomed = placement_of(WindowBounds::Maximized(rect), Some(3));
        assert_eq!(zoomed, WindowPlacement { maximized: true, ..saved(10., 20., 900., 700., Some(3)) });
        assert!(!placement_of(WindowBounds::Fullscreen(rect), None).maximized, "toàn màn không nhớ");
        assert!(plan_on_two(&zoomed).maximized);
    }

    /// C8 — màn đã mất mà đang phóng to: vẫn phóng to, trên màn chính.
    #[test]
    fn maximized_survives_fallback() {
        let at = WindowPlacement { maximized: true, ..saved(9000., 100., 900., 700., Some(2)) };
        let planned = plan_on_two(&at);
        assert!(planned.maximized);
        assert_eq!(planned.display, Some(1));
    }

    struct Marker;
    impl Render for Marker {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    thread_local! {
        static SAVED: RefCell<Vec<WindowPlacement>> = const { RefCell::new(Vec::new()) };
    }

    /// C9 — cửa sổ thật (ảo) mở qua `gpui_kit::open_window`: đổi cỡ thì lưu, kèm id màn.
    #[gpui::test]
    fn resize_reports_placement(cx: &mut TestAppContext) {
        SAVED.with_borrow_mut(Vec::clear);
        cx.update(|cx| {
            materiant::init(cx);
            track::<Marker>(cx, |placement, _| SAVED.with_borrow_mut(|s| s.push(placement)));
        });
        let handle =
            cx.update(|cx| gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Marker))).unwrap().0;
        cx.run_until_parked();
        cx.simulate_window_resize(handle, size(px(1000.), px(640.)));
        cx.run_until_parked();
        let last = SAVED.with_borrow(|s| s.last().cloned()).expect("đổi cỡ phải báo");
        assert_eq!((last.width, last.height), (1000., 640.));
        assert!(!last.maximized);
        let display = cx.update(|cx| cx.primary_display().map(|d| u64::from(d.id())));
        assert_eq!(last.display_id, display);
    }
}
