import type { ViewName } from "../types";
import { Icon, type IconName } from "./Icon";

const navigation: Array<{
  view: ViewName;
  label: string;
  icon: IconName;
}> = [
  { view: "overview", label: "总览", icon: "overview" },
  { view: "connect", label: "连接", icon: "link" },
  { view: "notifications", label: "通知", icon: "bell" },
  { view: "activity", label: "活动", icon: "activity" },
];

interface SidebarProps {
  currentView: ViewName;
  connectionReady: boolean;
  version?: string;
  onNavigate: (view: ViewName) => void;
}

export function Sidebar({
  currentView,
  connectionReady,
  version,
  onNavigate,
}: SidebarProps) {
  return (
    <aside className="sidebar" aria-label="主导航">
      <button
        className="brand"
        type="button"
        onClick={() => onNavigate("overview")}
        aria-label="返回 CodexBot 总览"
      >
        <span className="brand-mark"><Icon name="logo" /></span>
        <span className="brand-copy">
          <strong>CodexBot</strong>
          <small>Local bridge</small>
        </span>
      </button>

      <nav className="nav-list">
        {navigation.map((item) => {
          const active = currentView === item.view;
          return (
            <button
              className={`nav-item ${active ? "is-active" : ""}`}
              type="button"
              key={item.view}
              onClick={() => onNavigate(item.view)}
              aria-current={active ? "page" : undefined}
            >
              <Icon name={item.icon} />
              <span>{item.label}</span>
              {item.view === "connect" ? (
                <i className={connectionReady ? "is-ready" : ""} aria-hidden="true" />
              ) : null}
            </button>
          );
        })}
      </nav>

      <div className="sidebar-foot">
        <div className="local-note">
          <span><Icon name="shield" /></span>
          <div>
            <strong>仅在本机运行</strong>
            <small>密钥与任务数据不会经过第三方服务</small>
          </div>
        </div>
        <p>CodexBot {version ?? "0.1.0"}</p>
      </div>
    </aside>
  );
}
