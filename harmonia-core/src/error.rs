use thiserror::Error;

/// Unified error type for the Harmonia core library.
///
/// Every fallible operation in the core returns [`CoreResult`], which keeps
/// error handling uniform across the database, metadata, scanning and
/// playlist layers. The Tauri application layer converts these into user
/// facing strings.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("metadata error: {0}")]
    Metadata(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("playlist error: {0}")]
    Playlist(String),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
