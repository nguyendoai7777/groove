//! groove-gd1_07, C12: đỉnh RAM với ảnh lớn. Tệp test riêng vì bộ đếm cấp phát là toàn cục của tiến trình.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};

use groove_library::covers::{CoverSize, CoverStore};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use tempfile::TempDir;

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: chuyển nguyên yêu cầu cho `System`, chỉ thêm đếm số byte.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(current, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: usize = 1024 * 1024;

#[test]
fn large_cover_costs_about_one_decode_and_is_released() {
    const SIDE: u32 = 4000;
    let decoded = (SIDE * SIDE * 3) as usize;
    let image = RgbImage::from_fn(SIDE, SIDE, |x, y| Rgb([(x / 16) as u8, (y / 16) as u8, ((x ^ y) / 16) as u8]));
    let mut jpeg = Vec::new();
    DynamicImage::ImageRgb8(image).write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg).unwrap();

    let dir = TempDir::new().unwrap();
    let store = CoverStore::new(dir.path().join("covers"));

    let before = CURRENT.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let record = store.save(&jpeg).unwrap();
    let peak = PEAK.load(Ordering::Relaxed) - before;
    let retained = CURRENT.load(Ordering::Relaxed).saturating_sub(before);
    println!("decoded {} MiB, peak {} MiB, retained {retained} bytes", decoded / MIB, peak / MIB);

    assert!(store.path(&record.hash, CoverSize::Large).is_file());
    assert_eq!(record.width, Some(SIDE));
    assert!(peak < decoded + 16 * MIB, "peak {peak} bytes over a {decoded} bytes decode");
    assert!(retained < 64 * 1024, "retained {retained} bytes");
}
