//! groove-gd2_02: đọc PCM của một đoạn (symphonia, tua chính xác) và ghi WAV / FLAC cùng độ sâu bit với tệp gốc.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::error::{SourceError, Verify};
use flacenc::source::{Fill, Source};

use super::{Control, ExportError, quantize};
use crate::decoder::{AudioErrorKind, SourceSpec, TrackDecoder};

/// Đoạn `[start, end)` của một tệp, đọc từng lượt mẫu f32 xen kẽ theo kênh gốc.
pub(super) struct ClipReader {
    decoder: TrackDecoder,
    spec: SourceSpec,
    /// Lượt đầu đã đọc lúc mở (để biết tần số / số kênh), trả ở lần `next` đầu.
    pending: Option<Vec<f32>>,
    total_frames: u64,
    left_frames: u64,
}

impl ClipReader {
    pub(super) fn open(path: &Path, start: f64, end: f64) -> Result<Self, ExportError> {
        let mut decoder = TrackDecoder::open(path).map_err(ExportError::Source)?;
        decoder.seek(start).map_err(ExportError::Source)?;
        let mut first = Vec::new();
        let spec = decoder.next_frames(&mut first).map_err(ExportError::Source)?.ok_or(ExportError::EmptyRange)?;
        let total_frames = ((end - start) * f64::from(spec.sample_rate)).round().max(0.0) as u64;
        Ok(Self { decoder, spec, pending: Some(first), total_frames, left_frames: total_frames })
    }

    pub(super) fn spec(&self) -> SourceSpec {
        self.spec
    }

    /// Độ sâu bit ghi ra khi giữ PCM: 24 nếu gốc sâu hơn 16, còn lại 16.
    pub(super) fn output_bits(&self) -> u32 {
        if self.decoder.bits_per_sample().is_some_and(|bits| bits > 16) { 24 } else { 16 }
    }

    pub(super) fn progress(&self) -> f64 {
        if self.total_frames == 0 { 1.0 } else { 1.0 - self.left_frames as f64 / self.total_frames as f64 }
    }

    /// Ghi đè `out` bằng lượt kế tiếp (đã cắt ở `end`). `false` là hết đoạn.
    pub(super) fn next(&mut self, out: &mut Vec<f32>) -> Result<bool, ExportError> {
        if self.left_frames == 0 {
            return Ok(false);
        }
        match self.pending.take() {
            Some(first) => *out = first,
            None => match self.decoder.next_frames(out).map_err(ExportError::Source)? {
                None => {
                    self.left_frames = 0;
                    return Ok(false);
                }
                Some(spec) if spec.channels != self.spec.channels => {
                    log::warn!("export: channel count changed mid-stream");
                    return Err(ExportError::Source(AudioErrorKind::Unsupported));
                }
                Some(_) => {}
            },
        }
        let channels = self.spec.channels;
        let frames = (out.len() / channels) as u64;
        let keep = frames.min(self.left_frames);
        out.truncate(keep as usize * channels);
        self.left_frames -= keep;
        Ok(true)
    }
}

fn io_write(err: std::io::Error) -> ExportError {
    log::warn!("export: write failed: {err}");
    ExportError::Write
}

/// WAV PCM nguyên số (16 / 24 bit), cùng tần số / số kênh với gốc.
pub(super) fn write_wav(
    source: &Path,
    dest: &Path,
    start: f64,
    end: f64,
    control: &Control,
) -> Result<(), ExportError> {
    let mut clip = ClipReader::open(source, start, end)?;
    let spec = clip.spec();
    let bits = clip.output_bits();
    let bytes_per_sample = (bits / 8) as usize;
    let mut file = BufWriter::new(File::create(dest).map_err(io_write)?);
    file.write_all(&wav_header(spec, bits, 0)).map_err(io_write)?;
    let (mut buf, mut bytes, mut data_len) = (Vec::new(), Vec::new(), 0u64);
    while clip.next(&mut buf)? {
        control.check()?;
        bytes.clear();
        for &sample in &buf {
            bytes.extend_from_slice(&quantize(sample, bits).to_le_bytes()[..bytes_per_sample]);
        }
        file.write_all(&bytes).map_err(io_write)?;
        data_len += bytes.len() as u64;
        control.set_progress(clip.progress());
    }
    let data_len = u32::try_from(data_len).map_err(|_| ExportError::Write)?;
    let mut file = file.into_inner().map_err(|err| io_write(err.into_error()))?;
    file.seek(SeekFrom::Start(0)).map_err(io_write)?;
    file.write_all(&wav_header(spec, bits, data_len)).map_err(io_write)?;
    file.sync_all().map_err(io_write)
}

pub(super) fn wav_header(spec: SourceSpec, bits: u32, data_len: u32) -> [u8; 44] {
    let channels = spec.channels as u16;
    let block_align = channels * (bits / 8) as u16;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&spec.sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(spec.sample_rate * u32::from(block_align)).to_le_bytes());
    h[32..34].copy_from_slice(&block_align.to_le_bytes());
    h[34..36].copy_from_slice(&(bits as u16).to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_len.to_le_bytes());
    h
}

/// Nguồn của `flacenc`: kéo từng khối từ [`ClipReader`], huỷ thì trả lỗi để bộ mã dừng.
struct FlacSource<'a> {
    clip: ClipReader,
    control: &'a Control,
    bits: u32,
    buf: Vec<f32>,
    /// Mẫu f32 còn dư từ lượt trước (xen kẽ).
    carry: Vec<f32>,
    ints: Vec<i32>,
    done: bool,
    error: Option<ExportError>,
}

impl FlacSource<'_> {
    fn fail(&mut self, err: ExportError) -> SourceError {
        self.error = Some(err);
        SourceError::from_io_error(std::io::Error::other(err.to_string()))
    }
}

impl Source for FlacSource<'_> {
    fn channels(&self) -> usize {
        self.clip.spec().channels
    }

    fn bits_per_sample(&self) -> usize {
        self.bits as usize
    }

    fn sample_rate(&self) -> usize {
        self.clip.spec().sample_rate as usize
    }

    fn read_samples<F: Fill>(&mut self, block_size: usize, dest: &mut F) -> Result<usize, SourceError> {
        let channels = self.channels();
        let wanted = block_size * channels;
        while self.carry.len() < wanted && !self.done {
            if self.control.is_cancelled() {
                return Err(self.fail(ExportError::Cancelled));
            }
            match self.clip.next(&mut self.buf) {
                Ok(true) => self.carry.extend_from_slice(&self.buf),
                Ok(false) => self.done = true,
                Err(err) => return Err(self.fail(err)),
            }
            self.control.set_progress(self.clip.progress() * 0.95);
        }
        let take = self.carry.len().min(wanted);
        self.ints.clear();
        self.ints.extend(self.carry.drain(..take).map(|s| quantize(s, self.bits)));
        dest.fill_interleaved(&self.ints)?;
        Ok(take / channels)
    }
}

pub(super) fn write_flac(
    source: &Path,
    dest: &Path,
    start: f64,
    end: f64,
    control: &Control,
) -> Result<(), ExportError> {
    let clip = ClipReader::open(source, start, end)?;
    let bits = clip.output_bits();
    let mut source = FlacSource {
        clip,
        control,
        bits,
        buf: Vec::new(),
        carry: Vec::new(),
        ints: Vec::new(),
        done: false,
        error: None,
    };
    let config = flacenc::config::Encoder::default().into_verified().map_err(|_| ExportError::Encoder)?;
    let block_size = config.block_size;
    let mut stream = match flacenc::encode_with_fixed_block_size(&config, &mut source, block_size) {
        Ok(stream) => stream,
        Err(err) => {
            return Err(source.error.unwrap_or_else(|| {
                log::warn!("export: flac encode failed: {err:?}");
                ExportError::Encoder
            }));
        }
    };
    // flacenc ghi `min_block_size` = khối cuối (ngắn hơn); đặc tả FLAC không tính khối cuối, symphonia đọc STREAMINFO đó
    // thì báo hết tệp sớm. Mọi khối trừ khối cuối đều `block_size`.
    stream.stream_info_mut().set_block_sizes(block_size, block_size).map_err(|_| ExportError::Encoder)?;
    let mut sink = ByteSink::new();
    stream.write(&mut sink).map_err(|_| ExportError::Encoder)?;
    let mut file = File::create(dest).map_err(io_write)?;
    file.write_all(sink.as_slice()).map_err(io_write)?;
    file.sync_all().map_err(io_write)
}
