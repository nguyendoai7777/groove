//! Keeps the library in step with the folders the user imported.
//!
//! Without this, a track dropped into a watched folder stays invisible until the
//! next manual import, and one deleted on disk lingers as a broken row. The
//! watcher closes that gap by re-indexing exactly the paths that changed.
//!
//! Two details matter for correctness:
//!
//! * **Debouncing.** A file being copied in fires a burst of events while it is
//!   still partially written, and reading tags from a half-written MP3 yields
//!   nonsense. Events are collected over a quiet period before anything is read.
//! * **Ignoring our own writes.** Saving metadata rewrites the file, which the
//!   watcher would see as an external change and re-index — a pointless round trip
//!   that also refreshes the UI mid-edit. Paths GrooveX just wrote are skipped.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::{db, DbState};

/// How long a path stays exempt after GrooveX writes it.
const SELF_WRITE_GRACE: Duration = Duration::from_secs(5);
/// Quiet period before a burst of filesystem events is acted on.
const DEBOUNCE_DELAY: Duration = Duration::from_secs(2);

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "flac", "wav"];

#[derive(Default)]
pub struct WatcherState {
    debouncer: Mutex<Option<Debouncer<RecommendedWatcher, FileIdMap>>>,
    self_writes: Mutex<HashMap<String, Instant>>,
}

/// What a sync pass changed, forwarded to the UI so it can refresh.
#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

impl SyncReport {
    fn is_empty(&self) -> bool {
        self.added == 0 && self.updated == 0 && self.removed == 0
    }
}

use crate::paths::normalize_path as normalize;

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Marks a path as written by GrooveX so the watcher ignores the resulting event.
pub fn note_self_write(app: &AppHandle, path: &Path) {
    let state = app.state::<WatcherState>();
    let mut writes = state.self_writes.lock().unwrap();
    writes.retain(|_, at| at.elapsed() < SELF_WRITE_GRACE);
    writes.insert(normalize(path), Instant::now());
}

fn take_self_write(state: &WatcherState, path: &Path) -> bool {
    let mut writes = state.self_writes.lock().unwrap();
    writes.retain(|_, at| at.elapsed() < SELF_WRITE_GRACE);
    writes.remove(&normalize(path)).is_some()
}

/// Rebuilds the watch list from the roots recorded in the database.
///
/// Called at startup and after an import. Dropping the previous debouncer first
/// releases the old handles, so re-registering a still-watched root is harmless.
pub fn restart(app: &AppHandle) -> Result<Vec<String>, String> {
    let roots = {
        let db_state = app.state::<DbState>();
        let conn = db_state.conn.lock().unwrap();
        db::fetch_watched_roots(&conn).map_err(|e| e.to_string())?
    };

    let watcher_state = app.state::<WatcherState>();
    let mut slot = watcher_state.debouncer.lock().unwrap();
    *slot = None;

    if roots.is_empty() {
        return Ok(roots);
    }

    let handler_app = app.clone();
    let mut debouncer = new_debouncer(
        DEBOUNCE_DELAY,
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                let paths: Vec<PathBuf> = events
                    .into_iter()
                    .flat_map(|event| event.event.paths.clone())
                    .collect();
                handle_changes(&handler_app, paths);
            }
            Err(errors) => {
                for error in errors {
                    eprintln!("Library watcher error: {error:?}");
                }
            }
        },
    )
    .map_err(|e| format!("Failed to create watcher: {e}"))?;

    let mut watched = Vec::new();
    for root in &roots {
        let path = Path::new(root);
        if !path.exists() {
            continue;
        }
        match debouncer.watcher().watch(path, RecursiveMode::Recursive) {
            Ok(()) => {
                debouncer.cache().add_root(path, RecursiveMode::Recursive);
                watched.push(root.clone());
            }
            Err(e) => eprintln!("Failed to watch {root}: {e}"),
        }
    }

    *slot = Some(debouncer);
    Ok(watched)
}

fn handle_changes(app: &AppHandle, paths: Vec<PathBuf>) {
    // A rename shows up as two paths and a burst of writes repeats the same one,
    // so collapse to a unique set before touching the database.
    let mut unique: HashSet<String> = HashSet::new();
    let candidates: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| is_audio(p))
        .filter(|p| unique.insert(normalize(p)))
        .collect();

    if candidates.is_empty() {
        return;
    }

    let report = match sync_paths(app, &candidates) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("Library sync failed: {e}");
            return;
        }
    };

    if !report.is_empty() {
        let _ = app.emit("library-synced", report);
    }
}

/// Re-indexes the given paths: present files are upserted, missing ones deleted.
fn sync_paths(app: &AppHandle, paths: &[PathBuf]) -> Result<SyncReport, String> {
    let watcher_state = app.state::<WatcherState>();
    let db_state = app.state::<DbState>();
    let conn = db_state.conn.lock().unwrap();

    let mut report = SyncReport::default();

    for path in paths {
        if take_self_write(&watcher_state, path) {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();
        let existed = db::song_exists(&conn, &path_str).unwrap_or(false);

        if path.exists() {
            // A file can still be locked by whatever copied it; failing here is fine
            // because the writer's own final event triggers another pass.
            match crate::index_song_file(&conn, path) {
                Ok(()) => {
                    if existed {
                        report.updated += 1;
                    } else {
                        report.added += 1;
                    }
                }
                Err(e) => eprintln!("Failed to index {path_str}: {e}"),
            }
        } else if existed {
            if db::delete_song_by_path(&conn, &path_str).unwrap_or(false) {
                report.removed += 1;
            }
        }
    }

    if report.removed > 0 {
        conn.execute(
            "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)",
            [],
        )
        .ok();
        conn.execute(
            "DELETE FROM folders WHERE id NOT IN (SELECT DISTINCT folder_id FROM songs)",
            [],
        )
        .ok();
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::sync::Arc;

    /// Drives the same notify + debouncer setup `restart` uses, without the Tauri
    /// app handle, and checks that a file created *after* watching began — in a
    /// subdirectory that did not exist at the time — still produces an event.
    ///
    /// A recursive watch that misses new subdirectories would silently stop syncing
    /// exactly the case this feature exists for.
    fn collect_events(root: &Path, act: impl FnOnce()) -> Vec<PathBuf> {
        let (tx, rx) = mpsc::channel();
        let tx = Arc::new(Mutex::new(tx));

        let mut debouncer = new_debouncer(
            Duration::from_millis(300),
            None,
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    let paths: Vec<PathBuf> =
                        events.into_iter().flat_map(|e| e.event.paths.clone()).collect();
                    let _ = tx.lock().unwrap().send(paths);
                }
            },
        )
        .expect("debouncer");

        debouncer
            .watcher()
            .watch(root, RecursiveMode::Recursive)
            .expect("watch");
        debouncer.cache().add_root(root, RecursiveMode::Recursive);

        act();

        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(paths) => {
                    seen.extend(paths);
                    // Give the debouncer a moment to flush any trailing batch.
                    if !seen.is_empty() {
                        while let Ok(more) = rx.recv_timeout(Duration::from_millis(400)) {
                            seen.extend(more);
                        }
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        seen
    }

    #[test]
    fn watcher_sees_a_file_added_to_a_new_subdirectory() {
        let root = std::env::temp_dir().join("groovex_watch_test");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();

        let nested = root.join("Fresh Album");
        let track = nested.join("new track.mp3");

        let seen = collect_events(&root, || {
            // The subdirectory does not exist when watching starts.
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(&track, b"ID3\x03\x00\x00\x00\x00\x00\x00").unwrap();
        });

        let normalized: Vec<String> = seen.iter().map(|p| normalize(p)).collect();
        assert!(
            normalized.contains(&normalize(&track)),
            "watcher missed the new file; saw {normalized:?}"
        );

        // And the audio filter must let it through to the indexer.
        assert!(is_audio(&track));
        assert!(!is_audio(&nested.join("cover.jpg")));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn watcher_sees_a_deleted_file() {
        let root = std::env::temp_dir().join("groovex_watch_delete_test");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let track = root.join("gone.mp3");
        std::fs::write(&track, b"ID3\x03\x00\x00\x00\x00\x00\x00").unwrap();

        let seen = collect_events(&root, || {
            std::fs::remove_file(&track).unwrap();
        });

        let normalized: Vec<String> = seen.iter().map(|p| normalize(p)).collect();
        assert!(
            normalized.contains(&normalize(&track)),
            "watcher missed the deletion; saw {normalized:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
