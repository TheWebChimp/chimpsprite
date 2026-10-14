#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("not an Aseprite file (bad magic number)")]
    BadMagic,
    #[error("the file is truncated or corrupt: {0}")]
    Corrupt(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("zip: {0}")]
    Zip(String),
    #[error("png: {0}")]
    Png(String),
    #[error("gif: {0}")]
    Gif(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, IoError>;
