//! groove-gd1_10: symphonia đọc tệp thành `f32` xen kẽ, [`Converter`] đổi kênh và tần số mẫu về thiết bị.

use std::fs::File;
use std::path::Path;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase, Timestamp};

use crate::output::OutputConfig;

/// Bao nhiêu gói hỏng liền nhau thì coi cả tệp hỏng.
const MAX_CONSECUTIVE_BAD_PACKETS: u32 = 32;
const RESAMPLER_CHUNK_FRAMES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AudioErrorKind {
    #[error("file not found")]
    NotFound,
    #[error("cannot read file")]
    Io,
    /// Định dạng hoặc codec không giải được, gồm HE-AAC và Opus (đặc tả 5.1).
    #[error("unsupported format")]
    Unsupported,
    #[error("corrupt file")]
    Corrupt,
    /// Không có thiết bị xuất âm, hoặc bản Linux chưa bật `linux-alsa`.
    #[error("audio output unavailable")]
    Output,
}

fn classify(err: &SymphoniaError) -> AudioErrorKind {
    match err {
        SymphoniaError::IoError(io) if io.kind() == std::io::ErrorKind::NotFound => AudioErrorKind::NotFound,
        SymphoniaError::IoError(_) => AudioErrorKind::Io,
        SymphoniaError::Unsupported(_) => AudioErrorKind::Unsupported,
        _ => AudioErrorKind::Corrupt,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpec {
    pub sample_rate: u32,
    pub channels: usize,
}

/// Một tệp đang đọc. Có thể panic với tệp độc: bên gọi bọc `catch_unwind` (S25).
pub struct TrackDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    duration_sec: f64,
    /// Sau khi tua, bỏ mọi frame trước mốc này.
    seek_target: Option<Timestamp>,
    finished: bool,
}

impl TrackDecoder {
    pub fn open(path: &Path) -> Result<Self, AudioErrorKind> {
        let file = File::open(path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => AudioErrorKind::NotFound,
            _ => AudioErrorKind::Io,
        })?;
        let source = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(&extension.to_ascii_lowercase());
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, source, FormatOptions::default(), MetadataOptions::default())
            .map_err(|err| match classify(&err) {
                AudioErrorKind::Io => AudioErrorKind::Io,
                _ => AudioErrorKind::Unsupported,
            })?;
        let track = format.default_track(TrackType::Audio).ok_or(AudioErrorKind::Unsupported)?;
        let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or(AudioErrorKind::Unsupported)?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|_| AudioErrorKind::Unsupported)?;
        let duration_sec = match (track.num_frames, params.sample_rate, track.duration, track.time_base) {
            (Some(frames), Some(rate), _, _) if rate > 0 => frames as f64 / f64::from(rate),
            (_, _, Some(duration), Some(time_base)) => duration.get() as f64 * f64::from(time_base),
            _ => 0.0,
        };
        let (track_id, time_base) = (track.id, track.time_base);
        Ok(Self { format, decoder, track_id, time_base, duration_sec, seek_target: None, finished: false })
    }

    /// Độ dài theo khai báo của tệp, 0 nếu tệp không khai.
    pub fn duration_sec(&self) -> f64 {
        self.duration_sec
    }

    /// Tua chính xác. Mốc ngoài tệp thì coi như đã đọc hết, không phải lỗi.
    pub fn seek(&mut self, sec: f64) -> Result<(), AudioErrorKind> {
        self.finished = false;
        let time = Time::try_from_secs_f64(sec.max(0.0)).unwrap_or(Time::ZERO);
        let to = SeekTo::Time { time, track_id: Some(self.track_id) };
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(seeked) => {
                self.decoder.reset();
                self.seek_target = Some(seeked.required_ts);
                Ok(())
            }
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => {
                self.finished = true;
                Ok(())
            }
            Err(err) => Err(classify(&err)),
        }
    }

    /// Ghi đè `out` bằng lượt frame kế tiếp. `None` là hết tệp.
    pub fn next_frames(&mut self, out: &mut Vec<f32>) -> Result<Option<SourceSpec>, AudioErrorKind> {
        let mut bad_packets = 0;
        while !self.finished {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) | Err(SymphoniaError::ResetRequired) => break,
                Err(SymphoniaError::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(classify(&err)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                Err(SymphoniaError::DecodeError(_) | SymphoniaError::IoError(_)) => {
                    bad_packets += 1;
                    if bad_packets >= MAX_CONSECUTIVE_BAD_PACKETS {
                        return Err(AudioErrorKind::Corrupt);
                    }
                    continue;
                }
                Err(err) => return Err(classify(&err)),
            };
            bad_packets = 0;
            let spec = SourceSpec { sample_rate: buffer.spec().rate(), channels: buffer.spec().channels().count() };
            if spec.channels == 0 || spec.sample_rate == 0 {
                return Err(AudioErrorKind::Corrupt);
            }
            let skip = match (self.seek_target, self.time_base) {
                (Some(target), Some(time_base)) => {
                    let ticks = target.get().saturating_sub(packet.pts.get()).max(0) as f64;
                    (ticks * f64::from(time_base) * f64::from(spec.sample_rate)).round() as usize
                }
                _ => 0,
            };
            let frames = buffer.frames();
            if skip >= frames {
                // Vẫn trước mốc: gói kế tiếp tính lại từ `pts` của nó.
                continue;
            }
            self.seek_target = None;
            out.clear();
            out.resize(frames * spec.channels, 0.0);
            buffer.copy_to_slice_interleaved::<f32, _>(out.as_mut_slice());
            out.drain(..skip * spec.channels);
            return Ok(Some(spec));
        }
        self.finished = true;
        Ok(None)
    }
}

/// Đổi số kênh và tần số mẫu. Bộ đệm cấp phát một lần, không ở callback.
pub struct Converter {
    source: SourceSpec,
    target: OutputConfig,
    resampler: Option<Fft<f32>>,
    mixed: Vec<f32>,
    input_start: usize,
    resampled: Vec<f32>,
    delay_left: usize,
    frames_in: u64,
    frames_out: u64,
}

impl Converter {
    pub fn new(source: SourceSpec, target: OutputConfig) -> Result<Self, AudioErrorKind> {
        let channels = usize::from(target.channels.max(1));
        let resampler = (source.sample_rate != target.sample_rate)
            .then(|| {
                Fft::<f32>::new(
                    source.sample_rate as usize,
                    target.sample_rate as usize,
                    RESAMPLER_CHUNK_FRAMES,
                    channels,
                    FixedSync::Input,
                )
                .map_err(|_| AudioErrorKind::Unsupported)
            })
            .transpose()?;
        let (resampled, delay_left) = resampler.as_ref().map_or((Vec::new(), 0), |r: &Fft<f32>| {
            (vec![0.0; r.output_frames_max() * channels], r.output_delay())
        });
        Ok(Self {
            source,
            target,
            resampler,
            mixed: Vec::new(),
            input_start: 0,
            resampled,
            delay_left,
            frames_in: 0,
            frames_out: 0,
        })
    }

    pub fn source(&self) -> SourceSpec {
        self.source
    }

    /// Bỏ phần đang dở, dùng sau khi tua hoặc đổi bài.
    pub fn reset(&mut self) {
        self.mixed.clear();
        self.input_start = 0;
        self.frames_in = 0;
        self.frames_out = 0;
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
            self.delay_left = resampler.output_delay();
        }
    }

    /// Thêm `input` (xen kẽ theo nguồn), nối kết quả vào `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioErrorKind> {
        let channels = usize::from(self.target.channels.max(1));
        if self.resampler.is_none() {
            mix_channels(input, self.source.channels, channels, out);
            return Ok(());
        }
        mix_channels(input, self.source.channels, channels, &mut self.mixed);
        self.frames_in += (input.len() / self.source.channels) as u64;
        self.run_resampler(out, false)
    }

    /// Hết tệp: xả phần còn trong bộ đổi tần số để không mất đuôi bài.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), AudioErrorKind> {
        if self.resampler.is_some() {
            self.run_resampler(out, true)?;
        }
        Ok(())
    }

    fn run_resampler(&mut self, out: &mut Vec<f32>, draining: bool) -> Result<(), AudioErrorKind> {
        let channels = usize::from(self.target.channels.max(1));
        let Some(resampler) = &mut self.resampler else { return Ok(()) };
        let expected = (self.frames_in as f64 * f64::from(self.target.sample_rate)
            / f64::from(self.source.sample_rate))
        .round() as u64;
        loop {
            let available = (self.mixed.len() - self.input_start) / channels;
            let needed = resampler.input_frames_next();
            if available < needed && (!draining || self.frames_out >= expected) {
                break;
            }
            let frames = available.min(needed);
            let input = &self.mixed[self.input_start..self.input_start + frames * channels];
            let adapter_in = InterleavedSlice::new(input, channels, frames).map_err(|_| AudioErrorKind::Corrupt)?;
            let capacity = self.resampled.len() / channels;
            let mut adapter_out = InterleavedSlice::new_mut(&mut self.resampled, channels, capacity)
                .map_err(|_| AudioErrorKind::Corrupt)?;
            let mut indexing = Indexing::new();
            indexing.partial_len = (frames < needed).then_some(frames);
            let (_, produced) = resampler
                .process_into_buffer(&adapter_in, &mut adapter_out, Some(&indexing))
                .map_err(|_| AudioErrorKind::Corrupt)?;
            self.input_start += frames * channels;
            let skip = self.delay_left.min(produced);
            self.delay_left -= skip;
            let mut take = (produced - skip) as u64;
            if draining {
                take = take.min(expected.saturating_sub(self.frames_out));
            }
            self.frames_out += take;
            let start = skip * channels;
            out.extend_from_slice(&self.resampled[start..start + take as usize * channels]);
        }
        self.mixed.drain(..self.input_start);
        self.input_start = 0;
        Ok(())
    }
}

/// Nối `input` đã đổi sang `target` kênh vào `out`. Downmix giữ tâm và sau ở -3 dB, chuẩn hoá để khỏi vỡ tiếng.
fn mix_channels(input: &[f32], source: usize, target: usize, out: &mut Vec<f32>) {
    const SIDE: f32 = std::f32::consts::FRAC_1_SQRT_2;
    if source == target {
        out.extend_from_slice(input);
        return;
    }
    out.reserve(input.len() / source * target);
    for frame in input.chunks_exact(source) {
        match (source, target) {
            (1, _) => {
                let copies = target.min(2);
                out.extend(std::iter::repeat_n(frame[0], copies));
                out.extend(std::iter::repeat_n(0.0, target - copies));
            }
            (_, 1) => out.push(frame.iter().sum::<f32>() / source as f32),
            (2.., 2) => {
                // Thứ tự symphonia: FL FR FC LFE (RL RR, SL SR). Bỏ LFE như downmix thông thường.
                let center = frame.get(2).copied().unwrap_or(0.0) * SIDE;
                let (left, right) = frame[4..].chunks(2).fold((0.0, 0.0), |(l, r), pair| {
                    (l + pair[0] * SIDE, r + pair.get(1).copied().unwrap_or(0.0) * SIDE)
                });
                let norm = 1.0 + SIDE + SIDE * ((source.saturating_sub(4)).div_ceil(2)) as f32;
                out.push((frame[0] + center + left) / norm);
                out.push((frame[1] + center + right) / norm);
            }
            _ => {
                let shared = source.min(target);
                out.extend_from_slice(&frame[..shared]);
                out.extend(std::iter::repeat_n(0.0, target - shared));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEREO_48K: OutputConfig = OutputConfig { sample_rate: 48_000, channels: 2 };

    #[test]
    fn channel_mixing() {
        let mut out = Vec::new();
        mix_channels(&[0.5, -0.5], 1, 2, &mut out);
        assert_eq!(out, [0.5, 0.5, -0.5, -0.5]);
        out.clear();
        mix_channels(&[0.5], 1, 4, &mut out);
        assert_eq!(out, [0.5, 0.5, 0.0, 0.0]);
        out.clear();
        mix_channels(&[0.2, 0.4], 2, 1, &mut out);
        assert!((out[0] - 0.3).abs() < 1e-6);
        out.clear();
        mix_channels(&[0.1, 0.2], 2, 4, &mut out);
        assert_eq!(out, [0.1, 0.2, 0.0, 0.0]);
        out.clear();
        mix_channels(&[1.0; 6], 6, 2, &mut out);
        assert_eq!(out.len(), 2);
        assert!(out[0] <= 1.0 && out[0] > 0.9, "5.1 at full scale stays in range: {}", out[0]);
    }

    #[test]
    fn same_rate_passes_samples_through() {
        let source = SourceSpec { sample_rate: 48_000, channels: 2 };
        let mut converter = Converter::new(source, STEREO_48K).unwrap();
        let mut out = Vec::new();
        converter.push(&[0.1, 0.2, 0.3, 0.4], &mut out).unwrap();
        converter.finish(&mut out).unwrap();
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
    }

    fn resample_sine(from: u32, to: u32, frames: usize, freq: f64) -> Vec<f32> {
        let source = SourceSpec { sample_rate: from, channels: 1 };
        let target = OutputConfig { sample_rate: to, channels: 1 };
        let mut converter = Converter::new(source, target).unwrap();
        let input: Vec<f32> = (0..frames)
            .map(|i| (i as f64 * freq * std::f64::consts::TAU / f64::from(from)).sin() as f32 * 0.5)
            .collect();
        let mut out = Vec::new();
        for chunk in input.chunks(777) {
            converter.push(chunk, &mut out).unwrap();
        }
        converter.finish(&mut out).unwrap();
        out
    }

    #[test]
    fn resampling_keeps_length_pitch_and_alignment() {
        for (from, to) in [(44_100, 48_000), (48_000, 44_100), (96_000, 48_000), (22_050, 48_000)] {
            let frames = from as usize;
            let out = resample_sine(from, to, frames, 440.0);
            let expected = (frames as f64 * f64::from(to) / f64::from(from)).round() as usize;
            assert_eq!(out.len(), expected, "{from}->{to}: no tail lost, no padding");
            let middle = &out[to as usize / 4..to as usize * 3 / 4];
            let crossings = middle.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!((219..=221).contains(&crossings), "{from}->{to}: pitch kept, got {crossings}");
            // Trễ của bộ đổi đã được bỏ: pha khớp sin gốc tại cùng thời điểm.
            let at = to as usize / 2;
            let want = (at as f64 * 440.0 * std::f64::consts::TAU / f64::from(to)).sin() as f32 * 0.5;
            assert!((out[at] - want).abs() < 0.02, "{from}->{to}: aligned, {} vs {want}", out[at]);
        }
    }

    #[test]
    fn resampler_reset_drops_buffered_audio() {
        let source = SourceSpec { sample_rate: 44_100, channels: 2 };
        let mut converter = Converter::new(source, STEREO_48K).unwrap();
        let mut out = Vec::new();
        converter.push(&vec![0.5; 3000], &mut out).unwrap();
        converter.reset();
        out.clear();
        converter.push(&vec![0.0; 44_100 * 2], &mut out).unwrap();
        converter.finish(&mut out).unwrap();
        assert_eq!(out.len(), 48_000 * 2);
        assert!(out.iter().all(|s| s.abs() < 1e-4), "nothing from before the reset");
    }

    #[test]
    fn missing_and_garbage_files_are_errors() {
        let dir = std::env::temp_dir();
        let missing = dir.join("groove-audio-does-not-exist.mp3");
        assert_eq!(TrackDecoder::open(&missing).err(), Some(AudioErrorKind::NotFound));
        let garbage = dir.join(format!("groove-audio-garbage-{}.mp3", std::process::id()));
        std::fs::write(&garbage, vec![0x5Au8; 4096]).unwrap();
        let result = TrackDecoder::open(&garbage).err();
        std::fs::remove_file(&garbage).unwrap();
        assert_eq!(result, Some(AudioErrorKind::Unsupported));
    }
}
