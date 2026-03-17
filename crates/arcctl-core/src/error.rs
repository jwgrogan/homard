use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArcctlError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Store error: {0}")]
    Store(#[from] rusqlite::Error),

    #[error("Profile error: {0}")]
    Profile(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Plist error: {0}")]
    Plist(#[from] plist::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("Telegram error: {0}")]
    Telegram(String),
}

pub type Result<T> = std::result::Result<T, ArcctlError>;
