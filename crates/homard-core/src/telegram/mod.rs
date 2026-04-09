pub mod client;
pub mod poller;

pub use client::{TelegramClient, TelegramStreamReporter, chunk_text, TELEGRAM_MAX_CHARS};
