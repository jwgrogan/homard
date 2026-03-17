use std::sync::Arc;
use tokio::sync::Mutex;

use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId};

use crate::error::{ArcctlError, Result};

pub const TELEGRAM_MAX_CHARS: usize = 4000;

/// Splits text into chunks of at most `max_len` characters.
/// Tries to split on newlines first, then spaces, then hard-splits.
pub fn chunk_text(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while remaining.len() > max_len {
        let split_at = if let Some(pos) = remaining[..max_len].rfind('\n') {
            pos + 1
        } else if let Some(pos) = remaining[..max_len].rfind(' ') {
            pos + 1
        } else {
            max_len
        };
        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }
    if !remaining.is_empty() {
        chunks.push(remaining.to_string());
    }
    chunks
}

pub struct TelegramClient {
    bot: teloxide::Bot,
}

impl TelegramClient {
    pub fn new(token: impl Into<String>) -> Self {
        Self { bot: teloxide::Bot::new(token) }
    }

    pub async fn verify(&self) -> Result<String> {
        let me = self.bot.get_me().await
            .map_err(|e| ArcctlError::Telegram(e.to_string()))?;
        let username = me.user.username
            .as_deref()
            .unwrap_or("unknown_bot")
            .to_string();
        Ok(username)
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<i32> {
        let msg = self.bot
            .send_message(ChatId(chat_id), text)
            .await
            .map_err(|e| ArcctlError::Telegram(e.to_string()))?;
        Ok(msg.id.0)
    }

    pub async fn edit_message(&self, chat_id: i64, message_id: i32, text: &str) -> Result<()> {
        self.bot
            .edit_message_text(ChatId(chat_id), MessageId(message_id), text)
            .await
            .map_err(|e| ArcctlError::Telegram(e.to_string()))?;
        Ok(())
    }

    pub async fn chunk_and_send(&self, chat_id: i64, text: &str) -> Result<Vec<i32>> {
        let chunks = chunk_text(text, TELEGRAM_MAX_CHARS);
        let mut message_ids = Vec::new();
        for chunk in chunks {
            let id = self.send_message(chat_id, &chunk).await?;
            message_ids.push(id);
        }
        Ok(message_ids)
    }

    pub async fn send_with_retry(&self, chat_id: i64, text: &str, max_retries: u32) -> Result<Vec<i32>> {
        let mut last_err = ArcctlError::Telegram("no attempts".to_string());
        let backoff_secs = [1u64, 2, 4, 8, 16];
        for attempt in 0..=max_retries {
            match self.chunk_and_send(chat_id, text).await {
                Ok(ids) => return Ok(ids),
                Err(e) => {
                    last_err = e;
                    if attempt < max_retries {
                        let delay = backoff_secs.get(attempt as usize).copied().unwrap_or(16);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }
                }
            }
        }
        Err(last_err)
    }
}

/// Handles streaming preview: sends initial "running" message, then edits it in-place
/// as Claude generates output. Used by the job executor for live Telegram previews.
pub struct TelegramStreamReporter {
    client: Arc<TelegramClient>,
    chat_ids: Vec<i64>,
    /// (chat_id, message_id) for the active preview message in each chat
    active_messages: Mutex<Vec<(i64, i32)>>,
    accumulated: Mutex<String>,
    last_edit: Mutex<std::time::Instant>,
    edit_interval: std::time::Duration,
}

impl TelegramStreamReporter {
    pub fn new(client: Arc<TelegramClient>, chat_ids: Vec<i64>) -> Self {
        Self {
            client,
            chat_ids,
            active_messages: Mutex::new(Vec::new()),
            accumulated: Mutex::new(String::new()),
            last_edit: Mutex::new(std::time::Instant::now()),
            edit_interval: std::time::Duration::from_secs(2),
        }
    }

    /// Send initial "running" message to all chats. Call before spawning Claude.
    pub async fn send_start(&self, job_name: &str) {
        let text = format!("⏳ *{}* is running...", job_name);
        let mut active = self.active_messages.lock().await;
        for &chat_id in &self.chat_ids {
            if let Ok(msg_id) = self.client.send_message(chat_id, &text).await {
                active.push((chat_id, msg_id));
            }
        }
    }

    /// Feed a JSONL line from the Claude stream. Extracts text deltas and throttled-edits the message.
    pub async fn on_jsonl_line(&self, line: &str) {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else { return };
        if val.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
            if let Some(delta_text) = val.pointer("/delta/text").and_then(|t| t.as_str()) {
                let mut acc = self.accumulated.lock().await;
                acc.push_str(delta_text);
                drop(acc);
                self.maybe_edit().await;
            }
        }
    }

    async fn maybe_edit(&self) {
        let mut last = self.last_edit.lock().await;
        if last.elapsed() < self.edit_interval {
            return;
        }
        *last = std::time::Instant::now();
        drop(last);

        let text = {
            let acc = self.accumulated.lock().await;
            if acc.is_empty() { return; }
            let preview = if acc.len() > 3800 {
                format!("...{}", &acc[acc.len() - 3800..])
            } else {
                acc.clone()
            };
            format!("⏳ Live preview:\n\n{}", preview)
        };

        let active = self.active_messages.lock().await;
        for &(chat_id, msg_id) in active.iter() {
            let _ = self.client.edit_message(chat_id, msg_id, &text).await;
        }
    }

    /// Send final result. Edits the active "running" message with the completed output.
    pub async fn send_final(&self, job_name: &str, success: bool, error: Option<&str>, duration_ms: Option<i64>) {
        let status = if success { "✅" } else { "❌" };
        let duration_str = duration_ms
            .map(|ms| format!(" ({}s)", ms / 1000))
            .unwrap_or_default();

        let text = if let Some(err) = error {
            format!("{} *{}* failed{}\n\n`{}`", status, job_name, duration_str, err)
        } else {
            let acc = self.accumulated.lock().await;
            let summary = if acc.len() > 2000 {
                format!("...{}", &acc[acc.len() - 2000..])
            } else {
                acc.clone()
            };
            if summary.is_empty() {
                format!("{} *{}* completed{}", status, job_name, duration_str)
            } else {
                format!("{} *{}* completed{}\n\n{}", status, job_name, duration_str, summary)
            }
        };

        let active = self.active_messages.lock().await;
        for &(chat_id, msg_id) in active.iter() {
            let _ = self.client.edit_message(chat_id, msg_id, &text).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_short_message() {
        let chunks = chunk_text("Hello world", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello world");
    }

    #[test]
    fn test_chunk_text_exact_max() {
        let text = "a".repeat(4000);
        let chunks = chunk_text(&text, 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 4000);
    }

    #[test]
    fn test_chunk_text_splits_on_newline() {
        let line1 = "a".repeat(3000);
        let line2 = "b".repeat(2000);
        let text = format!("{}\n{}", line1, line2);
        let chunks = chunk_text(&text, 4000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], format!("{}\n", line1));
        assert_eq!(chunks[1], line2);
    }

    #[test]
    fn test_chunk_text_splits_on_space_when_no_newline() {
        let part1 = "a".repeat(3000);
        let part2 = "b".repeat(2000);
        let text = format!("{} {}", part1, part2);
        let chunks = chunk_text(&text, 4000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], format!("{} ", part1));
        assert_eq!(chunks[1], part2);
    }

    #[test]
    fn test_chunk_text_hard_split_when_no_whitespace() {
        let text = "a".repeat(8000);
        let chunks = chunk_text(&text, 4000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1].len(), 4000);
    }

    #[test]
    fn test_chunk_text_three_chunks() {
        let text = "x".repeat(9000);
        let chunks = chunk_text(&text, 4000);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1].len(), 4000);
        assert_eq!(chunks[2].len(), 1000);
    }

    #[test]
    fn test_chunk_text_empty_string() {
        let chunks = chunk_text("", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }
}
