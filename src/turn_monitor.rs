//! Poll Codex app-server metadata for turns whose final status is failed.

use crate::codex_login::{CodexAppServerClient, CodexAppServerSession};
use crate::logging_utils::ProcessSafeLogger;
use crate::security::redact_secrets;
use crate::store::{Store, TurnFailureNotification};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

pub const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);
const SCAN_OVERLAP_SECONDS: f64 = 60.0;
const PAGE_SIZE: u32 = 100;
const BASELINE_SETTING: &str = "turn_failure_monitor_baseline";
const CURSOR_SETTING: &str = "turn_failure_monitor_cursor";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page<T> {
    data: Vec<T>,
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadSummary {
    id: String,
    cwd: String,
    updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnSummary {
    id: String,
    status: String,
    error: Option<Value>,
    completed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TurnFailure {
    thread_id: String,
    turn_id: String,
    cwd: String,
    error_message: String,
    error_type: Option<String>,
    http_status: Option<u16>,
    occurred_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScanWindow {
    since: f64,
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn safe_detail(value: impl std::fmt::Display, limit: usize) -> String {
    redact_secrets(&value.to_string())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

fn setting_number(store: &Store, key: &str) -> Result<Option<f64>> {
    Ok(store
        .get_setting(key)?
        .and_then(|value| value.parse::<f64>().ok()))
}

fn prepare_scan(store: &Store, now: f64) -> Result<Option<ScanWindow>> {
    let baseline = setting_number(store, BASELINE_SETTING)?;
    let cursor = setting_number(store, CURSOR_SETTING)?;
    match (baseline, cursor) {
        (None, None) => {
            store.set_setting(BASELINE_SETTING, &now.to_string())?;
            store.set_setting(CURSOR_SETTING, &now.to_string())?;
            Ok(None)
        }
        (Some(baseline), Some(cursor)) => Ok(Some(ScanWindow {
            since: baseline.max(cursor - SCAN_OVERLAP_SECONDS),
        })),
        (Some(baseline), None) => {
            store.set_setting(CURSOR_SETTING, &baseline.to_string())?;
            Ok(Some(ScanWindow { since: baseline }))
        }
        (None, Some(cursor)) => {
            store.set_setting(BASELINE_SETTING, &cursor.to_string())?;
            Ok(Some(ScanWindow { since: cursor }))
        }
    }
}

fn error_type(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(safe_detail(value, 120)),
        Value::Object(value) => value.keys().next().map(|value| safe_detail(value, 120)),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn http_status(value: &Value) -> Option<u16> {
    match value {
        Value::Object(object) => {
            if let Some(status) = object.get("httpStatusCode").and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str()?.parse::<u64>().ok())
            }) {
                return u16::try_from(status).ok();
            }
            object.values().find_map(http_status)
        }
        Value::Array(values) => values.iter().find_map(http_status),
        _ => None,
    }
}

fn turn_failure(thread: &ThreadSummary, turn: TurnSummary, since: f64) -> Option<TurnFailure> {
    let occurred_at = turn.completed_at.unwrap_or(thread.updated_at);
    if (occurred_at as f64) < since || turn.status != "failed" {
        return None;
    }
    let error = turn.error.as_ref().and_then(Value::as_object);
    let message = error
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Codex 请求失败");
    let codex_error_info = error.and_then(|value| value.get("codexErrorInfo"));
    Some(TurnFailure {
        thread_id: thread.id.clone(),
        turn_id: turn.id,
        cwd: thread.cwd.clone(),
        error_message: safe_detail(message, 500),
        error_type: error_type(codex_error_info),
        http_status: codex_error_info.and_then(http_status),
        occurred_at,
    })
}

fn parse_thread_page(payload: Value) -> Result<Page<ThreadSummary>> {
    serde_json::from_value(payload).context("thread/list returned an invalid result")
}

fn parse_turn_page(
    thread: &ThreadSummary,
    payload: Value,
    since: f64,
) -> Result<(Vec<TurnFailure>, Option<String>, bool)> {
    let page: Page<TurnSummary> =
        serde_json::from_value(payload).context("thread/turns/list returned an invalid result")?;
    let reached_cutoff = page
        .data
        .iter()
        .any(|turn| (turn.completed_at.unwrap_or(thread.updated_at) as f64) < since);
    let failures = page
        .data
        .into_iter()
        .filter_map(|turn| turn_failure(thread, turn, since))
        .collect();
    Ok((failures, page.next_cursor, reached_cutoff))
}

async fn collect_thread_failures(
    session: &mut CodexAppServerSession,
    thread: &ThreadSummary,
    since: f64,
) -> Result<Vec<TurnFailure>> {
    let mut cursor: Option<String> = None;
    let mut failures = Vec::new();
    loop {
        let payload = session
            .request(
                "thread/turns/list",
                json!({
                    "threadId": thread.id,
                    "cursor": cursor,
                    "limit": PAGE_SIZE,
                    "sortDirection": "desc",
                    "itemsView": "notLoaded"
                }),
                None,
            )
            .await?;
        session.drain_notifications();
        let (page_failures, next_cursor, reached_cutoff) = parse_turn_page(thread, payload, since)?;
        failures.extend(page_failures);
        if reached_cutoff || next_cursor.is_none() {
            break;
        }
        cursor = next_cursor;
    }
    Ok(failures)
}

async fn collect_failures(
    session: &mut CodexAppServerSession,
    since: f64,
) -> Result<Vec<TurnFailure>> {
    let mut cursor: Option<String> = None;
    let mut failures = Vec::new();
    loop {
        let payload = session
            .request(
                "thread/list",
                json!({
                    "cursor": cursor,
                    "limit": PAGE_SIZE,
                    "sortKey": "updated_at",
                    "sortDirection": "desc",
                    "useStateDbOnly": true
                }),
                None,
            )
            .await?;
        session.drain_notifications();
        let page = parse_thread_page(payload)?;
        let reached_cutoff = page
            .data
            .iter()
            .any(|thread| (thread.updated_at as f64) < since);
        for thread in page
            .data
            .iter()
            .filter(|thread| thread.updated_at as f64 >= since)
        {
            failures.extend(collect_thread_failures(session, thread, since).await?);
        }
        if reached_cutoff || page.next_cursor.is_none() {
            break;
        }
        cursor = page.next_cursor;
    }
    Ok(failures)
}

fn finish_scan(store: &Store, cursor: f64, failures: Result<Vec<TurnFailure>>) -> Result<usize> {
    let failures = failures?;
    let mut inserted = 0;
    for failure in failures {
        if store.enqueue_turn_failure(TurnFailureNotification {
            thread_id: &failure.thread_id,
            cwd: &failure.cwd,
            turn_id: &failure.turn_id,
            error_message: &failure.error_message,
            error_type: failure.error_type.as_deref(),
            http_status: failure.http_status,
            occurred_at: failure.occurred_at as f64,
        })? {
            inserted += 1;
        }
    }
    store.set_setting(CURSOR_SETTING, &cursor.to_string())?;
    Ok(inserted)
}

async fn scan_once(
    store: &Store,
    session: &mut CodexAppServerSession,
    scan_started_at: f64,
) -> Result<usize> {
    let Some(window) = prepare_scan(store, scan_started_at)? else {
        return Ok(0);
    };
    let failures = collect_failures(session, window.since).await;
    finish_scan(store, scan_started_at, failures)
}

async fn wait_or_stop(stop: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    tokio::select! {
        changed = stop.changed() => changed.is_err() || *stop.borrow(),
        _ = tokio::time::sleep(delay) => false,
    }
}

pub async fn run(store: Arc<Store>, logger: ProcessSafeLogger, mut stop: watch::Receiver<bool>) {
    if let Err(error) = prepare_scan(&store, now_seconds()) {
        let _ = logger.warning(&format!(
            "Failed to initialize turn failure monitor: {}",
            safe_detail(error, 300)
        ));
    }

    let client = CodexAppServerClient::default();
    let mut reconnect_delay = Duration::from_secs(1);
    loop {
        if *stop.borrow() {
            return;
        }
        let opened = tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
                continue;
            }
            result = client.open_experimental_session(None) => result,
        };
        let mut session = match opened {
            Ok(session) => session,
            Err(error) => {
                let _ = logger.warning(&format!(
                    "Failed to connect turn failure monitor: {}",
                    safe_detail(error, 300)
                ));
                if wait_or_stop(&mut stop, reconnect_delay).await {
                    return;
                }
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
                continue;
            }
        };

        loop {
            match scan_once(&store, &mut session, now_seconds()).await {
                Ok(inserted) => {
                    reconnect_delay = Duration::from_secs(1);
                    if inserted > 0 {
                        let _ = logger.info(&format!(
                            "Queued {inserted} failed Codex turn notification(s)"
                        ));
                    }
                }
                Err(error) => {
                    let _ = logger.warning(&format!(
                        "Turn failure monitor scan failed: {}",
                        safe_detail(error, 300)
                    ));
                    session.close().await;
                    break;
                }
            }
            if wait_or_stop(&mut stop, POLL_INTERVAL).await {
                session.close().await;
                return;
            }
        }

        if wait_or_stop(&mut stop, reconnect_delay).await {
            return;
        }
        reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn thread() -> ThreadSummary {
        ThreadSummary {
            id: "thread-1".to_owned(),
            cwd: r"E:\work\demo".to_owned(),
            updated_at: 200,
        }
    }

    fn outbox_count(store: &Store) -> i64 {
        Connection::open(&store.path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn only_failed_turns_are_parsed_and_structured_statuses_are_preserved() {
        let payload = json!({
            "data": [
                {
                    "id": "failed-429",
                    "status": "failed",
                    "completedAt": 190,
                    "error": {
                        "message": "rate limited",
                        "codexErrorInfo": {"httpConnectionFailed": {"httpStatusCode": 429}}
                    }
                },
                {
                    "id": "failed-503",
                    "status": "failed",
                    "completedAt": 191,
                    "error": {
                        "message": "service unavailable",
                        "codexErrorInfo": {"responseTooManyFailedAttempts": {"httpStatusCode": 503}}
                    }
                },
                {
                    "id": "failed-without-completed-at",
                    "status": "failed",
                    "error": {"message": "connection failed", "codexErrorInfo": "other"}
                },
                {"id": "completed", "status": "completed", "completedAt": 192},
                {"id": "interrupted", "status": "interrupted", "completedAt": 193},
                {"id": "running", "status": "inProgress"}
            ],
            "nextCursor": null
        });

        let (failures, _, _) = parse_turn_page(&thread(), payload, 100.0).unwrap();
        assert_eq!(failures.len(), 3);
        assert_eq!(
            failures[0].error_type.as_deref(),
            Some("httpConnectionFailed")
        );
        assert_eq!(failures[0].http_status, Some(429));
        assert_eq!(
            failures[1].error_type.as_deref(),
            Some("responseTooManyFailedAttempts")
        );
        assert_eq!(failures[1].http_status, Some(503));
        assert_eq!(failures[2].occurred_at, 200);
    }

    #[test]
    fn failed_turn_messages_are_redacted_and_truncated() {
        let payload = json!({
            "data": [{
                "id": "failed",
                "status": "failed",
                "completedAt": 190,
                "error": {
                    "message": format!("api_key=monitor-secret {}", "x".repeat(600)),
                    "codexErrorInfo": "serverOverloaded",
                    "additionalDetails": "must not be sent"
                }
            }],
            "nextCursor": null
        });

        let (failures, _, _) = parse_turn_page(&thread(), payload, 100.0).unwrap();
        assert!(!failures[0].error_message.contains("monitor-secret"));
        assert!(failures[0].error_message.contains("[REDACTED]"));
        assert!(failures[0].error_message.chars().count() <= 500);
        assert_eq!(failures[0].error_type.as_deref(), Some("serverOverloaded"));
        assert!(!failures[0].error_message.contains("must not be sent"));
    }

    #[test]
    fn distinct_failures_enqueue_once_each() {
        let root = tempdir().unwrap();
        let store = Store::new(root.path().join("state.sqlite3")).unwrap();
        let failures: Vec<TurnFailure> = (1..=3)
            .map(|index| TurnFailure {
                thread_id: "thread-1".to_owned(),
                turn_id: format!("turn-{index}"),
                cwd: r"E:\work\demo".to_owned(),
                error_message: "failed".to_owned(),
                error_type: Some("other".to_owned()),
                http_status: None,
                occurred_at: 100 + index,
            })
            .collect();

        assert_eq!(finish_scan(&store, 200.0, Ok(failures.clone())).unwrap(), 3);
        assert_eq!(finish_scan(&store, 201.0, Ok(failures)).unwrap(), 0);
        assert_eq!(outbox_count(&store), 3);
    }

    #[test]
    fn first_run_sets_a_baseline_without_scanning_history() {
        let root = tempdir().unwrap();
        let store = Store::new(root.path().join("state.sqlite3")).unwrap();

        assert_eq!(prepare_scan(&store, 100.0).unwrap(), None);
        assert_eq!(
            setting_number(&store, BASELINE_SETTING).unwrap(),
            Some(100.0)
        );
        assert_eq!(setting_number(&store, CURSOR_SETTING).unwrap(), Some(100.0));
    }

    #[test]
    fn restart_uses_cursor_overlap_and_scan_errors_do_not_advance_it() {
        let root = tempdir().unwrap();
        let store = Store::new(root.path().join("state.sqlite3")).unwrap();
        store.set_setting(BASELINE_SETTING, "100").unwrap();
        store.set_setting(CURSOR_SETTING, "200").unwrap();

        assert_eq!(
            prepare_scan(&store, 500.0).unwrap(),
            Some(ScanWindow { since: 140.0 })
        );
        assert!(finish_scan(&store, 500.0, Err(anyhow!("app server down"))).is_err());
        assert_eq!(setting_number(&store, CURSOR_SETTING).unwrap(), Some(200.0));
    }
}
