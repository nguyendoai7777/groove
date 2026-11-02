//! groove-gd1_10: AudioCmd, AudioEvent, luồng điều khiển.
//!
//! ```text
//! UI ─ AudioCmd ─▶ luồng decoder ─ ring f32 ─▶ callback thực thời (output.rs)
//! UI ◀ AudioEvent ─ luồng decoder              popped: AtomicU64, mẫu đã phát
//! ```
//!
//! Luồng decoder là chủ duy nhất của decoder, EQ, đầu ghi ring và stream. Mỗi vòng: gom hết lệnh đang chờ (Load liên tiếp
//! chỉ giữ cái cuối, nên Next nhanh chỉ giải mã bài cuối), xử lý sự cố stream, rồi giải mã đến khi ring đầy.
//!
//! - **Chống race**: [`AudioEvents`] bỏ sự kiện của `generation` cũ ở phía nhận; `generation` mới nhất được ghi ngay lúc gửi
//!   lệnh, nên sự kiện đang nằm trong kênh cũng bị chặn.
//! - **Vị trí**: tính từ số mẫu đã rời ring, nên đã trừ sẵn độ trễ ring (đặc tả 5.1).
//! - **Hết bài**: `Ended` khi decoder xong, bộ đệm trung gian hết và callback đã lấy hết ring.
//! - **Đổi thiết bị**: stream lỗi hoặc đứng thì bỏ ring cũ, mở lại trên thiết bị mặc định, tua decoder về vị trí đang nghe.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::{Stream, StreamExt};
use ringbuf::HeapProd;
use ringbuf::traits::{Observer, Producer};

pub use crate::decoder::AudioErrorKind;
use crate::decoder::{Converter, TrackDecoder};
pub use crate::dsp::EqParams;
use crate::dsp::Equalizer;
use crate::output::{AudioOutput, Controls, OutputConfig, OutputStream, RingCounters, StreamFailure, new_ring};

#[derive(Clone, Debug, PartialEq)]
pub enum AudioCmd {
    /// `generation` phải tăng dần giữa các lần Load, lấy từ [`AudioEngine::next_generation`].
    Load {
        generation: u64,
        path: PathBuf,
        start_sec: f64,
        autoplay: bool,
    },
    Play,
    Pause,
    /// Lệnh của `generation` khác bài hiện tại bị bỏ. Tua không đổi trạng thái phát.
    Seek {
        generation: u64,
        sec: f64,
    },
    /// 0..=1, độ lớn thật là `v²` (S23).
    SetVolume(f32),
    SetMuted(bool),
    SetEq(EqParams),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioEvent {
    /// `duration_sec` là 0 khi tệp không khai độ dài.
    Loaded {
        generation: u64,
        duration_sec: f64,
    },
    Ended {
        generation: u64,
    },
    Error {
        generation: u64,
        kind: AudioErrorKind,
    },
    DeviceChanged,
}

impl AudioEvent {
    fn generation(&self) -> Option<u64> {
        match self {
            Self::Loaded { generation, .. } | Self::Ended { generation } | Self::Error { generation, .. } => {
                Some(*generation)
            }
            Self::DeviceChanged => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EngineOptions {
    /// Dừng từng này thì đóng stream, giữ vị trí (đặc tả 5.1: RAM).
    pub idle_close_after: Duration,
    /// Đang phát mà callback không lấy mẫu trong từng này thì coi stream hỏng, dù thiết bị không báo lỗi.
    pub stall_timeout: Duration,
    /// Mở lại stream hỏng cách nhau từng này, tổng cộng `reopen_attempts` lần rồi báo `Error`.
    pub reopen_interval: Duration,
    pub reopen_attempts: u32,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            idle_close_after: Duration::from_secs(30),
            stall_timeout: Duration::from_secs(3),
            reopen_interval: Duration::from_millis(500),
            reopen_attempts: 6,
        }
    }
}

enum Message {
    Cmd(AudioCmd),
    StreamFailed { stream_id: u64, failure: StreamFailure },
    Shutdown,
}

/// Mốc tính vị trí. Chỉ luồng decoder và UI đụng khoá này, callback chỉ ghi atomic `popped`.
#[derive(Default)]
struct Anchor {
    ring: Option<Arc<RingCounters>>,
    /// Số thứ tự mẫu trong ring ứng với `base_sec`.
    marker: u64,
    base_sec: f64,
    samples_per_sec: f64,
    duration_sec: f64,
}

impl Anchor {
    fn position_sec(&self) -> f64 {
        let played =
            self.ring.as_ref().map_or(0, |ring| ring.popped.load(Ordering::Acquire).saturating_sub(self.marker));
        let position = if played == 0 { self.base_sec } else { self.base_sec + played as f64 / self.samples_per_sec };
        if self.duration_sec > 0.0 { position.min(self.duration_sec) } else { position }
    }
}

struct Shared {
    latest_generation: AtomicU64,
    allocated_generation: AtomicU64,
    controls: Arc<Controls>,
    anchor: Mutex<Anchor>,
}

impl Shared {
    fn with_anchor<R>(&self, f: impl FnOnce(&mut Anchor) -> R) -> R {
        f(&mut self.anchor.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Đầu điều khiển, dùng từ UI thread. Mọi hàm đều không chặn. Drop thì dừng luồng và đóng stream.
pub struct AudioEngine {
    tx: Sender<Message>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl AudioEngine {
    /// Khởi động luồng decoder, chưa mở stream. Bản phát hành truyền [`crate::output::default_output`].
    pub fn spawn(output: Box<dyn AudioOutput>, options: EngineOptions) -> std::io::Result<(Self, AudioEvents)> {
        let shared = Arc::new(Shared {
            latest_generation: AtomicU64::new(0),
            allocated_generation: AtomicU64::new(0),
            controls: Arc::new(Controls::new()),
            anchor: Mutex::new(Anchor::default()),
        });
        let (tx, rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = unbounded();
        let (worker_shared, worker_tx) = (shared.clone(), tx.clone());
        // Dựng trong luồng: stream không `Send`, chỉ sống trên luồng này.
        let thread = std::thread::Builder::new()
            .name("groove-audio".into())
            .spawn(move || Worker::new(output, options, worker_shared, worker_tx, rx, event_tx).run())?;
        let events = AudioEvents { rx: event_rx, shared: shared.clone() };
        Ok((Self { tx, shared, thread: Some(thread) }, events))
    }

    /// Cấp `generation` cho lần Load kế tiếp.
    pub fn next_generation(&self) -> u64 {
        self.shared.allocated_generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn send(&self, cmd: AudioCmd) {
        // Có hiệu lực ngay, kể cả khi luồng decoder đang bận mở một tệp chậm.
        match &cmd {
            AudioCmd::Load { generation, .. } => {
                self.shared.latest_generation.fetch_max(*generation, Ordering::AcqRel);
            }
            AudioCmd::SetVolume(volume) => self.shared.controls.set_volume(*volume),
            AudioCmd::SetMuted(muted) => self.shared.controls.set_muted(*muted),
            AudioCmd::Pause => self.shared.controls.set_paused(true),
            _ => {}
        }
        let _ = self.tx.send(Message::Cmd(cmd));
    }

    /// Vị trí đang nghe, đã trừ phần còn trong ring.
    pub fn position_sec(&self) -> f64 {
        self.shared.with_anchor(|anchor| anchor.position_sec())
    }

    /// Nguồn duy nhất của trạng thái phát (đặc tả mục 3): sau `Ended`, `Error` thì về `false`.
    pub fn is_playing(&self) -> bool {
        !self.shared.controls.is_paused()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Đầu nhận sự kiện, đã lọc `generation` cũ. Dùng `next().await` hoặc [`AudioEvents::try_recv`].
pub struct AudioEvents {
    rx: UnboundedReceiver<AudioEvent>,
    shared: Arc<Shared>,
}

impl AudioEvents {
    fn is_current(&self, event: &AudioEvent) -> bool {
        event.generation().is_none_or(|g| g == self.shared.latest_generation.load(Ordering::Acquire))
    }

    pub fn try_recv(&mut self) -> Option<AudioEvent> {
        while let Ok(event) = self.rx.try_recv() {
            if self.is_current(&event) {
                return Some(event);
            }
        }
        None
    }
}

impl Stream for AudioEvents {
    type Item = AudioEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<AudioEvent>> {
        let this = self.get_mut();
        loop {
            match this.rx.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) if !this.is_current(&event) => {}
                other => return other,
            }
        }
    }
}

struct ActiveStream {
    id: u64,
    config: OutputConfig,
    producer: HeapProd<f32>,
    counters: Arc<RingCounters>,
    pushed: u64,
    last_popped: u64,
    last_progress: Instant,
    _stream: Box<dyn OutputStream>,
}

struct Track {
    generation: u64,
    decoder: TrackDecoder,
    /// Decoder đã đọc hết và đã xả bộ đổi tần số.
    decoded_all: bool,
    ended_sent: bool,
    /// Stream đã mở lại: decoder phải về vị trí đang nghe trước khi đọc tiếp.
    needs_reposition: bool,
}

struct LoadRequest {
    generation: u64,
    path: PathBuf,
    start_sec: f64,
    autoplay: bool,
}

struct Worker {
    output: Box<dyn AudioOutput>,
    options: EngineOptions,
    shared: Arc<Shared>,
    self_tx: Sender<Message>,
    rx: Receiver<Message>,
    events: UnboundedSender<AudioEvent>,
    equalizer: Equalizer,
    stream: Option<ActiveStream>,
    next_stream_id: u64,
    track: Option<Track>,
    current_generation: u64,
    converter: Option<Converter>,
    decoded: Vec<f32>,
    pending: Vec<f32>,
    pending_start: usize,
    idle_since: Option<Instant>,
    /// Số lần còn được thử mở lại, và lúc thử kế tiếp.
    reopen: Option<(u32, Instant)>,
    pending_load: Option<LoadRequest>,
    pending_seek: Option<f64>,
    pending_play: Option<bool>,
}

impl Worker {
    fn new(
        output: Box<dyn AudioOutput>,
        options: EngineOptions,
        shared: Arc<Shared>,
        self_tx: Sender<Message>,
        rx: Receiver<Message>,
        events: UnboundedSender<AudioEvent>,
    ) -> Self {
        Self {
            output,
            options,
            shared,
            self_tx,
            rx,
            events,
            equalizer: Equalizer::new(48_000, 2),
            stream: None,
            next_stream_id: 0,
            track: None,
            current_generation: 0,
            converter: None,
            decoded: Vec::new(),
            pending: Vec::new(),
            pending_start: 0,
            idle_since: None,
            reopen: None,
            pending_load: None,
            pending_seek: None,
            pending_play: None,
        }
    }

    fn run(mut self) {
        let mut wait = Duration::ZERO;
        loop {
            // Còn việc thì không gọi `recv_timeout`, kể cả với 0: nó `snooze` (có `yield_now`) trước khi xét hạn giờ, máy bận là
            // mỗi gói giải mã phải xếp hàng lại xin CPU, decoder tụt xuống dưới tốc độ phát.
            if !wait.is_zero() {
                match self.rx.recv_timeout(wait) {
                    Ok(message) => {
                        if self.collect(message) {
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            while let Ok(message) = self.rx.try_recv() {
                if self.collect(message) {
                    return;
                }
            }
            self.apply_pending();
            self.check_stream_health();
            let progressed = self.pump();
            self.check_ended();
            self.check_idle();
            wait = self.next_wait(progressed);
        }
    }

    /// Thời gian ngủ khi không có lệnh. Lệnh mới luôn đánh thức ngay.
    fn next_wait(&self, progressed: bool) -> Duration {
        if progressed {
            return Duration::ZERO;
        }
        if let Some((_, at)) = self.reopen {
            return at.saturating_duration_since(Instant::now()).min(Duration::from_millis(50));
        }
        match (&self.stream, self.shared.controls.is_paused()) {
            (Some(_), false) => Duration::from_millis(5),
            (Some(_), true) => Duration::from_millis(100),
            // Không stream: không có gì tự đổi, chỉ lệnh mới đánh thức.
            (None, _) => Duration::from_secs(3600),
        }
    }

    /// Ghi nhận một thông điệp; `true` là phải dừng luồng.
    fn collect(&mut self, message: Message) -> bool {
        match message {
            Message::Shutdown => return true,
            Message::StreamFailed { stream_id, failure } => {
                if self.stream.as_ref().is_some_and(|stream| stream.id == stream_id) {
                    self.on_stream_failure(failure);
                }
            }
            Message::Cmd(cmd) => self.collect_cmd(cmd),
        }
        false
    }

    fn collect_cmd(&mut self, cmd: AudioCmd) {
        match cmd {
            AudioCmd::Load { generation, path, start_sec, autoplay } => {
                let newest = self.pending_load.as_ref().map_or(self.current_generation, |load| load.generation);
                if generation > newest {
                    self.pending_load = Some(LoadRequest { generation, path, start_sec, autoplay });
                    self.pending_seek = None;
                    self.pending_play = None;
                }
            }
            AudioCmd::Seek { generation, sec } => match &mut self.pending_load {
                Some(load) if load.generation == generation => load.start_sec = sec,
                None if generation == self.current_generation => self.pending_seek = Some(sec),
                _ => {}
            },
            AudioCmd::Play => self.pending_play = Some(true),
            AudioCmd::Pause => self.pending_play = Some(false),
            AudioCmd::SetEq(params) => self.equalizer.set_params(&params),
            // Đã ghi atomic ngay ở đầu gửi.
            AudioCmd::SetVolume(_) | AudioCmd::SetMuted(_) => {}
        }
    }

    fn apply_pending(&mut self) {
        if let Some(load) = self.pending_load.take() {
            self.load(load);
        }
        if let Some(sec) = self.pending_seek.take() {
            self.seek(sec);
        }
        match self.pending_play.take() {
            Some(true) => self.play(),
            Some(false) => self.pause(),
            None => {}
        }
    }

    fn emit(&self, event: AudioEvent) {
        let _ = self.events.unbounded_send(event);
    }

    fn set_paused(&mut self, paused: bool) {
        self.shared.controls.set_paused(paused);
        self.idle_since = paused.then(Instant::now);
    }

    /// Bỏ mọi thứ đang giữ của vị trí cũ, callback tự fade và xoá ring. `base_sec` là mốc mới.
    fn flush(&mut self, base_sec: f64) {
        self.pending.clear();
        self.pending_start = 0;
        self.equalizer.reset();
        if let Some(converter) = &mut self.converter {
            converter.reset();
        }
        let marker = self.stream.as_ref().map_or(0, |stream| {
            stream.counters.discard_until.store(stream.pushed, Ordering::Release);
            stream.pushed
        });
        self.shared.with_anchor(|anchor| {
            anchor.marker = marker;
            anchor.base_sec = base_sec;
        });
    }

    fn fail(&mut self, generation: u64, kind: AudioErrorKind) {
        log::warn!("audio error: {kind}");
        self.track = None;
        self.set_paused(true);
        self.emit(AudioEvent::Error { generation, kind });
    }

    fn load(&mut self, load: LoadRequest) {
        self.current_generation = load.generation;
        self.track = None;
        self.reopen = None;
        self.converter = None;
        let start_sec = sanitize_sec(load.start_sec);
        self.flush(start_sec);
        self.shared.with_anchor(|anchor| anchor.duration_sec = 0.0);
        let opened = guarded(|| open_at(&load.path, start_sec));
        match opened {
            Err(kind) => self.fail(load.generation, kind),
            Ok((decoder, base_sec)) => {
                let duration_sec = decoder.duration_sec();
                self.shared.with_anchor(|anchor| {
                    anchor.base_sec = base_sec;
                    anchor.duration_sec = duration_sec;
                });
                self.track = Some(Track {
                    generation: load.generation,
                    decoder,
                    decoded_all: false,
                    ended_sent: false,
                    needs_reposition: false,
                });
                self.emit(AudioEvent::Loaded { generation: load.generation, duration_sec });
                if load.autoplay {
                    self.play();
                } else {
                    self.pause();
                }
            }
        }
    }

    fn seek(&mut self, sec: f64) {
        let Some(track) = &mut self.track else { return };
        let duration = track.decoder.duration_sec();
        let mut target = sanitize_sec(sec);
        if duration > 0.0 {
            target = target.min(duration);
        }
        let generation = track.generation;
        track.decoded_all = false;
        track.ended_sent = false;
        track.needs_reposition = false;
        let decoder = &mut track.decoder;
        let result = guarded(|| decoder.seek(target));
        self.flush(target);
        if let Err(kind) = result {
            self.fail(generation, kind);
        }
    }

    fn play(&mut self) {
        let Some(track) = &mut self.track else { return };
        // Phát tiếp khi đã hết bài: báo hết bài lần nữa thay vì đứng im ở trạng thái đang phát.
        track.ended_sent = false;
        self.set_paused(false);
        let was_reopening = self.reopen.take().is_some();
        if self.stream.is_some() {
            return;
        }
        match self.open_stream() {
            Ok(()) if was_reopening => self.emit(AudioEvent::DeviceChanged),
            Ok(()) => {}
            Err(err) => {
                log::warn!("cannot open audio output: {err}");
                let generation = self.current_generation;
                self.set_paused(true);
                self.emit(AudioEvent::Error { generation, kind: AudioErrorKind::Output });
            }
        }
    }

    fn pause(&mut self) {
        self.set_paused(true);
    }

    fn open_stream(&mut self) -> Result<(), crate::output::OutputError> {
        let config = self.output.default_config()?;
        let (producer, renderer, counters) = new_ring(config, self.shared.controls.clone());
        self.next_stream_id += 1;
        let id = self.next_stream_id;
        let tx = self.self_tx.clone();
        let on_failure = Box::new(move |failure| {
            let _ = tx.send(Message::StreamFailed { stream_id: id, failure });
        });
        let stream = self.output.open(config, renderer, on_failure)?;
        self.stream = Some(ActiveStream {
            id,
            config,
            producer,
            counters: counters.clone(),
            pushed: 0,
            last_popped: 0,
            last_progress: Instant::now(),
            _stream: stream,
        });
        self.equalizer.set_format(config.sample_rate, usize::from(config.channels));
        self.converter = None;
        self.shared.with_anchor(|anchor| {
            anchor.ring = Some(counters);
            anchor.marker = 0;
            anchor.samples_per_sec = f64::from(config.sample_rate) * f64::from(config.channels.max(1));
        });
        Ok(())
    }

    /// Đóng stream, giữ vị trí. Decoder sẽ tua lại khi mở stream mới, phần trong ring không mất.
    fn close_stream(&mut self) {
        self.shared.with_anchor(|anchor| {
            anchor.base_sec = anchor.position_sec();
            anchor.ring = None;
            anchor.marker = 0;
        });
        self.stream = None;
        self.converter = None;
        self.pending = Vec::new();
        self.pending_start = 0;
        self.decoded = Vec::new();
        if let Some(track) = &mut self.track {
            track.needs_reposition = true;
        }
    }

    fn on_stream_failure(&mut self, failure: StreamFailure) {
        let Some(stream) = &self.stream else { return };
        if failure == StreamFailure::Rerouted && self.output.default_config().ok() == Some(stream.config) {
            self.emit(AudioEvent::DeviceChanged);
            return;
        }
        log::info!("audio stream lost, reopening on the default device");
        self.close_stream();
        self.reopen = Some((self.options.reopen_attempts, Instant::now()));
    }

    fn check_stream_health(&mut self) {
        let now = Instant::now();
        if let Some(stream) = &mut self.stream {
            let popped = stream.counters.popped.load(Ordering::Acquire);
            let waiting = !self.shared.controls.is_paused() && stream.producer.occupied_len() > 0;
            if popped != stream.last_popped || !waiting {
                stream.last_popped = popped;
                stream.last_progress = now;
            } else if now.duration_since(stream.last_progress) >= self.options.stall_timeout {
                log::warn!("audio callback stalled");
                self.on_stream_failure(StreamFailure::DeviceLost);
            }
        }
        let Some((attempts, at)) = self.reopen else { return };
        if now < at {
            return;
        }
        // Đang dừng thì chỉ báo: lần Play kế tiếp mở trên thiết bị mới.
        if self.shared.controls.is_paused() || self.track.is_none() {
            self.reopen = None;
            self.emit(AudioEvent::DeviceChanged);
            return;
        }
        match self.open_stream() {
            Ok(()) => {
                self.reopen = None;
                self.emit(AudioEvent::DeviceChanged);
            }
            Err(err) if attempts <= 1 => {
                log::warn!("cannot reopen audio output: {err}");
                self.reopen = None;
                let generation = self.current_generation;
                self.set_paused(true);
                self.emit(AudioEvent::Error { generation, kind: AudioErrorKind::Output });
            }
            Err(_) => self.reopen = Some((attempts - 1, now + self.options.reopen_interval)),
        }
    }

    /// Một bước: đẩy phần đang chờ, hoặc giải mã một gói. `false` là ring đầy hoặc không có gì để làm.
    fn pump(&mut self) -> bool {
        if self.stream.is_none() || self.track.is_none() {
            return false;
        }
        if self.pending_start < self.pending.len() {
            return self.push_pending();
        }
        self.pending.clear();
        self.pending_start = 0;
        self.reposition_if_needed();
        let (Some(track), Some(stream)) = (&mut self.track, &self.stream) else { return false };
        if track.decoded_all {
            return false;
        }
        let target = stream.config;
        let decoder = &mut track.decoder;
        let decoded = &mut self.decoded;
        let generation = track.generation;
        let converter = &mut self.converter;
        let pending = &mut self.pending;
        let result = guarded(|| {
            let spec = decoder.next_frames(decoded)?;
            match spec {
                Some(spec) => {
                    if converter.as_ref().is_none_or(|c| c.source() != spec) {
                        if let Some(old) = converter {
                            old.finish(pending)?;
                        }
                        *converter = Some(Converter::new(spec, target)?);
                    }
                    if let Some(converter) = converter {
                        converter.push(decoded, pending)?;
                    }
                    Ok(false)
                }
                None => {
                    if let Some(converter) = converter {
                        converter.finish(pending)?;
                    }
                    Ok(true)
                }
            }
        });
        match result {
            Ok(finished) => {
                track.decoded_all = finished;
                self.equalizer.process(&mut self.pending);
                true
            }
            Err(kind) => {
                self.fail(generation, kind);
                false
            }
        }
    }

    fn reposition_if_needed(&mut self) {
        let Some(track) = &mut self.track else { return };
        if !std::mem::take(&mut track.needs_reposition) {
            return;
        }
        let position = self.shared.with_anchor(|anchor| anchor.base_sec);
        let generation = track.generation;
        let decoder = &mut track.decoder;
        track.decoded_all = false;
        if let Err(kind) = guarded(|| decoder.seek(position)) {
            self.fail(generation, kind);
        }
    }

    fn push_pending(&mut self) -> bool {
        let Some(stream) = &mut self.stream else { return false };
        let channels = usize::from(stream.config.channels.max(1));
        let left = &self.pending[self.pending_start..];
        // Chỉ đẩy nguyên frame: mốc xoá ring luôn rơi đúng ranh giới frame.
        let count = left.len().min(stream.producer.vacant_len()) / channels * channels;
        if count == 0 {
            return false;
        }
        let pushed = stream.producer.push_slice(&left[..count]);
        stream.pushed += pushed as u64;
        self.pending_start += pushed;
        pushed > 0
    }

    fn check_ended(&mut self) {
        let (Some(track), Some(stream)) = (&mut self.track, &self.stream) else { return };
        if track.ended_sent
            || !track.decoded_all
            || self.pending_start < self.pending.len()
            || self.shared.controls.is_paused()
            || stream.counters.popped.load(Ordering::Acquire) < stream.pushed
        {
            return;
        }
        track.ended_sent = true;
        let generation = track.generation;
        self.set_paused(true);
        self.emit(AudioEvent::Ended { generation });
    }

    fn check_idle(&mut self) {
        if self.stream.is_some()
            && self.idle_since.is_some_and(|since| since.elapsed() >= self.options.idle_close_after)
        {
            self.close_stream();
            self.idle_since = None;
        }
    }
}

fn sanitize_sec(sec: f64) -> f64 {
    if sec.is_finite() { sec.max(0.0) } else { 0.0 }
}

/// Tệp độc làm decoder panic thì thành `Error`, không sập app (S25).
fn guarded<T>(f: impl FnOnce() -> Result<T, AudioErrorKind>) -> Result<T, AudioErrorKind> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(AudioErrorKind::Corrupt))
}

fn open_at(path: &Path, start_sec: f64) -> Result<(TrackDecoder, f64), AudioErrorKind> {
    let mut decoder = TrackDecoder::open(path)?;
    let duration = decoder.duration_sec();
    let start = if duration > 0.0 { start_sec.min(duration) } else { start_sec };
    if start > 0.0 {
        decoder.seek(start)?;
    }
    Ok((decoder, start))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::output::{FakeOutput, StreamFailure};

    const RATE: u32 = 48_000;
    const TIMEOUT: Duration = Duration::from_secs(20);

    /// Tệp WAV tự sinh, xoá khi drop. Mỗi frame mang số thứ tự của nó trong giá trị mẫu: `value(i)`.
    struct TempWav(PathBuf);

    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

    fn value(frame: usize, total: usize) -> f32 {
        0.25 + 0.5 * frame as f32 / total as f32
    }

    fn frame_at(sample: f32, total: usize) -> usize {
        (((sample - 0.25) / 0.5) * total as f32).round() as usize
    }

    impl TempWav {
        fn new(rate: u32, frames: usize) -> Self {
            let mut data = Vec::with_capacity(44 + frames * 8);
            let bytes = (frames * 8) as u32;
            data.extend_from_slice(b"RIFF");
            data.extend_from_slice(&(bytes + 36).to_le_bytes());
            data.extend_from_slice(b"WAVEfmt ");
            data.extend_from_slice(&16u32.to_le_bytes());
            data.extend_from_slice(&3u16.to_le_bytes());
            data.extend_from_slice(&2u16.to_le_bytes());
            data.extend_from_slice(&rate.to_le_bytes());
            data.extend_from_slice(&(rate * 8).to_le_bytes());
            data.extend_from_slice(&8u16.to_le_bytes());
            data.extend_from_slice(&32u16.to_le_bytes());
            data.extend_from_slice(b"data");
            data.extend_from_slice(&bytes.to_le_bytes());
            for i in 0..frames {
                let sample = value(i, frames).to_le_bytes();
                data.extend_from_slice(&sample);
                data.extend_from_slice(&sample);
            }
            let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("groove-audio-{}-{id}.wav", std::process::id()));
            std::fs::write(&path, data).unwrap();
            Self(path)
        }
    }

    impl Drop for TempWav {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    struct Rig {
        engine: AudioEngine,
        events: AudioEvents,
        fake: FakeOutput,
    }

    fn rig_with(rate: u32, options: EngineOptions) -> Rig {
        let fake = FakeOutput::new(rate, 2);
        let (engine, events) = AudioEngine::spawn(Box::new(fake.clone()), options).unwrap();
        engine.send(AudioCmd::SetVolume(1.0));
        Rig { engine, events, fake }
    }

    fn rig() -> Rig {
        // Test tự kéo mẫu nên không bật canh callback đứng.
        rig_with(RATE, EngineOptions { stall_timeout: Duration::from_secs(3600), ..Default::default() })
    }

    impl Rig {
        fn load(&self, path: &Path, start_sec: f64, autoplay: bool) -> u64 {
            let generation = self.engine.next_generation();
            self.engine.send(AudioCmd::Load { generation, path: path.into(), start_sec, autoplay });
            generation
        }

        fn wait_event(&mut self) -> AudioEvent {
            let deadline = Instant::now() + TIMEOUT;
            loop {
                if let Some(event) = self.events.try_recv() {
                    return event;
                }
                assert!(Instant::now() < deadline, "no event in time");
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn wait_until(&self, what: &str, condition: impl Fn(&Self) -> bool) {
            let deadline = Instant::now() + TIMEOUT;
            while !condition(self) {
                assert!(Instant::now() < deadline, "timed out: {what}");
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        /// Chờ decoder đẩy đủ `frames` vào ring. Máy bận thì không biết trước bao lâu luồng decoder mới được chạy; kéo mẫu mà
        /// không chờ thì gặp ring rỗng, vị trí không tăng.
        fn wait_buffered(&self, frames: usize) {
            self.wait_until("ring filled", |r| r.fake.buffered_frames() >= frames);
        }

        /// Kéo mẫu trong khi chờ sự kiện, gom các mẫu khác 0 của kênh trái.
        fn drain_until_event(&mut self, audio: &mut Vec<f32>) -> AudioEvent {
            let deadline = Instant::now() + TIMEOUT;
            loop {
                if let Some(event) = self.events.try_recv() {
                    return event;
                }
                assert!(Instant::now() < deadline, "no event in time");
                match self.fake.pull(480) {
                    Some(samples) => audio.extend(samples.chunks(2).map(|f| f[0]).filter(|s| *s != 0.0)),
                    None => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        }

        fn silence_until(&self, what: &str, condition: impl Fn(&Self) -> bool) {
            let deadline = Instant::now() + TIMEOUT;
            while !condition(self) {
                assert!(Instant::now() < deadline, "timed out: {what}");
                let _ = self.fake.pull(480);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    #[test]
    fn plays_every_frame_and_ends_only_after_the_ring_is_empty() {
        let frames = RATE as usize / 2;
        let wav = TempWav::new(RATE, frames);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, true);
        assert_eq!(rig.wait_event(), AudioEvent::Loaded { generation, duration_sec: 0.5 });
        // Decoder chạy hết sớm vì ring còn chỗ, nhưng chưa ai kéo mẫu thì chưa được báo hết bài.
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(rig.events.try_recv(), None);
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert_eq!(audio.len(), frames, "no frame lost, tail included");
        let last = frame_at(*audio.last().unwrap(), frames);
        assert!(last.abs_diff(frames - 1) <= 2, "last frame {last}");
        assert!(!rig.engine.is_playing());
        assert!((rig.engine.position_sec() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rapid_next_only_opens_the_last_track() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        // Các đường dẫn trung gian không tồn tại: nếu bị mở thì phải có Error.
        for i in 0..200 {
            rig.load(Path::new(&format!("Z:/groove/missing-{i}.mp3")), 0.0, true);
        }
        let last = rig.load(&wav.0, 0.0, true);
        assert_eq!(rig.wait_event(), AudioEvent::Loaded { generation: last, duration_sec: 1.0 });
        assert_eq!(rig.events.try_recv(), None);
    }

    #[test]
    fn stale_ended_in_the_channel_is_dropped() {
        let wav = TempWav::new(RATE, 2400);
        let other = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        let first = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        // Kéo hết để Ended nằm trong kênh, chưa đọc.
        rig.silence_until("ended", |r| !r.engine.is_playing());
        let second = rig.load(&other.0, 0.0, false);
        assert_ne!(first, second);
        assert_eq!(rig.wait_event(), AudioEvent::Loaded { generation: second, duration_sec: 1.0 });
    }

    #[test]
    fn stale_seek_and_old_load_are_ignored() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, false);
        rig.wait_event();
        rig.engine.send(AudioCmd::Seek { generation: generation + 99, sec: 0.5 });
        rig.engine.send(AudioCmd::Seek { generation: generation - 1, sec: 0.5 });
        rig.engine.send(AudioCmd::Load { generation: 0, path: "Z:/old.mp3".into(), start_sec: 0.0, autoplay: true });
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(rig.engine.position_sec(), 0.0);
        assert_eq!(rig.events.try_recv(), None);
    }

    #[test]
    fn seek_lands_exactly_and_drops_old_audio() {
        let frames = RATE as usize * 2;
        let wav = TempWav::new(RATE, frames);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_buffered(1000);
        rig.fake.pull(1000).unwrap();
        rig.engine.send(AudioCmd::Seek { generation, sec: 1.5 });
        rig.wait_until("anchor", |r| r.engine.position_sec() >= 1.5);
        let mut left = Vec::new();
        let deadline = Instant::now() + TIMEOUT;
        while left.len() < 3000 {
            assert!(Instant::now() < deadline);
            let samples = rig.fake.pull(480).unwrap();
            left.extend(samples.chunks(2).map(|f| f[0]).filter(|s| *s != 0.0));
            std::thread::sleep(Duration::from_millis(1));
        }
        // Âm cũ chỉ còn trong 5 ms fade (240 frame); phần còn lại đếm ngược từ frame cuối phải rơi đúng frame 72 000.
        let last = frame_at(*left.last().unwrap(), frames);
        let new_frames = last + 1 - 72_000;
        let stale = left.len() as i64 - new_frames as i64;
        assert!((-4..=245).contains(&stale), "landed wrong or old audio left: {stale} stale frames");
        let tail = &left[left.len() - 200..];
        assert!(tail.windows(2).all(|w| frame_at(w[1], frames).abs_diff(frame_at(w[0], frames)) <= 2), "contiguous");
        let position = rig.engine.position_sec();
        assert!((1.5..1.6).contains(&position), "position {position}");
    }

    #[test]
    fn position_excludes_audio_still_in_the_ring() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        rig.load(&wav.0, 0.25, true);
        rig.wait_event();
        rig.wait_buffered(4800);
        assert_eq!(rig.engine.position_sec(), 0.25, "ring full but nothing played");
        rig.fake.pull(4800).unwrap();
        assert!((rig.engine.position_sec() - 0.35).abs() < 1e-6);
    }

    #[test]
    fn seek_after_end_can_replay_and_seek_past_end_ends() {
        let wav = TempWav::new(RATE, 4800);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        // Play khi đã hết bài không được kẹt ở trạng thái đang phát.
        rig.engine.send(AudioCmd::Play);
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert!(!rig.engine.is_playing());
        rig.engine.send(AudioCmd::Seek { generation, sec: 0.0 });
        rig.engine.send(AudioCmd::Play);
        audio.clear();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert_eq!(audio.len(), 4800);
        rig.engine.send(AudioCmd::Seek { generation, sec: 999.0 });
        rig.engine.send(AudioCmd::Play);
        audio.clear();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert!(audio.is_empty());
        assert!((rig.engine.position_sec() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn no_stream_until_played_and_restore_position_without_autoplay() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.5, false);
        rig.wait_event();
        assert_eq!(rig.fake.open_count(), 0);
        assert!(!rig.engine.is_playing());
        assert_eq!(rig.engine.position_sec(), 0.5);
        rig.engine.send(AudioCmd::Seek { generation, sec: 0.75 });
        rig.wait_until("seek", |r| r.engine.position_sec() == 0.75);
        rig.engine.send(AudioCmd::Play);
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert!(audio.len().abs_diff(12_000) <= 1, "{}", audio.len());
    }

    #[test]
    fn idle_closes_stream_and_keeps_position() {
        let frames = RATE as usize;
        let wav = TempWav::new(RATE, frames);
        let options = EngineOptions {
            idle_close_after: Duration::from_millis(30),
            stall_timeout: Duration::from_secs(3600),
            ..Default::default()
        };
        let mut rig = rig_with(RATE, options);
        let generation = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_buffered(9600);
        rig.fake.pull(9600).unwrap();
        rig.engine.send(AudioCmd::Pause);
        rig.silence_until("idle close", |r| !r.fake.is_open());
        let position = rig.engine.position_sec();
        assert!((0.2..0.21).contains(&position), "{position}");
        rig.engine.send(AudioCmd::Play);
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        assert_eq!(rig.fake.open_count(), 2);
        let expected = frames - (position * f64::from(RATE)).round() as usize;
        assert!(audio.len().abs_diff(expected) <= 1, "{} vs {expected}", audio.len());
        // Nghe tiếp đúng chỗ đã dừng, không nghe lại, không hụt: frame đầu sau 5 ms fade.
        let resumed = frame_at(audio[240], frames);
        assert!(resumed.abs_diff((position * f64::from(RATE)) as usize + 240) <= 4, "{resumed}");
    }

    #[test]
    fn device_loss_reopens_on_new_rate_and_keeps_position() {
        let frames = RATE as usize;
        let wav = TempWav::new(RATE, frames);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_buffered(4800);
        rig.fake.pull(4800).unwrap();
        rig.fake.set_config(44_100, 2);
        rig.fake.trigger_failure(StreamFailure::DeviceLost);
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged);
        assert_eq!(rig.fake.open_count(), 2);
        assert!(rig.engine.is_playing());
        let position = rig.engine.position_sec();
        assert!((position - 0.1).abs() < 0.001, "{position}");
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation });
        // Còn 0,9 giây ở tần số mới.
        assert!(audio.len().abs_diff(39_690) <= 8, "{}", audio.len());
        assert!((rig.engine.position_sec() - 1.0).abs() < 0.001);
    }

    #[test]
    fn device_loss_retries_then_reports_output_error() {
        let wav = TempWav::new(RATE, RATE as usize);
        let options = EngineOptions {
            reopen_interval: Duration::from_millis(5),
            reopen_attempts: 3,
            stall_timeout: Duration::from_secs(3600),
            ..Default::default()
        };
        let mut rig = rig_with(RATE, options);
        let generation = rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_until("stream", |r| r.fake.is_open());
        rig.fake.fail_next_opens(2);
        rig.fake.trigger_failure(StreamFailure::DeviceLost);
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged, "third attempt works");
        rig.fake.fail_next_opens(100);
        rig.fake.trigger_failure(StreamFailure::DeviceLost);
        assert_eq!(rig.wait_event(), AudioEvent::Error { generation, kind: AudioErrorKind::Output });
        assert!(!rig.engine.is_playing());
        // Có thiết bị lại: Play phát tiếp đúng chỗ.
        rig.fake.fail_next_opens(0);
        rig.engine.send(AudioCmd::Play);
        rig.wait_until("reopen", |r| r.fake.is_open());
        assert!(rig.engine.is_playing());
    }

    #[test]
    fn rerouted_same_config_keeps_the_stream() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_until("stream", |r| r.fake.is_open());
        rig.fake.trigger_failure(StreamFailure::Rerouted);
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged);
        assert_eq!(rig.fake.open_count(), 1);
        rig.fake.set_config(44_100, 2);
        rig.fake.trigger_failure(StreamFailure::Rerouted);
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged);
        assert_eq!(rig.fake.open_count(), 2);
    }

    #[test]
    fn device_loss_while_paused_reopens_only_on_play() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        rig.wait_until("stream", |r| r.fake.is_open());
        rig.engine.send(AudioCmd::Pause);
        rig.fake.trigger_failure(StreamFailure::DeviceLost);
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged);
        assert_eq!(rig.fake.open_count(), 1);
        rig.engine.send(AudioCmd::Play);
        rig.wait_until("reopen", |r| r.fake.open_count() == 2);
    }

    #[test]
    fn stalled_callback_counts_as_lost_stream() {
        let wav = TempWav::new(RATE, RATE as usize);
        let options = EngineOptions { stall_timeout: Duration::from_millis(30), ..Default::default() };
        let mut rig = rig_with(RATE, options);
        rig.load(&wav.0, 0.0, true);
        rig.wait_event();
        assert_eq!(rig.wait_event(), AudioEvent::DeviceChanged);
        assert!(rig.fake.open_count() >= 2);
    }

    #[test]
    fn no_output_device_reports_error_on_play_and_load_still_works() {
        let wav = TempWav::new(RATE, RATE as usize);
        let mut rig = rig();
        rig.fake.fail_next_opens(1);
        let generation = rig.load(&wav.0, 0.0, true);
        assert!(matches!(rig.wait_event(), AudioEvent::Loaded { .. }));
        assert_eq!(rig.wait_event(), AudioEvent::Error { generation, kind: AudioErrorKind::Output });
        assert!(!rig.engine.is_playing());
    }

    #[test]
    fn bad_files_become_errors_and_engine_keeps_working() {
        let garbage = std::env::temp_dir().join(format!("groove-audio-bad-{}.mp3", std::process::id()));
        std::fs::write(&garbage, vec![0x11u8; 8192]).unwrap();
        let wav = TempWav::new(RATE, 2400);
        let mut rig = rig();
        let missing = rig.load(Path::new("Z:/groove/missing.mp3"), 0.0, true);
        assert_eq!(rig.wait_event(), AudioEvent::Error { generation: missing, kind: AudioErrorKind::NotFound });
        assert!(!rig.engine.is_playing());
        let bad = rig.load(&garbage, 0.0, true);
        assert_eq!(rig.wait_event(), AudioEvent::Error { generation: bad, kind: AudioErrorKind::Unsupported });
        std::fs::remove_file(&garbage).unwrap();
        let good = rig.load(&wav.0, 0.0, true);
        assert!(matches!(rig.wait_event(), AudioEvent::Loaded { generation, .. } if generation == good));
        let mut audio = Vec::new();
        assert_eq!(rig.drain_until_event(&mut audio), AudioEvent::Ended { generation: good });
    }

    #[test]
    fn panic_in_decoder_becomes_an_error() {
        assert_eq!(guarded::<()>(|| panic!("poisoned file")), Err(AudioErrorKind::Corrupt));
    }

    #[test]
    fn invalid_seconds_are_sanitized() {
        assert_eq!(sanitize_sec(f64::NAN), 0.0);
        assert_eq!(sanitize_sec(-3.0), 0.0);
        assert_eq!(sanitize_sec(f64::INFINITY), 0.0);
    }

    #[test]
    fn hammering_commands_from_threads_settles_on_the_last_track() {
        let wav = TempWav::new(RATE, RATE as usize / 2);
        let last_wav = TempWav::new(RATE, 2400);
        let mut rig = rig();
        let fake = rig.fake.clone();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let clock = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = fake.pull(256);
                    std::thread::sleep(Duration::from_micros(200));
                }
            })
        };
        let mut generation = 0;
        for i in 0..2000u32 {
            match i % 5 {
                0 => generation = rig.load(&wav.0, 0.0, true),
                1 => rig.engine.send(AudioCmd::Seek { generation, sec: f64::from(i % 7) * 0.05 }),
                2 => rig.engine.send(AudioCmd::Pause),
                3 => rig.engine.send(AudioCmd::Play),
                _ => rig.engine.send(AudioCmd::SetVolume(i as f32 / 2000.0)),
            }
            let _ = rig.engine.position_sec();
            if i % 64 == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let last = rig.load(&last_wav.0, 0.0, true);
        let deadline = Instant::now() + TIMEOUT;
        let mut got = Vec::new();
        while Instant::now() < deadline {
            if let Some(event) = rig.events.try_recv() {
                let done = event == AudioEvent::Ended { generation: last };
                got.push(event);
                if done {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        stop.store(true, Ordering::Relaxed);
        clock.join().unwrap();
        assert_eq!(got.last(), Some(&AudioEvent::Ended { generation: last }), "{got:?}");
        assert!(got.iter().all(|event| event.generation() == Some(last)), "only current generation: {got:?}");
    }

    #[test]
    fn events_stream_works_with_async_consumer() {
        let wav = TempWav::new(RATE, 2400);
        let mut rig = rig();
        let generation = rig.load(&wav.0, 0.0, false);
        let event = futures::executor::block_on(rig.events.next());
        assert_eq!(event, Some(AudioEvent::Loaded { generation, duration_sec: 0.05 }));
    }
}
