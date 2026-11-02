//! groove-gd1_07: tệp bìa, accent (đặc tả 2.2, 5.2, S10, S13).
//!
//! Bìa nằm ở `<cache>/covers/<hash>_<256|512>.jpg`, DB chỉ giữ `covers(hash, accent_color, width, height)`.
//! `hash` là SHA-256 của byte ảnh gốc nên cùng một ảnh chỉ có một bộ tệp. Giải mã và thu nhỏ tốn CPU, gọi ở worker,
//! không gọi trên UI thread (N7). Module này không chạm DB.

use std::fs;
use std::io::{self, Cursor};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use groove_core::paths;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageDecoder, ImageError, ImageReader, Limits, RgbImage};
use sha2::{Digest, Sha256};

use crate::db::migrate::{CoverRecord, CoverSink};

/// Tệp vào lớn hơn mức này không giải mã. Bìa trong thẻ thực tế vài MB.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Trần bộ nhớ giải mã (`image::Limits`, chống ảnh bom S10): đủ cho ảnh RGB 8000 × 8000.
pub const MAX_DECODED_BYTES: u64 = 192 * 1024 * 1024;
/// Trần mỗi cạnh, chặn ảnh dẹt cực đoan mà trần bộ nhớ không bắt được.
pub const MAX_DIMENSION: u32 = 16_384;

/// Ảnh lớn hơn được thu nhanh về cạnh này rồi nhả bản gốc, trước khi lọc kỹ.
const WORK_SIZE: u32 = 1024;
const ACCENT_SAMPLE: u32 = 32;
/// Độ sáng tối đa của accent, để chữ trên quầng màu vẫn đọc được (đặc tả 1.5).
const MAX_ACCENT_BRIGHTNESS: f64 = 120.0;
const JPEG_QUALITY: u8 = 85;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Hai cỡ tệp bìa. Là cạnh dài **tối đa**: ảnh gốc nhỏ hơn thì không phóng to, ảnh không vuông thì giữ tỉ lệ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSize {
    Small,
    Large,
}

impl CoverSize {
    pub const fn pixels(self) -> u32 {
        match self {
            Self::Small => 256,
            Self::Large => 512,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoverError {
    #[error("cover image is empty")]
    Empty,
    #[error("cover image is {0} bytes, over the limit")]
    TooLarge(usize),
    /// Gồm cả ảnh vượt `Limits`: `ImageError::Limits`.
    #[error("cannot process cover image: {0}")]
    Image(#[from] ImageError),
    #[error("cover decoder panicked")]
    Panicked,
    #[error("cannot write cover file {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

/// SHA-256 của byte ảnh gốc, dạng hex thường. Tính được trước khi giải mã: caller đã có dòng `covers` cho `hash` này và
/// `has_files` đúng thì khỏi gọi `save`.
pub fn cover_hash(image: &[u8]) -> String {
    format!("{:x}", Sha256::digest(image))
}

/// Thư mục bìa. Không giữ trạng thái nên chép cho nhiều worker được; ghi tệp nguyên tử, gọi song song an toàn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverStore {
    dir: PathBuf,
}

impl CoverStore {
    /// Không chạm đĩa: thư mục được tạo ở lần `save` đầu.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `paths::covers_dir()`.
    pub fn default_location() -> io::Result<Self> {
        paths::covers_dir().map(Self::new)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, hash: &str, size: CoverSize) -> PathBuf {
        self.dir.join(format!("{hash}_{}.jpg", size.pixels()))
    }

    /// Đủ cả hai tệp. Thư mục cache có thể bị xoá trong khi DB còn dòng `covers`.
    pub fn has_files(&self, hash: &str) -> bool {
        [CoverSize::Small, CoverSize::Large].iter().all(|&size| file_present(&self.path(hash, size)))
    }

    /// Giải mã `image`, ghi hai tệp còn thiếu, trả dòng cho bảng `covers`. `width`, `height` là cỡ ảnh gốc sau khi xoay
    /// theo EXIF. Ảnh hỏng, ảnh bom, cả panic trong bộ giải mã đều thành `Err` và không để lại tệp nào.
    pub fn save(&self, image: &[u8]) -> Result<CoverRecord, CoverError> {
        if image.is_empty() {
            return Err(CoverError::Empty);
        }
        if image.len() > MAX_INPUT_BYTES {
            return Err(CoverError::TooLarge(image.len()));
        }
        let hash = cover_hash(image);
        // Tệp độc không được làm sập app (S25).
        catch_unwind(AssertUnwindSafe(|| self.render(image, hash))).unwrap_or(Err(CoverError::Panicked))
    }

    fn render(&self, image: &[u8], hash: String) -> Result<CoverRecord, CoverError> {
        let source = decode_limited(image)?;
        let (width, height) = (source.width(), source.height());
        // Từ đây bản gốc đã nhả, đỉnh RAM chỉ còn ở bước giải mã.
        let work = shrink_for_work(source);
        let accent_color = accent_color(&work);

        let large_path = self.path(&hash, CoverSize::Large);
        let small_path = self.path(&hash, CoverSize::Small);
        if !(file_present(&large_path) && file_present(&small_path)) {
            let large = fit(&flatten(work), CoverSize::Large.pixels());
            let small = fit(&large, CoverSize::Small.pixels());
            fs::create_dir_all(&self.dir).map_err(|source| CoverError::Write { path: self.dir.clone(), source })?;
            write_jpeg(&large_path, &large)?;
            write_jpeg(&small_path, &small)?;
        }
        Ok(CoverRecord { hash, accent_color, width: Some(width), height: Some(height) })
    }
}

impl CoverSink for CoverStore {
    fn store(&mut self, image: &[u8]) -> anyhow::Result<CoverRecord> {
        Ok(self.save(image)?)
    }
}

fn file_present(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
}

fn decode_limited(image: &[u8]) -> Result<DynamicImage, ImageError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES);

    let mut reader = ImageReader::new(Cursor::new(image)).with_guessed_format()?;
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    // `into_decoder` chỉ soát cạnh, `ImageReader::decode` mới soát tổng byte nhưng không cho đọc EXIF.
    limits.reserve(decoder.total_bytes())?;
    let orientation = decoder.orientation()?;
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    Ok(decoded)
}

/// `thumbnail` gộp theo vùng, không cần bộ đệm trung gian; `resize` trên ảnh gốc 8000 px sẽ tốn thêm hàng chục MB.
fn shrink_for_work(source: DynamicImage) -> DynamicImage {
    if source.width().max(source.height()) > WORK_SIZE { source.thumbnail(WORK_SIZE, WORK_SIZE) } else { source }
}

/// JPEG không có kênh alpha: nền đen, hợp giao diện tối.
fn flatten(image: DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.into_rgb8();
    }
    let rgba = image.into_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (out, pixel) in rgb.pixels_mut().zip(rgba.pixels()) {
        let alpha = u32::from(pixel[3]);
        for channel in 0..3 {
            out[channel] = ((u32::from(pixel[channel]) * alpha + 127) / 255) as u8;
        }
    }
    rgb
}

fn fit(image: &RgbImage, size: u32) -> RgbImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= size {
        return image.clone();
    }
    let scale = |side: u32| ((u64::from(side) * u64::from(size) / u64::from(longest)) as u32).max(1);
    imageops::resize(image, scale(width), scale(height), FilterType::CatmullRom)
}

fn write_jpeg(path: &Path, image: &RgbImage) -> Result<(), CoverError> {
    if file_present(path) {
        return Ok(());
    }
    let mut encoded = Vec::new();
    image.write_with_encoder(JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY))?;
    write_atomic(path, &encoded).map_err(|source| CoverError::Write { path: path.to_owned(), source })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("{}.{unique}.tmp", std::process::id()));
    let written = fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    if let Err(err) = written {
        let _ = fs::remove_file(&temp);
        // Windows từ chối đổi tên đè lên tệp đang mở. Tên theo mã băm nên tệp đã có cùng nội dung.
        if !file_present(path) {
            return Err(err);
        }
    }
    Ok(())
}

/// Trung bình 32 × 32 `Triangle`, tính theo alpha như canvas của bản cũ, rồi kẹp sáng (đặc tả 5.2).
fn accent_color(image: &DynamicImage) -> String {
    let sample = image.resize_exact(ACCENT_SAMPLE, ACCENT_SAMPLE, FilterType::Triangle).into_rgba8();
    let mut sums = [0u64; 3];
    let mut total_alpha = 0u64;
    for pixel in sample.pixels() {
        let alpha = u64::from(pixel[3]);
        total_alpha += alpha;
        for (sum, &value) in sums.iter_mut().zip(&pixel.0[..3]) {
            *sum += u64::from(value) * alpha;
        }
    }
    // Ảnh trong suốt hoàn toàn không có màu để lấy: thành đen.
    let average = sums.map(|sum| if total_alpha == 0 { 0.0 } else { (sum as f64 / total_alpha as f64).round() });
    let [r, g, b] = clamp_brightness(average);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// `src/pages/now-playing/now-playing.vue:400-406`.
fn clamp_brightness([r, g, b]: [f64; 3]) -> [u8; 3] {
    let brightness = (r * 299.0 + g * 587.0 + b * 114.0) / 1000.0;
    let factor = if brightness > MAX_ACCENT_BRIGHTNESS { MAX_ACCENT_BRIGHTNESS / brightness } else { 1.0 };
    [r, g, b].map(|channel| (channel * factor).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, Rgba, RgbaImage};
    use tempfile::TempDir;

    fn encode(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        image.write_to(&mut Cursor::new(&mut bytes), format).unwrap();
        bytes
    }

    fn gradient(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, ((x + y) % 256) as u8])
        }))
    }

    fn solid(color: [u8; 3]) -> Vec<u8> {
        encode(DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Rgb(color))), ImageFormat::Png)
    }

    fn store() -> (TempDir, CoverStore) {
        let dir = TempDir::new().unwrap();
        let store = CoverStore::new(dir.path().join("cache").join("covers"));
        (dir, store)
    }

    fn file_names(store: &CoverStore) -> Vec<String> {
        let Ok(entries) = fs::read_dir(store.dir()) else { return Vec::new() };
        let mut names: Vec<String> = entries.map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }

    fn dimensions_of(path: &Path) -> (u32, u32) {
        let reader = ImageReader::open(path).unwrap().with_guessed_format().unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::Jpeg));
        let decoded = reader.decode().unwrap();
        (decoded.width(), decoded.height())
    }

    /// PNG chỉ có phần đầu, khai cỡ tuỳ ý: ảnh bom không tốn công dựng.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            !bytes.iter().fold(!0u32, |crc, &byte| {
                (0..8).fold(
                    crc ^ u32::from(byte),
                    |crc, _| if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 },
                )
            })
        }
        fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            let body = [kind.as_slice(), data].concat();
            png.extend((data.len() as u32).to_be_bytes());
            png.extend(&body);
            png.extend(crc32(&body).to_be_bytes());
        }
        let mut header = Vec::new();
        header.extend(width.to_be_bytes());
        header.extend(height.to_be_bytes());
        header.extend([8, 6, 0, 0, 0]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        chunk(&mut png, b"IHDR", &header);
        // Bộ giải mã đọc phần đầu tới IDAT đầu tiên.
        chunk(&mut png, b"IDAT", &[]);
        png
    }

    // C1
    #[test]
    fn saves_both_sizes_named_by_hash() {
        let (_dir, store) = store();
        let bytes = encode(gradient(800, 800), ImageFormat::Png);
        let record = store.save(&bytes).unwrap();

        assert_eq!(record.hash, cover_hash(&bytes));
        assert_eq!(record.hash.len(), 64);
        assert_eq!((record.width, record.height), (Some(800), Some(800)));
        assert_eq!(file_names(&store), [format!("{}_256.jpg", record.hash), format!("{}_512.jpg", record.hash)]);
        assert_eq!(dimensions_of(&store.path(&record.hash, CoverSize::Large)), (512, 512));
        assert_eq!(dimensions_of(&store.path(&record.hash, CoverSize::Small)), (256, 256));
        assert!(store.has_files(&record.hash));
    }

    // C2
    #[test]
    fn same_image_twice_keeps_the_same_files() {
        let (_dir, store) = store();
        let bytes = encode(gradient(300, 300), ImageFormat::Png);
        let first = store.save(&bytes).unwrap();
        let path = store.path(&first.hash, CoverSize::Large);
        let written = fs::metadata(&path).unwrap().modified().unwrap();

        assert_eq!(store.save(&bytes).unwrap(), first);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), written);
        assert_eq!(file_names(&store).len(), 2);
    }

    #[test]
    fn restores_a_file_deleted_from_the_cache() {
        let (_dir, store) = store();
        let bytes = encode(gradient(300, 300), ImageFormat::Png);
        let record = store.save(&bytes).unwrap();
        fs::remove_file(store.path(&record.hash, CoverSize::Small)).unwrap();
        assert!(!store.has_files(&record.hash));

        store.save(&bytes).unwrap();
        assert!(store.has_files(&record.hash));
    }

    // C3
    #[test]
    fn keeps_aspect_ratio_and_never_upscales() {
        let (_dir, store) = store();
        let wide = store.save(&encode(gradient(1000, 500), ImageFormat::Png)).unwrap();
        assert_eq!(dimensions_of(&store.path(&wide.hash, CoverSize::Large)), (512, 256));
        assert_eq!(dimensions_of(&store.path(&wide.hash, CoverSize::Small)), (256, 128));

        let tiny = store.save(&encode(gradient(100, 50), ImageFormat::Png)).unwrap();
        assert_eq!(dimensions_of(&store.path(&tiny.hash, CoverSize::Large)), (100, 50));
        assert_eq!(dimensions_of(&store.path(&tiny.hash, CoverSize::Small)), (100, 50));

        let sliver = store.save(&encode(gradient(3000, 1), ImageFormat::Png)).unwrap();
        assert_eq!(dimensions_of(&store.path(&sliver.hash, CoverSize::Small)).1, 1);
        assert_eq!(sliver.width, Some(3000), "the record keeps the original size");
    }

    // C4
    #[test]
    fn accent_is_the_clamped_average() {
        let (_dir, store) = store();
        let accent = |color| store.save(&solid(color)).unwrap().accent_color;
        // Sáng 79,6: giữ nguyên.
        assert_eq!(accent([40, 80, 120]), "#285078");
        // Ngay ngưỡng 120: không kẹp.
        assert_eq!(accent([120, 120, 120]), "#787878");
        // Sáng 255 kéo về 120.
        assert_eq!(accent([255, 255, 255]), "#787878");
        // Sáng 155,614: nhân 120 / 155,614 ra (193, 99, 37).
        assert_eq!(accent([250, 128, 48]), "#c16325");
    }

    #[test]
    fn clamp_matches_the_old_formula_at_the_threshold() {
        assert_eq!(clamp_brightness([120.0, 120.0, 120.0]), [120, 120, 120]);
        assert_eq!(clamp_brightness([121.0, 121.0, 121.0]), [120, 120, 120]);
        assert_eq!(clamp_brightness([0.0, 0.0, 0.0]), [0, 0, 0]);
    }

    #[test]
    fn accent_averages_the_whole_image() {
        let half = RgbImage::from_fn(64, 64, |x, _| if x < 32 { Rgb([100, 0, 0]) } else { Rgb([0, 0, 100]) });
        assert_eq!(accent_color(&DynamicImage::ImageRgb8(half)), "#320032");
    }

    #[test]
    fn accent_ignores_transparent_pixels_like_the_canvas_did() {
        let half =
            RgbaImage::from_fn(64, 64, |x, _| if x < 32 { Rgba([255, 255, 255, 0]) } else { Rgba([0, 100, 0, 255]) });
        // Lọc Triangle làm màu ẩn lem chút ít ở mép, không đủ để thành xám như khi tính cả điểm trong suốt.
        let accent = accent_color(&DynamicImage::ImageRgba8(half));
        let channel = |at: std::ops::Range<usize>| u8::from_str_radix(&accent[at], 16).unwrap();
        assert!(channel(1..3) < 8 && channel(5..7) < 8, "{accent}");
        assert!((96..=108).contains(&channel(3..5)), "{accent}");
        let clear = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 0]));
        assert_eq!(accent_color(&DynamicImage::ImageRgba8(clear)), "#000000");
    }

    #[test]
    fn transparent_pixels_flatten_to_black_not_their_hidden_color() {
        let (_dir, store) = store();
        let clear = RgbaImage::from_pixel(64, 64, Rgba([255, 255, 255, 0]));
        let record = store.save(&encode(DynamicImage::ImageRgba8(clear), ImageFormat::Png)).unwrap();
        let saved = image::open(store.path(&record.hash, CoverSize::Small)).unwrap().into_rgb8();
        assert!(saved.get_pixel(32, 32)[0] < 8);
    }

    // C5
    #[test]
    fn broken_images_are_errors_and_leave_no_file() {
        let (_dir, store) = store();
        let png = encode(gradient(64, 64), ImageFormat::Png);
        let broken: [&[u8]; 4] = [b"not an image at all", &png[..8], &png[..png.len() / 2], &[0xFF, 0xD8, 0xFF]];
        assert!(matches!(store.save(&[]), Err(CoverError::Empty)));
        for bytes in broken {
            assert!(store.save(bytes).is_err(), "{} bytes", bytes.len());
        }
        assert!(file_names(&store).is_empty());
    }

    #[test]
    fn corrupted_bytes_never_escape_as_a_panic_or_leave_temp_files() {
        let (_dir, store) = store();
        let formats = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Gif, ImageFormat::Bmp, ImageFormat::WebP];
        let mut seed = 0x2545_F491u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as usize
        };
        for format in formats {
            let original = encode(gradient(48, 48), format);
            for _ in 0..40 {
                let mut bytes = original.clone();
                for _ in 0..4 {
                    let at = next() % bytes.len();
                    bytes[at] = next() as u8;
                }
                let _ = store.save(&bytes);
            }
        }
        assert!(file_names(&store).iter().all(|name| name.ends_with(".jpg")));
    }

    // C6
    #[test]
    fn image_bombs_are_refused_before_decoding() {
        let (_dir, store) = store();
        for (width, height) in [(9000, 9000), (MAX_DIMENSION + 1, 1), (1, MAX_DIMENSION + 1)] {
            let result = store.save(&png_header(width, height));
            assert!(matches!(result, Err(CoverError::Image(ImageError::Limits(_)))), "{width}x{height}: {result:?}");
        }
        assert!(file_names(&store).is_empty());
    }

    #[test]
    fn oversized_input_is_refused_without_reading_it() {
        let (_dir, store) = store();
        let big = vec![0u8; MAX_INPUT_BYTES + 1];
        assert!(matches!(store.save(&big), Err(CoverError::TooLarge(_))));
    }

    // C7
    #[test]
    fn reads_every_enabled_format_and_pixel_depth() {
        let (_dir, store) = store();
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Gif, ImageFormat::Bmp, ImageFormat::WebP] {
            let record = store.save(&encode(gradient(64, 64), format)).unwrap();
            assert!(store.has_files(&record.hash), "{format:?}");
        }
        let deep = DynamicImage::ImageRgba16(gradient(64, 64).into_rgba16());
        assert!(store.save(&encode(deep, ImageFormat::Png)).is_ok());
        let gray = DynamicImage::ImageLuma8(gradient(64, 64).into_luma8());
        assert!(store.save(&encode(gray, ImageFormat::Png)).is_ok());
    }

    // C8
    #[test]
    fn reports_a_directory_it_cannot_create() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("covers");
        fs::write(&blocker, b"a file where the directory should be").unwrap();
        let result = CoverStore::new(&blocker).save(&solid([1, 2, 3]));
        assert!(matches!(result, Err(CoverError::Write { .. })), "{result:?}");
    }

    // C9: EXIF xoay 90 độ, hay gặp ở ảnh chụp bằng điện thoại.
    #[test]
    fn applies_exif_orientation() {
        let exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut segment = vec![0xFF, 0xE1];
        segment.extend((exif.len() as u16 + 2).to_be_bytes());
        segment.extend_from_slice(exif);
        let jpeg = encode(gradient(200, 100), ImageFormat::Jpeg);
        let rotated = [&jpeg[..2], &segment, &jpeg[2..]].concat();

        let (_dir, store) = store();
        let record = store.save(&rotated).unwrap();
        assert_eq!((record.width, record.height), (Some(100), Some(200)));
        assert_eq!(dimensions_of(&store.path(&record.hash, CoverSize::Small)), (100, 200));
    }

    // C10
    #[test]
    fn concurrent_saves_of_one_image_all_succeed() {
        let (_dir, store) = store();
        let bytes = encode(gradient(256, 256), ImageFormat::Png);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8).map(|_| scope.spawn(|| store.save(&bytes).unwrap())).collect();
            let records: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
            assert!(records.windows(2).all(|pair| pair[0] == pair[1]));
        });
        assert_eq!(file_names(&store).len(), 2);
    }
}
