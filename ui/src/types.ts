export type ViewName = "overview" | "connect" | "notifications" | "activity";

export type SessionStatus =
  | "running"
  | "awaiting_approval"
  | "idle"
  | "completed";

export interface SessionSummary {
  project: string;
  model: string;
  status: SessionStatus;
  prompt_preview: string | null;
  updated_at: number;
}

export interface ReplySummary {
  reply_id: number;
  project: string;
  model: string;
  preview: string;
  created_at: number;
}

export interface DashboardState {
  version: string;
  integration_ready: boolean;
  runtime_installed: boolean;
  plugin_installed: boolean;
  credentials_configured: boolean;
  app_id_hint: string | null;
  qq_bound: boolean;
  daemon_running: boolean;
  daemon_pid: number | null;
  standalone: boolean;
  codex_running: boolean;
  muted: boolean;
  permission_notifications: boolean;
  queue_pending: boolean;
  pairing_active: boolean;
  pairing_expires_at: number | null;
  sessions: SessionSummary[];
  recent_replies: ReplySummary[];
}

export interface PairingCode {
  code: string;
  expires_at: number;
}

export interface ActionResult {
  message: string;
}

export interface CredentialsInput {
  app_id: string;
  app_secret: string;
}

export interface ActivityItem {
  id: string;
  project: string;
  model: string;
  summary: string;
  status: SessionStatus;
  timestamp: number;
}

export interface ToastMessage {
  id: number;
  message: string;
  error: boolean;
}
