use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use chrono::{DateTime, Utc};

use crate::types::{Run, RunStatus, Session, SessionStatus, Trigger};
use crate::error::Result;

pub struct Store {
    conn: Connection,
}

fn parse_trigger(s: &str) -> Trigger {
    match s {
        "cron" => Trigger::Cron,
        "telegram" => Trigger::Telegram,
        "email" => Trigger::Email,
        _ => Trigger::Manual,
    }
}

fn parse_status(s: &str) -> RunStatus {
    match s {
        "complete" => RunStatus::Complete,
        "error" => RunStatus::Error,
        "killed" => RunStatus::Killed,
        _ => RunStatus::Running,
    }
}

fn trigger_to_str(t: &Trigger) -> &'static str {
    let json = serde_json::to_string(t).unwrap_or_default();
    match json.trim_matches('"') {
        "cron" => "cron",
        "telegram" => "telegram",
        "email" => "email",
        _ => "manual",
    }
}

fn status_to_str(s: &RunStatus) -> &'static str {
    let json = serde_json::to_string(s).unwrap_or_default();
    match json.trim_matches('"') {
        "complete" => "complete",
        "error" => "error",
        "killed" => "killed",
        _ => "running",
    }
}

fn parse_session_status(s: &str) -> SessionStatus {
    match s {
        "stopped" => SessionStatus::Stopped,
        "error" => SessionStatus::Error,
        "killed" => SessionStatus::Killed,
        _ => SessionStatus::Running,
    }
}

fn session_status_to_str(s: &SessionStatus) -> &'static str {
    match s {
        SessionStatus::Running => "running",
        SessionStatus::Stopped => "stopped",
        SessionStatus::Error => "error",
        SessionStatus::Killed => "killed",
    }
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

    pub fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                schedule_id TEXT,
                agent TEXT,
                profile TEXT,
                directory TEXT,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER,
                error_message TEXT,
                delivery_status TEXT
            );

            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                draft_content TEXT NOT NULL,
                recipient TEXT,
                subject TEXT,
                metadata TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                channel TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snoozes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id TEXT NOT NULL,
                contact_name TEXT,
                reason TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                active INTEGER DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS chat_sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                profile TEXT,
                directory TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS email_threads (
                thread_id TEXT PRIMARY KEY,
                subject TEXT,
                participants TEXT,
                last_message_at TEXT,
                auto_reply INTEGER DEFAULT 0,
                metadata TEXT
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                cli_session_id TEXT,
                profile_name TEXT,
                provider TEXT NOT NULL DEFAULT 'claude',
                directory TEXT,
                terminal_pid INTEGER,
                trigger TEXT NOT NULL DEFAULT 'manual',
                status TEXT NOT NULL DEFAULT 'running',
                started_at TEXT NOT NULL,
                ended_at TEXT,
                duration_ms INTEGER,
                error_message TEXT,
                agent TEXT,
                parent_session_id TEXT,
                forked_from TEXT
            );
        ")?;

        // Backfill: copy runs into sessions if sessions is empty and runs has data
        let sessions_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions", [], |row| row.get(0)
        )?;
        let runs_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs", [], |row| row.get(0)
        )?;

        if sessions_count == 0 && runs_count > 0 {
            self.conn.execute_batch("
                INSERT INTO sessions (id, profile_name, provider, directory, trigger, status, started_at, ended_at, duration_ms, error_message, agent)
                SELECT id, profile, 'claude', directory, trigger,
                    CASE status
                        WHEN 'complete' THEN 'stopped'
                        WHEN 'running' THEN 'running'
                        WHEN 'error' THEN 'error'
                        WHEN 'killed' THEN 'killed'
                        ELSE 'stopped'
                    END,
                    started_at, finished_at, duration_ms, error_message, agent
                FROM runs;
            ")?;
        }

        Ok(())
    }

    pub fn insert_run(&self, run: &Run) -> Result<()> {
        let trigger_str = trigger_to_str(&run.trigger);
        let status_str = status_to_str(&run.status);
        let started_at = run.started_at.to_rfc3339();
        let finished_at = run.finished_at.as_ref().map(|dt| dt.to_rfc3339());
        let delivery_status = run.delivery_status.as_ref()
            .map(|v| v.to_string());

        self.conn.execute(
            "INSERT INTO runs (id, schedule_id, agent, profile, directory, trigger, status, started_at, finished_at, duration_ms, error_message, delivery_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                run.id,
                run.schedule_id,
                run.agent,
                run.profile,
                run.directory,
                trigger_str,
                status_str,
                started_at,
                finished_at,
                run.duration_ms,
                run.error_message,
                delivery_status,
            ],
        )?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> Result<Option<Run>> {
        let result = self.conn.query_row(
            "SELECT id, schedule_id, agent, profile, directory, trigger, status, started_at, finished_at, duration_ms, error_message, delivery_status FROM runs WHERE id = ?1",
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let schedule_id: Option<String> = row.get(1)?;
                let agent: Option<String> = row.get(2)?;
                let profile: Option<String> = row.get(3)?;
                let directory: Option<String> = row.get(4)?;
                let trigger_str: String = row.get(5)?;
                let status_str: String = row.get(6)?;
                let started_at_str: String = row.get(7)?;
                let finished_at_str: Option<String> = row.get(8)?;
                let duration_ms: Option<i64> = row.get(9)?;
                let error_message: Option<String> = row.get(10)?;
                let delivery_status_str: Option<String> = row.get(11)?;

                Ok((id, schedule_id, agent, profile, directory, trigger_str, status_str,
                    started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str))
            },
        ).optional()?;

        match result {
            None => Ok(None),
            Some((id, schedule_id, agent, profile, directory, trigger_str, status_str,
                  started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str)) => {
                let started_at = started_at_str.parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now());
                let finished_at = finished_at_str
                    .and_then(|s| s.parse::<DateTime<Utc>>().ok());
                let delivery_status = delivery_status_str
                    .and_then(|s| serde_json::from_str(&s).ok());

                Ok(Some(Run {
                    id,
                    schedule_id,
                    agent,
                    profile,
                    directory,
                    trigger: parse_trigger(&trigger_str),
                    status: parse_status(&status_str),
                    started_at,
                    finished_at,
                    duration_ms,
                    error_message,
                    delivery_status,
                }))
            }
        }
    }

    pub fn complete_run(&self, id: &str, status: RunStatus, error: Option<String>) -> Result<()> {
        let status_str = status_to_str(&status);
        let finished_at = Utc::now().to_rfc3339();

        // Get started_at to calculate duration
        let started_at_str: Option<String> = self.conn.query_row(
            "SELECT started_at FROM runs WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ).optional()?;

        let duration_ms: Option<i64> = started_at_str.and_then(|s| {
            s.parse::<DateTime<Utc>>().ok().map(|started| {
                let now = Utc::now();
                (now - started).num_milliseconds()
            })
        });

        self.conn.execute(
            "UPDATE runs SET status = ?1, finished_at = ?2, duration_ms = ?3, error_message = ?4 WHERE id = ?5",
            params![status_str, finished_at, duration_ms, error, id],
        )?;
        Ok(())
    }

    pub fn list_runs_by_schedule(&self, schedule_id: &str, limit: u32, offset: u32) -> Result<Vec<Run>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, schedule_id, agent, profile, directory, trigger, status, started_at, finished_at, duration_ms, error_message, delivery_status
             FROM runs
             WHERE schedule_id = ?1
             ORDER BY started_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let rows = stmt.query_map(params![schedule_id, limit, offset], |row| {
            let id: String = row.get(0)?;
            let schedule_id: Option<String> = row.get(1)?;
            let agent: Option<String> = row.get(2)?;
            let profile: Option<String> = row.get(3)?;
            let directory: Option<String> = row.get(4)?;
            let trigger_str: String = row.get(5)?;
            let status_str: String = row.get(6)?;
            let started_at_str: String = row.get(7)?;
            let finished_at_str: Option<String> = row.get(8)?;
            let duration_ms: Option<i64> = row.get(9)?;
            let error_message: Option<String> = row.get(10)?;
            let delivery_status_str: Option<String> = row.get(11)?;

            Ok((id, schedule_id, agent, profile, directory, trigger_str, status_str,
                started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str))
        })?;

        let mut runs = Vec::new();
        for row in rows {
            let (id, schedule_id, agent, profile, directory, trigger_str, status_str,
                 started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str) = row?;

            let started_at = started_at_str.parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            let finished_at = finished_at_str.and_then(|s| s.parse::<DateTime<Utc>>().ok());
            let delivery_status = delivery_status_str.and_then(|s| serde_json::from_str(&s).ok());

            runs.push(Run {
                id,
                schedule_id,
                agent,
                profile,
                directory,
                trigger: parse_trigger(&trigger_str),
                status: parse_status(&status_str),
                started_at,
                finished_at,
                duration_ms,
                error_message,
                delivery_status,
            });
        }
        Ok(runs)
    }

    pub fn list_runs(&self, limit: i64, offset: i64) -> Result<Vec<Run>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, schedule_id, agent, profile, directory, trigger, status, started_at, finished_at, duration_ms, error_message, delivery_status
             FROM runs
             ORDER BY started_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit, offset], |row| {
            let id: String = row.get(0)?;
            let schedule_id: Option<String> = row.get(1)?;
            let agent: Option<String> = row.get(2)?;
            let profile: Option<String> = row.get(3)?;
            let directory: Option<String> = row.get(4)?;
            let trigger_str: String = row.get(5)?;
            let status_str: String = row.get(6)?;
            let started_at_str: String = row.get(7)?;
            let finished_at_str: Option<String> = row.get(8)?;
            let duration_ms: Option<i64> = row.get(9)?;
            let error_message: Option<String> = row.get(10)?;
            let delivery_status_str: Option<String> = row.get(11)?;

            Ok((id, schedule_id, agent, profile, directory, trigger_str, status_str,
                started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str))
        })?;

        let mut runs = Vec::new();
        for row in rows {
            let (id, schedule_id, agent, profile, directory, trigger_str, status_str,
                 started_at_str, finished_at_str, duration_ms, error_message, delivery_status_str) = row?;

            let started_at = started_at_str.parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            let finished_at = finished_at_str.and_then(|s| s.parse::<DateTime<Utc>>().ok());
            let delivery_status = delivery_status_str.and_then(|s| serde_json::from_str(&s).ok());

            runs.push(Run {
                id,
                schedule_id,
                agent,
                profile,
                directory,
                trigger: parse_trigger(&trigger_str),
                status: parse_status(&status_str),
                started_at,
                finished_at,
                duration_ms,
                error_message,
                delivery_status,
            });
        }
        Ok(runs)
    }

    pub fn insert_session(&self, session: &Session) -> Result<()> {
        let trigger_str = trigger_to_str(&session.trigger);
        let status_str = session_status_to_str(&session.status);
        let started_at = session.started_at.to_rfc3339();
        let ended_at = session.ended_at.as_ref().map(|dt| dt.to_rfc3339());

        self.conn.execute(
            "INSERT INTO sessions (id, cli_session_id, profile_name, provider, directory, terminal_pid, trigger, status, started_at, ended_at, duration_ms, error_message, agent, parent_session_id, forked_from)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                session.id,
                session.cli_session_id,
                session.profile_name,
                session.provider,
                session.directory,
                session.terminal_pid,
                trigger_str,
                status_str,
                started_at,
                ended_at,
                session.duration_ms,
                session.error_message,
                session.agent,
                session.parent_session_id,
                session.forked_from,
            ],
        )?;
        Ok(())
    }

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let result = self.conn.query_row(
            "SELECT id, cli_session_id, profile_name, provider, directory, terminal_pid, trigger, status, started_at, ended_at, duration_ms, error_message, agent, parent_session_id, forked_from FROM sessions WHERE id = ?1",
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let cli_session_id: Option<String> = row.get(1)?;
                let profile_name: Option<String> = row.get(2)?;
                let provider: String = row.get(3)?;
                let directory: Option<String> = row.get(4)?;
                let terminal_pid: Option<u32> = row.get(5)?;
                let trigger_str: String = row.get(6)?;
                let status_str: String = row.get(7)?;
                let started_at_str: String = row.get(8)?;
                let ended_at_str: Option<String> = row.get(9)?;
                let duration_ms: Option<i64> = row.get(10)?;
                let error_message: Option<String> = row.get(11)?;
                let agent: Option<String> = row.get(12)?;
                let parent_session_id: Option<String> = row.get(13)?;
                let forked_from: Option<String> = row.get(14)?;

                Ok((id, cli_session_id, profile_name, provider, directory, terminal_pid,
                    trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                    error_message, agent, parent_session_id, forked_from))
            },
        ).optional()?;

        match result {
            None => Ok(None),
            Some((id, cli_session_id, profile_name, provider, directory, terminal_pid,
                  trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                  error_message, agent, parent_session_id, forked_from)) => {
                let started_at = started_at_str.parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now());
                let ended_at = ended_at_str.and_then(|s| s.parse::<DateTime<Utc>>().ok());

                Ok(Some(Session {
                    id,
                    cli_session_id,
                    profile_name,
                    provider,
                    directory,
                    terminal_pid,
                    trigger: parse_trigger(&trigger_str),
                    status: parse_session_status(&status_str),
                    started_at,
                    ended_at,
                    duration_ms,
                    error_message,
                    agent,
                    parent_session_id,
                    forked_from,
                }))
            }
        }
    }

    pub fn complete_session(&self, id: &str, status: SessionStatus, error: Option<String>) -> Result<()> {
        let status_str = session_status_to_str(&status);
        let ended_at = Utc::now().to_rfc3339();

        let started_at_str: Option<String> = self.conn.query_row(
            "SELECT started_at FROM sessions WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ).optional()?;

        let duration_ms: Option<i64> = started_at_str.and_then(|s| {
            s.parse::<DateTime<Utc>>().ok().map(|started| {
                let now = Utc::now();
                (now - started).num_milliseconds()
            })
        });

        self.conn.execute(
            "UPDATE sessions SET status = ?1, ended_at = ?2, duration_ms = ?3, error_message = ?4 WHERE id = ?5",
            params![status_str, ended_at, duration_ms, error, id],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: i64, offset: i64) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cli_session_id, profile_name, provider, directory, terminal_pid, trigger, status, started_at, ended_at, duration_ms, error_message, agent, parent_session_id, forked_from
             FROM sessions
             ORDER BY started_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit, offset], |row| {
            let id: String = row.get(0)?;
            let cli_session_id: Option<String> = row.get(1)?;
            let profile_name: Option<String> = row.get(2)?;
            let provider: String = row.get(3)?;
            let directory: Option<String> = row.get(4)?;
            let terminal_pid: Option<u32> = row.get(5)?;
            let trigger_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let started_at_str: String = row.get(8)?;
            let ended_at_str: Option<String> = row.get(9)?;
            let duration_ms: Option<i64> = row.get(10)?;
            let error_message: Option<String> = row.get(11)?;
            let agent: Option<String> = row.get(12)?;
            let parent_session_id: Option<String> = row.get(13)?;
            let forked_from: Option<String> = row.get(14)?;

            Ok((id, cli_session_id, profile_name, provider, directory, terminal_pid,
                trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                error_message, agent, parent_session_id, forked_from))
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            let (id, cli_session_id, profile_name, provider, directory, terminal_pid,
                 trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                 error_message, agent, parent_session_id, forked_from) = row?;

            let started_at = started_at_str.parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            let ended_at = ended_at_str.and_then(|s| s.parse::<DateTime<Utc>>().ok());

            sessions.push(Session {
                id,
                cli_session_id,
                profile_name,
                provider,
                directory,
                terminal_pid,
                trigger: parse_trigger(&trigger_str),
                status: parse_session_status(&status_str),
                started_at,
                ended_at,
                duration_ms,
                error_message,
                agent,
                parent_session_id,
                forked_from,
            });
        }
        Ok(sessions)
    }

    pub fn list_running_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cli_session_id, profile_name, provider, directory, terminal_pid, trigger, status, started_at, ended_at, duration_ms, error_message, agent, parent_session_id, forked_from
             FROM sessions
             WHERE status = 'running'
             ORDER BY started_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let cli_session_id: Option<String> = row.get(1)?;
            let profile_name: Option<String> = row.get(2)?;
            let provider: String = row.get(3)?;
            let directory: Option<String> = row.get(4)?;
            let terminal_pid: Option<u32> = row.get(5)?;
            let trigger_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let started_at_str: String = row.get(8)?;
            let ended_at_str: Option<String> = row.get(9)?;
            let duration_ms: Option<i64> = row.get(10)?;
            let error_message: Option<String> = row.get(11)?;
            let agent: Option<String> = row.get(12)?;
            let parent_session_id: Option<String> = row.get(13)?;
            let forked_from: Option<String> = row.get(14)?;

            Ok((id, cli_session_id, profile_name, provider, directory, terminal_pid,
                trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                error_message, agent, parent_session_id, forked_from))
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            let (id, cli_session_id, profile_name, provider, directory, terminal_pid,
                 trigger_str, status_str, started_at_str, ended_at_str, duration_ms,
                 error_message, agent, parent_session_id, forked_from) = row?;

            let started_at = started_at_str.parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            let ended_at = ended_at_str.and_then(|s| s.parse::<DateTime<Utc>>().ok());

            sessions.push(Session {
                id,
                cli_session_id,
                profile_name,
                provider,
                directory,
                terminal_pid,
                trigger: parse_trigger(&trigger_str),
                status: parse_session_status(&status_str),
                started_at,
                ended_at,
                duration_ms,
                error_message,
                agent,
                parent_session_id,
                forked_from,
            });
        }
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_run(id: &str) -> Run {
        Run {
            id: id.to_string(),
            schedule_id: None,
            agent: Some("test-agent".to_string()),
            profile: Some("default".to_string()),
            directory: Some("/tmp".to_string()),
            trigger: Trigger::Manual,
            status: RunStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            error_message: None,
            delivery_status: None,
        }
    }

    #[test]
    fn test_creates_tables() {
        let store = Store::open_in_memory().unwrap();
        // Query sqlite_master to verify runs table exists
        let count: i64 = store.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_insert_and_get_run() {
        let store = Store::open_in_memory().unwrap();
        let run = make_run("run-001");
        store.insert_run(&run).unwrap();

        let fetched = store.get_run("run-001").unwrap().expect("run should exist");
        assert_eq!(fetched.id, "run-001");
        assert_eq!(fetched.agent, Some("test-agent".to_string()));
        assert_eq!(fetched.profile, Some("default".to_string()));
        assert_eq!(fetched.trigger, Trigger::Manual);
        assert_eq!(fetched.status, RunStatus::Running);
        assert!(fetched.finished_at.is_none());
        assert!(fetched.duration_ms.is_none());
    }

    #[test]
    fn test_update_run_status() {
        let store = Store::open_in_memory().unwrap();
        let run = make_run("run-002");
        store.insert_run(&run).unwrap();

        // Verify initially running
        let before = store.get_run("run-002").unwrap().unwrap();
        assert_eq!(before.status, RunStatus::Running);
        assert!(before.finished_at.is_none());

        store.complete_run("run-002", RunStatus::Complete, None).unwrap();

        let after = store.get_run("run-002").unwrap().unwrap();
        assert_eq!(after.status, RunStatus::Complete);
        assert!(after.finished_at.is_some());
        assert!(after.duration_ms.is_some());
        assert!(after.duration_ms.unwrap() >= 0);
    }

    #[test]
    fn test_list_runs() {
        let store = Store::open_in_memory().unwrap();
        for i in 0..5 {
            let run = make_run(&format!("run-{:03}", i));
            store.insert_run(&run).unwrap();
        }

        let all = store.list_runs(10, 0).unwrap();
        assert_eq!(all.len(), 5);

        let limited = store.list_runs(3, 0).unwrap();
        assert_eq!(limited.len(), 3);

        let offset = store.list_runs(10, 3).unwrap();
        assert_eq!(offset.len(), 2);
    }

    fn make_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            cli_session_id: None,
            profile_name: Some("default".to_string()),
            provider: "claude".to_string(),
            directory: Some("/tmp".to_string()),
            terminal_pid: None,
            trigger: Trigger::Manual,
            status: SessionStatus::Running,
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            error_message: None,
            agent: Some("test-agent".to_string()),
            parent_session_id: None,
            forked_from: None,
        }
    }

    #[test]
    fn test_sessions_table_exists() {
        let store = Store::open_in_memory().unwrap();
        let count: i64 = store.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_insert_and_get_session() {
        let store = Store::open_in_memory().unwrap();
        let session = make_session("sess-001");
        store.insert_session(&session).unwrap();

        let fetched = store.get_session("sess-001").unwrap().expect("session should exist");
        assert_eq!(fetched.id, "sess-001");
        assert_eq!(fetched.provider, "claude");
        assert_eq!(fetched.profile_name, Some("default".to_string()));
        assert_eq!(fetched.agent, Some("test-agent".to_string()));
        assert_eq!(fetched.trigger, Trigger::Manual);
        assert_eq!(fetched.status, SessionStatus::Running);
        assert!(fetched.ended_at.is_none());
        assert!(fetched.duration_ms.is_none());
    }

    #[test]
    fn test_get_session_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_session("does-not-exist").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_complete_session() {
        let store = Store::open_in_memory().unwrap();
        let session = make_session("sess-002");
        store.insert_session(&session).unwrap();

        let before = store.get_session("sess-002").unwrap().unwrap();
        assert_eq!(before.status, SessionStatus::Running);
        assert!(before.ended_at.is_none());

        store.complete_session("sess-002", SessionStatus::Stopped, None).unwrap();

        let after = store.get_session("sess-002").unwrap().unwrap();
        assert_eq!(after.status, SessionStatus::Stopped);
        assert!(after.ended_at.is_some());
        assert!(after.duration_ms.is_some());
        assert!(after.duration_ms.unwrap() >= 0);
    }

    #[test]
    fn test_complete_session_with_error() {
        let store = Store::open_in_memory().unwrap();
        let session = make_session("sess-003");
        store.insert_session(&session).unwrap();

        store.complete_session("sess-003", SessionStatus::Error, Some("something failed".to_string())).unwrap();

        let after = store.get_session("sess-003").unwrap().unwrap();
        assert_eq!(after.status, SessionStatus::Error);
        assert_eq!(after.error_message, Some("something failed".to_string()));
    }

    #[test]
    fn test_list_sessions() {
        let store = Store::open_in_memory().unwrap();
        for i in 0..5 {
            let session = make_session(&format!("sess-{:03}", i));
            store.insert_session(&session).unwrap();
        }

        let all = store.list_sessions(10, 0).unwrap();
        assert_eq!(all.len(), 5);

        let limited = store.list_sessions(3, 0).unwrap();
        assert_eq!(limited.len(), 3);

        let offset = store.list_sessions(10, 3).unwrap();
        assert_eq!(offset.len(), 2);
    }

    #[test]
    fn test_list_running_sessions() {
        let store = Store::open_in_memory().unwrap();
        for i in 0..3 {
            let session = make_session(&format!("running-{:03}", i));
            store.insert_session(&session).unwrap();
        }
        // Insert one that gets completed
        let s = make_session("stopped-001");
        store.insert_session(&s).unwrap();
        store.complete_session("stopped-001", SessionStatus::Stopped, None).unwrap();

        let running = store.list_running_sessions().unwrap();
        assert_eq!(running.len(), 3);
        for s in &running {
            assert_eq!(s.status, SessionStatus::Running);
        }
    }

    #[test]
    fn test_backfill_runs_into_sessions_on_migration() {
        // Simulate an existing DB: open, insert runs (but no sessions), then close and reopen
        // to trigger the backfill migration.
        use tempfile::NamedTempFile;

        let tmp = NamedTempFile::new().unwrap();
        let db_path = tmp.path().to_path_buf();

        // First open: create schema and insert runs (sessions table will be empty)
        {
            let store = Store::open(&db_path).unwrap();

            // Insert a 'complete' run — should backfill as 'stopped'
            let mut run_complete = make_run("run-backfill-001");
            run_complete.profile = Some("my-profile".to_string());
            run_complete.agent = Some("agent-v1".to_string());
            store.insert_run(&run_complete).unwrap();
            store.complete_run("run-backfill-001", RunStatus::Complete, None).unwrap();

            // Insert an 'error' run
            let mut run_error = make_run("run-backfill-002");
            run_error.profile = Some("my-profile".to_string());
            store.insert_run(&run_error).unwrap();
            store.complete_run("run-backfill-002", RunStatus::Error, Some("oops".to_string())).unwrap();
        }
        // tmp keeps the file alive; the Store is dropped (connection closed) here.

        // Second open: migrate() should detect runs_count > 0 && sessions_count == 0
        // and backfill.
        {
            let store = Store::open(&db_path).unwrap();

            let sessions = store.list_sessions(100, 0).unwrap();
            assert_eq!(sessions.len(), 2, "sessions should have been backfilled from runs");

            // Find the backfilled sessions by id
            let s1 = sessions.iter().find(|s| s.id == "run-backfill-001")
                .expect("backfilled session for run-backfill-001 should exist");
            assert_eq!(s1.status, SessionStatus::Stopped, "complete run should map to stopped");
            assert_eq!(s1.provider, "claude");
            assert_eq!(s1.profile_name, Some("my-profile".to_string()));
            assert_eq!(s1.agent, Some("agent-v1".to_string()));

            let s2 = sessions.iter().find(|s| s.id == "run-backfill-002")
                .expect("backfilled session for run-backfill-002 should exist");
            assert_eq!(s2.status, SessionStatus::Error, "error run should map to error");
            assert_eq!(s2.provider, "claude");
        }
    }

    #[test]
    fn test_backfill_does_not_run_when_sessions_already_exist() {
        use tempfile::NamedTempFile;

        let tmp = NamedTempFile::new().unwrap();
        let db_path = tmp.path().to_path_buf();

        // First open: insert both a run and a session
        {
            let store = Store::open(&db_path).unwrap();

            let run = make_run("run-no-backfill-001");
            store.insert_run(&run).unwrap();

            let session = make_session("sess-existing-001");
            store.insert_session(&session).unwrap();
        }

        // Second open: sessions is not empty, so backfill should NOT add runs
        {
            let store = Store::open(&db_path).unwrap();
            let sessions = store.list_sessions(100, 0).unwrap();
            assert_eq!(sessions.len(), 1, "backfill should not run when sessions already exist");
            assert_eq!(sessions[0].id, "sess-existing-001");
        }
    }
}
