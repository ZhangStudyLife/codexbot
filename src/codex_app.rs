//! Windows Codex desktop lifecycle used around account activation.

use std::fmt;
use std::io;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::codex_accounts::find_running_codex_processes;
#[cfg(windows)]
use crate::subprocess_utils::hide_console_window;

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const GRACEFUL_EXIT_POLLS: usize = 30;
const FORCED_EXIT_POLLS: usize = 50;

#[derive(Debug, Error)]
pub enum CodexAppError {
    #[error("无法检查 Codex 进程：{0}")]
    Inspection(String),
    #[error("无法关闭 Codex 进程：{0}")]
    Close(#[source] io::Error),
    #[error("关闭 Codex 超时，仍有 {0} 个进程正在运行")]
    CloseTimeout(usize),
    #[error("未找到 Codex 桌面应用，或 Windows 无法打开它")]
    Open,
    #[error("当前平台不支持自动关闭并重新打开 Codex")]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountSwitchOutcome<T> {
    pub value: T,
    pub closed_processes: usize,
    pub app_opened: bool,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AccountSwitchWorkflowError {
    message: String,
}

impl AccountSwitchWorkflowError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

trait CodexAppRuntime {
    fn running_pids(&mut self) -> Result<Vec<u32>, CodexAppError>;
    fn terminate(&mut self, pid: u32, force: bool) -> Result<(), CodexAppError>;
    fn pause(&mut self);
    fn open(&mut self) -> Result<(), CodexAppError>;
}

struct NativeCodexAppRuntime;

impl CodexAppRuntime for NativeCodexAppRuntime {
    fn running_pids(&mut self) -> Result<Vec<u32>, CodexAppError> {
        find_running_codex_processes().map_err(|error| CodexAppError::Inspection(error.to_string()))
    }

    fn terminate(&mut self, pid: u32, force: bool) -> Result<(), CodexAppError> {
        terminate_codex_process(pid, force)
    }

    fn pause(&mut self) {
        thread::sleep(EXIT_POLL_INTERVAL);
    }

    fn open(&mut self) -> Result<(), CodexAppError> {
        open_codex_app()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RestartContext {
    was_running: bool,
    closed_processes: usize,
}

fn normalized_pids(mut pids: Vec<u32>) -> Vec<u32> {
    pids.retain(|pid| *pid > 0 && *pid != std::process::id());
    pids.sort_unstable();
    pids.dedup();
    pids
}

fn wait_for_exit<R: CodexAppRuntime>(
    runtime: &mut R,
    polls: usize,
) -> Result<Vec<u32>, CodexAppError> {
    let mut remaining = Vec::new();
    for _ in 0..polls {
        runtime.pause();
        remaining = normalized_pids(runtime.running_pids()?);
        if remaining.is_empty() {
            break;
        }
    }
    Ok(remaining)
}

fn prepare_for_switch<R: CodexAppRuntime>(
    runtime: &mut R,
) -> Result<RestartContext, CodexAppError> {
    let initial = normalized_pids(runtime.running_pids()?);
    if initial.is_empty() {
        return Ok(RestartContext {
            was_running: false,
            closed_processes: 0,
        });
    }

    for pid in initial.iter().copied() {
        runtime.terminate(pid, false)?;
    }
    let mut remaining = wait_for_exit(runtime, GRACEFUL_EXIT_POLLS)?;
    if !remaining.is_empty() {
        for pid in remaining.iter().copied() {
            runtime.terminate(pid, true)?;
        }
        remaining = wait_for_exit(runtime, FORCED_EXIT_POLLS)?;
    }
    if !remaining.is_empty() {
        return Err(CodexAppError::CloseTimeout(remaining.len()));
    }
    Ok(RestartContext {
        was_running: true,
        closed_processes: initial.len(),
    })
}

fn switch_account_and_open_with<R, T, F, E>(
    runtime: &mut R,
    switch: F,
) -> Result<AccountSwitchOutcome<T>, AccountSwitchWorkflowError>
where
    R: CodexAppRuntime,
    F: FnOnce() -> Result<T, E>,
    E: fmt::Display,
{
    let context = prepare_for_switch(runtime)
        .map_err(|error| AccountSwitchWorkflowError::new(error.to_string()))?;
    let value = match switch() {
        Ok(value) => value,
        Err(error) => {
            let mut message = error.to_string();
            if context.was_running {
                match runtime.open() {
                    Ok(()) => message.push_str("；切换未完成，已重新打开原 Codex"),
                    Err(recovery) => message.push_str(&format!(
                        "；切换未完成，且原 Codex 自动恢复失败：{recovery}"
                    )),
                }
            }
            return Err(AccountSwitchWorkflowError::new(message));
        }
    };
    let app_opened = runtime.open().is_ok();
    Ok(AccountSwitchOutcome {
        value,
        closed_processes: context.closed_processes,
        app_opened,
    })
}

pub fn switch_account_and_open<T, F, E>(
    switch: F,
) -> Result<AccountSwitchOutcome<T>, AccountSwitchWorkflowError>
where
    F: FnOnce() -> Result<T, E>,
    E: fmt::Display,
{
    static SWITCH_WORKFLOW_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = SWITCH_WORKFLOW_LOCK.get_or_init(|| Mutex::new(()));
    let deadline = Instant::now() + Duration::from_secs(2);
    let _guard = loop {
        if let Ok(guard) = lock.try_lock() {
            break guard;
        }
        if Instant::now() >= deadline {
            return Err(AccountSwitchWorkflowError::new(
                "另一个 Codex 账号切换正在进行，请稍后重试",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    switch_account_and_open_with(&mut NativeCodexAppRuntime, switch)
}

#[cfg(windows)]
fn terminate_codex_process(pid: u32, force: bool) -> Result<(), CodexAppError> {
    let mut command = Command::new("taskkill.exe");
    if force {
        command.arg("/F");
    }
    command
        .args(["/T", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    // A non-zero status commonly means another targeted parent already ended
    // this process. The subsequent process snapshot is authoritative.
    command.status().map(|_| ()).map_err(CodexAppError::Close)
}

#[cfg(not(windows))]
fn terminate_codex_process(_pid: u32, _force: bool) -> Result<(), CodexAppError> {
    Err(CodexAppError::Unsupported)
}

#[cfg(windows)]
fn open_registered_codex_app() -> bool {
    const SCRIPT: &str = r#"
$apps = Get-StartApps
$app = $apps | Where-Object { ([string]$_.AppID) -like 'OpenAI.Codex*' } | Select-Object -First 1
if ($null -eq $app) {
  $app = $apps | Where-Object {
    $name = [string]$_.Name
    $name -eq 'Codex' -or $name -eq 'OpenAI Codex'
  } | Select-Object -First 1
}
if ($null -eq $app) { exit 1 }
Start-Process explorer.exe -ArgumentList ('shell:AppsFolder\' + $app.AppID)
"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    command.status().is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn common_codex_executables() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        let Some(base) = std::env::var_os(key).map(PathBuf::from) else {
            continue;
        };
        candidates.push(base.join("Programs").join("Codex").join("Codex.exe"));
        candidates.push(base.join("OpenAI").join("Codex").join("Codex.exe"));
        candidates.push(base.join("OpenAI Codex").join("Codex.exe"));
        candidates.push(base.join("Codex").join("Codex.exe"));
    }
    candidates
}

#[cfg(windows)]
fn spawn_codex_executable(path: &Path) -> bool {
    let mut command = Command::new(path);
    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    command.spawn().is_ok()
}

pub fn open_codex_app() -> Result<(), CodexAppError> {
    #[cfg(windows)]
    {
        if open_registered_codex_app() {
            return Ok(());
        }
        if common_codex_executables()
            .into_iter()
            .any(|path| path.is_file() && spawn_codex_executable(&path))
        {
            return Ok(());
        }
        Err(CodexAppError::Open)
    }
    #[cfg(not(windows))]
    {
        Err(CodexAppError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeRuntime {
        running: Vec<u32>,
        exit_on_graceful: bool,
        exit_on_force: bool,
        terminations: Vec<(u32, bool)>,
        open_calls: usize,
        open_fails: bool,
    }

    impl CodexAppRuntime for FakeRuntime {
        fn running_pids(&mut self) -> Result<Vec<u32>, CodexAppError> {
            Ok(self.running.clone())
        }

        fn terminate(&mut self, pid: u32, force: bool) -> Result<(), CodexAppError> {
            self.terminations.push((pid, force));
            if (force && self.exit_on_force) || (!force && self.exit_on_graceful) {
                self.running.retain(|candidate| *candidate != pid);
            }
            Ok(())
        }

        fn pause(&mut self) {}

        fn open(&mut self) -> Result<(), CodexAppError> {
            self.open_calls += 1;
            if self.open_fails {
                Err(CodexAppError::Open)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn closes_running_codex_before_switch_and_reopens_it() {
        let mut runtime = FakeRuntime {
            running: vec![41],
            exit_on_graceful: true,
            ..FakeRuntime::default()
        };
        let outcome =
            switch_account_and_open_with(&mut runtime, || Ok::<_, &str>("account")).unwrap();
        assert_eq!(runtime.terminations, vec![(41, false)]);
        assert_eq!(runtime.open_calls, 1);
        assert_eq!(outcome.closed_processes, 1);
        assert!(outcome.app_opened);
    }

    #[test]
    fn force_closes_processes_that_ignore_the_graceful_request() {
        let mut runtime = FakeRuntime {
            running: vec![9],
            exit_on_force: true,
            ..FakeRuntime::default()
        };
        let outcome = switch_account_and_open_with(&mut runtime, || Ok::<_, &str>(())).unwrap();
        assert_eq!(runtime.terminations, vec![(9, false), (9, true)]);
        assert_eq!(outcome.closed_processes, 1);
    }

    #[test]
    fn failed_switch_reopens_the_previous_codex_session() {
        let mut runtime = FakeRuntime {
            running: vec![17],
            exit_on_graceful: true,
            ..FakeRuntime::default()
        };
        let error = switch_account_and_open_with::<_, (), _, _>(&mut runtime, || Err("bad auth"))
            .unwrap_err();
        assert_eq!(runtime.open_calls, 1);
        assert!(error.to_string().contains("已重新打开原 Codex"));
    }

    #[test]
    fn successful_switch_opens_codex_even_when_it_was_not_running() {
        let mut runtime = FakeRuntime::default();
        let outcome = switch_account_and_open_with(&mut runtime, || Ok::<_, &str>(())).unwrap();
        assert_eq!(runtime.open_calls, 1);
        assert_eq!(outcome.closed_processes, 0);
        assert!(outcome.app_opened);
    }

    #[test]
    fn account_switch_remains_successful_when_codex_cannot_be_opened() {
        let mut runtime = FakeRuntime {
            open_fails: true,
            ..FakeRuntime::default()
        };
        let outcome = switch_account_and_open_with(&mut runtime, || Ok::<_, &str>(())).unwrap();
        assert_eq!(runtime.open_calls, 1);
        assert!(!outcome.app_opened);
    }
}
