import { useState } from "react";
import type { ActivityItem, SessionStatus } from "../types";
import { Icon } from "./Icon";
import { TaskList } from "./TaskList";

type ActivityFilter = "all" | "running" | "completed";

interface ActivityViewProps {
  activities: ActivityItem[];
  onRefresh: () => Promise<void>;
}

function matchesFilter(status: SessionStatus, filter: ActivityFilter): boolean {
  if (filter === "all") return true;
  if (filter === "completed") return status === "completed";
  return status === "running" || status === "awaiting_approval" || status === "idle";
}

export function ActivityView({ activities, onRefresh }: ActivityViewProps) {
  const [filter, setFilter] = useState<ActivityFilter>("all");
  const visible = activities.filter((activity) => matchesFilter(activity.status, filter));

  return (
    <section className="view-stack" aria-label="活动记录">
      <header className="page-intro">
        <div>
          <p className="section-kicker">ACTIVITY</p>
          <h2>最近的 Codex 动态</h2>
          <p>这里只显示脱敏摘要。完整回复仍按隐私策略保存在本机数据库中。</p>
        </div>
        <button className="secondary-button" type="button" onClick={() => void onRefresh()}><Icon name="refresh" /> 刷新</button>
      </header>

      <article className="panel activity-panel">
        <header className="activity-toolbar">
          <div className="segmented-control" role="group" aria-label="活动筛选">
            {(["all", "running", "completed"] as const).map((value) => (
              <button className={filter === value ? "is-active" : ""} type="button" aria-pressed={filter === value} key={value} onClick={() => setFilter(value)}>
                {value === "all" ? "全部" : value === "running" ? "进行中" : "已完成"}
              </button>
            ))}
          </div>
          <span>{visible.length} 条活动</span>
        </header>
        <TaskList activities={visible} emptyTitle="这个筛选下还没有活动" />
      </article>
    </section>
  );
}
