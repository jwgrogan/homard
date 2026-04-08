use thiserror::Error;

#[derive(Error, Debug)]
pub enum HomardError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Store error: {0}")]
    Store(#[from] rusqlite::Error),

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

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("OAuth error: {0}")]
    OAuth(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("HTTP error: {0}")]
    Http(String),
}

pub type Result<T> = std::result::Result<T, HomardError>;
