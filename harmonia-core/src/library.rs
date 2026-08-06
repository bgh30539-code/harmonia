//! Library scanning.
//!
//! The scanner walks every configured folder recursively, reuses metadata for
//! files whose (mtime, size) are unchanged since the last scan, and extracts
//! metadata in parallel with Rayon. Database writes are batched into
//! transactions so a scan of a 100k-file library stays fast and never blocks
//! the UI thread (the scanner runs on a background thread from the app layer).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::db::{Database, TrackUpsert};
use crate::error::CoreResult;
use crate::metadata::{read_track_meta, TrackMeta};
use crate::models::ScanProgress;
use crate::util::is_audio_file;

/// Summary of a completed scan, surfaced to the UI as a toast.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ScanStats {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub failed: usize,
    pub elapsed_ms: u128,
}

struct Job {
    path: PathBuf,
    key: String,
    mtime: i64,
    size: i64,
    needs_parse: bool,
    exists_in_db: bool,
}

fn mtime_secs(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Scans `folders` into `db`, caching artwork under `art_cache_dir`.
///
/// `on_progress` is invoked periodically with [`ScanProgress`]; the app layer
/// forwards these events to the UI. When `force` is false only files whose
/// mtime/size changed are re-parsed.
pub fn scan_library(
    db: &Database,
    folders: &[String],
    art_cache_dir: &Path,
    force: bool,
    on_progress: &(dyn Fn(ScanProgress) + Sync),
) -> CoreResult<ScanStats> {
    let started = Instant::now();
    let mut stats = ScanStats::default();

    // Phase 1: walk the tree collecting audio files.
    let mut jobs: Vec<Job> = Vec::new();
    let mut walked_roots: Vec<String> = Vec::new();
    for root in folders {
        let root_path = Path::new(root);
        if !root_path.is_dir() {
            log::warn!("library root does not exist, skipping: {root}");
            continue;
        }
        walked_roots.push(root.clone());
        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() || !is_audio_file(entry.path()) {
                continue;
            }
            let path = entry.into_path();
            let meta = std::fs::metadata(&path).ok();
            let (mtime, size) = meta
                .map(|m| (mtime_secs(&m), m.len() as i64))
                .unwrap_or((0, 0));
            let key = path.to_string_lossy().into_owned();
            stats.found += 1;
            jobs.push(Job {
                path,
                key,
                mtime,
                size,
                needs_parse: true,
                exists_in_db: false,
            });
            if stats.found % 500 == 0 {
                on_progress(ScanProgress {
                    phase: "scan".into(),
                    current: stats.found as u64,
                    total: 0,
                });
            }
        }
    }

    if jobs.is_empty() {
        // Nothing to parse, but stale rows under the walked roots must still
        // be purged.
        let keep = HashSet::new();
        stats.removed = db.delete_tracks_not_in(&keep, &walked_roots)?;
        return Ok(stats);
    }

    // Phase 2: compare against the database for incremental rescanning.
    let existing = db.all_paths_with_mtime()?;
    for job in &mut jobs {
        if let Some(&(em, es)) = existing.get(&job.key) {
            job.exists_in_db = true;
            if !force && em == job.mtime && es == job.size {
                job.needs_parse = false;
            }
        }
    }

    // Phase 3: parse metadata in parallel.
    let parse_jobs: Vec<&Job> = jobs.iter().filter(|j| j.needs_parse).collect();
    let total = parse_jobs.len();
    on_progress(ScanProgress {
        phase: "parse".into(),
        current: 0,
        total: total as u64,
    });

    let parsed: Vec<(String, CoreResult<TrackMeta>)> = parse_jobs
        .par_iter()
        .enumerate()
        .map(|(i, job)| {
            let result = read_track_meta(&job.path, art_cache_dir);
            if i % 250 == 0 {
                on_progress(ScanProgress {
                    phase: "parse".into(),
                    current: i as u64,
                    total: total as u64,
                });
            }
            (job.key.clone(), result)
        })
        .collect();

    // Phase 4: batch-write to the database.
    let mut upserts: Vec<TrackUpsert> = Vec::new();
    for (key, result) in parsed {
        match result {
            Ok(meta) => {
                let job = jobs.iter().find(|j| j.key == key).unwrap();
                if job.exists_in_db {
                    stats.updated += 1;
                } else {
                    stats.added += 1;
                }
                upserts.push(crate::metadata::to_upsert(Path::new(&key), meta));
                if upserts.len() >= 500 {
                    db.upsert_tracks(&upserts)?;
                    upserts.clear();
                }
            }
            Err(e) => {
                stats.failed += 1;
                log::warn!("skipping unreadable file: {e}");
            }
        }
    }
    if !upserts.is_empty() {
        db.upsert_tracks(&upserts)?;
    }

    // Phase 5: purge rows for files that disappeared.
    let keep: HashSet<String> = jobs.iter().map(|j| j.key.clone()).collect();
    stats.removed = db.delete_tracks_not_in(&keep, &walked_roots)?;

    // Phase 6: record folder mtimes for fast future rescans.
    for root in &walked_roots {
        if let Ok(m) = std::fs::metadata(root) {
            db.update_folder_mtime(root, mtime_secs(&m))?;
        }
    }

    on_progress(ScanProgress {
        phase: "done".into(),
        current: total as u64,
        total: total as u64,
    });
    stats.elapsed_ms = started.elapsed().as_millis();
    log::info!("scan finished: {stats:?}");
    Ok(stats)
}
