//! groove-gd1_05: LibraryActor, luồng ghi duy nhất.
//!
//! N7: UI không chạm đĩa, không giữ khoá khi làm IO. Mỗi kết nối SQLite nằm trên một luồng riêng và chỉ luồng đó chạm vào
//! nó, nhận việc qua channel theo thứ tự đến. Gửi việc không bao giờ chặn; kết quả về qua `Reply`, một `Future` mà UI
//! `await` được. Khoá duy nhất là `Gate` và `EventBus`: chỉ giữ để đọc ghi vài trường, không bao giờ qua IO.
//!
//! Mở DB, kể cả chuyển DB cũ mất vài giây, cũng là một việc trên luồng ghi. Việc đọc đến sớm thì nằm chờ `Gate` trên luồng
//! đọc, không báo lỗi giả.

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll};
use std::{io, thread};

use crossbeam_channel::{Sender, unbounded};
use futures::channel::{mpsc, oneshot};
use rusqlite::{Connection, TransactionBehavior};

use crate::db::migrate::{self, CoverSink, MigrateError, OpenOutcome};
use crate::db::queries::{self, QueryError};

/// Số dòng tối đa một transaction của tác vụ lô, để việc khác không phải chờ lâu (N7).
pub const BATCH_ROWS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// Thư viện chưa mở được, xem `OpenFailure`.
    #[error("library is unavailable: {0}")]
    Unavailable(OpenFailure),
    /// Đã `shutdown`.
    #[error("library is closed")]
    Closed,
    /// Việc panic: transaction đã rollback, luồng vẫn sống (S25).
    #[error("library task panicked: {0}")]
    Panicked(String),
    /// Đã có lượt tìm mới hơn, kết quả này không còn ai cần (S18).
    #[error("search superseded by a newer one")]
    Superseded,
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type LibraryResult<T> = Result<T, LibraryError>;

/// Vì sao không mở được, đủ để giao diện báo và chọn lối ra.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenFailure {
    /// DB của bản mới hơn: không mở, không ghi đè (đặc tả 2.3 bước 6). Không có lối ra nào ngoài cập nhật app.
    #[error("{} has schema version {found}, newer than this build", path.display())]
    TooNew { path: PathBuf, found: i32 },
    /// `library.db` chưa có, tệp cũ nguyên vẹn. Lối ra: `Library::retry_open` hoặc `Library::start_fresh`. Không tự tạo thư
    /// viện trống: có `library.db` rồi thì không bao giờ chuyển lại nữa.
    #[error("could not migrate {}: {message}", path.display())]
    LegacyFailed { path: PathBuf, message: String },
    #[error("{} is not a Groove X library", .0.display())]
    NotLibrary(PathBuf),
    #[error("{0}")]
    Other(String),
}

impl From<MigrateError> for OpenFailure {
    fn from(err: MigrateError) -> OpenFailure {
        match err {
            MigrateError::TooNew { path, found } => OpenFailure::TooNew { path, found },
            MigrateError::LegacyFailed { path, source } => {
                OpenFailure::LegacyFailed { path, message: source.to_string() }
            }
            MigrateError::NotLibrary(path) => OpenFailure::NotLibrary(path),
            other => OpenFailure::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenStatus {
    Opening,
    Ready(OpenOutcome),
    Failed(OpenFailure),
    Closed,
}

/// `LibraryEvent::Synced`: sau khi import, quét lại, watcher ghi xong (đặc tả 2.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    /// `id` đã xoá, để hàng đợi bỏ đúng bài (N5).
    pub removed: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedSong {
    pub id: i64,
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    Import,
    Rescan,
    TagBackfill,
    MetadataBulk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub kind: ProgressKind,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryEvent {
    Opened(OpenOutcome),
    OpenFailed(OpenFailure),
    Synced(SyncReport),
    /// Đĩa đổi ngoài app (watcher, so khi mở app), đi sau `Synced` của cùng lô: để UI báo (đặc tả 1.11). Import, quét lại, thao
    /// tác trong app không phát sự kiện này vì đã có toast riêng.
    ExternalChange(SyncReport),
    Moved(Vec<MovedSong>),
    Progress(Progress),
    /// Tạo, xoá playlist hoặc đổi thành viên: lưới và danh sách đang mở nạp lại.
    PlaylistsChanged,
}

/// Kết quả của một việc đã gửi. Bỏ đi không huỷ việc. Phục vụ UI bằng `await`; `wait` chỉ cho luồng nền.
pub struct Reply<T> {
    rx: oneshot::Receiver<LibraryResult<T>>,
}

impl<T> Reply<T> {
    /// Chặn tới khi xong. Chỉ dùng trên luồng worker, ví dụ tạo backpressure cho indexer; không bao giờ trên luồng UI.
    pub fn wait(self) -> LibraryResult<T> {
        futures::executor::block_on(self)
    }
}

impl<T> Future for Reply<T> {
    type Output = LibraryResult<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Luồng đã thoát thì việc bị bỏ cùng đầu gửi.
        Pin::new(&mut self.rx).poll(cx).map(|reply| reply.unwrap_or(Err(LibraryError::Closed)))
    }
}

enum Message<S> {
    Job(Box<dyn FnOnce(&mut S) + Send>),
    Stop(oneshot::Sender<()>),
}

/// Một luồng, một trạng thái, việc chạy lần lượt. Trạng thái được drop trước khi `Stop` báo xong.
pub(crate) struct Worker<S> {
    tx: Sender<Message<S>>,
}

impl<S> Clone for Worker<S> {
    fn clone(&self) -> Self {
        Worker { tx: self.tx.clone() }
    }
}

impl<S: Send + 'static> Worker<S> {
    pub(crate) fn spawn(name: &str, state: S) -> io::Result<Worker<S>> {
        let (tx, rx) = unbounded::<Message<S>>();
        thread::Builder::new().name(name.to_owned()).spawn(move || {
            let mut state = state;
            for message in rx {
                match message {
                    Message::Job(job) => job(&mut state),
                    Message::Stop(done) => {
                        drop(state);
                        let _ = done.send(());
                        return;
                    }
                }
            }
        })?;
        Ok(Worker { tx })
    }

    /// Không chặn: channel không giới hạn. Panic trong việc thành `LibraryError::Panicked`, luồng sống tiếp.
    pub(crate) fn submit<T, F>(&self, job: F) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut S) -> LibraryResult<T> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let wrapped = Box::new(move |state: &mut S| {
            let result = catch_unwind(AssertUnwindSafe(|| job(state))).unwrap_or_else(|panic| {
                let message = panic_message(panic.as_ref());
                log::error!("library task panicked: {message}");
                Err(LibraryError::Panicked(message))
            });
            let _ = tx.send(result);
        });
        // Luồng đã thoát: `wrapped` bị trả về và drop cùng `tx`, `Reply` ra `Closed`.
        let _ = self.tx.send(Message::Job(wrapped));
        Reply { rx }
    }

    /// Việc đã gửi trước đó vẫn chạy xong. Trả về đầu nhận báo luồng đã thả trạng thái.
    pub(crate) fn stop(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(Message::Stop(tx));
        rx
    }
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = panic.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = panic.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_owned()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Khoá chỉ bọc gán trường, panic không để lại dữ liệu dở.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Trạng thái mở, luồng đọc chờ trên đây. Chờ qua `Condvar` nên không giữ khoá trong lúc luồng ghi làm IO.
pub(crate) struct Gate {
    status: Mutex<OpenStatus>,
    changed: Condvar,
}

impl Gate {
    pub(crate) fn new() -> Gate {
        Gate { status: Mutex::new(OpenStatus::Opening), changed: Condvar::new() }
    }

    pub(crate) fn status(&self) -> OpenStatus {
        lock(&self.status).clone()
    }

    fn set(&self, status: OpenStatus) {
        *lock(&self.status) = status;
        self.changed.notify_all();
    }

    /// Chỉ gọi trên luồng đọc.
    pub(crate) fn wait_settled(&self) -> OpenStatus {
        let mut status = lock(&self.status);
        while *status == OpenStatus::Opening {
            status = self.changed.wait(status).unwrap_or_else(PoisonError::into_inner);
        }
        status.clone()
    }
}

/// Mọi người nghe đều nhận mọi sự kiện. Đầu nhận bị bỏ thì tự rời.
#[derive(Default)]
pub(crate) struct EventBus {
    subscribers: Mutex<Vec<mpsc::UnboundedSender<LibraryEvent>>>,
}

impl EventBus {
    pub(crate) fn subscribe(&self) -> mpsc::UnboundedReceiver<LibraryEvent> {
        let (tx, rx) = mpsc::unbounded();
        lock(&self.subscribers).push(tx);
        rx
    }

    pub(crate) fn emit(&self, event: LibraryEvent) {
        lock(&self.subscribers).retain(|subscriber| subscriber.unbounded_send(event.clone()).is_ok());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenMode {
    /// Có `library.db` thì mở, chưa có thì chuyển DB cũ, không có gì thì tạo.
    MigrateOrCreate,
    /// Bỏ qua DB cũ. `library.db` có sẵn vẫn qua kiểm phiên bản.
    Fresh,
}

pub(crate) struct WriterState {
    target: PathBuf,
    legacy: Option<PathBuf>,
    /// Bỏ ngay khi mở xong, chỉ cần lúc chuyển đổi.
    sink: Option<Box<dyn CoverSink + Send>>,
    conn: Option<Connection>,
    gate: Arc<Gate>,
    bus: Arc<EventBus>,
}

impl WriterState {
    fn open(&mut self, mode: OpenMode) -> Result<OpenOutcome, OpenFailure> {
        if self.conn.is_some() {
            return Ok(OpenOutcome::Existing);
        }
        self.gate.set(OpenStatus::Opening);
        // Panic mà để cửa ở `Opening` thì luồng đọc chờ mãi.
        let opened = catch_unwind(AssertUnwindSafe(|| self.open_connection(mode))).unwrap_or_else(|panic| {
            Err(OpenFailure::Other(format!("panic while opening the library: {}", panic_message(panic.as_ref()))))
        });
        match opened {
            Ok((conn, outcome)) => {
                self.conn = Some(conn);
                self.sink = None;
                self.gate.set(OpenStatus::Ready(outcome.clone()));
                self.bus.emit(LibraryEvent::Opened(outcome.clone()));
                Ok(outcome)
            }
            Err(failure) => {
                log::error!("library not opened: {failure}");
                self.gate.set(OpenStatus::Failed(failure.clone()));
                self.bus.emit(LibraryEvent::OpenFailed(failure.clone()));
                Err(failure)
            }
        }
    }

    fn open_connection(&mut self, mode: OpenMode) -> Result<(Connection, OpenOutcome), OpenFailure> {
        let opened = match (mode, self.sink.as_mut()) {
            (OpenMode::MigrateOrCreate, Some(sink)) => {
                migrate::open_or_migrate(&self.target, self.legacy.as_deref(), sink.as_mut())
            }
            _ => migrate::open_fresh(&self.target).map(|conn| (conn, OpenOutcome::Created)),
        };
        let (conn, outcome) = opened.map_err(OpenFailure::from)?;
        if let OpenOutcome::Migrated(_) = outcome
            && let Err(err) = seed_roots(&conn)
        {
            log::warn!("could not seed watched roots: {err}");
        }
        Ok((conn, outcome))
    }

    fn connection(&mut self) -> LibraryResult<&mut Connection> {
        match self.conn.as_mut() {
            Some(conn) => Ok(conn),
            None => match self.gate.status() {
                OpenStatus::Failed(failure) => Err(LibraryError::Unavailable(failure)),
                _ => Err(LibraryError::Closed),
            },
        }
    }
}

fn seed_roots(conn: &Connection) -> LibraryResult<()> {
    let tx = conn.unchecked_transaction()?;
    queries::seed_watched_roots_from_folders(&tx)?;
    tx.commit()?;
    Ok(())
}

impl Drop for WriterState {
    fn drop(&mut self) {
        // Đóng trước khi báo `Closed`: WAL checkpoint xong thì tệp mới rảnh.
        if let Some(conn) = self.conn.take()
            && let Err((_, err)) = conn.close()
        {
            log::warn!("closing the library: {err}");
        }
        self.gate.set(OpenStatus::Closed);
    }
}

/// Luồng ghi duy nhất. Sao chép rẻ, gửi được sang worker. Mọi ghi của mọi task đi qua `transaction`.
#[derive(Clone)]
pub struct LibraryActor {
    worker: Worker<WriterState>,
    bus: Arc<EventBus>,
}

impl LibraryActor {
    /// Trả về ngay, việc mở DB chạy trên luồng ghi. Chỉ lỗi khi không tạo được luồng.
    pub(crate) fn spawn(
        target: &Path,
        legacy: Option<&Path>,
        sink: Box<dyn CoverSink + Send>,
        gate: Arc<Gate>,
        bus: Arc<EventBus>,
    ) -> io::Result<LibraryActor> {
        let state = WriterState {
            target: target.to_owned(),
            legacy: legacy.map(Path::to_owned),
            sink: Some(sink),
            conn: None,
            gate,
            bus: Arc::clone(&bus),
        };
        let worker = Worker::spawn("groove-library-writer", state)?;
        let actor = LibraryActor { worker, bus };
        drop(actor.submit_open(OpenMode::MigrateOrCreate));
        Ok(actor)
    }

    fn submit_open(&self, mode: OpenMode) -> Reply<OpenOutcome> {
        self.worker.submit(move |state| state.open(mode).map_err(LibraryError::Unavailable))
    }

    /// Một transaction: `f` trả `Ok` thì commit, trả `Err` hoặc panic thì rollback. `f` chỉ được chạm DB: đọc thẻ, ghi tệp,
    /// giải ảnh làm trước ở worker rồi mới gửi kết quả vào đây (N7), mỗi lần tối đa `BATCH_ROWS` dòng.
    pub fn transaction<T, F>(&self, f: F) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> LibraryResult<T> + Send + 'static,
    {
        self.transaction_inner(f, || {})
    }

    /// Như `transaction`, phát `event` ngay sau commit. Rollback thì không phát.
    pub fn transaction_then_emit<T, F>(&self, event: LibraryEvent, f: F) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> LibraryResult<T> + Send + 'static,
    {
        let bus = Arc::clone(&self.bus);
        self.transaction_inner(f, move || bus.emit(event))
    }

    fn transaction_inner<T, F, C>(&self, f: F, committed: C) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> LibraryResult<T> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        self.worker.submit(move |state| {
            let conn = state.connection()?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let value = f(&tx)?;
            tx.commit()?;
            committed();
            Ok(value)
        })
    }

    /// Phát sau khi commit. Từ transaction ra thì gọi sau khi `Reply` xong, để người nghe đọc được dữ liệu mới.
    pub fn emit(&self, event: LibraryEvent) {
        self.bus.emit(event);
    }

    pub(crate) fn retry_open(&self) -> Reply<OpenOutcome> {
        self.submit_open(OpenMode::MigrateOrCreate)
    }

    pub(crate) fn start_fresh(&self) -> Reply<OpenOutcome> {
        self.submit_open(OpenMode::Fresh)
    }

    pub(crate) fn stop(&self) -> oneshot::Receiver<()> {
        self.worker.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_survives_a_panicking_job_and_reports_it() {
        let worker = Worker::spawn("test", 0_i32).unwrap();
        let panicked: LibraryResult<()> = worker.submit(|_| panic!("độc")).wait();
        assert!(matches!(panicked, Err(LibraryError::Panicked(message)) if message == "độc"));
        assert_eq!(
            worker
                .submit(|state| {
                    *state += 1;
                    Ok(*state)
                })
                .wait()
                .unwrap(),
            1
        );
    }

    #[test]
    fn jobs_after_stop_report_closed_and_earlier_jobs_finish() {
        let worker = Worker::spawn("test", 0_i32).unwrap();
        let before = worker.submit(|_| Ok(7));
        futures::executor::block_on(worker.stop()).unwrap();
        assert_eq!(before.wait().unwrap(), 7);
        assert!(matches!(worker.submit(|_| Ok(())).wait(), Err(LibraryError::Closed)));
    }

    #[test]
    fn dropped_subscribers_leave_the_bus() {
        let bus = EventBus::default();
        let mut alive = bus.subscribe();
        drop(bus.subscribe());
        bus.emit(LibraryEvent::PlaylistsChanged);
        assert_eq!(lock(&bus.subscribers).len(), 1);
        assert_eq!(alive.try_recv().unwrap(), LibraryEvent::PlaylistsChanged);
    }

    #[test]
    fn migrate_errors_map_to_open_failures() {
        let too_new = MigrateError::TooNew { path: "a".into(), found: 9 };
        assert_eq!(OpenFailure::from(too_new), OpenFailure::TooNew { path: "a".into(), found: 9 });
        let legacy =
            MigrateError::LegacyFailed { path: "b".into(), source: Box::new(MigrateError::NotLegacy("b".into())) };
        assert!(matches!(OpenFailure::from(legacy), OpenFailure::LegacyFailed { path, .. } if path == Path::new("b")));
    }
}
