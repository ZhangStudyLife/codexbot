import type { ActivityItem, DashboardState, SessionStatus } from "../types";

export function buildActivities(state: DashboardState | null): ActivityItem[] {
  if (!state) return [];

  const activities: ActivityItem[] = [];
  const fingerprints = new Set<string>();

  for (const session of state.sessions) {
    const item: ActivityItem = {
      id: `session-${session.project}-${session.updated_at}`,
      project: session.project || "未命名项目",
      model: session.model || "Codex",
      summary: session.prompt_preview || "Codex 会话状态已更新",
      status: session.status,
      timestamp: session.updated_at,
    };
    fingerprints.add(`${item.project}|${item.status}|${Math.round(item.timestamp)}`);
    activities.push(item);
  }

  for (const reply of state.recent_replies) {
    const fingerprint = `${reply.project}|completed|${Math.round(reply.created_at)}`;
    if (fingerprints.has(fingerprint)) continue;
    fingerprints.add(fingerprint);
    activities.push({
      id: `reply-${reply.reply_id}`,
      project: reply.project || "未命名项目",
      model: reply.model || "Codex",
      summary: reply.preview || "Codex 已完成任务",
      status: "completed",
      timestamp: reply.created_at,
    });
  }

  return activities.sort((left, right) => right.timestamp - left.timestamp);
}

export function relativeTime(timestamp: number): string {
  const seconds = Math.max(0, Math.round(Date.now() / 1000 - timestamp));
  if (seconds < 50) return "刚刚";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  return `${Math.floor(seconds / 86400)} 天前`;
}

export function statusLabel(status: SessionStatus): string {
  switch (status) {
    case "running": return "进行中";
    case "awaiting_approval": return "等待确认";
    case "idle": return "已暂停";
    case "completed": return "已完成";
  }
}
