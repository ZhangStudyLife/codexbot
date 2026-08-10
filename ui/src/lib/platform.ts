import type {
  ActionResult,
  CredentialsInput,
  DashboardState,
  PairingCode,
} from "../types";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

const now = Math.floor(Date.now() / 1000);

let mockState: DashboardState = {
  version: "0.1.0-preview",
  integration_ready: true,
  runtime_installed: true,
  plugin_installed: true,
  credentials_configured: true,
  app_id_hint: "100••••0001",
  qq_bound: true,
  daemon_running: true,
  daemon_pid: 4242,
  standalone: true,
  codex_running: true,
  muted: false,
  permission_notifications: false,
  queue_pending: false,
  pairing_active: false,
  pairing_expires_at: null,
  sessions: [
    {
      project: "codexbot",
      model: "gpt-5.6",
      status: "running",
      prompt_preview: "设计连接 Codex 与 QQ 的本机桌面控制台",
      updated_at: now - 68,
    },
    {
      project: "docs-site",
      model: "gpt-5.4",
      status: "awaiting_approval",
      prompt_preview: "更新知识笔记页面并运行构建检查",
      updated_at: now - 936,
    },
  ],
  recent_replies: [
    {
      reply_id: 12,
      project: "api-service",
      model: "gpt-5.4",
      preview: "已完成消息分段策略调整，全部测试通过。",
      created_at: now - 7280,
    },
    {
      reply_id: 11,
      project: "desktop-tool",
      model: "gpt-5.4",
      preview: "账号切换流程已更新并完成回归验证。",
      created_at: now - 18540,
    },
  ],
};

export function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    "__TAURI_INTERNALS__" in (window as TauriWindow)
  );
}

export async function invokeBackend<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauriRuntime()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }

  await new Promise((resolve) => window.setTimeout(resolve, 260));
  return invokePreview<T>(command, args);
}

function invokePreview<T>(
  command: string,
  args?: Record<string, unknown>,
): T {
  switch (command) {
    case "get_dashboard_state":
      return structuredClone(mockState) as T;
    case "install_integration":
      mockState = {
        ...mockState,
        integration_ready: true,
        runtime_installed: true,
        plugin_installed: true,
      };
      return { message: "Codex 集成已安装并更新本地运行时" } as T;
    case "save_qq_credentials": {
      const credentials = args?.credentials as CredentialsInput | undefined;
      const appId = credentials?.app_id.trim() ?? "";
      if (!appId || !credentials?.app_secret.trim()) {
        throw new Error("AppID 和 AppSecret 均不能为空");
      }
      mockState = {
        ...mockState,
        credentials_configured: true,
        app_id_hint: maskAppId(appId),
      };
      return undefined as T;
    }
    case "start_bridge":
      if (!mockState.credentials_configured) {
        throw new Error("请先配置 QQ 机器人凭据");
      }
      mockState = {
        ...mockState,
        daemon_running: true,
        daemon_pid: 4242,
        standalone: true,
      };
      return { message: "桥接服务已启动" } as T;
    case "stop_bridge":
      mockState = {
        ...mockState,
        daemon_running: false,
        daemon_pid: null,
        standalone: false,
      };
      return { message: "桥接服务已停止；Codex Hooks 会在需要时重新唤起它" } as T;
    case "create_pairing_code": {
      if (!mockState.credentials_configured) {
        throw new Error("请先配置 QQ 机器人凭据");
      }
      const expiresAt = Math.floor(Date.now() / 1000) + 30 * 60;
      mockState = {
        ...mockState,
        pairing_active: true,
        pairing_expires_at: expiresAt,
      };
      return { code: "Q7NK-4M2P", expires_at: expiresAt } as T;
    }
    case "set_notifications_muted":
      mockState = { ...mockState, muted: Boolean(args?.muted) };
      return undefined as T;
    default:
      throw new Error(`Unknown preview command: ${command}`);
  }
}

function maskAppId(appId: string): string {
  if (appId.length <= 4) {
    return `${appId.slice(0, 1)}••${appId.slice(-1)}`;
  }
  return `${appId.slice(0, 3)}••••${appId.slice(-4)}`;
}

export async function saveCredentials(
  credentials: CredentialsInput,
): Promise<void> {
  return invokeBackend<void>("save_qq_credentials", { credentials });
}

export async function installIntegration(): Promise<ActionResult> {
  return invokeBackend<ActionResult>("install_integration");
}

export async function startBridge(): Promise<ActionResult> {
  return invokeBackend<ActionResult>("start_bridge");
}

export async function stopBridge(): Promise<ActionResult> {
  return invokeBackend<ActionResult>("stop_bridge");
}

export async function createPairingCode(): Promise<PairingCode> {
  return invokeBackend<PairingCode>("create_pairing_code");
}

export async function setNotificationsMuted(muted: boolean): Promise<void> {
  return invokeBackend<void>("set_notifications_muted", { muted });
}
