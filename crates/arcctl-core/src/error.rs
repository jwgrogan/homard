use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArcctlError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ArcctlError>;
