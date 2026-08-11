//! QQ C2C command keyboards used by the remote control console.

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct MenuReply {
    pub text: String,
    pub keyboard: Option<Value>,
}

impl MenuReply {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            keyboard: None,
        }
    }

    pub fn menu(text: impl Into<String>, rows: Vec<Vec<MenuButton>>) -> Self {
        Self {
            text: text.into(),
            keyboard: Some(keyboard(rows)),
        }
    }

    pub fn fallback_text(&self) -> String {
        let Some(rows) = self
            .keyboard
            .as_ref()
            .and_then(|value| value.pointer("/content/rows"))
            .and_then(Value::as_array)
        else {
            return self.text.clone();
        };
        let actions = rows
            .iter()
            .flat_map(|row| {
                row.get("buttons")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(|button| {
                Some(format!(
                    "{}：{}",
                    button.pointer("/render_data/label")?.as_str()?,
                    button.pointer("/action/data")?.as_str()?
                ))
            })
            .collect::<Vec<_>>();
        if actions.is_empty() {
            self.text.clone()
        } else {
            format!("{}\n\n{}", self.text, actions.join("\n"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuButton {
    pub label: String,
    pub command: String,
}

impl MenuButton {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
        }
    }
}

pub fn main_menu(running: usize) -> MenuReply {
    MenuReply::menu(
        format!(
            "CodexBot 远程控制台\n运行中：{running} 个任务\n\n按钮不可用时可发送：/new、/tasks、/last、/status"
        ),
        vec![
            vec![
                MenuButton::new("新建任务", "/new"),
                MenuButton::new("运行中", "/tasks running"),
            ],
            vec![
                MenuButton::new("任务列表", "/tasks"),
                MenuButton::new("最近结果", "/last"),
            ],
            vec![
                MenuButton::new("项目目录", "/new"),
                MenuButton::new("状态", "/status"),
            ],
        ],
    )
}

pub fn task_notification_keyboard(thread_id: &str, can_continue: bool) -> Value {
    let mut first_row = vec![MenuButton::new("查看任务", format!("/task {thread_id}"))];
    if can_continue {
        first_row.push(MenuButton::new(
            "继续任务",
            format!("/continue {thread_id}"),
        ));
    }
    keyboard(vec![first_row, vec![MenuButton::new("新建任务", "/new")]])
}

pub fn keyboard(rows: Vec<Vec<MenuButton>>) -> Value {
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(row_index, buttons)| {
            json!({
                "buttons": buttons.into_iter().enumerate().map(|(button_index, button)| {
                    json!({
                        "id": format!("codexbot-{row_index}-{button_index}"),
                        "render_data": {
                            "label": button.label,
                            "visited_label": button.label,
                            "style": 1
                        },
                        "action": {
                            "type": 2,
                            "permission": {"type": 2},
                            "data": button.command,
                            "reply": true,
                            "enter": true,
                            "unsupport_tips": "请发送按钮显示的命令"
                        }
                    })
                }).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({"content": {"rows": rows}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_buttons_send_commands_immediately() {
        let value = keyboard(vec![vec![MenuButton::new("新建任务", "/new")]]);
        let action = &value["content"]["rows"][0]["buttons"][0]["action"];
        assert_eq!(action["type"], 2);
        assert_eq!(action["enter"], true);
        assert_eq!(action["data"], "/new");
    }

    #[test]
    fn text_fallback_preserves_every_action() {
        let reply = MenuReply::menu("菜单", vec![vec![MenuButton::new("新建任务", "/new")]]);
        assert!(reply.fallback_text().contains("新建任务：/new"));
    }
}
