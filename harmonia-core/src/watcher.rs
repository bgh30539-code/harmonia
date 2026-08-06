//! Live filesystem watching for library folders.
//!
//! Uses the `notify` crate (inotify on Linux). Events are batched and
//! debounced for 1.5 seconds of quiet so that metadata-heavy operations are
//! only triggered once the filesystem has settled — a mass copy or a download
//! produces hundreds of events but a single update cycle.

use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{CoreError, CoreResult};
use crate::util::is_audio_file;

const DEBOUNCE_MS: u64 = 1_500;

/// Keeps the underlying watcher alive. Dropping it stops watching.
pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,
}

impl LibraryWatcher {
    /// Starts watching `folders`. Batches of changed paths are handed to
    /// `handler` on a background thread after the debounce window.
    pub fn spawn<F>(folders: &[PathBuf], handler: F) -> CoreResult<Self>
    where
        F: FnMut(Vec<PathBuf>) + Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<PathBuf>>();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let _ = tx.send(event.paths);
            }
        })
        .map_err(|e| CoreError::Invalid(format!("failed to start filesystem watcher: {e}")))?;

        let mut watcher = watcher;
        for folder in folders {
            watcher
                .watch(folder, RecursiveMode::Recursive)
                .map_err(|e| {
                    CoreError::Invalid(format!("failed to watch {}: {e}", folder.display()))
                })?;
        }

        let mut handler = handler;
        std::thread::Builder::new()
            .name("harmonia-watcher".into())
            .spawn(move || debounce_loop(rx, &mut handler))
            .map_err(CoreError::Io)?;

        Ok(Self { _watcher: watcher })
    }
}

fn debounce_loop<F>(rx: std::sync::mpsc::Receiver<Vec<PathBuf>>, handler: &mut F)
where
    F: FnMut(Vec<PathBuf>),
{
    let mut pending: Vec<PathBuf> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)) {
            Ok(paths) => pending.extend(paths),
            Err(RecvTimeoutError::Timeout) => {
                if !pending.is_empty() {
                    let batch = std::mem::take(&mut pending);
                    handler(batch);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Upserts or deletes a batch of filesystem paths, returning what happened.
///
/// The app layer calls this when the watcher reports changes: existing audio
/// files are re-parsed, directories are rescanned recursively, and removed
/// files are deleted from the library.
pub fn sync_paths<F>(
    db: &crate::db::Database,
    paths: &[PathBuf],
    art_cache_dir: &Path,
    on_progress: F,
) -> CoreResult<usize>
where
    F: Fn(crate::models::ScanProgress),
{
    let mut touched = 0usize;
    for path in paths {
        if path.is_dir() {
            // A directory changed: walk it and upsert anything audio inside.
            let mut files: Vec<PathBuf> = Vec::new();
            for entry in walkdir::WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() && is_audio_file(entry.path()) {
                    files.push(entry.into_path());
                }
            }
            touched += files.len();
            for file in files {
                match crate::metadata::read_track_meta(&file, art_cache_dir) {
                    Ok(meta) => {
                        let upsert = crate::metadata::to_upsert(&file, meta);
                        db.upsert_track(&upsert)?;
                    }
                    Err(e) => log::warn!("watcher: {e}"),
                }
            }
        } else if is_audio_file(path) {
            touched += 1;
            if path.exists() {
                match crate::metadata::read_track_meta(path, art_cache_dir) {
                    Ok(meta) => {
                        let upsert = crate::metadata::to_upsert(path, meta);
                        db.upsert_track(&upsert)?;
                    }
                    Err(e) => log::warn!("watcher: {e}"),
                }
            } else {
                let key = path.to_string_lossy().into_owned();
                db.delete_track_by_path(&key)?;
            }
        }
        on_progress(crate::models::ScanProgress {
            phase: "watch".into(),
            current: touched as u64,
            total: 0,
        });
    }
    Ok(touched)
}
