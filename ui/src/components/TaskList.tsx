import { relativeTime, statusLabel } from "../lib/activity";
import type { ActivityItem } from "../types";
import { Icon, type IconName } from "./Icon";

interface TaskListProps {
  activities: ActivityItem[];
  limit?: number;
  emptyTitle?: string;
}

export function TaskList({ activities, limit, emptyTitle = "还没有 Codex 活动" }: TaskListProps) {
  const visible = limit ? activities.slice(0, limit) : activities;

  if (visible.length === 0) {
    return (
      <div className="empty-state">
        <Icon name="code" />
        <strong>{emptyTitle}</strong>
        <span>开始一个 Codex 任务后，这里会显示脱敏后的状态摘要。</span>
      </div>
    );
  }

  return (
    <div className="task-list">
      {visible.map((activity) => {
        const icon: IconName = activity.status === "completed" ? "check" : activity.status === "awaiting_approval" ? "alert" : activity.status === "idle" ? "pause" : "refresh";
        return (
          <article className="task-item" key={activity.id}>
            <span className={`task-icon status-${activity.status}`}><Icon name={icon} /></span>
            <div className="task-copy">
              <div className="task-heading"><strong>{activity.project}</strong><span>{activity.model}</span></div>
              <p>{statusLabel(activity.status)} · {activity.summary}</p>
            </div>
            <time dateTime={new Date(activity.timestamp * 1000).toISOString()}>{relativeTime(activity.timestamp)}</time>
          </article>
        );
      })}
    </div>
  );
}
