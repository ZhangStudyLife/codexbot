//! Tauri desktop shell for the CodexBot local runtime.

use std::env;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use codexbot::daemon::STANDALONE_SETTING;
use codexbot::installer::{install_personal_plugin, marketplace_contains_plugin};
use codexbot::paths::{data_dir, database_path, ensure_data_dir, installed_executable};
use codexbot::processes::{discover_running_codex_host, ensure_daemon, process_matches};
use codexbot::security::{
    generate_pairing_code, load_credentials, prompt_preview, redact_secrets, store_credentials,
};
use codexbot::store::Store;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};
use zeroize::Zeroize;

const RUNTIME_VERSION_SETTING: &str = "desktop_runtime_version";
const PLUGIN_MANIFEST: &[u8] = include_bytes!("../../../plugin/codexbot/.codex-plugin/plugin.json");
const PLUGIN_HOOKS: &[u8] = include_bytes!("../../../plugin/codexbot/hooks/hooks.json");
const PLUGIN_ENTRY: &[u8] = include_bytes!("../../../plugin/codexbot/hooks/entry.cmd");

#[derive(Debug, Serialize)]
struct SessionSummary {
    project: String,
    model: String,
    status: String,
    prompt_preview: Option<String>,
    updated_at: f64,
}

#[derive(Debug, Serialize)]
struct ReplySummary {
    reply_id: i64,
    project: String,
    model: String,
    preview: String,
    created_at: f64,
}

#[derive(Debug, Serialize)]
struct DashboardState {
    version: &'static str,
    integration_ready: bool,
    runtime_installed: bool,
    plugin_installed: bool,
    credentials_configured: bool,
    app_id_hint: Option<String>,
    qq_bound: bool,
    daemon_running: bool,
    daemon_pid: Option<u32>,
    standalone: bool,
    codex_running: bool,
    muted: bool,
    permission_notifications: bool,
    queue_pending: bool,
    pairing_active: bool,
    pairing_expires_at: Option<f64>,
    sessions: Vec<SessionSummary>,
    recent_replies: Vec<ReplySummary>,
}

#[derive(Debug, Deserialize)]
struct CredentialsInput {
    app_id: String,
    app_secret: String,
}

#[derive(Debug, Serialize)]
struct PairingCode {
    code: String,
    expires_at: f64,
}

#[derive(Debug, Serialize)]
struct ActionResult {
    message: String,
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn safe_error(error: impl std::fmt::Display) -> String {
    redact_secrets(&error.to_string())
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(400)
        .collect()
}

fn mask_app_id(app_id: &str) -> String {
    let characters: Vec<char> = app_id.chars().collect();
    match characters.len() {
        0 => String::new(),
        1..=4 => format!(
            "{}••{}",
            characters.first().copied().unwrap_or_default(),
            characters.last().copied().unwrap_or_default()
        ),
        length => format!(
            "{}••••{}",
            characters[..3].iter().collect::<String>(),
            characters[length - 4..].iter().collect::<String>()
        ),
    }
}

fn user_home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn integration_status() -> (bool, bool) {
    let runtime_installed = installed_executable().is_file();
    let home = user_home_dir();
    let plugin_manifest = home
        .join("plugins")
        .join("codexbot")
        .join(".codex-plugin")
        .join("plugin.json");
    let marketplace = home
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    let plugin_installed = plugin_manifest.is_file() && marketplace_contains_plugin(&marketplace);
    (runtime_installed, plugin_installed)
}

fn files_equal(left: &Path, right: &Path) -> Result<bool> {
    let left_metadata = fs::metadata(left)?;
    let right_metadata = fs::metadata(right)?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let mut left = BufReader::new(fs::File::open(left)?);
    let mut right = BufReader::new(fs::File::open(right)?);
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_read = left.read(&mut left_buffer)?;
        let right_read = right.read(&mut right_buffer)?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn replace_runtime_binary(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("本地运行时路径缺少父目录"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!("codexbot.{}.new.exe", std::process::id()));
    let backup = parent.join("codexbot.previous.exe");
    let _ = fs::remove_file(&temporary);
    let _ = fs::remove_file(&backup);
    fs::copy(source, &temporary).with_context(|| {
        format!(
            "无法复制桌面运行时 {} -> {}",
            source.display(),
            temporary.display()
        )
    })?;

    if destination.exists() {
        fs::rename(destination, &backup)
            .with_context(|| format!("无法备份现有运行时 {}", destination.display()))?;
    }
    if let Err(error) = fs::rename(&temporary, destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        let _ = fs::remove_file(&temporary);
        return Err(error).context("无法启用新的 CodexBot 本地运行时");
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn sync_runtime_binary(store: &Store) -> Result<bool> {
    let source = env::current_exe().context("无法定位 CodexBot 桌面程序")?;
    let destination = installed_executable();
    if source == destination || (destination.is_file() && files_equal(&source, &destination)?) {
        store.set_setting(RUNTIME_VERSION_SETTING, codexbot::VERSION)?;
        return Ok(false);
    }

    if store
        .get_daemon_info()?
        .is_some_and(|(pid, created)| process_matches(pid, created))
    {
        stop_bridge_blocking()?;
    }
    replace_runtime_binary(&source, &destination)?;
    store.set_setting(RUNTIME_VERSION_SETTING, codexbot::VERSION)?;
    Ok(true)
}

fn materialize_embedded_plugin(root: &Path) -> Result<()> {
    let plugin = root.join("plugin").join("codexbot");
    let manifest = plugin.join(".codex-plugin").join("plugin.json");
    let hooks = plugin.join("hooks").join("hooks.json");
    let entry = plugin.join("hooks").join("entry.cmd");
    for path in [&manifest, &hooks, &entry] {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| anyhow!("嵌入式插件路径缺少父目录"))?,
        )?;
    }
    fs::write(manifest, PLUGIN_MANIFEST)?;
    fs::write(hooks, PLUGIN_HOOKS)?;
    fs::write(entry, PLUGIN_ENTRY)?;
    Ok(())
}

fn install_integration_blocking() -> Result<ActionResult> {
    ensure_data_dir()?;
    let store = Store::new(database_path())?;
    let runtime_updated = sync_runtime_binary(&store)?;
    let stage_root = data_dir().join(format!("integration-stage-{}", std::process::id()));
    if stage_root.exists() {
        fs::remove_dir_all(&stage_root)?;
    }
    materialize_embedded_plugin(&stage_root)?;
    let install_result = install_personal_plugin(
        &stage_root,
        None,
        true,
        Store::permission_notifications_enabled(),
    );
    let _ = fs::remove_dir_all(&stage_root);
    let install_result = install_result?;

    Ok(ActionResult {
        message: format!(
            "Codex 集成已安装{}；插件位置：{}",
            if runtime_updated {
                "并更新本地运行时"
            } else {
                ""
            },
            install_result.plugin_path.display()
        ),
    })
}

fn dashboard_state_blocking() -> Result<DashboardState> {
    ensure_data_dir()?;
    let store = Store::new(database_path())?;
    let credentials = load_credentials()?;
    let app_id_hint = credentials
        .as_ref()
        .map(|credentials| mask_app_id(&credentials.app_id));
    let daemon_info = store.get_daemon_info()?;
    let daemon_running = daemon_info.is_some_and(|(pid, created)| process_matches(pid, created));
    let sessions = store.get_sessions_for_status()?;
    let codex_running = sessions.iter().any(|session| {
        matches!(
            session.status.as_str(),
            "running" | "awaiting_approval" | "idle"
        )
    }) || store
        .list_hosts()?
        .iter()
        .any(|host| process_matches(host.pid, host.create_time))
        || discover_running_codex_host().is_some();
    let (pairing_active, pairing_expires_at) = store.pairing_status()?;
    let replies = store
        .get_last_replies(None, None, Some(8))?
        .into_iter()
        .map(|reply| ReplySummary {
            reply_id: reply.reply_id,
            project: reply.project,
            model: reply.model,
            preview: prompt_preview(&reply.content, 140),
            created_at: reply.created_at,
        })
        .collect();
    let (runtime_installed, plugin_installed) = integration_status();

    Ok(DashboardState {
        version: codexbot::VERSION,
        integration_ready: runtime_installed && plugin_installed,
        runtime_installed,
        plugin_installed,
        credentials_configured: credentials.is_some(),
        app_id_hint,
        qq_bound: store.get_bound_openid()?.is_some(),
        daemon_running,
        daemon_pid: daemon_info.filter(|_| daemon_running).map(|value| value.0),
        standalone: daemon_running
            && store.get_setting(STANDALONE_SETTING)?.as_deref() == Some("1"),
        codex_running,
        muted: store.is_muted()?,
        permission_notifications: Store::permission_notifications_enabled(),
        queue_pending: store.has_pending_outbox()?,
        pairing_active,
        pairing_expires_at,
        sessions: sessions
            .into_iter()
            .map(|session| SessionSummary {
                project: session.project,
                model: session.model,
                status: session.status,
                prompt_preview: session.prompt_preview,
                updated_at: session.updated_at,
            })
            .collect(),
        recent_replies: replies,
    })
}

fn start_bridge_blocking() -> Result<ActionResult> {
    ensure_data_dir()?;
    if load_credentials()?.is_none() {
        bail!("请先配置 QQ 机器人凭据");
    }
    let store = Store::new(database_path())?;
    let (runtime_installed, plugin_installed) = integration_status();
    if !runtime_installed || !plugin_installed {
        bail!("请先安装 Codex 集成");
    }
    let launched = ensure_daemon(&store, true)?;
    store.set_setting(STANDALONE_SETTING, "1")?;
    let pid = store.get_daemon_info()?.map(|value| value.0);
    Ok(ActionResult {
        message: if launched {
            format!(
                "桥接服务已启动{}",
                pid.map(|value| format!("（PID {value}）"))
                    .unwrap_or_default()
            )
        } else {
            "桥接服务已在运行，并已切换为常驻模式".to_owned()
        },
    })
}

fn stop_bridge_blocking() -> Result<ActionResult> {
    ensure_data_dir()?;
    let store = Store::new(database_path())?;
    let Some((pid, created)) = store.get_daemon_info()? else {
        store.delete_settings([STANDALONE_SETTING])?;
        return Ok(ActionResult {
            message: "桥接服务当前未运行".to_owned(),
        });
    };
    if !process_matches(pid, created) {
        store.clear_daemon_info(pid)?;
        store.delete_settings([STANDALONE_SETTING])?;
        return Ok(ActionResult {
            message: "已清理失效的桥接进程记录".to_owned(),
        });
    }
    if pid == std::process::id() {
        bail!("安全检查阻止了停止当前桌面进程");
    }

    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    if system
        .process(Pid::from_u32(pid))
        .is_some_and(|process| !process.kill())
    {
        bail!("无法停止 CodexBot 桥接进程（PID {pid}）");
    }
    for _ in 0..50 {
        if !process_matches(pid, created) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if process_matches(pid, created) {
        bail!("桥接进程（PID {pid}）未在预期时间内退出");
    }
    store.clear_daemon_info(pid)?;
    store.delete_settings([STANDALONE_SETTING])?;
    Ok(ActionResult {
        message: "桥接服务已停止；Codex Hooks 会在需要时重新唤起它".to_owned(),
    })
}

fn create_pairing_code_blocking() -> Result<PairingCode> {
    ensure_data_dir()?;
    if load_credentials()?.is_none() {
        bail!("请先配置 QQ 机器人凭据");
    }
    let store = Store::new(database_path())?;
    ensure_daemon(&store, true)?;
    store.set_setting(STANDALONE_SETTING, "1")?;
    let code = generate_pairing_code();
    let expires_at = now_seconds() + 30.0 * 60.0;
    store.create_pairing(&code, expires_at)?;
    Ok(PairingCode { code, expires_at })
}

#[tauri::command]
async fn get_dashboard_state() -> Result<DashboardState, String> {
    tokio::task::spawn_blocking(dashboard_state_blocking)
        .await
        .map_err(safe_error)?
        .map_err(safe_error)
}

#[tauri::command]
async fn install_integration() -> Result<ActionResult, String> {
    tokio::task::spawn_blocking(install_integration_blocking)
        .await
        .map_err(safe_error)?
        .map_err(safe_error)
}

#[tauri::command]
async fn save_qq_credentials(mut credentials: CredentialsInput) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let app_id = credentials.app_id.trim();
        let app_secret = credentials.app_secret.trim();
        let result = if app_id.is_empty() || app_secret.is_empty() {
            Err(anyhow!("AppID 和 AppSecret 均不能为空"))
        } else if app_id.chars().count() > 128 || app_secret.chars().count() > 512 {
            Err(anyhow!("QQ 凭据长度超出允许范围"))
        } else {
            store_credentials(app_id, app_secret).map_err(anyhow::Error::from)
        };
        credentials.app_secret.zeroize();
        result
    })
    .await
    .map_err(safe_error)?
    .map_err(safe_error)
}

#[tauri::command]
async fn start_bridge() -> Result<ActionResult, String> {
    tokio::task::spawn_blocking(start_bridge_blocking)
        .await
        .map_err(safe_error)?
        .map_err(safe_error)
}

#[tauri::command]
async fn stop_bridge() -> Result<ActionResult, String> {
    tokio::task::spawn_blocking(stop_bridge_blocking)
        .await
        .map_err(safe_error)?
        .map_err(safe_error)
}

#[tauri::command]
async fn create_pairing_code() -> Result<PairingCode, String> {
    tokio::task::spawn_blocking(create_pairing_code_blocking)
        .await
        .map_err(safe_error)?
        .map_err(safe_error)
}

#[tauri::command]
async fn set_notifications_muted(muted: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        ensure_data_dir()?;
        Store::new(database_path())?.set_muted(muted)?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(safe_error)?
    .map_err(safe_error)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_dashboard_state,
            install_integration,
            save_qq_credentials,
            start_bridge,
            stop_bridge,
            create_pairing_code,
            set_notifications_muted,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodexBot desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_mask_keeps_only_a_small_hint() {
        assert_eq!(mask_app_id("10234567391"), "102••••7391");
        assert_eq!(mask_app_id("1234"), "1••4");
    }

    #[test]
    fn errors_are_redacted_and_single_line() {
        let output = safe_error("failed\napp_secret=very-secret-value");
        assert!(!output.contains("very-secret-value"));
        assert!(!output.contains('\n'));
    }

    #[test]
    fn embedded_plugin_tree_is_valid() {
        let directory = tempfile::tempdir().unwrap();
        materialize_embedded_plugin(directory.path()).unwrap();
        codexbot::installer::validate_plugin_tree(
            &directory.path().join("plugin").join("codexbot"),
        )
        .unwrap();
    }
}
