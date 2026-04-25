use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::agent::r#loop::AgentLoop;
use crate::config::HomardDirs;
use crate::schedule::list_schedules;
use crate::store::Store;
use crate::telegram::client::TelegramClient;
use crate::types::Trigger;

/// Run the cron check loop. Checks every 60 seconds for due schedules.
pub async fn run_scheduler(
    dirs: HomardDirs,
    agent: Arc<AgentLoop>,
    store: Arc<tokio::sync::Mutex<Store>>,
    telegram_client: Option<Arc<TelegramClient>>,
    cancel: CancellationToken,
) {
    info!("Cron scheduler started");

    // Track last run times per schedule
    let mut last_runs: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>> =
        std::collections::HashMap::new();

    loop {
        if cancel.is_cancelled() {
            info!("Cron scheduler: cancelled");
            break;
        }

        // Check schedules
        match list_schedules(&dirs) {
            Ok(schedules) => {
                let now = chrono::Utc::now();
                for schedule in schedules {
                    if !schedule.enabled {
                        continue;
                    }

                    // Check if this schedule is due
                    if let Some(last) = last_runs.get(&schedule.id) {
                        if !is_due(&schedule.schedule, last, &now) {
                            continue;
                        }
                    }
                    // First run: check if it should run now based on cron expression
                    else if !is_due_first_run(&schedule.schedule, &now) {
                        continue;
                    }

                    info!("Running scheduled job: {} ({})", schedule.name, schedule.id);
                    last_runs.insert(schedule.id.clone(), now);

                    // Track cron run
                    let run_id = {
                        let s = store.lock().await;
                        s.insert_cron_run(&schedule.id, &schedule.name).unwrap_or(0)
                    };

                    // Run through agent loop
                    let channel =
                        format!("cron_{}", schedule.name.to_lowercase().replace(' ', "_"));
                    match agent.run(&channel, &schedule.message, Trigger::Cron).await {
                        Ok((response, _run_id)) => {
                            {
                                let s = store.lock().await;
                                let _ = s.complete_cron_run(run_id, true, None);
                            }
                            // Deliver to configured channels
                            for delivery in &schedule.deliver_to {
                                if delivery == "telegram" {
                                    if let Some(ref tg) = telegram_client {
                                        // Send to all paired chats
                                        let config = crate::config::HomardConfig::load_or_default(
                                            &dirs.config_path(),
                                        );
                                        for chat_id_str in &config.telegram.paired_chat_ids {
                                            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                                let header = format!("[{}] ", schedule.name);
                                                let _ = tg
                                                    .chunk_and_send(
                                                        chat_id,
                                                        &format!("{}{}", header, response),
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                }
                                // "chat" delivery is automatic -- stored in conversation history
                            }
                        }
                        Err(e) => {
                            {
                                let s = store.lock().await;
                                let _ = s.complete_cron_run(run_id, false, Some(&e.to_string()));
                            }
                            error!("Cron job '{}' failed: {}", schedule.name, e);
                            // Notify via telegram on failure too
                            if let Some(ref tg) = telegram_client {
                                let config = crate::config::HomardConfig::load_or_default(
                                    &dirs.config_path(),
                                );
                                for chat_id_str in &config.telegram.paired_chat_ids {
                                    if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                        let _ = tg
                                            .send_message(
                                                chat_id,
                                                &format!(
                                                    "Cron job '{}' failed: {}",
                                                    schedule.name, e
                                                ),
                                            )
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to list schedules: {}", e);
            }
        }

        // Sleep 60 seconds (or until cancelled)
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            _ = cancel.cancelled() => { break; }
        }
    }

    info!("Cron scheduler stopped");
}

/// Simple cron check: parse "M H * * *" style expressions
/// Returns true if the schedule should run now (within the last 60s window)
fn is_due(
    cron_expr: &str,
    last_run: &chrono::DateTime<chrono::Utc>,
    now: &chrono::DateTime<chrono::Utc>,
) -> bool {
    // Must have been at least 55 seconds since last run (avoid double-fire)
    if (*now - *last_run).num_seconds() < 55 {
        return false;
    }
    matches_cron(cron_expr, now)
}

fn is_due_first_run(cron_expr: &str, now: &chrono::DateTime<chrono::Utc>) -> bool {
    matches_cron(cron_expr, now)
}

fn matches_cron(cron_expr: &str, now: &chrono::DateTime<chrono::Utc>) -> bool {
    let local = now.with_timezone(&chrono::Local);
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() < 5 {
        return false;
    }

    let minute = local.format("%M").to_string().parse::<u32>().unwrap_or(0);
    let hour = local.format("%H").to_string().parse::<u32>().unwrap_or(0);
    let day = local.format("%d").to_string().parse::<u32>().unwrap_or(0);
    let month = local.format("%m").to_string().parse::<u32>().unwrap_or(0);
    let dow = local.format("%u").to_string().parse::<u32>().unwrap_or(0); // 1=Mon, 7=Sun

    let min_match = parts[0] == "*" || parts[0].parse::<u32>().ok() == Some(minute);
    let hour_match = parts[1] == "*" || parts[1].parse::<u32>().ok() == Some(hour);
    let dom_match = parts[2] == "*" || parts[2].parse::<u32>().ok() == Some(day);
    let mon_match = parts[3] == "*" || parts[3].parse::<u32>().ok() == Some(month);
    // Convert cron dow (0=Sun, 1=Mon..6=Sat) to chrono (1=Mon..7=Sun)
    let dow_match = parts[4] == "*" || {
        let cron_dow = parts[4].parse::<u32>().unwrap_or(99);
        let chrono_dow = if cron_dow == 0 { 7 } else { cron_dow };
        chrono_dow == dow
    };

    min_match && hour_match && dom_match && mon_match && dow_match
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc, Local};

    #[test]
    fn test_matches_cron_day_and_month() {
        // Use local time to avoid timezone confusion in tests since matches_cron uses Local
        let now_local = Local.with_ymd_and_hms(2024, 6, 15, 9, 0, 0).unwrap();
        let now_utc = now_local.with_timezone(&Utc);

        // Matches day and month
        assert!(matches_cron("0 9 15 6 *", &now_utc), "Should match 15th of June at 9:00");

        // Fails day
        assert!(!matches_cron("0 9 16 6 *", &now_utc), "Should NOT match 16th of June");

        // Fails month
        assert!(!matches_cron("0 9 15 7 *", &now_utc), "Should NOT match 15th of July");

        // Matches *
        assert!(matches_cron("0 9 * * *", &now_utc), "Should match * * at 9:00");
    }
}
