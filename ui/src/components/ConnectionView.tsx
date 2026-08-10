import { useEffect, useState } from "react";
import type { DashboardState, PairingCode } from "../types";
import { Icon } from "./Icon";
import { StatusBadge } from "./StatusBadge";

interface ConnectionViewProps {
  state: DashboardState | null;
  pairing: PairingCode | null;
  pendingAction: string | null;
  onInstallIntegration: () => Promise<unknown>;
  onOpenCredentials: () => void;
  onToggleBridge: () => Promise<void>;
  onGeneratePair: () => Promise<PairingCode | null>;
  onRefresh: () => Promise<void>;
}

interface DiagnosticProps {
  label: string;
  detail: string;
  ready: boolean;
}

function Diagnostic({ label, detail, ready }: DiagnosticProps) {
  return (
    <div className={`diagnostic-item ${ready ? "is-ready" : "is-pending"}`}>
      <span aria-hidden="true" />
      <div><strong>{label}</strong><small>{detail}</small></div>
      <em>{ready ? "正常" : "待处理"}</em>
    </div>
  );
}

interface PairCodeBlockProps {
  state: DashboardState | null;
  pairing: PairingCode | null;
}

function PairCodeBlock({ state, pairing }: PairCodeBlockProps) {
  const [secondsLeft, setSecondsLeft] = useState(0);

  useEffect(() => {
    if (!pairing) {
      setSecondsLeft(0);
      return;
    }
    const update = () => setSecondsLeft(Math.max(0, Math.floor(pairing.expires_at - Date.now() / 1000)));
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [pairing]);

  const copyCode = async () => {
    if (!pairing || secondsLeft <= 0) return;
    try {
      await navigator.clipboard.writeText(`/bind ${pairing.code}`);
    } catch {
      // The command remains visible for manual copying when clipboard access is unavailable.
    }
  };

  if (pairing && secondsLeft > 0) {
    const minutes = Math.floor(secondsLeft / 60);
    const seconds = String(secondsLeft % 60).padStart(2, "0");
    return (
      <div className="pair-code-block active">
        <small>在 QQ 私聊机器人发送，点击命令可复制</small>
        <button type="button" onClick={() => void copyCode()} aria-label="复制 QQ 配对命令">
          /bind {pairing.code} <Icon name="copy" />
        </button>
        <span>有效期剩余 {minutes}:{seconds}</span>
      </div>
    );
  }

  if (state?.qq_bound) {
    return (
      <div className="pair-code-block bound">
        <Icon name="check" />
        <strong>接收账号已安全绑定</strong>
        <span>重新生成配对码可换绑账号</span>
      </div>
    );
  }

  if (state?.pairing_active) {
    return (
      <div className="pair-code-block">
        <strong>已有配对码仍在有效期内</strong>
        <span>为保护安全，重开窗口后不再显示原代码</span>
      </div>
    );
  }

  return (
    <div className="pair-code-block">
      <strong>••••-••••</strong>
      <span>代码有效期为 30 分钟，仅可使用一次</span>
    </div>
  );
}

export function ConnectionView({
  state,
  pairing,
  pendingAction,
  onInstallIntegration,
  onOpenCredentials,
  onToggleBridge,
  onGeneratePair,
  onRefresh,
}: ConnectionViewProps) {
  const integrationReady = state?.integration_ready === true;
  const credentialsReady = state?.credentials_configured === true;
  const bridgeReady = state?.daemon_running === true;
  const qqReady = state?.qq_bound === true;

  return (
    <section className="view-stack" aria-label="连接设置">
      <header className="page-intro">
        <div>
          <p className="section-kicker">CONNECTION</p>
          <h2>把两端接起来</h2>
          <p>凭据只写入 Windows Credential Manager；桌面界面不会显示、记录或回传 AppSecret。</p>
        </div>
        <span className="privacy-chip"><Icon name="shield" /> 本机安全存储</span>
      </header>

      <article className={`panel integration-panel ${integrationReady ? "is-ready" : ""}`}>
        <span className="integration-mark"><Icon name={integrationReady ? "check" : "link"} /></span>
        <div className="integration-copy">
          <p className="section-kicker">WINDOWS INTEGRATION</p>
          <h3>{integrationReady ? "Codex 集成已就绪" : "先安装本机运行时与 Codex Hooks"}</h3>
          <p>
            桌面软件会把签入的运行时安装到当前用户目录，并注册随软件内置的 CodexBot 插件。
            运行时：{state?.runtime_installed ? "已部署" : "待部署"} · 插件：{state?.plugin_installed ? "已安装" : "待安装"}
          </p>
        </div>
        <button className={integrationReady ? "secondary-button" : "primary-button"} type="button" onClick={() => void onInstallIntegration()} disabled={pendingAction === "integration"}>
          {pendingAction === "integration" ? "正在安装…" : integrationReady ? "重新安装 / 修复" : "安装 Codex 集成"}
        </button>
      </article>

      <div className="connection-grid">
        <article className="panel connection-card">
          <div className="connection-card-top">
            <span className="connection-icon qq"><Icon name="qq" /></span>
            <StatusBadge ready={credentialsReady} readyLabel="已配置" pendingLabel="待配置" />
          </div>
          <p className="section-kicker">第一步</p>
          <h3>QQ 机器人凭据</h3>
          <p>使用 QQ 开放平台沙箱应用的 AppID 和 AppSecret。密钥不会进入 SQLite 或日志。</p>
          <div className="value-row"><span>AppID</span><strong>{state?.app_id_hint ?? "尚未配置"}</strong></div>
          <button className="secondary-button full" type="button" onClick={onOpenCredentials}>
            {credentialsReady ? "更新机器人凭据" : "配置机器人凭据"}
          </button>
        </article>

        <article className="panel connection-card">
          <div className="connection-card-top">
            <span className="connection-icon bridge"><Icon name="logo" /></span>
            <StatusBadge ready={bridgeReady} readyLabel="运行中" pendingLabel="未启动" />
          </div>
          <p className="section-kicker">第二步</p>
          <h3>本机桥接服务</h3>
          <p>常驻服务接收 Codex Hooks，将事件放入可靠队列，再投递到 QQ 官方沙箱。</p>
          <div className="value-row"><span>运行状态</span><strong>{bridgeReady ? `PID ${state?.daemon_pid ?? "—"} · ${state?.standalone ? "常驻" : "伴随"}` : "当前未运行"}</strong></div>
          <button className={`secondary-button full ${bridgeReady ? "danger" : ""}`} type="button" onClick={() => void onToggleBridge()} disabled={!integrationReady || !credentialsReady || pendingAction === "start" || pendingAction === "stop"}>
            {pendingAction === "start" ? "正在启动…" : pendingAction === "stop" ? "正在停止…" : bridgeReady ? "停止常驻服务" : "启动桥接服务"}
          </button>
        </article>

        <article className="panel connection-card pair-card">
          <div className="connection-card-top">
            <span className="connection-icon user"><Icon name="user" /></span>
            <StatusBadge ready={qqReady} readyLabel="已绑定" pendingLabel={state?.pairing_active ? "等待绑定" : "未绑定"} />
          </div>
          <p className="section-kicker">第三步</p>
          <h3>绑定接收账号</h3>
          <p>生成一次性代码，然后在 QQ 私聊机器人完成绑定。当前只允许一个接收账号。</p>
          <PairCodeBlock state={state} pairing={pairing} />
          <button className="primary-button full" type="button" onClick={() => void onGeneratePair()} disabled={!integrationReady || !credentialsReady || !bridgeReady || pendingAction === "pair"}>
            {pendingAction === "pair" ? "正在生成…" : qqReady ? "重新生成配对码" : "生成配对码"}
          </button>
        </article>
      </div>

      <article className="panel diagnostics-panel">
        <header className="panel-header">
          <div><p className="section-kicker">诊断</p><h3>连接链路</h3></div>
          <button className="text-button compact" type="button" onClick={() => void onRefresh()}>重新检查</button>
        </header>
        <div className="diagnostic-grid">
          <Diagnostic label="Codex 本机集成" ready={integrationReady} detail={integrationReady ? "本地运行时与 Hooks 插件已安装" : "点击上方按钮完成安装"} />
          <Diagnostic label="QQ 沙箱凭据" ready={credentialsReady} detail={credentialsReady ? `AppID ${state?.app_id_hint ?? "已保存"}` : "需要 AppID 与 AppSecret"} />
          <Diagnostic label="CodexBot 桥接服务" ready={bridgeReady} detail={bridgeReady ? `本机进程 PID ${state?.daemon_pid ?? "—"}` : "桥接服务尚未启动"} />
          <Diagnostic label="QQ 接收账号" ready={qqReady} detail={qqReady ? "唯一接收账号已绑定" : "需要发送一次性配对命令"} />
        </div>
      </article>
    </section>
  );
}
