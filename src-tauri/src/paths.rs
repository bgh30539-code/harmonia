//! XDG-compliant application directories.
//!
//! - Database lives under `~/.local/share/dev.harmonia.player`
//! - Artwork cache lives under `~/.cache/dev.harmonia.player/art`

use std::path::PathBuf;

use tauri::Manager;

pub struct AppPaths {
    pub art_dir: PathBuf,
    pub db_path: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &tauri::App) -> Result<Self, String> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("cannot resolve app data dir: {e}"))?;
        let cache_dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("cannot resolve app cache dir: {e}"))?;
        let art_dir = cache_dir.join("art");
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("cannot create data dir: {e}"))?;
        std::fs::create_dir_all(&art_dir)
            .map_err(|e| format!("cannot create art cache dir: {e}"))?;
        Ok(Self {
            db_path: data_dir.join("harmonia.db"),
            art_dir,
        })
    }
}
