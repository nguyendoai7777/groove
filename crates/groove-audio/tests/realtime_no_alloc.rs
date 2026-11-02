//! groove-gd1_10: callback thực thời không được cấp phát. Đếm thật bằng bộ cấp phát toàn cục của riêng tệp test này.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use groove_audio::output::{OutputConfig, renderer_for_alloc_test};

struct Counting;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ENABLED.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn render_does_not_allocate() {
    let config = OutputConfig { sample_rate: 48_000, channels: 2 };
    let mut renderer = renderer_for_alloc_test(config, &vec![0.5; 8000]);
    let mut out = vec![0.0f32; 2048];

    ENABLED.store(true, Ordering::Relaxed);
    // Đủ các nhánh: có dữ liệu, fade, rồi thiếu dữ liệu.
    for _ in 0..16 {
        renderer.render(&mut out);
    }
    ENABLED.store(false, Ordering::Relaxed);

    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
