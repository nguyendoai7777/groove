//! groove-gd1_05: Library, đọc qua kết nối riêng.
//!
//! Mặt tiền cho UI và task khác. Đọc chạy trên luồng đọc với kết nối chỉ đọc, nhờ WAL không chờ transaction đang ghi (N7).
//! Mọi hàm trả về ngay: `Reply` để `await`, không có hàm nào chạm đĩa trên luồng gọi.

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::channel::mpsc::UnboundedReceiver;
use groove_core::model::{CategoryRef, Song, TrackRef};
use groove_core::paths;
use rusqlite::{Connection, OpenFlags};

use crate::actor::{
    EventBus, Gate, LibraryActor, LibraryError, LibraryEvent, LibraryResult, OpenStatus, Reply, Worker,
};
use crate::covers::CoverStore;
use crate::db::migrate::{CoverSink, OpenOutcome};
use crate::db::queries::{self, Categories, CategorySummary, PendingTagRead, SongFileState, SongText};
use crate::db::schema;

/// Vị trí DB. Test trỏ vào thư mục tạm, đường chạy thật dùng `LibraryConfig::standard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryConfig {
    pub target: PathBuf,
    /// DB của bản Tauri, chuyển một lần khi `target` chưa có.
    pub legacy: Option<PathBuf>,
}

impl LibraryConfig {
    /// Đúng hai đường dẫn của `migrate::open_default`. Chỉ tính đường dẫn, chưa chạm tệp.
    pub fn standard() -> io::Result<LibraryConfig> {
        Ok(LibraryConfig { target: paths::library_db()?, legacy: Some(paths::legacy_db()?) })
    }
}

/// Một lượt tìm. Giữ `seq` lớn nhất đã hiện, lượt về muộn có `seq` nhỏ hơn thì bỏ (S18).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    pub seq: u64,
    pub songs: Vec<Song>,
}

struct ReaderState {
    target: PathBuf,
    gate: Arc<Gate>,
    conn: Option<Connection>,
}

impl ReaderState {
    /// Chờ luồng ghi mở xong: bản đọc mở trước có thể thấy tệp chưa có schema.
    fn connection(&mut self) -> LibraryResult<&Connection> {
        match self.gate.wait_settled() {
            OpenStatus::Ready(_) => {}
            OpenStatus::Failed(failure) => return Err(LibraryError::Unavailable(failure)),
            _ => return Err(LibraryError::Closed),
        }
        let conn = match self.conn.take() {
            Some(conn) => conn,
            None => {
                let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
                let conn = Connection::open_with_flags(&self.target, flags)?;
                schema::apply_pragmas(&conn)?;
                conn
            }
        };
        Ok(self.conn.insert(conn))
    }
}

/// Sao chép rẻ, gửi được giữa các luồng. Thả hết bản sao mà chưa `shutdown` thì hai luồng tự thoát.
#[derive(Clone)]
pub struct Library {
    writer: LibraryActor,
    reader: Worker<ReaderState>,
    gate: Arc<Gate>,
    bus: Arc<EventBus>,
    latest_search: Arc<AtomicU64>,
}

impl Library {
    /// Trả về ngay, mở và chuyển DB chạy nền. Theo dõi kết quả qua `status`, `subscribe`; việc gửi trong lúc đó tự xếp hàng.
    /// Đường chạy thật dùng `open_standard`; hàm này cho test và nơi cần chỗ khác.
    pub fn open(config: LibraryConfig, sink: Box<dyn CoverSink + Send>) -> io::Result<Library> {
        let gate = Arc::new(Gate::new());
        let bus = Arc::new(EventBus::default());
        let reader = Worker::spawn(
            "groove-library-reader",
            ReaderState { target: config.target.clone(), gate: Arc::clone(&gate), conn: None },
        )?;
        let writer =
            LibraryActor::spawn(&config.target, config.legacy.as_deref(), sink, Arc::clone(&gate), Arc::clone(&bus))?;
        Ok(Library { writer, reader, gate, bus, latest_search: Arc::new(AtomicU64::new(0)) })
    }

    /// Đường chạy thật: DB ở đường dẫn chuẩn, bìa của DB cũ tách ra `<cache>/covers`.
    pub fn open_standard() -> io::Result<Library> {
        Library::open(LibraryConfig::standard()?, Box::new(CoverStore::default_location()?))
    }

    /// Không chờ. Người nghe đăng ký muộn đọc ở đây thay vì sự kiện `Opened`.
    pub fn status(&self) -> OpenStatus {
        self.gate.status()
    }

    /// Thử chuyển DB cũ lại, sau `OpenFailure::LegacyFailed`. Đang mở rồi thì không làm gì.
    pub fn retry_open(&self) -> Reply<OpenOutcome> {
        self.writer.retry_open()
    }

    /// Người dùng chọn bỏ qua DB cũ. Sau đó không chuyển lại nữa; tệp cũ vẫn nguyên. `TooNew` vẫn bị từ chối.
    pub fn start_fresh(&self) -> Reply<OpenOutcome> {
        self.writer.start_fresh()
    }

    /// `Stream` sự kiện, `next().await` được trên luồng UI.
    pub fn subscribe(&self) -> UnboundedReceiver<LibraryEvent> {
        self.bus.subscribe()
    }

    /// Cho task 08, 09, 18: `writer().transaction(...)`, `emit`.
    pub fn writer(&self) -> &LibraryActor {
        &self.writer
    }

    /// Đọc tuỳ ý, cho truy vấn chỉ một task cần.
    pub fn read<T, F>(&self, f: F) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> LibraryResult<T> + Send + 'static,
    {
        self.reader.submit(move |state| f(state.connection()?))
    }

    pub fn categories(&self) -> Reply<Categories> {
        self.read(|conn| Ok(queries::categories(conn)?))
    }

    pub fn playlists(&self) -> Reply<Vec<CategorySummary>> {
        self.read(|conn| Ok(queries::playlists(conn)?))
    }

    pub fn category(&self, reference: CategoryRef) -> Reply<Option<CategorySummary>> {
        self.read(move |conn| Ok(queries::category(conn, reference)?))
    }

    pub fn songs(&self, reference: CategoryRef) -> Reply<Vec<Song>> {
        self.read(move |conn| Ok(queries::songs(conn, reference)?))
    }

    pub fn song_ids(&self, reference: CategoryRef) -> Reply<Vec<i64>> {
        self.read(move |conn| Ok(queries::song_ids(conn, reference)?))
    }

    /// Lượt mới làm lượt cũ còn trong hàng ra `Superseded` mà không chạy: gõ nhanh không xếp hàng truy vấn thừa (S18).
    pub fn search(&self, query: impl Into<String>, limit: usize) -> Reply<SearchResults> {
        let seq = self.latest_search.fetch_add(1, Ordering::Relaxed) + 1;
        let latest = Arc::clone(&self.latest_search);
        let query = query.into();
        self.read(move |conn| {
            if latest.load(Ordering::Relaxed) != seq {
                return Err(LibraryError::Superseded);
            }
            Ok(SearchResults { seq, songs: queries::search(conn, &query, limit)? })
        })
    }

    /// `None` khi `id` không còn, hoặc tệp ngoài thật: caller dùng `Song::external`.
    pub fn song(&self, track: TrackRef) -> Reply<Option<Song>> {
        self.read(move |conn| Ok(queries::song(conn, &track)?))
    }

    /// Giữ thứ tự, bỏ tham chiếu không tra được (N5).
    pub fn songs_for_tracks(&self, tracks: Vec<TrackRef>) -> Reply<Vec<Song>> {
        self.read(move |conn| Ok(queries::songs_for_tracks(conn, &tracks)?))
    }

    /// Đường dẫn thành `TrackRef::Library` nếu có trong thư viện, không thì `External` (N6).
    pub fn track_for_path(&self, path: impl Into<PathBuf>) -> Reply<TrackRef> {
        let path = path.into();
        self.read(move |conn| Ok(queries::track_for_path(conn, &path)?))
    }

    pub fn song_text(&self, id: i64) -> Reply<Option<SongText>> {
        self.read(move |conn| Ok(queries::song_text(conn, id)?))
    }

    pub fn watched_roots(&self) -> Reply<Vec<PathBuf>> {
        self.read(|conn| Ok(queries::watched_roots(conn)?))
    }

    pub fn song_file_states(&self) -> Reply<Vec<SongFileState>> {
        self.read(|conn| Ok(queries::song_file_states(conn)?))
    }

    pub fn pending_tag_reads(&self, limit: usize) -> Reply<Vec<PendingTagRead>> {
        self.read(move |conn| Ok(queries::pending_tag_reads(conn, limit)?))
    }

    pub fn create_playlist(&self, name: impl Into<String>, color: impl Into<String>) -> Reply<i64> {
        let (name, color) = (name.into(), color.into());
        self.playlist_write(move |conn| Ok(queries::create_playlist(conn, &name, &color)?))
    }

    pub fn delete_playlist(&self, id: i64) -> Reply<bool> {
        self.playlist_write(move |conn| Ok(queries::delete_playlist(conn, id)?))
    }

    pub fn add_song_to_playlist(&self, playlist_id: i64, song_id: i64) -> Reply<bool> {
        self.playlist_write(move |conn| Ok(queries::add_song_to_playlist(conn, playlist_id, song_id)?))
    }

    pub fn remove_song_from_playlist(&self, playlist_id: i64, song_id: i64) -> Reply<bool> {
        self.playlist_write(move |conn| Ok(queries::remove_song_from_playlist(conn, playlist_id, song_id)?))
    }

    fn playlist_write<T, F>(&self, f: F) -> Reply<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> LibraryResult<T> + Send + 'static,
    {
        self.writer.transaction_then_emit(LibraryEvent::PlaylistsChanged, f)
    }

    /// Việc đã gửi chạy nốt, đóng kết nối (checkpoint WAL). Khi thoát app: `block_on` tương lai này ở luồng thoát. Sau đó mọi
    /// việc ra `LibraryError::Closed`.
    pub fn shutdown(&self) -> impl Future<Output = ()> + Send + use<> {
        let writer_done = self.writer.stop();
        let reader_done = self.reader.stop();
        async move {
            let _ = futures::join!(writer_done, reader_done);
        }
    }
}
