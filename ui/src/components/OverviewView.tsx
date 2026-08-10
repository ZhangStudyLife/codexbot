import type {
  ActivityItem,
  DashboardState,
  PairingCode,
  ViewName,
} from "../types";
import { Icon, type IconName } from "./Icon";
import { TaskList } from "./TaskList";

interface OverviewViewProps {
  state: DashboardState | null;
  activities: ActivityItem[];
  pendingAction: string | null;
  onNavigate: (view: ViewName) => void;
  onInstallIntegration: () => Promise<unknown>;
  onOpenCredentials: () => void;
  onToggleBridge: () => Promise<void>;
  onGeneratePair: () => Promise<PairingCode | null>;
}

interface RouteNodeProps {
  icon: IconName;
  label: string;
  detail: string;
  online: boolean;
  accent: string;
}

function RouteNode({ icon, label, detail, online, accent }: RouteNodeProps) {
  return (
    <div className={`route-node ${online ? "is-online" : ""}`}>
      <span className={`route-logo ${accent}`}><Icon name={icon} /></span>
      <strong>{label}</strong>
      <small>{detail}</small>
    </div>
  );
}

interface MetricCardProps {
  icon: IconName;
  accent: string;
  label: string;
  value: string;
  detail: string;
}

function MetricCard({ icon, accent, label, value, detail }: MetricCardProps) {
  return (
    <article className="metric-card">
      <span className={`metric-icon ${accent}`}><Icon name={icon} /></span>
      <div>
        <small>{label}</small>
        <strong>{value}</strong>
        <p>{detail}</p>
      </div>
    </article>
  );
}

export function OverviewView({
  state,
  activities,
  pendingAction,
  onNavigate,
  onInstallIntegration,
  onOpenCredentials,
  onToggleBridge,
  onGeneratePair,
}: OverviewViewProps) {
  const integrationReady = state?.integration_ready === true;
  const credentialsReady = state?.credentials_configured === true;
  const bridgeReady = state?.daemon_running === true;
  const qqReady = state?.qq_bound === true;
  const connectionReady = integrationReady && credentialsReady && bridgeReady && qqReady;
  const completedSteps = [integrationReady, credentialsReady, bridgeReady, qqReady].filter(Boolean).length;

  const handlePrimaryAction = async () => {
    if (!integrationReady) {
      await onInstallIntegration();
      return;
    }
    if (!credentialsReady) {
      onOpenCredentials();
      return;
    }
    if (!bridgeReady) {
      await onToggleBridge();
      return;
    }
    if (!qqReady) {
      await onGeneratePair();
      onNavigate("connect");
      return;
    }
    onNavigate("activity");
  };

  const primaryLabel = !state
    ? "正在读取状态"
    : !integrationReady
      ? "安装 Codex 集成"
      : !credentialsReady
        ? "开始连接"
        : !bridgeReady
          ? "启动桥接服务"
          : !qqReady
            ? "生成 QQ 配对码"
            : "查看最近任务";

  const handleSetupAction = async (step: "integration" | "credentials" | "bridge" | "qq") => {
    if (step === "integration") {
      await onInstallIntegration();
    } else if (step === "credentials") {
      onOpenCredentials();
    } else if (step === "bridge") {
      await onToggleBridge();
    } else {
      await onGeneratePair();
      onNavigate("connect");
    }
  };

  return (
    <section className="view-stack" aria-label="连接总览">
      <article className="hero-card">
        <div className="hero-copy">
          <p className="section-kicker">本机连接中枢</p>
          <h2>离开屏幕，<br />也不错过 Codex 的关键时刻。</h2>
          <p>
            CodexBot 只把任务开始、需要关注和完成结果送到你的 QQ。批准与操作仍留在 Codex，边界清晰，也更安心。
          </p>
          <div className="hero-actions">
            <button className="primary-button" type="button" onClick={() => void handlePrimaryAction()} disabled={!state || pendingAction !== null}>
              <Icon name="chevron" />
              {pendingAction ? "处理中…" : primaryLabel}
            </button>
            <button className="text-button" type="button" onClick={() => onNavigate("connect")}>查看连接设置 <span>→</span></button>
          </div>
        </div>

        <div className="route-panel" aria-label="Codex 到 QQ 的连接链路">
          <div className="route-grid">
            <RouteNode icon="code" label="Codex" detail={state?.codex_running ? "已检测到" : "等待 Codex"} online={state?.codex_running === true} accent="codex" />
            <span className={`route-line ${state?.codex_running && bridgeReady ? "is-online" : ""}`}><i /><i /><i /></span>
            <div className={`bridge-core ${bridgeReady ? "is-online" : ""}`}>
              <span aria-hidden="true" />
              <Icon name="logo" />
              <small>本机桥接</small>
            </div>
            <span className={`route-line ${bridgeReady && qqReady ? "is-online" : ""}`}><i /><i /><i /></span>
            <RouteNode icon="qq" label="QQ" detail={qqReady ? "账号已绑定" : "等待配对"} online={qqReady} accent="qq" />
          </div>
          <p>{connectionReady ? "链路已就绪，关键状态将自动送达 QQ" : "完成下方步骤即可打通通知链路"}</p>
        </div>
      </article>

      <div className="metric-grid">
        <MetricCard icon="logo" accent="blue" label="桥接服务" value={bridgeReady ? "在线" : "未运行"} detail={bridgeReady ? `PID ${state?.daemon_pid ?? "—"} · ${state?.standalone ? "常驻" : "伴随"}` : "启动后保持 QQ 在线"} />
        <MetricCard icon="bell" accent="cyan" label="主动通知" value={state?.muted ? "已静音" : "已开启"} detail={qqReady ? "QQ 接收账号已绑定" : "完成配对后开始推送"} />
        <MetricCard icon="queue" accent="amber" label="投递队列" value={state?.queue_pending ? "等待投递" : "队列清空"} detail={state?.queue_pending ? "后台将自动重试" : "没有待发送事件"} />
      </div>

      <div className="overview-grid">
        <article className="panel recent-panel">
          <header className="panel-header">
            <div><p className="section-kicker">最近活动</p><h3>Codex 任务</h3></div>
            <button className="text-button compact" type="button" onClick={() => onNavigate("activity")}>查看全部 <span>→</span></button>
          </header>
          <TaskList activities={activities} limit={3} />
        </article>

        <article className="panel setup-panel">
          <header className="panel-header">
            <div><p className="section-kicker">连接进度</p><h3>四步开始接收通知</h3></div>
            <span className="progress-count">{completedSteps} / 4</span>
          </header>
          <ol className="setup-steps">
            <li className={integrationReady ? "is-complete" : ""}>
              <span>{integrationReady ? <Icon name="check" /> : "1"}</span>
              <div><strong>安装 Codex 集成</strong><small>部署本地运行时与 Hooks 插件</small></div>
              <button type="button" onClick={() => void handleSetupAction("integration")} disabled={integrationReady || pendingAction === "integration"}>{integrationReady ? "完成" : pendingAction === "integration" ? "安装中" : "安装"}</button>
            </li>
            <li className={credentialsReady ? "is-complete" : ""}>
              <span>{credentialsReady ? <Icon name="check" /> : "2"}</span>
              <div><strong>配置 QQ 机器人</strong><small>凭据安全保存到 Windows</small></div>
              <button type="button" onClick={() => void handleSetupAction("credentials")} disabled={credentialsReady}>{credentialsReady ? "完成" : "配置"}</button>
            </li>
            <li className={bridgeReady ? "is-complete" : ""}>
              <span>{bridgeReady ? <Icon name="check" /> : "3"}</span>
              <div><strong>启动本机桥接</strong><small>连接 QQ 沙箱 Gateway</small></div>
              <button type="button" onClick={() => void handleSetupAction("bridge")} disabled={bridgeReady || !integrationReady || !credentialsReady}>{bridgeReady ? "完成" : "启动"}</button>
            </li>
            <li className={qqReady ? "is-complete" : ""}>
              <span>{qqReady ? <Icon name="check" /> : "4"}</span>
              <div><strong>绑定你的 QQ</strong><small>使用 30 分钟一次性配对码</small></div>
              <button type="button" onClick={() => void handleSetupAction("qq")} disabled={qqReady || !bridgeReady}>{qqReady ? "完成" : "配对"}</button>
            </li>
          </ol>
        </article>
      </div>
    </section>
  );
}
