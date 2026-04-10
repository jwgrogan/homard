use rusqlite::{Connection, params};
use std::path::Path;
use crate::types::*;
use crate::error::Result;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_call_id TEXT,
                tool_calls TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_conv_channel ON conversations(channel);

            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fact TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'general',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                fact, category, content=memories, content_rowid=id
            );

            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, fact, category) VALUES (new.id, new.fact, new.category);
            END;
            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, fact, category) VALUES('delete', old.id, old.fact, old.category);
            END;

            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                channel TEXT NOT NULL,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER,
                error_message TEXT,
                iterations INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                arguments TEXT,
                result TEXT,
                approved INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS cron_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schedule_id TEXT NOT NULL,
                schedule_name TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_cron_runs_schedule ON cron_runs(schedule_id);

            CREATE TABLE IF NOT EXISTS cli_sessions (
                id TEXT PRIMARY KEY,
                cli TEXT NOT NULL,
                prompt TEXT NOT NULL,
                directory TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                output TEXT,
                error TEXT,
                pid INTEGER,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER
            );
        ")?;
        Ok(())
    }

    // Conversation methods
    pub fn list_channels(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT channel FROM conversations GROUP BY channel ORDER BY MAX(rowid) DESC"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut channels: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        // Always include "chat" even if empty
        if !channels.contains(&"chat".to_string()) {
            channels.insert(0, "chat".to_string());
        }
        Ok(channels)
    }

    pub fn save_message(&self, channel: &str, msg: &ChatMessage) -> Result<()> {
        let tool_calls_json = msg.tool_calls.as_ref().map(|tc| serde_json::to_string(tc).unwrap_or_default());
        self.conn.execute(
            "INSERT INTO conversations (channel, role, content, tool_call_id, tool_calls) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![channel, msg.role, msg.content, msg.tool_call_id, tool_calls_json],
        )?;
        Ok(())
    }

    pub fn get_history(&self, channel: &str, limit: usize) -> Result<Vec<ChatMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content, tool_call_id, tool_calls, created_at FROM conversations WHERE channel = ?1 ORDER BY id DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![channel, limit as i64], |row| {
            let tool_calls_str: Option<String> = row.get(3)?;
            let tool_calls = tool_calls_str.and_then(|s| serde_json::from_str(&s).ok());
            let ts: Option<String> = row.get(4)?;
            let timestamp = ts.and_then(|s| {
                chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()
                    .map(|naive| naive.and_utc())
                    .or_else(|| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc)))
            });
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
                tool_call_id: row.get(2)?,
                tool_calls,
                timestamp,
            })
        })?;
        let mut messages: Vec<ChatMessage> = rows.filter_map(|r| r.ok()).collect();
        messages.reverse(); // oldest first
        Ok(messages)
    }

    // Memory methods
    pub fn save_memory(&self, fact: &str, category: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO memories (fact, category) VALUES (?1, ?2)",
            params![fact, category],
        )?;
        Ok(())
    }

    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT fact, category FROM memories_fts WHERE memories_fts MATCH ?1 ORDER BY rank LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![query, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // Run tracking
    pub fn insert_run(&self, run: &AgentRun) -> Result<()> {
        self.conn.execute(
            "INSERT INTO runs (id, channel, trigger, status, started_at, iterations) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run.id,
                run.channel,
                serde_json::to_string(&run.trigger).unwrap_or_default().trim_matches('"'),
                serde_json::to_string(&run.status).unwrap_or_default().trim_matches('"'),
                run.started_at.to_rfc3339(),
                run.iterations,
            ],
        )?;
        Ok(())
    }

    pub fn complete_run(&self, id: &str, status: RunStatus, error: Option<&str>, iterations: u32) -> Result<()> {
        let now = chrono::Utc::now();
        self.conn.execute(
            "UPDATE runs SET status = ?1, finished_at = ?2, error_message = ?3, iterations = ?4, duration_ms = (strftime('%s', ?2) - strftime('%s', started_at)) * 1000 WHERE id = ?5",
            params![
                serde_json::to_string(&status).unwrap_or_default().trim_matches('"'),
                now.to_rfc3339(),
                error,
                iterations,
                id,
            ],
        )?;
        Ok(())
    }

    pub fn list_runs(&self, limit: usize) -> Result<Vec<AgentRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, channel, trigger, status, started_at, finished_at, duration_ms, error_message, iterations FROM runs ORDER BY started_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let trigger_str: String = row.get(2)?;
            let status_str: String = row.get(3)?;
            let started_str: String = row.get(4)?;
            let finished_str: Option<String> = row.get(5)?;
            Ok(AgentRun {
                id: row.get(0)?,
                channel: row.get(1)?,
                trigger: serde_json::from_str(&format!("\"{}\"", trigger_str)).unwrap_or(Trigger::Chat),
                status: serde_json::from_str(&format!("\"{}\"", status_str)).unwrap_or(RunStatus::Running),
                started_at: chrono::DateTime::parse_from_rfc3339(&started_str).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now()),
                finished_at: finished_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
                duration_ms: row.get(6)?,
                error_message: row.get(7)?,
                iterations: row.get::<_, Option<u32>>(8)?.unwrap_or(0),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // CLI session tracking
    pub fn insert_session(&self, session: &CliSession) -> Result<()> {
        self.conn.execute(
            "INSERT INTO cli_sessions (id, cli, prompt, directory, status, pid, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id,
                serde_json::to_string(&session.cli).unwrap_or_default().trim_matches('"'),
                session.prompt,
                session.directory,
                serde_json::to_string(&session.status).unwrap_or_default().trim_matches('"'),
                session.pid,
                session.started_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn complete_session(&self, id: &str, status: SessionStatus, output: Option<&str>, error: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now();
        self.conn.execute(
            "UPDATE cli_sessions SET status = ?1, output = ?2, error = ?3, finished_at = ?4, duration_ms = (strftime('%s', ?4) - strftime('%s', started_at)) * 1000 WHERE id = ?5",
            params![
                serde_json::to_string(&status).unwrap_or_default().trim_matches('"'),
                output,
                error,
                now.to_rfc3339(),
                id,
            ],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<CliSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cli, prompt, directory, status, output, error, pid, started_at, finished_at, duration_ms FROM cli_sessions ORDER BY started_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let cli_str: String = row.get(1)?;
            let status_str: String = row.get(4)?;
            let started_str: String = row.get(8)?;
            let finished_str: Option<String> = row.get(9)?;
            Ok(CliSession {
                id: row.get(0)?,
                cli: serde_json::from_str(&format!("\"{}\"", cli_str)).unwrap_or(CliType::Claude),
                prompt: row.get(2)?,
                directory: row.get(3)?,
                status: serde_json::from_str(&format!("\"{}\"", status_str)).unwrap_or(SessionStatus::Running),
                output: row.get(5)?,
                error: row.get(6)?,
                pid: row.get(7)?,
                started_at: chrono::DateTime::parse_from_rfc3339(&started_str).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now()),
                finished_at: finished_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
                duration_ms: row.get(10)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_running_sessions(&self) -> Result<Vec<CliSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cli, prompt, directory, status, output, error, pid, started_at, finished_at, duration_ms FROM cli_sessions WHERE status = 'running'"
        )?;
        let rows = stmt.query_map([], |row| {
            let cli_str: String = row.get(1)?;
            let started_str: String = row.get(8)?;
            Ok(CliSession {
                id: row.get(0)?,
                cli: serde_json::from_str(&format!("\"{}\"", cli_str)).unwrap_or(CliType::Claude),
                prompt: row.get(2)?,
                directory: row.get(3)?,
                status: SessionStatus::Running,
                output: row.get(5)?,
                error: row.get(6)?,
                pid: row.get(7)?,
                started_at: chrono::DateTime::parse_from_rfc3339(&started_str).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now()),
                finished_at: None,
                duration_ms: None,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // Cron run tracking
    pub fn insert_cron_run(&self, schedule_id: &str, schedule_name: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO cron_runs (schedule_id, schedule_name, status, started_at) VALUES (?1, ?2, 'running', datetime('now'))",
            params![schedule_id, schedule_name],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn complete_cron_run(&self, id: i64, success: bool, error: Option<&str>) -> Result<()> {
        let status = if success { "complete" } else { "error" };
        self.conn.execute(
            "UPDATE cron_runs SET status = ?1, error = ?2, finished_at = datetime('now'), duration_ms = (strftime('%s', datetime('now')) - strftime('%s', started_at)) * 1000 WHERE id = ?3",
            params![status, error, id],
        )?;
        Ok(())
    }

    pub fn get_cron_health(&self) -> Result<Vec<CronHealth>> {
        let mut stmt = self.conn.prepare(
            "SELECT schedule_name,
                    COUNT(*) as total_runs,
                    SUM(CASE WHEN status = 'complete' THEN 1 ELSE 0 END) as successes,
                    SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END) as failures,
                    MAX(started_at) as last_run,
                    AVG(duration_ms) as avg_duration_ms
             FROM cron_runs
             GROUP BY schedule_name
             ORDER BY last_run DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CronHealth {
                name: row.get(0)?,
                total_runs: row.get(1)?,
                successes: row.get(2)?,
                failures: row.get(3)?,
                last_run: row.get(4)?,
                avg_duration_ms: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Clean up runs and sessions left in 'running' state from a previous daemon crash
    pub fn cleanup_stale_runs(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET status = 'error', error_message = 'Daemon restarted' WHERE status = 'running'",
            [],
        )?;
        self.conn.execute(
            "UPDATE cli_sessions SET status = 'error', error = 'Daemon restarted' WHERE status = 'running'",
            [],
        )?;
        Ok(())
    }

    // Audit log
    pub fn log_audit(&self, tool_name: &str, arguments: Option<&str>, result: Option<&str>, approved: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit_log (tool_name, arguments, result, approved) VALUES (?1, ?2, ?3, ?4)",
            params![tool_name, arguments, result, approved as i32],
        )?;
        Ok(())
    }
}
