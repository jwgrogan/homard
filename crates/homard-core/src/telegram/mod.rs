pub mod client;
pub mod poller;

pub use client::{chunk_text, TelegramClient, TelegramStreamReporter, TELEGRAM_MAX_CHARS};
