use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum DenoteError {
    #[error("Bear database not found at {0}")]
    DbNotFound(PathBuf),

    #[error("Failed to read Bear database: {0}")]
    DbRead(#[from] rusqlite::Error),

    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Export failed for note {id}: {reason}")]
    Export { id: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File watcher error: {0}")]
    Watch(#[from] notify::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DenoteError>;
