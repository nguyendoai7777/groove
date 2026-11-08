//! groove-gd2_02: "Save as new" — cắt đoạn `[start, end)` của một tệp nhạc, ghi tệp mới ở định dạng gốc, MP3 hoặc M4R.
//!
//! Ưu tiên không mã hoá lại: MP3 chép nguyên frame ([`mp3_copy`]), AAC trong MP4 chép nguyên mẫu (Media Foundation, Windows).
//! WAV / FLAC giải mã rồi ghi lại cùng độ sâu bit ([`pcm`]); còn lại giải mã rồi mã MP3 / AAC bằng bộ mã hoá của OS ([`mf`]).
//! Bộ mã hoá MP3 / AAC chỉ có ở Windows (Media Foundation): macOS / Linux trả [`Unavailable::NoEncoderOnThisOs`], hộp thoại khoá
//! lựa chọn đó (mục 9 của task). Không bao giờ ghi đè tệp gốc; ghi ra `<đích>.part` rồi đổi tên, huỷ / lỗi thì xoá.

mod mp3_copy;
mod pcm;

#[cfg(windows)]
mod mf;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crossbeam_channel::{Receiver, TryRecvError};

use crate::decoder::AudioErrorKind;

/// Nhạc chuông iPhone dài quá 40 s thì iOS không nhận (hộp thoại cảnh báo, vẫn cho lưu).
pub const RINGTONE_MAX_SEC: f64 = 40.0;
/// Đoạn ngắn hơn thì coi như rỗng.
pub const MIN_CLIP_SEC: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Giữ định dạng của tệp gốc.
    Current,
    Mp3,
    /// AAC trong MP4, đuôi `.m4r` (nhạc chuông iPhone).
    M4r,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 3] = [ExportFormat::Current, ExportFormat::Mp3, ExportFormat::M4r];
}

/// Cách làm cho một cặp (nguồn, định dạng đích).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Chép nguyên frame MPEG, không mã lại.
    Mp3FrameCopy,
    /// Giải mã rồi ghi WAV PCM cùng độ sâu bit.
    Wav,
    /// Giải mã rồi mã FLAC (`flacenc`) cùng độ sâu bit.
    Flac,
    /// Chép nguyên mẫu AAC trong MP4 (Windows); bộ ghi từ chối thì mã lại AAC.
    AacCopy,
    /// Giải mã rồi mã MP3 (Windows).
    EncodeMp3,
    /// Giải mã rồi mã AAC trong MP4 (Windows).
    EncodeAac,
}

/// Vì sao một định dạng không dùng được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// OS này chưa có bộ mã hoá hợp lệ (D-2) cho định dạng đó.
    NoEncoderOnThisOs,
    /// Không có bộ mã hoá cho định dạng gốc (OGG Vorbis…): chọn MP3 / M4R.
    NoEncoderForSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceKind {
    Mp3,
    Wav,
    Flac,
    Mp4,
    Other,
}

fn source_kind(source: &Path) -> SourceKind {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => SourceKind::Mp3,
        "wav" => SourceKind::Wav,
        "flac" => SourceKind::Flac,
        "m4a" | "m4r" | "mp4" | "aac" => SourceKind::Mp4,
        _ => SourceKind::Other,
    }
}

/// Bộ mã hoá của OS (Media Foundation) có trên máy này không.
const OS_ENCODER: bool = cfg!(windows);

/// Cách xuất `source` sang `format`, hoặc vì sao không được.
pub fn plan(source: &Path, format: ExportFormat) -> Result<Method, Unavailable> {
    plan_for(source_kind(source), format, OS_ENCODER)
}

fn plan_for(kind: SourceKind, format: ExportFormat, os_encoder: bool) -> Result<Method, Unavailable> {
    let os = |method| if os_encoder { Ok(method) } else { Err(Unavailable::NoEncoderOnThisOs) };
    match (format, kind) {
        (ExportFormat::Current | ExportFormat::Mp3, SourceKind::Mp3) => Ok(Method::Mp3FrameCopy),
        (ExportFormat::Current, SourceKind::Wav) => Ok(Method::Wav),
        (ExportFormat::Current, SourceKind::Flac) => Ok(Method::Flac),
        (ExportFormat::Current | ExportFormat::M4r, SourceKind::Mp4) => os(Method::AacCopy),
        (ExportFormat::Current, SourceKind::Other) => Err(Unavailable::NoEncoderForSource),
        (ExportFormat::Mp3, _) => os(Method::EncodeMp3),
        (ExportFormat::M4r, _) => os(Method::EncodeAac),
    }
}

/// Đuôi tệp đích (chữ thường, không dấu chấm).
pub fn extension(source: &Path, format: ExportFormat) -> String {
    match format {
        ExportFormat::Current => {
            source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_else(|| "mp3".to_owned())
        }
        ExportFormat::Mp3 => "mp3".to_owned(),
        ExportFormat::M4r => "m4r".to_owned(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportRequest {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub start_sec: f64,
    pub end_sec: f64,
    pub format: ExportFormat,
    /// Đích đã có (người dùng đã đồng ý ghi đè ở hộp chọn tệp). Tệp gốc thì không bao giờ.
    pub overwrite: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    #[error("destination is the source file")]
    SameAsSource,
    #[error("destination already exists")]
    Exists,
    #[error("clip is empty")]
    EmptyRange,
    #[error("format unavailable: {0:?}")]
    Unavailable(Unavailable),
    #[error("cannot read source: {0}")]
    Source(AudioErrorKind),
    #[error("encoder failed")]
    Encoder,
    #[error("cannot write destination")]
    Write,
    #[error("cancelled")]
    Cancelled,
}

/// Tiến độ + huỷ, dùng chung giữa luồng xuất và UI.
#[derive(Default)]
pub struct Control {
    cancel: AtomicBool,
    /// Bit của `f32` 0..=1.
    progress: AtomicU32,
}

impl Control {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    fn set_progress(&self, value: f64) {
        let value = (value as f32).clamp(0.0, 1.0);
        // Chỉ tăng: bước đầu của bộ ghi có thể báo lùi một chút.
        if value > self.progress() {
            self.progress.store(value.to_bits(), Ordering::Relaxed);
        }
    }

    fn check(&self) -> Result<(), ExportError> {
        if self.is_cancelled() { Err(ExportError::Cancelled) } else { Ok(()) }
    }
}

/// Một lượt xuất chạy ở luồng riêng (N7).
pub struct ExportJob {
    control: Arc<Control>,
    done: Receiver<Result<PathBuf, ExportError>>,
}

impl ExportJob {
    pub fn spawn(request: ExportRequest) -> std::io::Result<Self> {
        let control = Arc::new(Control::default());
        let (tx, done) = crossbeam_channel::bounded(1);
        let shared = control.clone();
        std::thread::Builder::new().name("groove-export".into()).spawn(move || {
            let _ = tx.send(run(&request, &shared));
        })?;
        Ok(Self { control, done })
    }

    pub fn progress(&self) -> f32 {
        self.control.progress()
    }

    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Kết quả khi xong, `None` khi còn chạy. Trả kết quả đúng một lần.
    pub fn try_finish(&self) -> Option<Result<PathBuf, ExportError>> {
        match self.done.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

/// Xuất đồng bộ trên luồng gọi. Panic của decoder / bộ ghi (tệp độc, S25) thành lỗi.
pub fn run(request: &ExportRequest, control: &Control) -> Result<PathBuf, ExportError> {
    let method = plan(&request.source, request.format).map_err(ExportError::Unavailable)?;
    check_destination(request)?;
    if request.end_sec - request.start_sec.max(0.0) < MIN_CLIP_SEC {
        return Err(ExportError::EmptyRange);
    }
    let part = part_path(&request.dest);
    let _ = std::fs::remove_file(&part);
    let result = catch_unwind(AssertUnwindSafe(|| write_part(method, request, &part, control)))
        .unwrap_or_else(|_| {
            log::error!("export: panic while writing {}", request.dest.display());
            Err(ExportError::Source(AudioErrorKind::Corrupt))
        })
        .and_then(|()| control.check());
    match result {
        Ok(()) => {
            std::fs::rename(&part, &request.dest).map_err(|err| {
                log::error!("export: rename failed: {err}");
                let _ = std::fs::remove_file(&part);
                ExportError::Write
            })?;
            control.set_progress(1.0);
            log::info!("export: {:?} -> {}", method, request.dest.display());
            Ok(request.dest.clone())
        }
        Err(err) => {
            let _ = std::fs::remove_file(&part);
            if err != ExportError::Cancelled {
                log::warn!("export: {method:?} failed: {err}");
            }
            Err(err)
        }
    }
}

fn write_part(method: Method, request: &ExportRequest, part: &Path, control: &Control) -> Result<(), ExportError> {
    let (start, end) = (request.start_sec.max(0.0), request.end_sec);
    match method {
        Method::Mp3FrameCopy => mp3_copy::copy(&request.source, part, start, end, control),
        Method::Wav => pcm::write_wav(&request.source, part, start, end, control),
        Method::Flac => pcm::write_flac(&request.source, part, start, end, control),
        #[cfg(windows)]
        Method::AacCopy => mf::copy_aac(&request.source, part, start, end, control),
        #[cfg(windows)]
        Method::EncodeMp3 => mf::encode(&request.source, part, start, end, mf::Codec::Mp3, control),
        #[cfg(windows)]
        Method::EncodeAac => mf::encode(&request.source, part, start, end, mf::Codec::Aac, control),
        #[cfg(not(windows))]
        Method::AacCopy | Method::EncodeMp3 | Method::EncodeAac => {
            Err(ExportError::Unavailable(Unavailable::NoEncoderOnThisOs))
        }
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Đích không được là tệp gốc (kể cả khác hoa thường trên Windows, khác dạng đường dẫn); đã có thì cần `overwrite`.
fn check_destination(request: &ExportRequest) -> Result<(), ExportError> {
    if same_file(&request.source, &request.dest) {
        return Err(ExportError::SameAsSource);
    }
    if request.dest.exists() && !request.overwrite {
        return Err(ExportError::Exists);
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| -> Option<PathBuf> {
        match p.canonicalize() {
            Ok(path) => Some(path),
            // Đích chưa có: chuẩn hoá thư mục cha rồi ghép tên.
            Err(_) => Some(p.parent()?.canonicalize().ok()?.join(p.file_name()?)),
        }
    };
    match (canonical(a), canonical(b)) {
        (Some(a), Some(b)) => groove_core::paths::normalize_path(&a) == groove_core::paths::normalize_path(&b),
        _ => groove_core::paths::normalize_path(a) == groove_core::paths::normalize_path(b),
    }
}

/// Mẫu f32 ⇒ số nguyên `bits` bit (16 / 24), có kẹp.
fn quantize(sample: f32, bits: u32) -> i32 {
    let scale = ((1i64 << (bits - 1)) - 1) as f32;
    (sample.clamp(-1.0, 1.0) * scale).round() as i32
}

#[cfg(test)]
mod tests;
