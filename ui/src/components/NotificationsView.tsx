import type { ReactNode } from "react";
import type { DashboardState } from "../types";
import { Icon, type IconName } from "./Icon";

interface NotificationsViewProps {
  state: DashboardState | null;
  busy: boolean;
  onSetMuted: (muted: boolean) => Promise<boolean>;
}

interface NotificationSettingProps {
  icon: IconName;
  accent: string;
  title: string;
  description: string;
  control: ReactNode;
}

function NotificationSetting({ icon, accent, title, description, control }: NotificationSettingProps) {
  return (
    <div className="notification-setting">
      <span className={`setting-icon ${accent}`}><Icon name={icon} /></span>
      <div><strong>{title}</strong><p>{description}</p></div>
      {control}
    </div>
  );
}

export function NotificationsView({ state, busy, onSetMuted }: NotificationsViewProps) {
  const active = state?.muted !== true;

  return (
    <section className="view-stack" aria-label="通知设置">
      <header className="page-intro">
        <div>
          <p className="section-kicker">NOTIFICATIONS</p>
          <h2>只推送真正重要的时刻</h2>
          <p>默认策略保持安静：子智能体和高频工具事件留在本机，QQ 只接收主任务节点。</p>
        </div>
      </header>

      <div className="settings-grid">
        <article className="panel notification-settings">
          <NotificationSetting
            icon="bell"
            accent="blue"
            title="主动通知"
            description="暂停后仍记录任务状态，但不会补发静音期间的旧消息。"
            control={
              <label className="toggle-control">
                <span>{active ? "已开启" : "已暂停"}</span>
                <input type="checkbox" checked={active} disabled={busy || !state} onChange={(event) => void onSetMuted(!event.target.checked)} />
                <i aria-hidden="true" />
              </label>
            }
          />
          <NotificationSetting
            icon="play"
            accent="cyan"
            title="任务开始"
            description="发送项目、模型、时间以及脱敏后的提示词摘要。"
            control={<span className="fixed-state">始终开启</span>}
          />
          <NotificationSetting
            icon="check"
            accent="violet"
            title="任务完成"
            description="发送完整最终回复；内容过长时自动安全分段。"
            control={<span className="fixed-state">始终开启</span>}
          />
          <NotificationSetting
            icon="alert"
            accent="amber"
            title="权限请求提醒"
            description="只做提醒，不能从 QQ 批准或拒绝；需用环境变量显式开启。"
            control={<span className="fixed-state">{state?.permission_notifications ? "已开启" : "默认关闭"}</span>}
          />
        </article>

        <aside className="panel boundary-panel">
          <span className="boundary-icon"><Icon name="shield" /></span>
          <p className="section-kicker">安全边界</p>
          <h3>QQ 是通知端，<br />不是远程终端</h3>
          <p>机器人不会执行任意命令，也不能代替你批准权限请求或提交新提示词。</p>
          <ul>
            <li><span><Icon name="check" /></span>可查询状态与最近回复</li>
            <li><span><Icon name="check" /></span>可管理已保存的 Codex 账号</li>
            <li className="blocked"><span><Icon name="close" /></span>不可运行任意本机命令</li>
          </ul>
        </aside>
      </div>
    </section>
  );
}
