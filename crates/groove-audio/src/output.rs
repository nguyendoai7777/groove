//! groove-gd1_10: lớp xuất âm sau trait, ring và callback thực thời.
//!
//! [`Renderer::render`] là phần chạy trong callback: chỉ atomic và ring SPSC, không khoá, không cấp phát, không chạm đĩa
//! (`tests/realtime_no_alloc.rs` đếm cấp phát thật). Xoá ring không cần callback xác nhận: luồng decoder ghi số thứ tự
//! mẫu cuối cần bỏ vào `discard_until`, callback bỏ đúng tới đó, mẫu mới đẩy ngay sau không bị mất.
//!
//! cpal chỉ có ở [`default_output`]. Linux đi qua feature `linux-alsa`, bật mặc định: `libasound` (LGPL) link động, không
//! kèm vào bản cài (G-2). Tắt feature thì nhánh còn lại trả [`OutputError`]. Test dùng [`FakeOutput`], không phát ra loa.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use ringbuf::traits::{Consumer, Observer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Độ dài ring: đổi EQ trễ bằng từng này (đặc tả 5.1).
pub(crate) const RING_MS: u64 = 200;
/// Thiết bị có chu kỳ lớn vẫn phải đủ chỗ cho vài lần gọi callback.
const MIN_RING_FRAMES: usize = 4096;
/// Fade khi tua, đổi bài, dừng, phát lại để khỏi lách tách.
const FADE_SEC: f32 = 0.005;
/// Hằng thời gian của âm lượng, bằng `setTargetAtTime(…, 0.01)` của bản cũ.
const VOLUME_TAU_SEC: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct OutputError(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamFailure {
    /// Thiết bị mất, hoặc thiết bị mặc định đã đổi: stream này không dùng được nữa.
    DeviceLost,
    /// Hệ điều hành đã tự chuyển đường ra, stream còn sống; cấu hình có thể đã đổi.
    Rerouted,
}

pub type FailureCallback = Box<dyn FnMut(StreamFailure) + Send + 'static>;

/// Stream đang mở. Drop là đóng, và sau khi drop xong callback không còn chạy. Không đòi `Send` (`cpal::Stream` không
/// `Send` trên một số OS): tạo và huỷ cùng trên luồng decoder.
pub trait OutputStream {}

pub trait AudioOutput: Send {
    /// Cấu hình của thiết bị mặc định lúc này.
    fn default_config(&mut self) -> Result<OutputConfig, OutputError>;
    /// Mở stream trên thiết bị mặc định và bắt đầu gọi `renderer`.
    fn open(
        &mut self,
        config: OutputConfig,
        renderer: Renderer,
        on_failure: FailureCallback,
    ) -> Result<Box<dyn OutputStream>, OutputError>;
}

/// Điều khiển đọc từ callback, ghi từ luồng khác.
pub(crate) struct Controls {
    /// Bit của `f32`, đã qua đường cong `v²` (S23).
    gain_bits: AtomicU32,
    muted: AtomicBool,
    paused: AtomicBool,
}

impl Controls {
    pub(crate) fn new() -> Self {
        Self { gain_bits: AtomicU32::new(1f32.to_bits()), muted: AtomicBool::new(false), paused: AtomicBool::new(true) }
    }

    pub(crate) fn set_volume(&self, volume: f32) {
        let volume = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { 1.0 };
        self.gain_bits.store((volume * volume).to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    fn target_gain(&self) -> f32 {
        if self.muted.load(Ordering::Relaxed) { 0.0 } else { f32::from_bits(self.gain_bits.load(Ordering::Relaxed)) }
    }
}

/// Bộ đếm theo mẫu của một ring, tính từ 0 khi ring được tạo.
#[derive(Default)]
pub(crate) struct RingCounters {
    /// Mẫu đã rời ring, kể cả mẫu bị bỏ. Chỉ callback ghi.
    pub(crate) popped: AtomicU64,
    /// Mẫu có số thứ tự nhỏ hơn đây là của bài hoặc vị trí cũ. Chỉ luồng decoder ghi.
    pub(crate) discard_until: AtomicU64,
}

/// Chạy trong callback của thiết bị, sở hữu đầu đọc của ring.
pub struct Renderer {
    consumer: HeapCons<f32>,
    counters: Arc<RingCounters>,
    controls: Arc<Controls>,
    channels: usize,
    fade_step: f32,
    fade: f32,
    volume_coeff: f32,
    gain: f32,
    popped: u64,
}

impl Renderer {
    /// Ghi `out` xen kẽ theo kênh. Thiếu dữ liệu thì ghi im lặng, không tính vào vị trí đã phát.
    pub fn render(&mut self, out: &mut [f32]) {
        let discard_until = self.counters.discard_until.load(Ordering::Acquire);
        let paused = self.controls.is_paused();
        let target_gain = self.controls.target_gain();
        let channels = self.channels;
        let mut frames = out.chunks_exact_mut(channels);
        for frame in &mut frames {
            let mut flushing = self.popped < discard_until;
            if (flushing || paused) && self.fade <= 0.0 {
                if flushing {
                    let stale = (discard_until - self.popped) as usize;
                    self.popped += self.consumer.skip(stale) as u64;
                    flushing = self.popped < discard_until;
                }
                if flushing || paused {
                    frame.fill(0.0);
                    continue;
                }
            }
            if self.consumer.occupied_len() < channels {
                frame.fill(0.0);
                self.fade = 0.0;
                continue;
            }
            self.consumer.pop_slice(frame);
            self.popped += channels as u64;
            self.fade = if flushing || paused {
                (self.fade - self.fade_step).max(0.0)
            } else {
                (self.fade + self.fade_step).min(1.0)
            };
            self.gain += (target_gain - self.gain) * self.volume_coeff;
            let scale = self.fade * self.gain;
            for sample in frame.iter_mut() {
                *sample = (*sample * scale).clamp(-1.0, 1.0);
            }
        }
        frames.into_remainder().fill(0.0);
        self.counters.popped.store(self.popped, Ordering::Release);
    }
}

pub(crate) fn new_ring(config: OutputConfig, controls: Arc<Controls>) -> (HeapProd<f32>, Renderer, Arc<RingCounters>) {
    let channels = usize::from(config.channels.max(1));
    let frames = (u64::from(config.sample_rate) * RING_MS / 1000) as usize;
    let (producer, consumer) = HeapRb::<f32>::new(frames.max(MIN_RING_FRAMES) * channels).split();
    let counters = Arc::new(RingCounters::default());
    let rate = config.sample_rate.max(1) as f32;
    let gain = controls.target_gain();
    let renderer = Renderer {
        consumer,
        counters: counters.clone(),
        controls,
        channels,
        fade_step: 1.0 / (rate * FADE_SEC).max(1.0),
        fade: 0.0,
        volume_coeff: 1.0 - (-1.0 / (rate * VOLUME_TAU_SEC)).exp(),
        gain,
        popped: 0,
    };
    (producer, renderer, counters)
}

/// Dựng một [`Renderer`] có sẵn dữ liệu, để đo ngoài luồng phát (`tests/realtime_no_alloc.rs`).
#[doc(hidden)]
pub fn renderer_for_alloc_test(config: OutputConfig, samples: &[f32]) -> Renderer {
    use ringbuf::traits::Producer;
    let controls = Arc::new(Controls::new());
    controls.set_paused(false);
    let (mut producer, renderer, _) = new_ring(config, controls);
    producer.push_slice(samples);
    renderer
}

/// Đường ra thật của bản phát hành.
pub fn default_output() -> Box<dyn AudioOutput> {
    Box::new(platform::PlatformOutput)
}

#[cfg(any(not(target_os = "linux"), feature = "linux-alsa"))]
mod platform {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{ErrorKind, FromSample, Sample, SampleFormat, SizedSample};

    use super::{AudioOutput, FailureCallback, OutputConfig, OutputError, OutputStream, Renderer, StreamFailure};

    /// Đủ cho chu kỳ thường gặp; chu kỳ lớn hơn thì `render` nhiều lần, callback không cấp phát.
    const SCRATCH_SAMPLES: usize = 8192;

    pub(super) struct PlatformOutput;

    struct CpalStream {
        _stream: cpal::Stream,
    }

    impl OutputStream for CpalStream {}

    fn device() -> Result<cpal::Device, OutputError> {
        cpal::default_host().default_output_device().ok_or_else(|| OutputError("no default output device".into()))
    }

    fn error(err: impl std::fmt::Display) -> OutputError {
        OutputError(err.to_string())
    }

    impl AudioOutput for PlatformOutput {
        fn default_config(&mut self) -> Result<OutputConfig, OutputError> {
            let config = device()?.default_output_config().map_err(error)?;
            Ok(OutputConfig { sample_rate: config.sample_rate(), channels: config.channels() })
        }

        fn open(
            &mut self,
            config: OutputConfig,
            renderer: Renderer,
            on_failure: FailureCallback,
        ) -> Result<Box<dyn OutputStream>, OutputError> {
            let device = device()?;
            let supported = device.default_output_config().map_err(error)?;
            let mut stream_config = supported.config();
            stream_config.sample_rate = config.sample_rate;
            stream_config.channels = config.channels;
            let stream = match supported.sample_format() {
                SampleFormat::F32 => build::<f32>(&device, stream_config, renderer, on_failure),
                SampleFormat::F64 => build::<f64>(&device, stream_config, renderer, on_failure),
                SampleFormat::I16 => build::<i16>(&device, stream_config, renderer, on_failure),
                SampleFormat::I32 => build::<i32>(&device, stream_config, renderer, on_failure),
                SampleFormat::U16 => build::<u16>(&device, stream_config, renderer, on_failure),
                SampleFormat::U8 => build::<u8>(&device, stream_config, renderer, on_failure),
                other => Err(OutputError(format!("unsupported sample format {other}"))),
            }?;
            stream.play().map_err(error)?;
            Ok(Box::new(CpalStream { _stream: stream }))
        }
    }

    fn build<T>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        mut renderer: Renderer,
        mut on_failure: FailureCallback,
    ) -> Result<cpal::Stream, OutputError>
    where
        T: SizedSample + FromSample<f32>,
    {
        let channels = usize::from(config.channels.max(1));
        let mut scratch = vec![0f32; SCRATCH_SAMPLES / channels * channels];
        device
            .build_output_stream::<T, _, _>(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    for chunk in data.chunks_mut(scratch.len()) {
                        let source = &mut scratch[..chunk.len()];
                        renderer.render(source);
                        for (out, sample) in chunk.iter_mut().zip(source.iter()) {
                            *out = T::from_sample(*sample);
                        }
                    }
                },
                move |err: cpal::Error| match err.kind() {
                    ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated | ErrorKind::BackendError => {
                        on_failure(StreamFailure::DeviceLost);
                    }
                    ErrorKind::DeviceChanged => on_failure(StreamFailure::Rerouted),
                    _ => log::debug!("audio stream: {err}"),
                },
                None,
            )
            .map_err(error)
    }
}

#[cfg(not(any(not(target_os = "linux"), feature = "linux-alsa")))]
mod platform {
    use super::{AudioOutput, FailureCallback, OutputConfig, OutputError, OutputStream, Renderer};

    const DISABLED: &str = "audio output needs feature linux-alsa on Linux";

    pub(super) struct PlatformOutput;

    impl AudioOutput for PlatformOutput {
        fn default_config(&mut self) -> Result<OutputConfig, OutputError> {
            Err(OutputError(DISABLED.into()))
        }

        fn open(
            &mut self,
            _config: OutputConfig,
            _renderer: Renderer,
            _on_failure: FailureCallback,
        ) -> Result<Box<dyn OutputStream>, OutputError> {
            Err(OutputError(DISABLED.into()))
        }
    }
}

/// Đường ra giả cho test và cho task sau: không chạm thiết bị, người gọi tự kéo mẫu bằng [`FakeOutput::pull`].
#[derive(Clone)]
pub struct FakeOutput {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    config: OutputConfig,
    renderer: Option<Renderer>,
    on_failure: Option<FailureCallback>,
    opens: usize,
    open_failures: usize,
}

struct FakeStream {
    state: Arc<Mutex<FakeState>>,
}

impl OutputStream for FakeStream {}

impl Drop for FakeStream {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        state.renderer = None;
        state.on_failure = None;
    }
}

fn lock(state: &Mutex<FakeState>) -> MutexGuard<'_, FakeState> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeOutput {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let state = FakeState {
            config: OutputConfig { sample_rate, channels },
            renderer: None,
            on_failure: None,
            opens: 0,
            open_failures: 0,
        };
        Self { state: Arc::new(Mutex::new(state)) }
    }

    /// Gọi callback một lần. Không có stream thì trả `None`.
    pub fn pull(&self, frames: usize) -> Option<Vec<f32>> {
        let mut state = lock(&self.state);
        let channels = usize::from(state.config.channels);
        let renderer = state.renderer.as_mut()?;
        let mut out = vec![f32::NAN; frames * channels];
        renderer.render(&mut out);
        Some(out)
    }

    pub fn is_open(&self) -> bool {
        lock(&self.state).renderer.is_some()
    }

    /// Số lần mở stream thành công.
    pub fn open_count(&self) -> usize {
        lock(&self.state).opens
    }

    /// `count` lần mở tới sẽ lỗi, như lúc không còn thiết bị nào.
    pub fn fail_next_opens(&self, count: usize) {
        lock(&self.state).open_failures = count;
    }

    /// Đổi cấu hình của thiết bị mặc định, có hiệu lực từ lần mở sau.
    pub fn set_config(&self, sample_rate: u32, channels: u16) {
        lock(&self.state).config = OutputConfig { sample_rate, channels };
    }

    /// Báo lỗi stream như khi rút tai nghe.
    pub fn trigger_failure(&self, failure: StreamFailure) {
        let callback = lock(&self.state).on_failure.take();
        if let Some(mut callback) = callback {
            callback(failure);
            let mut state = lock(&self.state);
            if state.renderer.is_some() && state.on_failure.is_none() {
                state.on_failure = Some(callback);
            }
        }
    }
}

impl AudioOutput for FakeOutput {
    fn default_config(&mut self) -> Result<OutputConfig, OutputError> {
        let state = lock(&self.state);
        if state.open_failures > 0 {
            return Err(OutputError("no device".into()));
        }
        Ok(state.config)
    }

    fn open(
        &mut self,
        _config: OutputConfig,
        renderer: Renderer,
        on_failure: FailureCallback,
    ) -> Result<Box<dyn OutputStream>, OutputError> {
        let mut state = lock(&self.state);
        if state.open_failures > 0 {
            state.open_failures -= 1;
            return Err(OutputError("no device".into()));
        }
        state.opens += 1;
        state.renderer = Some(renderer);
        state.on_failure = Some(on_failure);
        Ok(Box::new(FakeStream { state: self.state.clone() }))
    }
}

#[cfg(test)]
mod tests {
    use ringbuf::traits::Producer;

    use super::*;

    const CONFIG: OutputConfig = OutputConfig { sample_rate: 48_000, channels: 2 };
    const FADE_FRAMES: usize = 240;

    fn setup(frames: usize) -> (HeapProd<f32>, Renderer, Arc<RingCounters>, Arc<Controls>) {
        let controls = Arc::new(Controls::new());
        controls.set_paused(false);
        let (mut producer, renderer, counters) = new_ring(CONFIG, controls.clone());
        producer.push_slice(&vec![0.5; frames * 2]);
        (producer, renderer, counters, controls)
    }

    fn render(renderer: &mut Renderer, frames: usize) -> Vec<f32> {
        let mut out = vec![9.0; frames * 2];
        renderer.render(&mut out);
        out
    }

    #[test]
    fn fades_in_then_reaches_full_level() {
        let (_producer, mut renderer, counters, _) = setup(1000);
        let out = render(&mut renderer, 1000);
        assert!(out[0] < 0.01, "first frame {}", out[0]);
        assert!(out[FADE_FRAMES / 2 * 2] < 0.3);
        assert!((out[1990] - 0.5).abs() < 1e-3);
        assert_eq!(counters.popped.load(Ordering::Acquire), 2000);
    }

    #[test]
    fn pause_fades_out_and_stops_consuming() {
        let (_producer, mut renderer, counters, controls) = setup(2000);
        render(&mut renderer, 500);
        controls.set_paused(true);
        let out = render(&mut renderer, 500);
        assert!(out[0] > 0.4);
        assert_eq!(out[(FADE_FRAMES + 10) * 2], 0.0);
        assert_eq!(counters.popped.load(Ordering::Acquire), ((500 + FADE_FRAMES) * 2) as u64);
        render(&mut renderer, 500);
        assert_eq!(counters.popped.load(Ordering::Acquire), ((500 + FADE_FRAMES) * 2) as u64);
    }

    #[test]
    fn flush_fades_out_old_data_and_keeps_data_pushed_after_marker() {
        let (mut producer, mut renderer, counters, _) = setup(1000);
        render(&mut renderer, 500);
        counters.discard_until.store(2000, Ordering::Release);
        producer.push_slice(&vec![-0.5; 400]);
        let out = render(&mut renderer, 700);
        assert!(out[0] > 0.4, "old data still fades out");
        let new_samples = out.iter().filter(|s| **s < 0.0).count();
        assert_eq!(new_samples, 398, "only the first fade-in frame is exactly zero");
        assert_eq!(counters.popped.load(Ordering::Acquire), 2400);
    }

    #[test]
    fn underrun_writes_silence_and_is_not_counted() {
        let (_producer, mut renderer, counters, _) = setup(10);
        let out = render(&mut renderer, 20);
        assert_eq!(&out[20..], &[0.0; 20]);
        assert_eq!(counters.popped.load(Ordering::Acquire), 20);
    }

    #[test]
    fn mute_and_volume_ramp_without_a_step() {
        let (mut producer, mut renderer, _, controls) = setup(2000);
        render(&mut renderer, 1000);
        controls.set_volume(0.5);
        let out = render(&mut renderer, 1000);
        assert!((out[0] - 0.5).abs() < 0.02, "no jump: {}", out[0]);
        assert!((out[1998] - 0.125).abs() < 1e-3, "v² curve: {}", out[1998]);
        controls.set_muted(true);
        producer.push_slice(&vec![0.5; 4000]);
        let out = render(&mut renderer, 2000);
        assert!(out[3998].abs() < 1e-4);
    }

    #[test]
    fn volume_ignores_invalid_values_and_output_is_clamped() {
        let controls = Arc::new(Controls::new());
        controls.set_paused(false);
        controls.set_volume(f32::NAN);
        assert_eq!(controls.target_gain(), 1.0);
        controls.set_volume(7.0);
        assert_eq!(controls.target_gain(), 1.0);
        let (mut producer, mut renderer, _) = new_ring(CONFIG, controls);
        producer.push_slice(&vec![4.0; 2000]);
        let out = render(&mut renderer, 1000);
        assert!(out.iter().all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn fake_output_needs_open_stream() {
        let fake = FakeOutput::new(48_000, 2);
        assert!(fake.pull(10).is_none());
        assert!(!fake.is_open());
    }
}
