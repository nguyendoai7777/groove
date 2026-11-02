//! groove-gd1_13: Player (N5, N6, N8).
//!
//! Nối `Queue` (danh sách, hàm thuần), `AudioEngine` (tiếng), `Library` (tra bài), `Settings` (âm lượng, EQ), `session_sync`.
//! Mỗi thao tác hàng đợi trả `QueueOutcome`, `Player::apply_outcome` đổi thành lệnh engine. Trạng thái phát chỉ có một nguồn là
//! `engine.is_playing()` (N8); hàng đợi chỉ giữ `TrackRef`, bài thật tra lúc phát, nên đổi đường dẫn tự đúng theo `id` (N5).
//!
//! # Cho task sau
//!
//! `player::player(cx)` trả `Entity<Player>`: đọc bằng `.read(cx)`, thao tác bằng `.update(cx, |p, cx| p.next(cx))`, nghe bằng
//! `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` ([`PlayerEvent`]). Lỗi phát chỉ phát sự kiện, chữ
//! toast là của view sở hữu vùng i18n.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use groove_audio::engine::{AudioCmd, AudioEngine, AudioErrorKind, AudioEvent, AudioEvents, EngineOptions, EqParams};
use groove_audio::output::{self, AudioOutput};
use groove_core::model::{Song, TrackRef};
use groove_core::paths;
use groove_core::queue::{Queue, QueueOutcome};
use groove_core::session::{SAVE_INTERVAL, Session};
use groove_core::settings::{EqSettings, Settings};
use groove_core::timeline::{self, Segment};
use groove_library::actor::{LibraryEvent, LibraryResult};
use groove_library::store::Library;
use groove_library::tags::{self, CoverArt, ScannedTrack};
use materiant::gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Global, Task};

use crate::app;
use crate::session_sync::{self, SessionSync};

/// Lỗi tệp liên tiếp tối đa rồi dừng, không chạy hết hàng đợi đầy bài chết (N5).
pub const MAX_CONSECUTIVE_ERRORS: u32 = 3;
/// Nhịp báo vị trí cho UI khi đang phát.
const TICK: Duration = Duration::from_millis(250);
/// Phát được từng này giây rồi mới lỗi thì không còn là "liên tiếp": tệp hỏng giữa chừng không tính dồn với bài trước.
const PLAYED_OK_SEC: f64 = 1.0;

/// Sự kiện cho UI. Đổi trạng thái thường đi qua `cx.observe`.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    /// Bài đang chọn đổi, kể cả về `None`, hoặc thông tin của nó vừa đọc xong.
    TrackChanged,
    /// Tệp không phát được: toast. `gave_up` là đã tới [`MAX_CONSECUTIVE_ERRORS`] và dừng; ngược lại đã tự sang bài kế nếu còn.
    TrackFailed {
        track: TrackRef,
        title: Option<String>,
        kind: AudioErrorKind,
        gave_up: bool,
    },
    /// Không có thiết bị ra tiếng. Không phải lỗi tệp: không sang bài, không tính vào giới hạn; Play thử lại.
    OutputFailed,
    DeviceChanged,
}

impl EventEmitter<PlayerEvent> for Player {}

/// Bài hiện tại đang ở đâu trên đường tới engine.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LoadState {
    Idle,
    /// Đang tra bài trong thư viện, chưa gửi `Load`.
    Resolving {
        start_sec: f64,
    },
    /// Đã gửi `Load`, chờ `Loaded`.
    Loading {
        start_sec: f64,
    },
    Loaded {
        start_sec: f64,
    },
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
struct AudioPrefs {
    volume: u8,
    muted: bool,
    eq: EqParams,
}

impl AudioPrefs {
    fn of(settings: &Settings) -> AudioPrefs {
        AudioPrefs { volume: settings.volume, muted: settings.is_muted, eq: EqParams::from(&settings.eq) }
    }
}

/// `app::settings` cần global riêng của `app.rs`, test dùng bản cục bộ.
enum SettingsSource {
    App,
    #[cfg(test)]
    Local(Box<Settings>),
}

pub struct PlayerDeps {
    /// `None` khi thư viện không khởi động được: tệp ngoài vẫn phát.
    pub library: Option<Library>,
    pub output: Box<dyn AudioOutput>,
    pub options: EngineOptions,
    /// Nơi đọc, ghi phiên. `None` thì không khôi phục, không lưu.
    pub session_file: Option<PathBuf>,
}

pub struct Player {
    engine: AudioEngine,
    library: Option<Library>,
    queue: Queue,
    sync: SessionSync,
    settings: SettingsSource,
    applied: AudioPrefs,

    song: Option<Song>,
    lyrics: Option<String>,
    timeline_text: Option<String>,
    timeline: Vec<Segment>,
    /// Bìa đọc từ thẻ, chỉ cho tệp ngoài (N6); bài thư viện dùng `cover_hash`.
    external_cover: Option<CoverArt>,
    loaded_duration_sec: f64,

    /// 0 là không có `Load` nào đang hiệu lực: sự kiện nào cũng bị bỏ.
    generation: u64,
    load: LoadState,
    /// Người dùng muốn nghe. Khác `is_playing` lúc đang nạp; lỗi tệp chỉ tự sang bài khi cờ này bật (khôi phục phiên không tự phát).
    wants_playback: bool,
    consecutive_errors: u32,

    /// Phiên đã đọc xong. Trước đó không được ghi, kẻo ghi đè phiên cũ bằng phiên trống.
    session_ready: bool,
    /// Người dùng đã thao tác trước khi phiên đọc xong: phiên cũ bị bỏ.
    touched: bool,
    /// Tệp mở lúc khởi động, chờ phiên đọc xong; ưu tiên hơn bài của phiên (đặc tả 5.6).
    pending_paths: Vec<PathBuf>,
    ticks_since_save: u32,

    load_task: Option<Task<()>>,
    text_task: Option<Task<()>>,
    tick_task: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
}

impl Player {
    /// Lỗi duy nhất là không tạo được luồng âm thanh.
    pub fn spawn(deps: PlayerDeps, cx: &mut App) -> std::io::Result<Entity<Player>> {
        Self::spawn_with(deps, SettingsSource::App, cx)
    }

    fn spawn_with(deps: PlayerDeps, settings: SettingsSource, cx: &mut App) -> std::io::Result<Entity<Player>> {
        let (engine, events) = AudioEngine::spawn(deps.output, deps.options)?;
        let session_file = deps.session_file;
        let sync = SessionSync::new(session_file.clone());
        let library = deps.library;
        Ok(cx.new(move |cx| {
            let mut tasks = vec![Self::listen_audio(events, cx)];
            if let Some(library) = &library {
                tasks.push(Self::listen_library(library, cx));
            }
            tasks.push(Self::restore_session(session_file, cx));
            cx.on_app_quit(|this: &mut Player, _| {
                this.save_session();
                this.sync.flush();
                async {}
            })
            .detach();

            let prefs = settings_prefs(&settings, cx);
            // Gửi cả khi EQ phẳng (bàn giao groove-gd1_11).
            engine.send(AudioCmd::SetVolume(volume_gain(prefs.volume)));
            engine.send(AudioCmd::SetMuted(prefs.muted));
            engine.send(AudioCmd::SetEq(prefs.eq));
            Player {
                engine,
                library,
                queue: Queue::new(),
                sync,
                settings,
                applied: prefs,
                song: None,
                lyrics: None,
                timeline_text: None,
                timeline: Vec::new(),
                external_cover: None,
                loaded_duration_sec: 0.0,
                generation: 0,
                load: LoadState::Idle,
                wants_playback: false,
                consecutive_errors: 0,
                session_ready: false,
                touched: false,
                pending_paths: Vec::new(),
                ticks_since_save: 0,
                load_task: None,
                text_task: None,
                tick_task: None,
                _tasks: tasks,
            }
        }))
    }

    // ---- Đọc ----

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn current_track(&self) -> Option<&TrackRef> {
        self.queue.current()
    }

    /// Bài đang chọn. `None`: thanh phát mờ (đặc tả 1.3).
    pub fn current_song(&self) -> Option<&Song> {
        self.song.as_ref()
    }

    pub fn is_playing(&self) -> bool {
        self.engine.is_playing()
    }

    pub fn position_sec(&self) -> f64 {
        match self.load {
            LoadState::Idle | LoadState::Failed => 0.0,
            LoadState::Resolving { start_sec } | LoadState::Loading { start_sec } => start_sec,
            LoadState::Loaded { .. } => self.engine.position_sec(),
        }
    }

    /// Của tệp khi engine biết, không thì của thư viện, thẻ.
    pub fn duration_sec(&self) -> f64 {
        if self.loaded_duration_sec > 0.0 {
            self.loaded_duration_sec
        } else {
            self.song.as_ref().map_or(0.0, |song| song.duration_sec)
        }
    }

    pub fn lyrics(&self) -> Option<&str> {
        self.lyrics.as_deref()
    }

    /// Đã parse theo thời lượng thật, đổi khi engine báo `Loaded`.
    pub fn timeline(&self) -> &[Segment] {
        &self.timeline
    }

    pub fn external_cover(&self) -> Option<&CoverArt> {
        self.external_cover.as_ref()
    }

    // ---- Hàng đợi (bảng 1.4) ----

    /// Phát kèm danh sách mới. Trùng bài đang phát thì giữ nguyên tiếng (S2).
    pub fn play_list(&mut self, track: TrackRef, list: Vec<TrackRef>, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.play_list(track, list);
        self.apply_outcome(outcome, cx);
    }

    /// Phát không kèm danh sách (bấm trong hàng đợi, bảng lệnh, tệp ngoài).
    pub fn play_track(&mut self, track: TrackRef, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.play_track(track);
        self.apply_outcome(outcome, cx);
    }

    /// Thêm vào cuối hàng đợi. Chưa có bài nào thì phát bài đầu.
    pub fn append(&mut self, tracks: Vec<TrackRef>, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.append(tracks);
        self.apply_outcome(outcome, cx);
    }

    /// Mở tệp bằng đường dẫn (N6): tệp đầu phát ngay, các tệp sau vào hàng đợi. Tệp trong thư viện thành `Library`. Gọi trước khi
    /// phiên đọc xong thì chờ, và thắng bài của phiên (đặc tả 5.6). Lọc đuôi là việc của bên gọi (S22).
    pub fn play_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        if !self.session_ready {
            self.pending_paths.extend(paths);
            return;
        }
        // Thắng bài của phiên ngay từ lúc này, không đợi tra đường dẫn xong.
        self.touched = true;
        if self.load_task.is_none() {
            self.load = LoadState::Idle;
        }
        let library = self.library.clone();
        cx.spawn(async move |this, cx| {
            let mut tracks = Vec::with_capacity(paths.len());
            for path in paths {
                let found = match &library {
                    Some(library) => library.track_for_path(path.clone()).await.ok(),
                    None => None,
                };
                tracks.push(found.unwrap_or(TrackRef::External(path)));
            }
            let _ = this.update(cx, |this, cx| {
                let mut tracks = tracks.into_iter();
                if let Some(first) = tracks.next() {
                    this.play_track(first, cx);
                    this.append(tracks.collect(), cx);
                }
            });
        })
        .detach();
    }

    pub fn next(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.next_track();
        self.apply_outcome(outcome, cx);
    }

    pub fn prev(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.prev_track();
        self.apply_outcome(outcome, cx);
    }

    pub fn play_random(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = self.queue.play_random();
        self.apply_outcome(outcome, cx);
    }

    pub fn toggle_shuffle(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        self.queue.toggle_shuffle();
        self.changed(cx);
    }

    pub fn toggle_repeat(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        self.queue.toggle_repeat();
        self.changed(cx);
    }

    /// Xoá hàng đợi và bài đang chọn, dừng tiếng.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        self.queue.clear();
        self.drop_current(cx);
        self.changed(cx);
    }

    // ---- Điều khiển ----

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.wants_playback || self.engine.is_playing() {
            self.pause(cx);
        } else {
            self.play(cx);
        }
    }

    /// Không có bài đang chọn thì lấy bài kế (bài vừa phát bị xoá khỏi thư viện). Bài vừa lỗi thì thử nạp lại.
    pub fn play(&mut self, cx: &mut Context<Self>) {
        self.user_action();
        let outcome = if self.queue.current().is_some() { QueueOutcome::Resume } else { self.queue.next_track() };
        self.apply_outcome(outcome, cx);
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.wants_playback = false;
        self.engine.send(AudioCmd::Pause);
        self.changed(cx);
    }

    /// Chỉ tua khi thả chuột (S20). Không đổi trạng thái phát.
    pub fn seek(&mut self, sec: f64, cx: &mut Context<Self>) {
        let sec = if sec.is_finite() { sec.max(0.0) } else { 0.0 };
        match self.load {
            LoadState::Idle | LoadState::Failed => return,
            LoadState::Resolving { .. } => self.load = LoadState::Resolving { start_sec: sec },
            LoadState::Loading { .. } => {
                self.load = LoadState::Loading { start_sec: sec };
                self.engine.send(AudioCmd::Seek { generation: self.generation, sec });
            }
            LoadState::Loaded { .. } => {
                self.load = LoadState::Loaded { start_sec: sec };
                self.engine.send(AudioCmd::Seek { generation: self.generation, sec });
            }
        }
        self.save_session_at(sec);
        cx.notify();
    }

    /// Tua tương đối, cho phím `seek_step`.
    pub fn seek_by(&mut self, delta_sec: f64, cx: &mut Context<Self>) {
        let mut target = self.position_sec() + delta_sec;
        let duration = self.duration_sec();
        if duration > 0.0 {
            target = target.min(duration);
        }
        self.seek(target, cx);
    }

    pub fn set_volume(&mut self, volume: u8, cx: &mut Context<Self>) {
        self.change_settings(cx, |settings| settings.volume = volume);
    }

    /// Bánh xe, phím `volume_step`. Giữ nguyên mute.
    pub fn adjust_volume(&mut self, delta: i16, cx: &mut Context<Self>) {
        let volume = i16::from(self.prefs(cx).volume).saturating_add(delta).clamp(0, 100);
        self.set_volume(volume as u8, cx);
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.change_settings(cx, |settings| settings.is_muted = !settings.is_muted);
    }

    /// Sửa EQ: lưu và gửi engine. Gọi liên tục khi kéo thanh trượt được, DSP tự trượt 10 ms.
    pub fn update_eq(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut EqSettings)) {
        self.change_settings(cx, |settings| change(&mut settings.eq));
    }

    /// Đưa âm lượng, mute, EQ trong cài đặt xuống engine, chỉ phần đã đổi. Ai sửa ba trường này bằng `app::update_settings` thay vì
    /// các hàm trên thì gọi thêm hàm này (`app.rs` không cho quan sát `Settings` từ ngoài).
    pub fn apply_settings(&mut self, cx: &mut Context<Self>) {
        let now = self.prefs(cx);
        if now.volume != self.applied.volume {
            self.engine.send(AudioCmd::SetVolume(volume_gain(now.volume)));
        }
        if now.muted != self.applied.muted {
            self.engine.send(AudioCmd::SetMuted(now.muted));
        }
        if now.eq != self.applied.eq {
            self.engine.send(AudioCmd::SetEq(now.eq));
        }
        if now != self.applied {
            self.applied = now;
            cx.notify();
        }
    }

    /// Nạp lại thông tin bài đang chọn, sau khi sửa thẻ, lyrics, timeline. Tiếng không đổi.
    pub fn refresh_current(&mut self, cx: &mut Context<Self>) {
        let Some(track) = self.queue.current().cloned() else { return };
        let Some(library) = self.library.clone() else {
            self.load_text(cx);
            return;
        };
        cx.spawn(async move |this, cx| {
            let found = library.song(track.clone()).await;
            let _ = this.update(cx, |this, cx| {
                if this.queue.current() != Some(&track) {
                    return;
                }
                if let Ok(Some(song)) = found {
                    this.song = Some(song);
                    cx.emit(PlayerEvent::TrackChanged);
                }
                this.load_text(cx);
                cx.notify();
            });
        })
        .detach();
    }

    // ---- QueueOutcome thành lệnh engine (bàn giao groove-gd1_12) ----

    fn apply_outcome(&mut self, outcome: QueueOutcome, cx: &mut Context<Self>) {
        match outcome {
            QueueOutcome::Play(track) => self.start_load(track, 0.0, true, cx),
            QueueOutcome::Resume => self.resume(cx),
            QueueOutcome::Replay => {
                self.wants_playback = true;
                self.engine.send(AudioCmd::Seek { generation: self.generation, sec: 0.0 });
                self.engine.send(AudioCmd::Play);
                self.start_tick(cx);
            }
            QueueOutcome::SeekToStart => self.seek(0.0, cx),
            QueueOutcome::Stop => {
                // N8: thật sự dừng rồi mới về 0, bản cũ chỉ đặt cờ nên tiếng chạy lại từ đầu.
                self.wants_playback = false;
                self.engine.send(AudioCmd::Pause);
                self.engine.send(AudioCmd::Seek { generation: self.generation, sec: 0.0 });
                if let LoadState::Loaded { .. } | LoadState::Loading { .. } = self.load {
                    self.load = LoadState::Loaded { start_sec: 0.0 };
                }
                self.save_session_at(0.0);
                cx.notify();
                return;
            }
            QueueOutcome::Nothing => {}
        }
        self.changed(cx);
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        self.wants_playback = true;
        match self.load {
            LoadState::Idle | LoadState::Failed => {
                if let Some(track) = self.queue.current().cloned() {
                    self.start_load(track, 0.0, true, cx);
                }
                return;
            }
            // `Load` chưa gửi: `finish_lookup` đọc `wants_playback`.
            LoadState::Resolving { .. } => {}
            LoadState::Loading { .. } | LoadState::Loaded { .. } => self.engine.send(AudioCmd::Play),
        }
        self.start_tick(cx);
    }

    /// Tra bài rồi mới gửi `Load`: hàng đợi chỉ có `TrackRef` (N5). Thao tác mới thay `load_task`, lần tra cũ bị huỷ, nên Next nhanh
    /// chỉ nạp bài cuối.
    fn start_load(&mut self, track: TrackRef, start_sec: f64, autoplay: bool, cx: &mut Context<Self>) {
        self.generation = 0;
        self.load = LoadState::Resolving { start_sec };
        self.wants_playback = autoplay;
        self.loaded_duration_sec = 0.0;
        let lookup = self.library.as_ref().map(|library| library.song(track.clone()));
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let found = match lookup {
                Some(reply) => Some(reply.await),
                None => None,
            };
            let _ = this.update(cx, |this, cx| this.finish_lookup(track, found, cx));
        }));
    }

    fn finish_lookup(&mut self, track: TrackRef, found: Option<LibraryResult<Option<Song>>>, cx: &mut Context<Self>) {
        let song = match (found, &track) {
            (Some(Ok(Some(song))), _) => song,
            // Tệp ngoài thật, hoặc thư viện hỏng: tệp ngoài không cần thư viện.
            (_, TrackRef::External(path)) => Song::external(path.clone()),
            (Some(Ok(None)), TrackRef::Library(_)) => return self.lookup_failed(AudioErrorKind::NotFound, cx),
            (_, TrackRef::Library(_)) => return self.lookup_failed(AudioErrorKind::Io, cx),
        };
        let LoadState::Resolving { start_sec } = self.load else { return };
        self.generation = self.engine.next_generation();
        self.load = LoadState::Loading { start_sec };
        self.engine.send(AudioCmd::Load {
            generation: self.generation,
            path: song.path.clone(),
            start_sec,
            autoplay: self.wants_playback,
        });
        self.song = Some(song);
        self.clear_text();
        self.load_text(cx);
        cx.emit(PlayerEvent::TrackChanged);
        if self.wants_playback {
            self.start_tick(cx);
        }
        self.changed(cx);
    }

    /// Bài trong hàng đợi không còn trong thư viện: đi đúng đường của tệp lỗi. Bài cũ phải im.
    fn lookup_failed(&mut self, kind: AudioErrorKind, cx: &mut Context<Self>) {
        self.engine.send(AudioCmd::Pause);
        self.song = None;
        self.clear_text();
        cx.emit(PlayerEvent::TrackChanged);
        self.on_track_failed(kind, cx);
    }

    fn clear_text(&mut self) {
        self.text_task = None;
        self.lyrics = None;
        self.timeline_text = None;
        self.timeline.clear();
        self.external_cover = None;
    }

    /// Lyrics, timeline của bài đang chọn (S16). Tệp ngoài đọc từ thẻ ở nền, kèm bìa (N6).
    fn load_text(&mut self, cx: &mut Context<Self>) {
        let Some(song) = &self.song else { return };
        match (&song.track, &self.library) {
            (TrackRef::Library(id), Some(library)) => {
                let reply = library.song_text(*id);
                self.text_task = Some(cx.spawn(async move |this, cx| {
                    let Ok(Some(text)) = reply.await else { return };
                    let _ = this.update(cx, |this, cx| {
                        this.set_text(text.lyrics, text.timeline);
                        cx.notify();
                    });
                }));
            }
            (TrackRef::External(_), _) => {
                let path = song.path.clone();
                let read = cx.background_spawn(async move { tags::read_track(&path) });
                self.text_task = Some(cx.spawn(async move |this, cx| {
                    let scanned = read.await.map_err(|err| log::debug!("external tags: {err}")).ok();
                    let Some(scanned) = scanned else { return };
                    let _ = this.update(cx, |this, cx| this.apply_external_tags(scanned, cx));
                }));
            }
            (TrackRef::Library(_), None) => {}
        }
    }

    fn apply_external_tags(&mut self, scanned: ScannedTrack, cx: &mut Context<Self>) {
        let Some(song) = &mut self.song else { return };
        let fields = scanned.fields;
        song.title = fields.title;
        song.artist = fields.artist;
        song.album = fields.album;
        song.album_artist = fields.album_artist;
        if song.duration_sec <= 0.0 {
            song.duration_sec = scanned.duration_ms as f64 / 1000.0;
        }
        self.external_cover = scanned.cover;
        self.set_text(fields.lyrics, fields.timeline);
        cx.emit(PlayerEvent::TrackChanged);
        cx.notify();
    }

    fn set_text(&mut self, lyrics: Option<String>, timeline: Option<String>) {
        self.lyrics = lyrics;
        self.timeline_text = timeline;
        self.reparse_timeline();
    }

    fn reparse_timeline(&mut self) {
        let duration = self.duration_sec();
        self.timeline = self.timeline_text.as_deref().map_or_else(Vec::new, |text| timeline::parse(text, duration));
    }

    // ---- Sự kiện engine ----

    fn listen_audio(mut events: AudioEvents, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |this, cx| this.on_audio_event(event, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn on_audio_event(&mut self, event: AudioEvent, cx: &mut Context<Self>) {
        match event {
            AudioEvent::Loaded { generation, duration_sec } if generation == self.generation => {
                if let LoadState::Loading { start_sec } = self.load {
                    self.load = LoadState::Loaded { start_sec };
                }
                self.loaded_duration_sec = duration_sec;
                self.reparse_timeline();
                cx.notify();
            }
            AudioEvent::Ended { generation } if generation == self.generation => {
                self.consecutive_errors = 0;
                let outcome = self.queue.track_ended();
                self.apply_outcome(outcome, cx);
            }
            AudioEvent::Error { generation, kind: AudioErrorKind::Output } if generation == self.generation => {
                // N5: lỗi thiết bị không phải lỗi tệp, sang bài thì chạy hết hàng đợi.
                self.wants_playback = false;
                cx.emit(PlayerEvent::OutputFailed);
                self.changed(cx);
            }
            AudioEvent::Error { generation, kind } if generation == self.generation => {
                if self.position_sec() - self.load_start_sec() > PLAYED_OK_SEC {
                    self.consecutive_errors = 0;
                }
                self.on_track_failed(kind, cx);
            }
            AudioEvent::DeviceChanged => cx.emit(PlayerEvent::DeviceChanged),
            // `generation` khác: bài đã đổi trong lúc sự kiện đang tới, hoặc đang tra bài kế.
            AudioEvent::Loaded { .. } | AudioEvent::Ended { .. } | AudioEvent::Error { .. } => {}
        }
    }

    fn load_start_sec(&self) -> f64 {
        match self.load {
            LoadState::Resolving { start_sec } | LoadState::Loading { start_sec } | LoadState::Loaded { start_sec } => {
                start_sec
            }
            LoadState::Idle | LoadState::Failed => 0.0,
        }
    }

    /// N5: báo, tự sang bài kế, tối đa [`MAX_CONSECUTIVE_ERRORS`] lần liên tiếp. Lúc người dùng không muốn nghe (bài của phiên cũ
    /// hỏng) thì chỉ báo, không tự phát.
    fn on_track_failed(&mut self, kind: AudioErrorKind, cx: &mut Context<Self>) {
        self.generation = 0;
        self.load = LoadState::Failed;
        let Some(track) = self.queue.current().cloned() else { return };
        let title = self.song.as_ref().map(|song| song.display_title().to_owned());
        log::warn!("track failed to play: {kind:?}");
        if !self.wants_playback {
            cx.emit(PlayerEvent::TrackFailed { track, title, kind, gave_up: false });
            self.changed(cx);
            return;
        }
        self.consecutive_errors += 1;
        let gave_up = self.consecutive_errors >= MAX_CONSECUTIVE_ERRORS;
        cx.emit(PlayerEvent::TrackFailed { track, title, kind, gave_up });
        if gave_up {
            self.wants_playback = false;
            self.engine.send(AudioCmd::Pause);
            self.changed(cx);
            return;
        }
        let outcome = self.queue.next_track();
        self.apply_outcome(outcome, cx);
    }

    // ---- Sự kiện thư viện ----

    fn listen_library(library: &Library, cx: &mut Context<Self>) -> Task<()> {
        let mut events = library.subscribe();
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |this, cx| this.on_library_event(event, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn on_library_event(&mut self, event: LibraryEvent, cx: &mut Context<Self>) {
        match event {
            LibraryEvent::Synced(report) => {
                let removed: HashSet<TrackRef> = report.removed.iter().copied().map(TrackRef::Library).collect();
                self.remove_tracks(&removed, cx);
                if report.updated > 0 || report.added > 0 {
                    self.refresh_current(cx);
                }
            }
            LibraryEvent::Moved(_) => self.refresh_current(cx),
            _ => {}
        }
    }

    /// N5: bài mất thì bỏ khỏi hàng đợi. Mất bài đang phát thì sang bài kế nếu đang nghe, không thì dừng ở trạng thái không có bài.
    fn remove_tracks(&mut self, removed: &HashSet<TrackRef>, cx: &mut Context<Self>) {
        if removed.is_empty() {
            return;
        }
        let was_listening = self.wants_playback;
        if self.queue.retain_tracks(|track| !removed.contains(track)) {
            self.drop_current(cx);
            if was_listening {
                let outcome = self.queue.next_track();
                self.apply_outcome(outcome, cx);
            }
        }
        self.changed(cx);
    }

    fn drop_current(&mut self, cx: &mut Context<Self>) {
        self.engine.send(AudioCmd::Pause);
        self.wants_playback = false;
        self.generation = 0;
        self.load = LoadState::Idle;
        self.load_task = None;
        self.song = None;
        self.clear_text();
        self.loaded_duration_sec = 0.0;
        cx.emit(PlayerEvent::TrackChanged);
    }

    // ---- Phiên ----

    /// Đọc phiên ở nền (N7). Không tự phát (đặc tả 1.4).
    fn restore_session(session_file: Option<PathBuf>, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let read = cx.background_spawn(async move { session_file.map(|path| Session::load_from(&path)) });
            let session = read.await.unwrap_or_default();
            let Ok(Some((tracks, library))) = this.update(cx, |this, cx| this.apply_session(&session, cx)) else {
                return;
            };

            // Soát bài chết sau khi đã có hàng đợi: thư viện đang chuyển DB cũ thì câu này chờ lâu.
            let externals = tracks.clone();
            let mut dead = cx.background_spawn(async move { session_sync::missing_external(&externals) }).await;
            if let Some(library) = library {
                let in_library: Vec<TrackRef> =
                    tracks.iter().filter(|track| matches!(track, TrackRef::Library(_))).cloned().collect();
                if let Ok(songs) = library.songs_for_tracks(in_library.clone()).await {
                    let alive: HashSet<TrackRef> = songs.into_iter().map(|song| song.track).collect();
                    dead.extend(in_library.into_iter().filter(|track| !alive.contains(track)));
                }
            }
            let _ = this.update(cx, |this, cx| this.finish_restore(&dead, cx));
        })
    }

    /// Trả về việc còn phải soát; `None` khi người dùng đã thao tác trước, phiên cũ bị bỏ.
    fn apply_session(&mut self, session: &Session, cx: &mut Context<Self>) -> Option<(Vec<TrackRef>, Option<Library>)> {
        self.session_ready = true;
        let restore = !self.touched;
        if restore {
            self.queue = Queue::from_session(session);
            // Chưa tra bài, nhưng thoát app lúc này vẫn phải lưu đúng vị trí cũ, và Play phải chờ `finish_restore`.
            if self.queue.current().is_some() {
                self.load = LoadState::Resolving { start_sec: session.position_sec };
            }
        }
        let pending = std::mem::take(&mut self.pending_paths);
        self.play_paths(pending, cx);
        cx.notify();
        restore.then(|| (session_sync::tracks_of(&self.queue), self.library.clone()))
    }

    fn finish_restore(&mut self, dead: &HashSet<TrackRef>, cx: &mut Context<Self>) {
        // Không đi qua `remove_tracks`: bài của phiên chưa nạp, mất nó thì không có gì phải dừng.
        self.queue.retain_tracks(|track| !dead.contains(track));
        // Có `load_task`, hoặc không còn `Resolving`: người dùng đã đổi bài, mở tệp trong lúc chờ soát.
        if let (LoadState::Resolving { start_sec }, None) = (self.load, &self.load_task) {
            match self.queue.current().cloned() {
                // Không tự phát, trừ khi người dùng vừa bấm Play trong lúc chờ.
                Some(track) => self.start_load(track, start_sec, self.wants_playback, cx),
                None => {
                    self.load = LoadState::Idle;
                    if self.wants_playback {
                        let outcome = self.queue.next_track();
                        self.apply_outcome(outcome, cx);
                    }
                }
            }
        }
        cx.notify();
    }

    fn save_session(&mut self) {
        self.save_session_at(self.position_sec());
    }

    fn save_session_at(&mut self, position_sec: f64) {
        if self.session_ready {
            self.sync.save(&self.queue, position_sec);
        }
    }

    // ---- Phụ ----

    fn user_action(&mut self) {
        self.consecutive_errors = 0;
        self.touched = true;
    }

    /// Sau mọi đổi trạng thái: lưu phiên (debounce), báo UI.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.save_session();
        cx.notify();
    }

    /// Chỉ chạy khi đang phát: dừng thì không có timer nào đánh thức app.
    fn start_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick_task.is_some() {
            return;
        }
        let save_every = (SAVE_INTERVAL.as_millis() / TICK.as_millis()) as u32;
        self.tick_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let keep_going = this.update(cx, |this, cx| {
                    cx.notify();
                    this.ticks_since_save += 1;
                    if this.ticks_since_save >= save_every {
                        this.ticks_since_save = 0;
                        this.save_session();
                    }
                    this.wants_playback || this.engine.is_playing()
                });
                if !matches!(keep_going, Ok(true)) {
                    let _ = this.update(cx, |this, _| this.tick_task = None);
                    break;
                }
            }
        }));
    }

    fn prefs(&self, cx: &App) -> AudioPrefs {
        settings_prefs(&self.settings, cx)
    }

    fn change_settings(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Settings)) {
        match &mut self.settings {
            SettingsSource::App => app::update_settings(cx, change),
            #[cfg(test)]
            SettingsSource::Local(settings) => {
                change(settings);
                settings.sanitize();
            }
        }
        self.apply_settings(cx);
    }
}

fn settings_prefs(source: &SettingsSource, cx: &App) -> AudioPrefs {
    match source {
        SettingsSource::App => AudioPrefs::of(app::settings(cx)),
        #[cfg(test)]
        SettingsSource::Local(settings) => AudioPrefs::of(settings),
    }
}

fn volume_gain(volume: u8) -> f32 {
    f32::from(volume.min(100)) / 100.0
}

// ---- Global ----

struct PlayerHandle(Entity<Player>);

impl Global for PlayerHandle {}

/// Player dùng chung. Gọi sau [`install`].
pub fn player(cx: &App) -> Entity<Player> {
    cx.global::<PlayerHandle>().0.clone()
}

/// Dựng player thật: đầu ra mặc định, thư viện của app, `session.json`. Gọi một lần trong `app::run`, sau khi có thư viện.
pub fn install(cx: &mut App) {
    let session_file = paths::session_file().map_err(|err| log::warn!("session path: {err}")).ok();
    let deps = PlayerDeps {
        library: app::try_library(cx).cloned(),
        output: output::default_output(),
        options: EngineOptions::default(),
        session_file,
    };
    let player = Player::spawn(deps, cx).expect("start audio thread");
    cx.set_global(PlayerHandle(player));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;
    use std::time::Instant;

    use gpui::TestAppContext;
    use groove_audio::output::FakeOutput;
    use groove_core::model::RepeatMode;
    use groove_library::actor::{MovedSong, SyncReport};
    use groove_library::db::migrate::{CoverRecord, CoverSink};
    use groove_library::db::queries::{self, SongUpsert};
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::store::LibraryConfig;
    use tempfile::TempDir;

    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(15);
    const SILENCE: f32 = 1e-3;

    struct Rig {
        dir: TempDir,
        library: Library,
        fake: FakeOutput,
        player: Entity<Player>,
        events: Rc<RefCell<Vec<PlayerEvent>>>,
    }

    fn no_covers() -> Box<dyn CoverSink + Send> {
        Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no covers in this test") })
    }

    /// Thư mục tạm, DB tạm, `FakeOutput`: không chạm loa, không chạm DB thật.
    fn rig(cx: &mut TestAppContext, prepare: impl FnOnce(&Path, &Library)) -> Rig {
        // Engine và thư viện chạy luồng thật, đánh thức foreground task từ luồng khác.
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let library = Library::open(config, no_covers()).unwrap();
        prepare(dir.path(), &library);
        let fake = FakeOutput::new(48_000, 2);
        let deps = PlayerDeps {
            library: Some(library.clone()),
            output: Box::new(fake.clone()),
            options: EngineOptions {
                stall_timeout: Duration::from_secs(3600),
                reopen_interval: Duration::from_millis(1),
                reopen_attempts: 1,
                ..EngineOptions::default()
            },
            session_file: Some(dir.path().join("session.json")),
        };
        let settings = SettingsSource::Local(Box::default());
        let player = cx.update(|cx| Player::spawn_with(deps, settings, cx)).unwrap();
        let events: Rc<RefCell<Vec<PlayerEvent>>> = Rc::default();
        let log = events.clone();
        cx.update(|cx| {
            cx.subscribe(&player, move |_, event: &PlayerEvent, _| log.borrow_mut().push(event.clone())).detach();
        });
        let rig = Rig { dir, library, fake, player, events };
        rig.until(cx, "session read", |p| p.session_ready);
        rig
    }

    impl Rig {
        fn wav(&self, name: &str) -> PathBuf {
            fixtures::write_sample(self.dir.path(), name, AudioKind::Wav).unwrap()
        }

        fn gone(&self, name: &str) -> PathBuf {
            self.dir.path().join(format!("{name}.wav"))
        }

        fn song(&self, path: &Path) -> i64 {
            add_song(&self.library, path)
        }

        /// Chạy vòng sự kiện và kéo mẫu cho tới khi điều kiện đúng.
        fn until(&self, cx: &mut TestAppContext, what: &str, condition: impl Fn(&Player) -> bool) {
            let deadline = Instant::now() + TIMEOUT;
            loop {
                cx.run_until_parked();
                if self.player.read_with(cx, |p, _| condition(p)) {
                    return;
                }
                assert!(Instant::now() < deadline, "timed out: {what}");
                let _ = self.fake.pull(480);
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        /// Như `until` nhưng không kéo mẫu: tiếng đứng yên.
        fn until_quiet(&self, cx: &mut TestAppContext, what: &str, condition: impl Fn(&Player) -> bool) {
            let deadline = Instant::now() + TIMEOUT;
            loop {
                cx.run_until_parked();
                if self.player.read_with(cx, |p, _| condition(p)) {
                    return;
                }
                assert!(Instant::now() < deadline, "timed out: {what}");
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn update(&self, cx: &mut TestAppContext, f: impl FnOnce(&mut Player, &mut Context<Player>)) {
            self.player.update(cx, f);
        }

        fn read<T>(&self, cx: &TestAppContext, f: impl FnOnce(&Player) -> T) -> T {
            self.player.read_with(cx, |p, _| f(p))
        }

        fn failures(&self) -> Vec<(TrackRef, bool)> {
            self.events
                .borrow()
                .iter()
                .filter_map(|event| match event {
                    PlayerEvent::TrackFailed { track, gave_up, .. } => Some((track.clone(), *gave_up)),
                    _ => None,
                })
                .collect()
        }

        fn flush(&self, cx: &mut TestAppContext) -> Session {
            self.update(cx, |p, _| {
                p.save_session();
                p.sync.flush();
            });
            Session::load_from(&self.dir.path().join("session.json"))
        }
    }

    fn add_song(library: &Library, path: &Path) -> i64 {
        let path = path.to_owned();
        library
            .writer()
            .transaction(move |conn| {
                let upsert = SongUpsert {
                    path,
                    folder_id: 1,
                    album_id: None,
                    title: Some("Bài".into()),
                    artist: None,
                    album_artist: None,
                    duration_sec: 1.0,
                    track_no: None,
                    disc_no: None,
                    lyrics: Some("lời".into()),
                    timeline: Some("0:00 Đầu\n0:00.5 Cuối".into()),
                    cover_hash: None,
                    file_mtime: None,
                    file_size: None,
                    tags_read_at: None,
                };
                conn.execute("INSERT OR IGNORE INTO folders (id, name, path) VALUES (1, 'f', 'f')", [])
                    .map_err(queries::QueryError::from)?;
                Ok(queries::upsert_song(conn, &upsert)?.id)
            })
            .wait()
            .unwrap()
    }

    /// Tổng biên độ một khung, sau đoạn nội suy tránh click.
    fn loudness(fake: &FakeOutput) -> f32 {
        for _ in 0..20 {
            let _ = fake.pull(480);
            std::thread::sleep(Duration::from_millis(1));
        }
        fake.pull(480).map_or(0.0, |s| s.iter().map(|x| x.abs()).sum::<f32>())
    }

    fn lib(id: i64) -> TrackRef {
        TrackRef::Library(id)
    }

    fn ext(path: &Path) -> TrackRef {
        TrackRef::from_path(path)
    }

    fn loaded(p: &Player) -> bool {
        matches!(p.load, LoadState::Loaded { .. })
    }

    /// C1 (N8): Next ở bài cuối dừng thật, về 0, bài vẫn chọn; Play sau đó phát lại từ đầu.
    #[gpui::test]
    fn next_on_the_last_track_really_stops(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let path = rig.wav("a");
        let id = rig.song(&path);
        rig.update(cx, |p, cx| p.play_list(lib(id), vec![lib(id)], cx));
        rig.until(cx, "playing", |p| p.is_playing() && p.position_sec() > 0.1);

        rig.update(cx, |p, cx| p.next(cx));
        assert!(!rig.read(cx, Player::is_playing), "Pause có hiệu lực ngay");
        rig.until_quiet(cx, "back to 0", |p| p.position_sec() == 0.0);
        assert!(loudness(&rig.fake) < SILENCE, "không còn tiếng sau khi dừng");
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(lib(id)));
        assert_eq!(rig.flush(cx).position_sec, 0.0);

        rig.update(cx, |p, cx| p.toggle_play(cx));
        rig.until(cx, "plays again", |p| p.is_playing() && p.position_sec() > 0.1);
    }

    /// C2 (N8): bài cuối hết tự nhiên cũng dừng, không kẹt ở đang phát.
    #[gpui::test]
    fn natural_end_of_the_last_track_stops(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let path = rig.wav("a");
        rig.update(cx, |p, cx| p.play_track(ext(&path), cx));
        rig.until(cx, "started", |p| p.wants_playback && loaded(p));
        rig.until(cx, "stopped", |p| !p.wants_playback && !p.is_playing());
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(ext(&path)));
    }

    /// C3 (N5): ba tệp lỗi liên tiếp thì dừng, tệp thứ tư không được thử.
    #[gpui::test]
    fn three_errors_in_a_row_stop_playback(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let dead: Vec<TrackRef> = ["a", "b", "c", "d"].map(|n| ext(&rig.gone(n))).to_vec();
        rig.update(cx, |p, cx| p.play_list(dead[0].clone(), dead.clone(), cx));
        rig.until(cx, "gave up", |p| !p.wants_playback && p.consecutive_errors == MAX_CONSECUTIVE_ERRORS);
        let failures = rig.failures();
        assert_eq!(
            failures,
            vec![(dead[0].clone(), false), (dead[1].clone(), false), (dead[2].clone(), true)],
            "báo đủ ba lần, lần ba là bỏ cuộc"
        );
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(dead[2].clone()));
        assert!(!rig.read(cx, Player::is_playing));

        // Next sau khi bỏ cuộc là thao tác của người dùng: đếm lại, thử bài kế.
        rig.update(cx, |p, cx| p.next(cx));
        rig.until(cx, "tried next", |p| p.current_track() == Some(&dead[3]) && p.load == LoadState::Failed);
        assert_eq!(rig.read(cx, |p| p.consecutive_errors), 1);
    }

    /// C4 (N5): tệp lỗi xen giữa thì tự sang bài kế và phát.
    #[gpui::test]
    fn a_bad_file_is_skipped(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (bad, good) = (ext(&rig.gone("bad")), ext(&rig.wav("good")));
        rig.update(cx, |p, cx| p.play_list(bad.clone(), vec![bad.clone(), good.clone()], cx));
        rig.until(cx, "plays the good one", |p| p.current_track() == Some(&good) && p.is_playing());
        assert_eq!(rig.failures(), vec![(bad, false)]);
    }

    /// C5 (N5): bài thư viện đã mất khi tới lượt phát đi đường lỗi, không kẹt.
    #[gpui::test]
    fn library_track_that_vanished_is_skipped(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let good = ext(&rig.wav("good"));
        rig.update(cx, |p, cx| p.play_list(lib(999), vec![lib(999), good.clone()], cx));
        rig.until(cx, "plays the good one", |p| p.current_track() == Some(&good) && p.is_playing());
        assert_eq!(rig.failures(), vec![(lib(999), false)]);
    }

    /// C6 (N5): lỗi thiết bị không sang bài, không tính vào giới hạn.
    #[gpui::test]
    fn output_error_does_not_run_through_the_queue(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b) = (ext(&rig.wav("a")), ext(&rig.wav("b")));
        rig.fake.fail_next_opens(1000);
        rig.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b], cx));
        rig.until(cx, "output failed", |p| !p.wants_playback && loaded(p));
        assert!(rig.events.borrow().contains(&PlayerEvent::OutputFailed));
        assert!(rig.failures().is_empty());
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(a));
        assert_eq!(rig.read(cx, |p| p.consecutive_errors), 0);

        rig.fake.fail_next_opens(0);
        rig.update(cx, |p, cx| p.play(cx));
        rig.until(cx, "plays after retry", |p| p.is_playing() && p.position_sec() > 0.1);
    }

    /// C7 (N6): tệp ngoài thứ hai mở khi tệp thứ nhất đang phát thì phát, tệp đầu vào history.
    #[gpui::test]
    fn second_external_file_plays(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b) = (rig.wav("a"), rig.wav("b"));
        rig.update(cx, |p, cx| p.play_paths(vec![a.clone()], cx));
        rig.until(cx, "first", |p| p.current_song().is_some_and(|s| s.path == a) && p.is_playing());
        rig.update(cx, |p, cx| p.play_paths(vec![b.clone()], cx));
        rig.until(cx, "second", |p| p.current_song().is_some_and(|s| s.path == b) && p.is_playing());
        assert_eq!(rig.read(cx, |p| p.queue().history().to_vec()), vec![ext(&a)]);
    }

    /// C8 (N6): mở nhiều tệp: tệp đầu phát, tệp sau vào hàng đợi; tệp trong thư viện đổi thành `Library`.
    #[gpui::test]
    fn opening_many_paths_fills_the_queue(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b, c) = (rig.wav("a"), rig.wav("b"), rig.wav("c"));
        let id = rig.song(&b);
        rig.update(cx, |p, cx| p.play_paths(vec![a.clone(), b, c.clone()], cx));
        rig.until(cx, "first", |p| p.current_track() == Some(&ext(&a)) && loaded(p));
        assert_eq!(rig.read(cx, |p| p.queue().queue().to_vec()), vec![lib(id), ext(&c)]);
    }

    /// C9 (N6): tệp ngoài không có thư viện đỡ vẫn lên `Song`, hết bài không kẹt, Next ở cuối dừng.
    #[gpui::test]
    fn external_song_gets_tags_from_the_file(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let path = fixtures::write_mp3_with_frames(rig.dir.path(), "tagged", &[("TIT2", "Tên trong thẻ")]).unwrap();
        rig.update(cx, |p, cx| p.play_track(ext(&path), cx));
        rig.until(cx, "tags", |p| p.current_song().is_some_and(|s| s.title.as_deref() == Some("Tên trong thẻ")));
    }

    /// C10 (bảng 1.4): repeat một bài phát lại đúng bài này khi hết tự nhiên.
    #[gpui::test]
    fn repeat_one_replays_after_ended(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b) = (ext(&rig.wav("a")), ext(&rig.wav("b")));
        rig.update(cx, |p, cx| {
            p.play_list(a.clone(), vec![a.clone(), b.clone()], cx);
            p.toggle_repeat(cx);
        });
        rig.until(cx, "started", |p| p.position_sec() > 0.5);
        // Không có `Ended` nào đổi bài: chờ vị trí quay vòng, nghĩa là đã qua hết bài và phát lại.
        rig.until(cx, "wrapped", |p| p.position_sec() < 0.3 && p.is_playing());
        rig.until(cx, "again", |p| p.position_sec() > 0.3);
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(a));
        assert_eq!(rig.read(cx, |p| p.queue().queue().to_vec()), vec![b]);
    }

    /// C11: `Ended` của bài cũ tới muộn trong lúc đang tra bài kế không làm nhảy bài.
    #[gpui::test]
    fn stale_events_are_ignored(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b, c) = (ext(&rig.wav("a")), ext(&rig.wav("b")), ext(&rig.wav("c")));
        rig.update(cx, |p, cx| p.play_list(a.clone(), vec![a, b.clone(), c.clone()], cx));
        rig.until(cx, "loaded", loaded);
        let old = rig.read(cx, |p| p.generation);
        rig.update(cx, |p, cx| {
            p.next(cx);
            p.on_audio_event(AudioEvent::Ended { generation: old }, cx);
        });
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(b));
        assert_eq!(rig.read(cx, |p| p.queue().queue().to_vec()), vec![c]);
    }

    /// C12: Prev khi history rỗng về 0, giữ nguyên trạng thái dừng.
    #[gpui::test]
    fn prev_without_history_seeks_to_start(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let a = ext(&rig.wav("a"));
        rig.update(cx, |p, cx| p.play_track(a, cx));
        rig.until(cx, "moving", |p| p.position_sec() > 0.2);
        rig.update(cx, |p, cx| p.pause(cx));
        rig.update(cx, |p, cx| p.prev(cx));
        rig.until_quiet(cx, "at 0", |p| p.position_sec() == 0.0);
        assert!(!rig.read(cx, Player::is_playing));
    }

    /// C13 (N5): thư viện báo bài mất: bỏ khỏi hàng đợi; mất bài đang phát thì sang bài kế.
    #[gpui::test]
    fn removed_tracks_leave_the_queue(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let ids: Vec<i64> = ["a", "b", "c"].map(|n| rig.song(&rig.wav(n))).to_vec();
        rig.update(cx, |p, cx| p.play_list(lib(ids[0]), ids.iter().copied().map(lib).collect(), cx));
        rig.until(cx, "first", |p| loaded(p) && p.is_playing());

        let report = SyncReport { added: 0, updated: 0, removed: vec![ids[1]] };
        rig.library.writer().emit(LibraryEvent::Synced(report));
        rig.until(cx, "b gone", |p| p.queue().queue() == [lib(ids[2])]);
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(lib(ids[0])));

        let report = SyncReport { added: 0, updated: 0, removed: vec![ids[0]] };
        rig.library.writer().emit(LibraryEvent::Synced(report));
        rig.until(cx, "moved on", |p| p.current_track() == Some(&lib(ids[2])) && p.is_playing());
    }

    /// C14 (N5): đang dừng mà mất bài đang chọn thì không tự phát, thanh phát về trống.
    #[gpui::test]
    fn removing_the_paused_track_does_not_start_playing(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let ids: Vec<i64> = ["a", "b"].map(|n| rig.song(&rig.wav(n))).to_vec();
        rig.update(cx, |p, cx| p.play_list(lib(ids[0]), ids.iter().copied().map(lib).collect(), cx));
        rig.until(cx, "loaded", loaded);
        rig.update(cx, |p, cx| p.pause(cx));
        let report = SyncReport { added: 0, updated: 0, removed: vec![ids[0]] };
        rig.library.writer().emit(LibraryEvent::Synced(report));
        rig.until_quiet(cx, "gone", |p| p.current_track().is_none());
        assert!(rig.read(cx, |p| p.current_song().is_none() && !p.is_playing()));
        assert_eq!(rig.read(cx, |p| p.queue().queue().to_vec()), vec![lib(ids[1])]);

        rig.update(cx, |p, cx| p.play(cx));
        rig.until(cx, "next one", |p| p.current_track() == Some(&lib(ids[1])) && p.is_playing());
    }

    /// C15 (N5): đổi đường dẫn: `Moved` làm mới `Song`, lần phát sau theo đường dẫn mới vì tra theo `id`.
    #[gpui::test]
    fn moved_song_refreshes_by_id(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let path = rig.wav("a");
        let id = rig.song(&path);
        rig.update(cx, |p, cx| p.play_track(lib(id), cx));
        rig.until(cx, "loaded", loaded);
        let moved = rig.dir.path().join("moved.wav");
        std::fs::copy(&path, &moved).unwrap();
        let (from, to) = (path.clone(), moved.clone());
        rig.library
            .writer()
            .transaction(move |conn| {
                conn.execute("UPDATE songs SET file_path = ?1", [to.to_string_lossy().into_owned()])
                    .map_err(queries::QueryError::from)?;
                let _ = from;
                Ok(())
            })
            .wait()
            .unwrap();
        rig.library.writer().emit(LibraryEvent::Moved(vec![MovedSong { id, from: path, to: moved.clone() }]));
        rig.until(cx, "new path", |p| p.current_song().is_some_and(|s| s.path == moved));
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(lib(id)));
    }

    /// C16 (đặc tả 1.4, N5): mở lại nạp bài cũ ở vị trí cũ, không tự phát; bài chết bị bỏ.
    #[gpui::test]
    fn restore_loads_at_the_old_position_without_playing(cx: &mut TestAppContext) {
        let wanted = Rc::new(RefCell::new(None));
        let slot = wanted.clone();
        let rig = rig(cx, move |dir, library| {
            let a = fixtures::write_sample(dir, "a", AudioKind::Wav).unwrap();
            let b = fixtures::write_sample(dir, "b", AudioKind::Wav).unwrap();
            let (a, b) = (add_song(library, &a), add_song(library, &b));
            *slot.borrow_mut() = Some(a);
            let session = Session {
                current: Some(lib(a)),
                position_sec: 0.5,
                original: vec![lib(a), lib(b), lib(999)],
                history: vec![lib(998)],
                queue: vec![lib(b), lib(999), TrackRef::from_path(dir.join("mất.wav"))],
                shuffle: false,
                repeat: RepeatMode::One,
            };
            session.save_to(&dir.join("session.json")).unwrap();
        });
        let a = wanted.borrow().unwrap();
        rig.until_quiet(cx, "restored", loaded);
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(lib(a)));
        assert!(!rig.read(cx, Player::is_playing), "không tự phát");
        assert!(rig.read(cx, |p| (p.position_sec() - 0.5).abs() < 0.01));
        let (queue, history, repeat) =
            rig.read(cx, |p| (p.queue().queue().to_vec(), p.queue().history().to_vec(), p.queue().repeat()));
        assert_eq!(queue.len(), 1, "chỉ còn bài sống: {queue:?}");
        assert!(history.is_empty());
        assert_eq!(repeat, RepeatMode::One);
        // Thông tin bài lấy từ thư viện, timeline chốt theo thời lượng thật.
        rig.until_quiet(cx, "text", |p| p.lyrics() == Some("lời"));
        assert_eq!(rig.read(cx, |p| p.timeline().last().map(|s| s.end)), Some(1.0));
    }

    /// C17: bài của phiên đã mất thì không có bài chọn, hàng đợi còn, không tự phát; Play lấy bài kế.
    #[gpui::test]
    fn restore_with_a_dead_current_keeps_the_queue(cx: &mut TestAppContext) {
        let slot = Rc::new(RefCell::new(None));
        let out = slot.clone();
        let rig = rig(cx, move |dir, library| {
            let b = add_song(library, &fixtures::write_sample(dir, "b", AudioKind::Wav).unwrap());
            *out.borrow_mut() = Some(b);
            let session =
                Session { current: Some(lib(999)), position_sec: 3.0, queue: vec![lib(b)], ..Session::default() };
            session.save_to(&dir.join("session.json")).unwrap();
        });
        let b = slot.borrow().unwrap();
        rig.until_quiet(cx, "checked", |p| p.current_track().is_none());
        assert_eq!(rig.read(cx, |p| p.queue().queue().to_vec()), vec![lib(b)]);
        assert!(rig.failures().is_empty(), "không báo lỗi khi chỉ đang khôi phục");
        rig.update(cx, |p, cx| p.play(cx));
        rig.until(cx, "plays", |p| p.current_track() == Some(&lib(b)) && p.is_playing());
    }

    /// C18 (đặc tả 5.6): tệp mở lúc khởi động thắng bài của phiên.
    #[gpui::test]
    fn opened_file_beats_the_restored_track(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let a = fixtures::write_sample(dir.path(), "a", AudioKind::Wav).unwrap();
        let b = fixtures::write_sample(dir.path(), "b", AudioKind::Wav).unwrap();
        let session_file = dir.path().join("session.json");
        Session { current: Some(ext(&a)), position_sec: 0.5, ..Session::default() }.save_to(&session_file).unwrap();
        let fake = FakeOutput::new(48_000, 2);
        let deps = PlayerDeps {
            library: None,
            output: Box::new(fake.clone()),
            options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
            session_file: Some(session_file),
        };
        let player = cx.update(|cx| Player::spawn_with(deps, SettingsSource::Local(Box::default()), cx)).unwrap();
        player.update(cx, |p, cx| p.play_paths(vec![b.clone()], cx));
        let deadline = Instant::now() + TIMEOUT;
        loop {
            cx.run_until_parked();
            if player.read_with(cx, |p, _| p.current_song().is_some_and(|s| s.path == b) && p.is_playing()) {
                break;
            }
            assert!(Instant::now() < deadline, "opened file did not play");
            let _ = fake.pull(480);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// C19 (S12): phiên ghi đủ bài và vị trí, không ghi đè phiên cũ trước khi đọc xong.
    #[gpui::test]
    fn session_is_saved_with_position(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let (a, b) = (ext(&rig.wav("a")), ext(&rig.wav("b")));
        rig.update(cx, |p, cx| p.play_list(a.clone(), vec![a.clone(), b.clone()], cx));
        rig.until(cx, "moving", |p| p.position_sec() > 0.2);
        rig.update(cx, |p, cx| {
            p.pause(cx);
            p.toggle_repeat(cx);
        });
        let saved = rig.flush(cx);
        assert_eq!(saved.current, Some(a));
        assert_eq!(saved.queue, vec![b]);
        assert_eq!(saved.repeat, RepeatMode::One);
        assert!(saved.position_sec > 0.2);
    }

    #[test]
    fn nothing_is_saved_before_the_session_is_read() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("session.json");
        let mut sync = SessionSync::new(Some(path.clone()));
        sync.flush();
        assert!(!path.exists());
        sync.save(&Queue::with_seed(1), 0.0);
        sync.flush();
        assert!(path.exists());
    }

    /// C21: trong lúc soát phiên: thoát vẫn lưu đúng vị trí cũ; bấm Play không kẹt, bài của phiên phát từ vị trí cũ.
    #[gpui::test]
    fn play_while_the_session_is_being_checked(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let a = ext(&rig.wav("a"));
        let session = Session { current: Some(a.clone()), position_sec: 0.5, ..Session::default() };
        rig.update(cx, |p, cx| {
            p.touched = false;
            p.apply_session(&session, cx);
        });
        assert_eq!(rig.flush(cx).position_sec, 0.5);
        rig.update(cx, |p, cx| {
            p.play(cx);
            p.finish_restore(&HashSet::new(), cx);
        });
        rig.until(cx, "plays from the old position", |p| p.is_playing() && p.position_sec() >= 0.5);
        assert_eq!(rig.read(cx, |p| p.current_track().cloned()), Some(a));
    }

    /// C20: âm lượng, mute đi thẳng xuống engine, EQ gửi được; kẹp ở 0..=100.
    #[gpui::test]
    fn volume_and_mute_reach_the_engine(cx: &mut TestAppContext) {
        let rig = rig(cx, |_, _| {});
        let a = ext(&rig.wav("a"));
        rig.update(cx, |p, cx| p.play_track(a, cx));
        rig.until(cx, "moving", |p| p.position_sec() > 0.05);
        rig.update(cx, |p, cx| p.toggle_mute(cx));
        assert!(loudness(&rig.fake) < SILENCE);
        assert!(rig.read(cx, |p| p.applied.muted));
        rig.update(cx, |p, cx| {
            p.toggle_mute(cx);
            p.adjust_volume(1000, cx);
        });
        assert!(loudness(&rig.fake) > SILENCE, "bỏ mute thì có tiếng lại");
        assert_eq!(rig.read(cx, |p| p.applied.volume), 100);
        rig.update(cx, |p, cx| p.adjust_volume(-1000, cx));
        assert_eq!(rig.read(cx, |p| p.applied.volume), 0);
        assert!(loudness(&rig.fake) < SILENCE);
        rig.update(cx, |p, cx| p.update_eq(cx, |eq| eq.set_band(0, 5.0)));
        assert_eq!(rig.read(cx, |p| p.applied.eq.gains_db[0]), 5.0);
    }
}
