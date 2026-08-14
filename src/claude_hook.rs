use crate::paths::{database_path, ensure_data_dir};
use crate::store::Store;
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, Read, Write};

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn run_inner(payload: &Value) -> Result<()> {
    let object = payload
        .as_object()
        .context("Claude Hook payload must be an object")?;
    let session_id = text(
        object
            .get("session_id")
            .or_else(|| object.get("conversation_id")),
    );
    let session_id = if session_id.is_empty() {
        "unknown"
    } else {
        &session_id
    };
    let cwd = text(object.get("cwd").or_else(|| object.get("project_dir")));
    let event = text(
        object
            .get("hook_event_name")
            .or_else(|| object.get("event")),
    );
    let status = text(object.get("status").or_else(|| object.get("result")));
    let answer = text(
        object
            .get("last_assistant_message")
            .or_else(|| object.get("response"))
            .or_else(|| object.get("content"))
            .or_else(|| object.get("message")),
    );
    let error = text(
        object
            .get("error")
            .or_else(|| object.get("error_message"))
            .or_else(|| object.get("reason")),
    );
    let failed = !error.is_empty()
        || matches!(
            status.to_ascii_lowercase().as_str(),
            "failed" | "error" | "cancelled"
        )
        || event.to_ascii_lowercase().contains("error");
    let project = if cwd.is_empty() {
        "Claude Code".to_owned()
    } else {
        std::path::Path::new(&cwd)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&cwd)
            .to_owned()
    };
    let content = if failed {
        format!(
            "❌ Claude Code 任务失败\n项目：{project}\n错误：{}",
            if error.is_empty() {
                "未知错误"
            } else {
                &error
            }
        )
    } else {
        format!(
            "✅ Claude Code 本轮已结束\n项目：{project}\n\n{}",
            if answer.is_empty() {
                "（没有可显示的回答）"
            } else {
                &answer
            }
        )
    };
    let store = Store::new(database_path()).context("failed to open state store")?;
    store.enqueue_claude_reply(session_id, &cwd, &content, failed)?;
    Ok(())
}

pub fn run() -> i32 {
    let mut raw = String::new();
    let result = io::stdin().read_to_string(&mut raw).and_then(|_| {
        let payload: Value = serde_json::from_str(&raw).map_err(io::Error::other)?;
        ensure_data_dir().map_err(io::Error::other)?;
        run_inner(&payload).map_err(io::Error::other)
    });
    let _ = io::stdout().write_all(b"{}");
    if result.is_err() { 1 } else { 0 }
}
