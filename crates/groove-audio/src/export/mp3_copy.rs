//! groove-gd2_02: cắt MP3 theo frame, không mã lại. Giữ nguyên các thẻ ID3v2 đầu tệp (tên, nghệ sĩ, bìa), bỏ frame
//! Xing / Info / VBRI (số frame, mục lục trong đó là của tệp gốc), bỏ ID3v1 / APE cuối tệp. Chỉ MPEG Layer III.
//!
//! Frame đầu của đoạn cắt có thể trỏ `main_data_begin` vào frame trước (bit reservoir) — decoder bỏ frame đó, đoạn ra hụt
//! đầu tối đa ~26 ms; mọi công cụ cắt MP3 không mã lại đều vậy.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;

use super::{Control, ExportError};
use crate::decoder::AudioErrorKind;

/// Đọc / ghi theo khối lớn: tệp nonstop cả trăm MB.
const IO_BUFFER: usize = 256 * 1024;
/// Dò lại đồng bộ quá chừng này byte mà không thấy frame ⇒ tệp hỏng.
const MAX_RESYNC: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameHeader {
    /// 3 = MPEG-1, 2 = MPEG-2, 0 = MPEG-2.5 (giá trị bit trong header).
    version: u8,
    pub(super) sample_rate: u32,
    pub(super) samples: u32,
    pub(super) len: usize,
    mono: bool,
}

const BITRATE_V1: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const BITRATE_V2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// Header 4 byte của một frame MPEG Layer III, `None` nếu không phải.
pub(super) fn parse_header(bytes: &[u8]) -> Option<FrameHeader> {
    let [b0, b1, b2, b3] = *bytes.get(..4)? else { return None };
    if b0 != 0xFF || b1 & 0xE0 != 0xE0 {
        return None;
    }
    let version = (b1 >> 3) & 0b11;
    let layer = (b1 >> 1) & 0b11;
    if version == 1 || layer != 0b01 {
        return None;
    }
    let bitrate_ix = usize::from(b2 >> 4);
    let rate_ix = usize::from((b2 >> 2) & 0b11);
    if bitrate_ix == 0 || bitrate_ix == 15 || rate_ix == 3 {
        return None;
    }
    let base_rate = [44_100, 48_000, 32_000][rate_ix];
    let sample_rate = match version {
        3 => base_rate,
        2 => base_rate / 2,
        _ => base_rate / 4,
    };
    let kbps = if version == 3 { BITRATE_V1[bitrate_ix] } else { BITRATE_V2[bitrate_ix] };
    let padding = u32::from((b2 >> 1) & 1);
    let samples = if version == 3 { 1152 } else { 576 };
    let len = (samples / 8 * kbps * 1000 / sample_rate + padding) as usize;
    Some(FrameHeader { version, sample_rate, samples, len, mono: b3 >> 6 == 0b11 })
}

/// Frame chỉ chứa thông tin VBR (Xing / Info của LAME, VBRI của Fraunhofer), không có âm thanh đáng giữ.
fn is_info_frame(frame: &[u8], header: &FrameHeader) -> bool {
    let side_info = match (header.version == 3, header.mono) {
        (true, true) => 17,
        (true, false) => 32,
        (false, true) => 9,
        (false, false) => 17,
    };
    let at = |offset: usize, tag: &[u8]| frame.get(offset..offset + tag.len()) == Some(tag);
    at(4 + side_info, b"Xing") || at(4 + side_info, b"Info") || at(36, b"VBRI")
}

fn io_read(err: std::io::Error) -> ExportError {
    log::warn!("export: mp3 read failed: {err}");
    ExportError::Source(AudioErrorKind::Io)
}

fn io_write(err: std::io::Error) -> ExportError {
    log::warn!("export: mp3 write failed: {err}");
    ExportError::Write
}

/// Chép các frame giao với `[start, end)` của `source` sang `dest`.
pub(super) fn copy(source: &Path, dest: &Path, start: f64, end: f64, control: &Control) -> Result<(), ExportError> {
    let file = File::open(source).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ExportError::Source(AudioErrorKind::NotFound),
        _ => io_read(err),
    })?;
    let mut reader = BufReader::with_capacity(IO_BUFFER, file);
    let mut writer = BufWriter::with_capacity(IO_BUFFER, File::create(dest).map_err(io_write)?);

    copy_id3v2(&mut reader, &mut writer)?;

    let mut time = 0.0f64;
    let mut first: Option<FrameHeader> = None;
    let mut written = 0usize;
    let mut skipped = 0usize;
    let mut frame = Vec::with_capacity(2048);
    loop {
        let buf = reader.fill_buf().map_err(io_read)?;
        if buf.len() < 4 || buf.starts_with(b"TAG") || buf.starts_with(b"APETAGEX") || buf.starts_with(b"LYRICS") {
            break;
        }
        let header = parse_header(buf).filter(|h| {
            // Sau frame đầu: cùng phiên bản và tần số, chặn đồng bộ giả giữa dữ liệu.
            first.is_none_or(|f| f.version == h.version && f.sample_rate == h.sample_rate)
        });
        let Some(header) = header else {
            reader.consume(1);
            skipped += 1;
            if skipped > MAX_RESYNC {
                return Err(ExportError::Source(AudioErrorKind::Corrupt));
            }
            continue;
        };
        skipped = 0;
        frame.resize(header.len, 0);
        match reader.read_exact(&mut frame) {
            Ok(()) => {}
            // Frame cuối cụt: dừng ở đó.
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(io_read(err)),
        }
        let is_first = first.is_none();
        first.get_or_insert(header);
        if is_first && is_info_frame(&frame, &header) {
            continue;
        }
        let duration = f64::from(header.samples) / f64::from(header.sample_rate);
        if time + duration > start && time < end {
            writer.write_all(&frame).map_err(io_write)?;
            written += 1;
            control.set_progress((time + duration - start) / (end - start));
        }
        time += duration;
        if time >= end {
            break;
        }
        if written.is_multiple_of(64) {
            control.check()?;
        }
    }
    if written == 0 {
        return Err(if first.is_some() {
            ExportError::EmptyRange
        } else {
            ExportError::Source(AudioErrorKind::Corrupt)
        });
    }
    writer.flush().map_err(io_write)?;
    Ok(())
}

/// Chép nguyên mọi thẻ ID3v2 liền nhau ở đầu tệp.
fn copy_id3v2(reader: &mut BufReader<File>, writer: &mut impl Write) -> Result<(), ExportError> {
    loop {
        let buf = reader.fill_buf().map_err(io_read)?;
        if buf.len() < 10 || !buf.starts_with(b"ID3") {
            return Ok(());
        }
        let size = buf[6..10].iter().fold(0u64, |acc, byte| (acc << 7) | u64::from(byte & 0x7F));
        // Cờ footer (ID3v2.4): thêm 10 byte.
        let footer = if buf[5] & 0x10 != 0 { 10 } else { 0 };
        let total = 10 + size + footer;
        let copied = std::io::copy(&mut reader.by_ref().take(total), writer).map_err(io_write)?;
        if copied < total {
            return Ok(());
        }
    }
}
