//! Long-lived Codex app-server control session for QQ remote tasks.

use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::codex_login::{AppServerError, CodexAppServerClient, CodexAppServerSession};
use crate::store::{QQTaskInfo, Store, StoreError};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_RUNNING_TASKS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub model: String,
    pub display_name: String,
    pub is_default: bool,
    pub default_effort: String,
    pub efforts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadInfo {
    pub id: String,
    pub cwd: String,
    pub preview: String,
    pub status: String,
    pub updated_at: i64,
    pub turn_id: Option<String>,
    pub last_output: Option<String>,
    pub last_prompt: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedTask {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Error)]
pub enum ControlError {
    #[error(transparent)]
    AppServer(#[from] AppServerError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("Codex app-server returned an invalid {0} response")]
    InvalidResponse(&'static str),
    #[error("最多同时运行 {MAX_RUNNING_TASKS} 个 QQ 任务")]
    TooManyRunningTasks,
    #[error("该任务当前已有运行中的回合")]
    AlreadyRunning,
}

#[derive(Clone)]
pub struct CodexControlRuntime {
    client: CodexAppServerClient,
    session: Arc<Mutex<Option<CodexAppServerSession>>>,
    mutation_lock: Arc<Mutex<()>>,
    store: Arc<Store>,
}

impl std::fmt::Debug for CodexControlRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexControlRuntime")
            .field("store", &self.store.path)
            .finish_non_exhaustive()
    }
}

impl CodexControlRuntime {
    pub fn new(store: Arc<Store>) -> Self {
        Self {
            client: CodexAppServerClient::default(),
            session: Arc::new(Mutex::new(None)),
            mutation_lock: Arc::new(Mutex::new(())),
            store,
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, ControlError> {
        let mut session = self.session.lock().await;
        if session.is_none() {
            *session = Some(
                self.client
                    .open_experimental_session(Some(REQUEST_TIMEOUT))
                    .await?,
            );
        }
        let result = session
            .as_mut()
            .expect("session was initialized")
            .request(method, params, Some(REQUEST_TIMEOUT))
            .await;
        if result.is_err() {
            if let Some(session) = session.as_mut() {
                session.close().await;
            }
            *session = None;
        }
        Ok(result?)
    }

    pub async fn pump_once(&self) -> Result<(), ControlError> {
        let mut session_guard = self.session.lock().await;
        let Some(session) = session_guard.as_mut() else {
            return Ok(());
        };
        match session.pump_notification(Duration::from_millis(5)).await {
            Ok(Some(notification)) => self.record_notification(&notification)?,
            Ok(None) => {}
            Err(error) => {
                session.close().await;
                *session_guard = None;
                return Err(error.into());
            }
        }
        Ok(())
    }

    fn record_notification(&self, notification: &Value) -> Result<(), StoreError> {
        let method = notification
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !method.eq_ignore_ascii_case("turn/completed") {
            return Ok(());
        }
        let params = notification.get("params").unwrap_or(&Value::Null);
        let thread_id = params
            .get("threadId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let turn = params.get("turn").unwrap_or(&Value::Null);
        let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or_default();
        let state = turn
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("completed");
        if !thread_id.is_empty() && !turn_id.is_empty() {
            self.store.update_qq_task_state(thread_id, turn_id, state)?;
        }
        Ok(())
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ControlError> {
        let payload = self.request("model/list", json!({})).await?;
        Ok(payload
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| !item.get("hidden").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|item| {
                let model = item.get("model")?.as_str()?.to_owned();
                let efforts = item
                    .get("supportedReasoningEfforts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|effort| {
                        effort
                            .get("reasoningEffort")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                    .collect();
                Some(ModelInfo {
                    display_name: item
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&model)
                        .to_owned(),
                    is_default: item
                        .get("isDefault")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    default_effort: item
                        .get("defaultReasoningEffort")
                        .and_then(Value::as_str)
                        .unwrap_or("medium")
                        .to_owned(),
                    model,
                    efforts,
                })
            })
            .collect())
    }

    pub async fn list_threads(&self, limit: usize) -> Result<Vec<ThreadInfo>, ControlError> {
        let payload = self
            .request(
                "thread/list",
                json!({"limit": limit.min(100), "sortKey": "updated_at", "sortDirection": "desc"}),
            )
            .await?;
        let threads = payload
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_thread)
            .collect::<Vec<_>>();
        for thread in &threads {
            let Some(task) = self.store.get_qq_task(&thread.id)? else {
                continue;
            };
            if task.state == "active" && !matches!(thread.status.as_str(), "active" | "inProgress")
            {
                let state = match thread.status.as_str() {
                    "failed" | "systemError" => "failed",
                    "interrupted" => "interrupted",
                    _ => "completed",
                };
                self.store
                    .update_qq_task_state(&thread.id, &task.turn_id, state)?;
            }
        }
        Ok(threads)
    }

    pub async fn read_thread(&self, thread_id: &str) -> Result<ThreadInfo, ControlError> {
        let payload = self
            .request(
                "thread/read",
                json!({"threadId": thread_id, "includeTurns": true}),
            )
            .await?;
        parse_thread(payload.get("thread").unwrap_or(&Value::Null))
            .ok_or(ControlError::InvalidResponse("thread/read"))
    }

    pub async fn start_task(
        &self,
        cwd: &str,
        model: &str,
        effort: &str,
        prompt: &str,
    ) -> Result<StartedTask, ControlError> {
        let _mutation = self.mutation_lock.lock().await;
        if self.store.count_running_qq_tasks()? >= MAX_RUNNING_TASKS {
            return Err(ControlError::TooManyRunningTasks);
        }
        let thread = self
            .request(
                "thread/start",
                json!({
                    "cwd": cwd,
                    "model": model,
                    "approvalPolicy": "never",
                    "sandbox": "danger-full-access"
                }),
            )
            .await?;
        let thread_id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or(ControlError::InvalidResponse("thread/start"))?
            .to_owned();
        let turn = self
            .request(
                "turn/start",
                turn_start_params(&thread_id, cwd, model, effort, prompt),
            )
            .await?;
        let turn_id = turn
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or(ControlError::InvalidResponse("turn/start"))?
            .to_owned();
        self.store.upsert_qq_task(&QQTaskInfo {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            cwd: cwd.to_owned(),
            model: model.to_owned(),
            effort: effort.to_owned(),
            state: "active".to_owned(),
            updated_at: now_seconds(),
        })?;
        Ok(StartedTask { thread_id, turn_id })
    }

    pub async fn continue_task(
        &self,
        thread_id: &str,
        prompt: &str,
    ) -> Result<StartedTask, ControlError> {
        let _mutation = self.mutation_lock.lock().await;
        if self.store.count_running_qq_tasks()? >= MAX_RUNNING_TASKS {
            return Err(ControlError::TooManyRunningTasks);
        }
        let saved = self.store.get_qq_task(thread_id)?;
        let detail = self.read_thread(thread_id).await?;
        if matches!(detail.status.as_str(), "active" | "inProgress") {
            return Err(ControlError::AlreadyRunning);
        }
        let cwd = saved
            .as_ref()
            .map(|task| task.cwd.clone())
            .unwrap_or(detail.cwd);
        let (model, effort) = if let Some(saved) = saved {
            (saved.model, saved.effort)
        } else {
            let models = self.list_models().await?;
            let last_model = self.store.get_setting("qq_last_model")?;
            let selected = models
                .iter()
                .find(|model| last_model.as_deref() == Some(model.model.as_str()))
                .or_else(|| models.iter().find(|model| model.is_default))
                .or_else(|| models.first())
                .ok_or(ControlError::InvalidResponse("model/list"))?;
            let effort = self
                .store
                .get_setting("qq_last_effort")?
                .filter(|effort| {
                    selected.efforts.is_empty()
                        || selected.efforts.iter().any(|value| value == effort)
                })
                .unwrap_or_else(|| selected.default_effort.clone());
            (selected.model.clone(), effort)
        };
        self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": &cwd,
                "model": &model,
                "approvalPolicy": "never",
                "sandbox": "danger-full-access"
            }),
        )
        .await?;
        let turn = self
            .request(
                "turn/start",
                turn_start_params(thread_id, &cwd, &model, &effort, prompt),
            )
            .await?;
        let turn_id = turn
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or(ControlError::InvalidResponse("turn/start"))?
            .to_owned();
        self.store.upsert_qq_task(&QQTaskInfo {
            thread_id: thread_id.to_owned(),
            turn_id: turn_id.clone(),
            cwd,
            model,
            effort,
            state: "active".to_owned(),
            updated_at: now_seconds(),
        })?;
        Ok(StartedTask {
            thread_id: thread_id.to_owned(),
            turn_id,
        })
    }

    pub async fn steer(
        &self,
        thread_id: &str,
        turn_id: &str,
        prompt: &str,
    ) -> Result<(), ControlError> {
        self.request(
            "turn/steer",
            json!({
                "threadId": thread_id,
                "expectedTurnId": turn_id,
                "input": [{"type": "text", "text": prompt}]
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn interrupt(&self, thread_id: &str, turn_id: &str) -> Result<(), ControlError> {
        self.request(
            "turn/interrupt",
            json!({"threadId": thread_id, "turnId": turn_id}),
        )
        .await?;
        self.store
            .update_qq_task_state(thread_id, turn_id, "interrupted")?;
        Ok(())
    }
}

fn turn_start_params(thread_id: &str, cwd: &str, model: &str, effort: &str, prompt: &str) -> Value {
    json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": prompt}],
        "cwd": cwd,
        "model": model,
        "effort": effort,
        "approvalPolicy": "never",
        "sandboxPolicy": {"type": "dangerFullAccess"}
    })
}

fn parse_thread(value: &Value) -> Option<ThreadInfo> {
    let turns = value.get("turns").and_then(Value::as_array);
    let last_turn = turns.and_then(|turns| turns.last());
    let items = turns
        .into_iter()
        .flatten()
        .flat_map(|turn| {
            turn.get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .collect::<Vec<_>>();
    let thread_status = value
        .pointer("/status/type")
        .and_then(Value::as_str)
        .unwrap_or("notLoaded");
    Some(ThreadInfo {
        id: value.get("id")?.as_str()?.to_owned(),
        cwd: value.get("cwd")?.as_str()?.to_owned(),
        preview: value
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| value.get("preview").and_then(Value::as_str))
            .unwrap_or("未命名任务")
            .to_owned(),
        status: last_turn
            .and_then(|turn| turn.get("status"))
            .and_then(Value::as_str)
            .unwrap_or(thread_status)
            .to_owned(),
        updated_at: value
            .get("updatedAt")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        turn_id: last_turn
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        last_output: items.iter().rev().find_map(|item| {
            (item.get("type").and_then(Value::as_str) == Some("agentMessage"))
                .then(|| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .flatten()
        }),
        last_prompt: items.iter().rev().find_map(|item| {
            if item.get("type").and_then(Value::as_str) != Some("userMessage") {
                return None;
            }
            item.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find_map(|content| {
                    (content.get("type").and_then(Value::as_str) == Some("text"))
                        .then(|| {
                            content
                                .get("text")
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned)
                        })
                        .flatten()
                })
        }),
        error: last_turn
            .and_then(|turn| turn.pointer("/error/message"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_start_uses_never_and_full_access() {
        let value = turn_start_params("thread", r"E:\work", "gpt-test", "high", "fix it");
        assert_eq!(value["approvalPolicy"], "never");
        assert_eq!(value["sandboxPolicy"]["type"], "dangerFullAccess");
        assert_eq!(value["input"][0]["text"], "fix it");
    }

    #[test]
    fn parses_structured_thread_status() {
        let value = json!({
            "id": "thread-1",
            "cwd": "E:\\work",
            "preview": "hello",
            "status": {"type": "active", "activeFlags": []},
            "updatedAt": 7,
            "turns": []
        });
        assert_eq!(parse_thread(&value).unwrap().status, "active");
    }

    #[test]
    fn parses_latest_prompt_output_and_error() {
        let value = json!({
            "id": "thread-1",
            "cwd": "E:\\work",
            "preview": "hello",
            "status": {"type": "idle"},
            "updatedAt": 7,
            "turns": [{
                "id": "turn-1",
                "status": "failed",
                "error": {"message": "capacity"},
                "items": [
                    {"id": "u", "type": "userMessage", "content": [{"type": "text", "text": "do it"}]},
                    {"id": "a", "type": "agentMessage", "text": "partial"}
                ]
            }]
        });
        let thread = parse_thread(&value).unwrap();
        assert_eq!(thread.last_prompt.as_deref(), Some("do it"));
        assert_eq!(thread.last_output.as_deref(), Some("partial"));
        assert_eq!(thread.error.as_deref(), Some("capacity"));
    }
}
