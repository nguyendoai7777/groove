//! groove-gd2_02 (bổ sung 2026-11-09): M4R trên Linux / macOS — mã AAC-LC bằng `filmcraft-aac`, đóng gói MPEG-4 bằng
//! `filmcraft-isobmff` (cả hai thuần Rust, MIT OR Apache-2.0, không `unsafe`, chỉ kéo thêm `thiserror` — D-2). Không cần thư
//! viện hệ thống, không ffmpeg.
//! Cùng hành vi với nhánh Media Foundation của Windows (`mf.rs`): PCM ⇒ AAC 192 kbps stereo 44,1 / 48 kHz; nguồn đã là
//! AAC trong MP4 thì chép nguyên mẫu giao với đoạn, nguồn không phải AAC (ALAC…) thì mã lại.
//! Độ dài chính xác nhờ edit list (`elst`): bỏ 1024 mẫu mồi của bộ mã hoá, hoặc phần thừa của mẫu đầu / cuối khi chép nguyên.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use filmcraft_aac::{Encoder, EncoderConfig};
use filmcraft_isobmff::{
    Brand, CodecConfig, Edit, Mp4Writer, SampleEntry, TrackConfig, TrackKind, WriteSample, WriterOptions,
};

use super::pcm::ClipReader;
use super::{Control, ExportError};
use crate::decoder::{AudioErrorKind, Converter};
use crate::output::OutputConfig;

/// Như bộ mã hoá AAC của Windows (`mf::Codec::Aac`).
const BITRATE: u32 = 192_000;
/// Số mẫu mỗi kênh của một khung AAC-LC.
const FRAME_LEN: u32 = 1024;
/// Tốc độ thường (16.16) của một đoạn edit list.
const NORMAL_RATE: i32 = 0x1_0000;

/// Tần số đích: giữ 44,1 / 48 kHz của nguồn, còn lại về mức gần nhất (cùng quy tắc với Windows).
fn sample_rate(source: u32) -> u32 {
    match source {
        44_100 | 48_000 => source,
        rate if rate > 44_100 => 48_000,
        _ => 44_100,
    }
}

fn mux_failed(err: filmcraft_isobmff::Error) -> ExportError {
    match err {
        filmcraft_isobmff::Error::Io(err) => {
            log::warn!("export: write failed: {err}");
            ExportError::Write
        }
        err => {
            log::warn!("export: mp4 writer failed: {err}");
            ExportError::Encoder
        }
    }
}

fn io_write(err: std::io::Error) -> ExportError {
    log::warn!("export: write failed: {err}");
    ExportError::Write
}

/// Tệp MPEG-4 âm thanh. Ghi `moov` lên trước `mdat` (faststart) rồi đổi brand chính sang `M4A ` như iTunes ghi `.m4a` /
/// `.m4r`; `isom` vẫn nằm trong danh sách tương thích.
struct M4aWriter {
    mp4: Mp4Writer<File>,
}

impl M4aWriter {
    fn create(dest: &Path, movie_timescale: u32, track: TrackConfig) -> Result<Self, ExportError> {
        let file =
            OpenOptions::new().read(true).write(true).create(true).truncate(true).open(dest).map_err(io_write)?;
        let options = WriterOptions { movie_timescale, ..WriterOptions::new(Brand::Mp4) };
        let mut mp4 = Mp4Writer::new(file, options).map_err(mux_failed)?;
        mp4.add_track(track).map_err(mux_failed)?;
        Ok(Self { mp4 })
    }

    fn write(&mut self, data: &[u8], duration: u32) -> Result<(), ExportError> {
        let sample = WriteSample { data, duration, composition_offset: 0, is_sync: true };
        self.mp4.write_sample(0, sample).map_err(mux_failed)
    }

    fn finish(self) -> Result<(), ExportError> {
        let mut file = self.mp4.finish_faststart().map_err(mux_failed)?;
        // `ftyp` mở đầu tệp: cỡ (4) + `ftyp` (4) + brand chính (4).
        file.seek(SeekFrom::Start(8)).map_err(io_write)?;
        file.write_all(b"M4A ").map_err(io_write)?;
        file.sync_all().map_err(io_write)
    }
}

/// Giải mã đoạn rồi mã AAC trong MP4.
pub(super) fn encode(source: &Path, dest: &Path, start: f64, end: f64, control: &Control) -> Result<(), ExportError> {
    let mut clip = ClipReader::open(source, start, end)?;
    let spec = clip.spec();
    // Luôn stereo như Windows: nhạc chuông nghe trên loa điện thoại, không cần giữ mono.
    let target = OutputConfig { sample_rate: sample_rate(spec.sample_rate), channels: 2 };
    let mut converter = (spec.sample_rate != target.sample_rate || spec.channels != usize::from(target.channels))
        .then(|| Converter::new(spec, target))
        .transpose()
        .map_err(ExportError::Source)?;
    let mut sink = AacSink::new(dest, target.sample_rate)?;
    let (mut buf, mut converted) = (Vec::new(), Vec::new());
    while clip.next(&mut buf)? {
        control.check()?;
        match &mut converter {
            Some(converter) => {
                converter.push(&buf, &mut converted).map_err(ExportError::Source)?;
                sink.push(&converted)?;
                converted.clear();
            }
            None => sink.push(&buf)?,
        }
        control.set_progress(clip.progress() * 0.95);
    }
    if let Some(converter) = &mut converter {
        converter.finish(&mut converted).map_err(ExportError::Source)?;
        sink.push(&converted)?;
    }
    sink.finish(control)
}

/// PCM stereo xen kẽ ⇒ AU AAC. Edit list cần độ dài thật trước khi thêm track vào bộ ghi MP4, mà độ dài chỉ biết khi mã
/// xong: AU ghi tạm ra `<đích>.part.aac` (không giữ cả đoạn trong RAM — đoạn 45 phút là ~65 MB), xong mới đóng gói.
struct AacSink<'a> {
    encoder: Encoder,
    rate: u32,
    /// Số frame PCM đã đưa vào bộ mã hoá.
    frames: u64,
    planar: [Vec<f32>; 2],
    units: Spool,
    dest: &'a Path,
}

/// Tệp tạm chứa AU nối nhau + cỡ từng AU; tự xoá khi bỏ.
struct Spool {
    path: std::path::PathBuf,
    file: BufWriter<File>,
    sizes: Vec<u32>,
}

impl Drop for Spool {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl<'a> AacSink<'a> {
    fn new(dest: &'a Path, rate: u32) -> Result<Self, ExportError> {
        let encoder = Encoder::new(EncoderConfig::cbr(rate, 2, BITRATE)).map_err(|err| {
            log::warn!("export: aac encoder: {err}");
            ExportError::Encoder
        })?;
        let mut name = dest.as_os_str().to_os_string();
        name.push(".aac");
        let path = std::path::PathBuf::from(name);
        let file = BufWriter::new(File::create(&path).map_err(io_write)?);
        let units = Spool { path, file, sizes: Vec::new() };
        Ok(Self { encoder, rate, frames: 0, planar: [Vec::new(), Vec::new()], units, dest })
    }

    fn push(&mut self, interleaved: &[f32]) -> Result<(), ExportError> {
        if interleaved.is_empty() {
            return Ok(());
        }
        let [left, right] = &mut self.planar;
        left.clear();
        right.clear();
        for &[l, r] in interleaved.as_chunks::<2>().0 {
            left.push(l);
            right.push(r);
        }
        self.frames += left.len() as u64;
        let units = self.encoder.encode(&[left, right]);
        self.spool(units)
    }

    fn spool(&mut self, units: Vec<Vec<u8>>) -> Result<(), ExportError> {
        for unit in units {
            self.units.file.write_all(&unit).map_err(io_write)?;
            self.units.sizes.push(unit.len() as u32);
        }
        Ok(())
    }

    fn finish(mut self, control: &Control) -> Result<(), ExportError> {
        if self.frames == 0 {
            return Err(ExportError::EmptyRange);
        }
        let rest = self.encoder.flush();
        self.spool(rest)?;
        self.units.file.flush().map_err(io_write)?;
        let mut track =
            TrackConfig::new(SampleEntry::aac(self.encoder.audio_specific_config(), 2, self.rate), self.rate);
        // Bỏ mẫu mồi ở đầu và phần đệm 0 của khung cuối.
        track.edits = vec![Edit {
            segment_duration: self.frames,
            media_time: i64::from(self.encoder.priming_samples()),
            media_rate: NORMAL_RATE,
        }];
        let mut writer = M4aWriter::create(self.dest, self.rate, track)?;
        let mut spooled = BufReader::new(File::open(&self.units.path).map_err(io_write)?);
        let mut unit = Vec::new();
        for &size in &self.units.sizes {
            control.check()?;
            unit.resize(size as usize, 0);
            spooled.read_exact(&mut unit).map_err(io_write)?;
            writer.write(&unit, FRAME_LEN)?;
        }
        writer.finish()
    }
}

/// AAC trong MP4 ⇒ chép nguyên các mẫu giao với đoạn (thêm một mẫu trước làm mồi cho phần chồng MDCT); nguồn không phải AAC
/// ⇒ mã lại.
pub(super) fn copy_aac(source: &Path, dest: &Path, start: f64, end: f64, control: &Control) -> Result<(), ExportError> {
    match try_copy_aac(source, dest, start, end, control)? {
        true => Ok(()),
        false => {
            let _ = std::fs::remove_file(dest);
            encode(source, dest, start, end, control)
        }
    }
}

/// `Ok(false)` = không chép nguyên được (chưa ghi gì), bên gọi mã lại.
fn try_copy_aac(source: &Path, dest: &Path, start: f64, end: f64, control: &Control) -> Result<bool, ExportError> {
    let file = File::open(source).map_err(|err| {
        log::warn!("export: cannot open source: {err}");
        ExportError::Source(AudioErrorKind::Io)
    })?;
    let mp4 = match filmcraft_isobmff::open(&file) {
        Ok(mp4) => mp4,
        Err(err) => {
            log::info!("export: not a readable mp4 ({err}), re-encoding");
            return Ok(false);
        }
    };
    let Some(index) = mp4.track_of_kind(TrackKind::Audio) else { return Ok(false) };
    let track = &mp4.tracks[index];
    let Some(CodecConfig::Aac(aac)) = track.codec() else { return Ok(false) };
    if mp4.fragmented || track.pcm_chunked || track.timescale == 0 || track.samples.is_empty() {
        return Ok(false);
    }
    let channels = track.audio().map(|a| a.channels).filter(|&c| c > 0).unwrap_or_else(|| aac.channels().max(2));
    let rate = track.audio().map(|a| a.sample_rate as u32).filter(|&r| r > 0).unwrap_or(track.timescale);

    let ts = f64::from(track.timescale);
    let (start_t, end_t) = ((start * ts).round() as i64, (end * ts).round() as i64);
    let presented = |i: usize| track.samples[i].pts + track.edit_offset;
    let Some(first) = (0..track.samples.len()).find(|&i| presented(i) + i64::from(track.samples[i].duration) > start_t)
    else {
        return Err(ExportError::EmptyRange);
    };
    let last =
        (first..track.samples.len()).take_while(|&i| presented(i) < end_t).last().ok_or(ExportError::EmptyRange)?;
    let preroll = first.saturating_sub(1);
    let begin = presented(preroll);
    let stop = presented(last) + i64::from(track.samples[last].duration);
    let (cut_from, cut_to) = (start_t.max(presented(first)), end_t.min(stop));
    if cut_to <= cut_from {
        return Err(ExportError::EmptyRange);
    }

    let mut config = TrackConfig::new(SampleEntry::aac(aac.asc.clone(), channels, rate), track.timescale);
    config.edits = vec![Edit {
        segment_duration: (cut_to - cut_from) as u64,
        media_time: cut_from - begin,
        media_rate: NORMAL_RATE,
    }];
    let mut writer = M4aWriter::create(dest, track.timescale, config)?;
    let count = last + 1 - preroll;
    for (n, i) in (preroll..=last).enumerate() {
        control.check()?;
        let data = mp4.read_sample(&file, index, i).map_err(|err| {
            log::warn!("export: cannot read aac sample {i}: {err}");
            ExportError::Source(AudioErrorKind::Corrupt)
        })?;
        writer.write(&data, track.samples[i].duration)?;
        control.set_progress((n + 1) as f64 / count as f64 * 0.98);
    }
    writer.finish()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::sample_rate;

    #[test]
    fn rate_follows_the_windows_rule() {
        assert_eq!(sample_rate(44_100), 44_100);
        assert_eq!(sample_rate(48_000), 48_000);
        assert_eq!(sample_rate(96_000), 48_000);
        assert_eq!(sample_rate(22_050), 44_100);
    }
}
