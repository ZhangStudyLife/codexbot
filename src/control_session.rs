//! Stateful QQ wizard for creating and controlling Codex tasks.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::codex_control::{CodexControlRuntime, ControlError, ModelInfo, ThreadInfo};
use crate::commands::hmac_equal;
use crate::qq_menu::{MenuButton, MenuReply, main_menu};
use crate::security::redact_secrets;
use crate::store::{Store, StoreError};

const WIZARD_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    Idle,
    AwaitPath,
    AwaitPrompt,
    Confirm,
    AwaitContinue(String),
    AwaitSteer { thread_id: String, turn_id: String },
}

#[derive(Debug)]
struct Wizard {
    stage: Stage,
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    prompt: Option<String>,
    expires_at: Instant,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            cwd: None,
            model: None,
            effort: None,
            prompt: None,
            expires_at: Instant::now(),
        }
    }
}

impl Wizard {
    fn touch(&mut self) {
        self.expires_at = Instant::now() + WIZARD_TTL;
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn active(&mut self) -> bool {
        if self.stage != Stage::Idle && Instant::now() >= self.expires_at {
            self.reset();
        }
        self.stage != Stage::Idle
    }
}

#[derive(Clone)]
pub struct ControlSession {
    store: Arc<Store>,
    codex: CodexControlRuntime,
    wizard: Arc<Mutex<Wizard>>,
}

impl std::fmt::Debug for ControlSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ControlSession")
            .field("store", &self.store.path)
            .finish_non_exhaustive()
    }
}

impl ControlSession {
    pub fn new(store: Arc<Store>, codex: CodexControlRuntime) -> Self {
        Self {
            store,
            codex,
            wizard: Arc::new(Mutex::new(Wizard::default())),
        }
    }

    pub async fn should_handle(&self, content: &str) -> bool {
        let command = normalize(content);
        if command.starts_with("/menu")
            || command.starts_with("/new")
            || command.starts_with("/tasks")
            || command.starts_with("/task ")
            || command.starts_with("/continue ")
            || command.starts_with("/steer ")
            || command.starts_with("/stop ")
            || command.starts_with("/ui ")
            || command == "/cancel"
        {
            return true;
        }
        self.wizard.lock().await.active()
    }

    pub async fn handle(&self, openid: &str, content: &str) -> MenuReply {
        match self.authorized(openid) {
            Ok(true) => {}
            Ok(false) => return MenuReply::text("无权操作此 CodexBot。"),
            Err(error) => return error_reply(error),
        }
        let command = normalize(content);
        let result = if command == "/menu" {
            self.menu()
        } else if command == "/new" {
            self.begin_new().await
        } else if command == "/tasks" || command.starts_with("/tasks ") {
            self.tasks(command.ends_with(" running")).await
        } else if let Some(thread_id) = command.strip_prefix("/task ") {
            self.task_detail(thread_id.trim()).await
        } else if let Some(thread_id) = command.strip_prefix("/continue ") {
            self.await_continue(thread_id.trim()).await
        } else if let Some(arguments) = command.strip_prefix("/steer ") {
            self.await_steer(arguments).await
        } else if let Some(arguments) = command.strip_prefix("/stop ") {
            self.stop_task(arguments).await
        } else if command == "/cancel" {
            self.wizard.lock().await.reset();
            self.menu()
        } else if let Some(arguments) = command.strip_prefix("/ui ") {
            self.handle_ui(arguments).await
        } else {
            self.handle_input(content.trim()).await
        };
        result.unwrap_or_else(error_reply)
    }

    pub async fn pump_once(&self) -> Result<(), ControlError> {
        self.codex.pump_once().await
    }

    fn authorized(&self, openid: &str) -> Result<bool, StoreError> {
        Ok(self
            .store
            .get_bound_openid()?
            .is_some_and(|bound| hmac_equal(&bound, openid)))
    }

    fn menu(&self) -> Result<MenuReply, ControlError> {
        Ok(main_menu(self.store.count_running_qq_tasks()?))
    }

    async fn begin_new(&self) -> Result<MenuReply, ControlError> {
        let threads = self.codex.list_threads(20).await?;
        let mut directories = Vec::new();
        for thread in threads {
            if !directories.iter().any(|cwd| cwd == &thread.cwd) && Path::new(&thread.cwd).is_dir()
            {
                directories.push(thread.cwd);
            }
            if directories.len() == 4 {
                break;
            }
        }
        let mut rows = directories
            .iter()
            .map(|cwd| {
                vec![MenuButton::new(
                    shorten(cwd, 24),
                    format!("/ui cwd {}", encode(cwd)),
                )]
            })
            .collect::<Vec<_>>();
        rows.push(vec![MenuButton::new("输入绝对路径", "/ui path")]);
        rows.push(vec![MenuButton::new("取消", "/cancel")]);
        let mut wizard = self.wizard.lock().await;
        wizard.reset();
        wizard.touch();
        Ok(MenuReply::menu(
            "新建任务 · 选择工作目录\n可选最近目录，或输入电脑上的任意绝对路径。",
            rows,
        ))
    }

    async fn handle_ui(&self, arguments: &str) -> Result<MenuReply, ControlError> {
        if arguments == "path" {
            let mut wizard = self.wizard.lock().await;
            wizard.stage = Stage::AwaitPath;
            wizard.touch();
            return Ok(MenuReply::text(
                "请发送电脑上的绝对目录路径，例如 E:\\CodexWorkspace\\Project。发送 /cancel 取消。",
            ));
        }
        if arguments == "models" {
            return self.model_menu().await;
        }
        if arguments == "last" {
            return self.use_last_config().await;
        }
        if arguments == "start" {
            return self.start_confirmed().await;
        }
        if let Some(encoded) = arguments.strip_prefix("cwd ") {
            return self.select_path(&decode(encoded)?).await;
        }
        if let Some(encoded) = arguments.strip_prefix("model ") {
            return self.select_model(&decode(encoded)?).await;
        }
        if let Some(encoded) = arguments.strip_prefix("effort ") {
            return self.select_effort(&decode(encoded)?).await;
        }
        Err(ControlError::InvalidResponse("menu action"))
    }

    async fn handle_input(&self, content: &str) -> Result<MenuReply, ControlError> {
        let stage = self.wizard.lock().await.stage.clone();
        match stage {
            Stage::AwaitPath => self.select_path(content).await,
            Stage::AwaitPrompt => self.confirm_prompt(content).await,
            Stage::AwaitContinue(thread_id) => self.continue_task(&thread_id, content).await,
            Stage::AwaitSteer { thread_id, turn_id } => {
                self.steer_task(&thread_id, &turn_id, content).await
            }
            Stage::Confirm => Ok(MenuReply::text("请点击“启动任务”，或发送 /cancel。")),
            Stage::Idle => self.menu(),
        }
    }

    async fn select_path(&self, cwd: &str) -> Result<MenuReply, ControlError> {
        let path = Path::new(cwd);
        if !path.is_absolute() || !path.is_dir() {
            return Ok(MenuReply::text(
                "目录无效或不存在，请重新发送一个存在的绝对目录路径。",
            ));
        }
        {
            let mut wizard = self.wizard.lock().await;
            wizard.cwd = Some(cwd.to_owned());
            wizard.touch();
        }
        let models = self.codex.list_models().await?;
        let last_model = self.store.get_setting("qq_last_model")?;
        let last_effort = self.store.get_setting("qq_last_effort")?;
        if let Some(model) =
            valid_saved_model(&models, last_model.as_deref(), last_effort.as_deref())
        {
            return Ok(MenuReply::menu(
                format!(
                    "工作目录：{}\n上次配置：{} / {}",
                    shorten(cwd, 80),
                    model.model,
                    last_effort.as_deref().unwrap_or(&model.default_effort)
                ),
                vec![
                    vec![
                        MenuButton::new("直接使用", "/ui last"),
                        MenuButton::new("修改配置", "/ui models"),
                    ],
                    vec![MenuButton::new("取消", "/cancel")],
                ],
            ));
        }
        self.model_menu_from(models)
    }

    async fn model_menu(&self) -> Result<MenuReply, ControlError> {
        let models = self.codex.list_models().await?;
        self.model_menu_from(models)
    }

    fn model_menu_from(&self, models: Vec<ModelInfo>) -> Result<MenuReply, ControlError> {
        if models.is_empty() {
            return Err(ControlError::InvalidResponse("model/list"));
        }
        let rows = models
            .into_iter()
            .take(8)
            .map(|model| {
                let marker = if model.is_default { "（默认）" } else { "" };
                vec![MenuButton::new(
                    format!("{}{}", model.display_name, marker),
                    format!("/ui model {}", encode(&model.model)),
                )]
            })
            .chain(std::iter::once(vec![MenuButton::new("取消", "/cancel")]))
            .collect();
        Ok(MenuReply::menu("选择模型", rows))
    }

    async fn use_last_config(&self) -> Result<MenuReply, ControlError> {
        let model = self
            .store
            .get_setting("qq_last_model")?
            .ok_or(ControlError::InvalidResponse("saved model"))?;
        let effort = self
            .store
            .get_setting("qq_last_effort")?
            .ok_or(ControlError::InvalidResponse("saved effort"))?;
        let models = self.codex.list_models().await?;
        if valid_saved_model(&models, Some(&model), Some(&effort)).is_none() {
            return self.model_menu_from(models);
        }
        let mut wizard = self.wizard.lock().await;
        wizard.model = Some(model);
        wizard.effort = Some(effort);
        wizard.stage = Stage::AwaitPrompt;
        wizard.touch();
        Ok(MenuReply::text(
            "请发送任务内容。完整内容只保存在内存中，提交后即清除。",
        ))
    }

    async fn select_model(&self, model_name: &str) -> Result<MenuReply, ControlError> {
        let models = self.codex.list_models().await?;
        let model = models
            .into_iter()
            .find(|model| model.model == model_name)
            .ok_or(ControlError::InvalidResponse("selected model"))?;
        {
            let mut wizard = self.wizard.lock().await;
            wizard.model = Some(model.model.clone());
            wizard.touch();
        }
        let efforts = if model.efforts.is_empty() {
            vec![model.default_effort]
        } else {
            model.efforts
        };
        Ok(MenuReply::menu(
            format!("模型：{}\n选择推理强度", model.display_name),
            efforts
                .into_iter()
                .map(|effort| {
                    vec![MenuButton::new(
                        effort.clone(),
                        format!("/ui effort {}", encode(&effort)),
                    )]
                })
                .chain(std::iter::once(vec![MenuButton::new("取消", "/cancel")]))
                .collect(),
        ))
    }

    async fn select_effort(&self, effort: &str) -> Result<MenuReply, ControlError> {
        let mut wizard = self.wizard.lock().await;
        wizard.effort = Some(effort.to_owned());
        wizard.stage = Stage::AwaitPrompt;
        wizard.touch();
        Ok(MenuReply::text(
            "请发送任务内容。完整内容只保存在内存中，提交后即清除。",
        ))
    }

    async fn confirm_prompt(&self, prompt: &str) -> Result<MenuReply, ControlError> {
        if prompt.is_empty() {
            return Ok(MenuReply::text("任务内容不能为空，请重新发送。"));
        }
        let mut wizard = self.wizard.lock().await;
        wizard.prompt = Some(prompt.to_owned());
        wizard.stage = Stage::Confirm;
        wizard.touch();
        Ok(MenuReply::menu(
            format!(
                "确认启动\n目录：{}\n模型：{} / {}\n任务：{}",
                wizard.cwd.as_deref().unwrap_or("未选择"),
                wizard.model.as_deref().unwrap_or("未选择"),
                wizard.effort.as_deref().unwrap_or("未选择"),
                shorten(prompt, 120)
            ),
            vec![
                vec![MenuButton::new("启动任务", "/ui start")],
                vec![MenuButton::new("取消", "/cancel")],
            ],
        ))
    }

    async fn start_confirmed(&self) -> Result<MenuReply, ControlError> {
        let (cwd, model, effort, prompt) = {
            let mut wizard = self.wizard.lock().await;
            if wizard.stage != Stage::Confirm {
                return Ok(MenuReply::text(
                    "启动确认已失效，请发送 /new 重新创建任务。",
                ));
            }
            let values = (
                wizard.cwd.clone().unwrap_or_default(),
                wizard.model.clone().unwrap_or_default(),
                wizard.effort.clone().unwrap_or_default(),
                wizard.prompt.take().unwrap_or_default(),
            );
            wizard.reset();
            values
        };
        let task = self
            .codex
            .start_task(&cwd, &model, &effort, &prompt)
            .await?;
        self.store.set_setting("qq_last_model", &model)?;
        self.store.set_setting("qq_last_effort", &effort)?;
        Ok(MenuReply::menu(
            format!("任务已启动\n任务 ID：{}\n工作目录：{}", task.thread_id, cwd),
            vec![
                vec![MenuButton::new(
                    "查看任务",
                    format!("/task {}", task.thread_id),
                )],
                vec![MenuButton::new("返回主菜单", "/menu")],
            ],
        ))
    }

    async fn tasks(&self, running_only: bool) -> Result<MenuReply, ControlError> {
        let threads = self.codex.list_threads(20).await?;
        let threads = threads
            .into_iter()
            .filter(|thread| {
                !running_only || thread.status == "active" || thread.status == "inProgress"
            })
            .take(5)
            .collect::<Vec<_>>();
        if threads.is_empty() {
            return Ok(MenuReply::menu(
                "没有符合条件的任务。",
                vec![vec![MenuButton::new("返回主菜单", "/menu")]],
            ));
        }
        let text = threads
            .iter()
            .enumerate()
            .map(|(index, thread)| {
                format!(
                    "{}. [{}] {}\n   {}",
                    index + 1,
                    status_text(&thread.status),
                    shorten(&thread.preview, 45),
                    shorten(&thread.cwd, 60)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut rows = threads
            .iter()
            .map(|thread| {
                vec![MenuButton::new(
                    shorten(&thread.preview, 22),
                    format!("/task {}", thread.id),
                )]
            })
            .collect::<Vec<_>>();
        rows.push(vec![MenuButton::new("返回主菜单", "/menu")]);
        Ok(MenuReply::menu(text, rows))
    }

    async fn task_detail(&self, thread_id: &str) -> Result<MenuReply, ControlError> {
        let thread = self.codex.read_thread(thread_id).await?;
        let mut rows = Vec::new();
        if matches!(thread.status.as_str(), "active" | "inProgress") {
            if let Some(turn_id) = thread.turn_id.as_deref() {
                rows.push(vec![
                    MenuButton::new("追加引导", format!("/steer {} {}", thread.id, turn_id)),
                    MenuButton::new("停止任务", format!("/stop {} {}", thread.id, turn_id)),
                ]);
            }
        } else {
            rows.push(vec![MenuButton::new(
                "继续任务",
                format!("/continue {}", thread.id),
            )]);
        }
        rows.push(vec![
            MenuButton::new("刷新", format!("/task {}", thread.id)),
            MenuButton::new("任务列表", "/tasks"),
        ]);
        Ok(MenuReply::menu(task_text(&thread), rows))
    }

    async fn await_continue(&self, thread_id: &str) -> Result<MenuReply, ControlError> {
        self.codex.read_thread(thread_id).await?;
        let mut wizard = self.wizard.lock().await;
        wizard.reset();
        wizard.stage = Stage::AwaitContinue(thread_id.to_owned());
        wizard.touch();
        Ok(MenuReply::text("请发送要继续补充给 Codex 的内容。"))
    }

    async fn continue_task(
        &self,
        thread_id: &str,
        prompt: &str,
    ) -> Result<MenuReply, ControlError> {
        self.wizard.lock().await.reset();
        let task = self.codex.continue_task(thread_id, prompt).await?;
        Ok(MenuReply::menu(
            "已继续任务，Codex 正在处理。",
            vec![vec![MenuButton::new(
                "查看任务",
                format!("/task {}", task.thread_id),
            )]],
        ))
    }

    async fn await_steer(&self, arguments: &str) -> Result<MenuReply, ControlError> {
        let mut values = arguments.split_whitespace();
        let thread_id = values.next().unwrap_or_default();
        let turn_id = values.next().unwrap_or_default();
        if thread_id.is_empty() || turn_id.is_empty() {
            return Err(ControlError::InvalidResponse("steer command"));
        }
        let mut wizard = self.wizard.lock().await;
        wizard.reset();
        wizard.stage = Stage::AwaitSteer {
            thread_id: thread_id.to_owned(),
            turn_id: turn_id.to_owned(),
        };
        wizard.touch();
        Ok(MenuReply::text("请发送要追加给当前运行回合的引导。"))
    }

    async fn steer_task(
        &self,
        thread_id: &str,
        turn_id: &str,
        prompt: &str,
    ) -> Result<MenuReply, ControlError> {
        self.wizard.lock().await.reset();
        self.codex.steer(thread_id, turn_id, prompt).await?;
        Ok(MenuReply::menu(
            "已追加引导。",
            vec![vec![MenuButton::new(
                "查看任务",
                format!("/task {thread_id}"),
            )]],
        ))
    }

    async fn stop_task(&self, arguments: &str) -> Result<MenuReply, ControlError> {
        let mut values = arguments.split_whitespace();
        let thread_id = values.next().unwrap_or_default();
        let turn_id = values.next().unwrap_or_default();
        if thread_id.is_empty() || turn_id.is_empty() {
            return Err(ControlError::InvalidResponse("stop command"));
        }
        self.codex.interrupt(thread_id, turn_id).await?;
        Ok(MenuReply::menu(
            "任务已停止，历史仍保留，可随时继续。",
            vec![vec![MenuButton::new(
                "查看任务",
                format!("/task {thread_id}"),
            )]],
        ))
    }
}

fn valid_saved_model<'a>(
    models: &'a [ModelInfo],
    model: Option<&str>,
    effort: Option<&str>,
) -> Option<&'a ModelInfo> {
    let model = models
        .iter()
        .find(|item| Some(item.model.as_str()) == model)?;
    let effort = effort?;
    (model.efforts.is_empty() || model.efforts.iter().any(|item| item == effort)).then_some(model)
}

fn task_text(thread: &ThreadInfo) -> String {
    format!(
        "任务：{}\n状态：{}\n目录：{}\n任务 ID：{}",
        shorten(&thread.preview, 100),
        status_text(&thread.status),
        thread.cwd,
        thread.id
    )
}

fn status_text(status: &str) -> &str {
    match status {
        "active" | "inProgress" => "运行中",
        "completed" | "idle" => "已完成",
        "failed" | "systemError" => "失败",
        "interrupted" => "已停止",
        _ => "未知",
    }
}

fn normalize(content: &str) -> String {
    let mut value = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.starts_with('\\') {
        value.replace_range(..1, "/");
    }
    value
}

fn encode(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(value.as_bytes())
}

fn decode(value: &str) -> Result<String, ControlError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value.trim())
        .map_err(|_| ControlError::InvalidResponse("encoded menu value"))?;
    String::from_utf8(bytes).map_err(|_| ControlError::InvalidResponse("encoded menu value"))
}

fn shorten(value: &str, limit: usize) -> String {
    let clean = redact_secrets(value)
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.chars().count() <= limit {
        clean
    } else {
        format!("{}…", clean.chars().take(limit).collect::<String>())
    }
}

fn error_reply(error: impl std::fmt::Display) -> MenuReply {
    MenuReply::menu(
        format!("操作失败：{}", shorten(&error.to_string(), 240)),
        vec![
            vec![MenuButton::new("重试菜单", "/menu")],
            vec![MenuButton::new("任务列表", "/tasks")],
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_model_requires_supported_effort() {
        let models = vec![ModelInfo {
            model: "gpt-test".into(),
            display_name: "GPT Test".into(),
            is_default: true,
            default_effort: "medium".into(),
            efforts: vec!["medium".into(), "high".into()],
        }];
        assert!(valid_saved_model(&models, Some("gpt-test"), Some("high")).is_some());
        assert!(valid_saved_model(&models, Some("gpt-test"), Some("ultra")).is_none());
    }

    #[test]
    fn menu_values_round_trip_windows_paths() {
        let path = r"E:\Codex Workspace\Demo";
        assert_eq!(decode(&encode(path)).unwrap(), path);
    }
}
