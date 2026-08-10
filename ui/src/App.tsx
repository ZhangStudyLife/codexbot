import { useCallback, useMemo, useState } from "react";
import { ActivityView } from "./components/ActivityView";
import { ConnectionView } from "./components/ConnectionView";
import { CredentialDialog } from "./components/CredentialDialog";
import { Icon } from "./components/Icon";
import { NotificationsView } from "./components/NotificationsView";
import { OverviewView } from "./components/OverviewView";
import { Sidebar } from "./components/Sidebar";
import { useDashboard } from "./hooks/useDashboard";
import { buildActivities } from "./lib/activity";
import type { ViewName } from "./types";

const viewTitles: Record<ViewName, string> = {
  overview: "连接中心",
  connect: "连接设置",
  notifications: "通知设置",
  activity: "活动记录",
};

export default function App() {
  const [currentView, setCurrentView] = useState<ViewName>("overview");
  const [credentialDialogOpen, setCredentialDialogOpen] = useState(false);
  const {
    state,
    loading,
    error,
    pendingAction,
    pairing,
    toast,
    dismissToast,
    refresh,
    setupIntegration,
    updateCredentials,
    toggleBridge,
    generatePairing,
    setMuted,
  } = useDashboard();
  const activities = useMemo(() => buildActivities(state), [state]);
  const connectionReady = Boolean(
    state?.integration_ready &&
      state.credentials_configured &&
      state.daemon_running &&
      state.qq_bound,
  );
  const closeCredentialDialog = useCallback(
    () => setCredentialDialogOpen(false),
    [],
  );

  return (
    <div className="app-shell">
      <div className="ambient ambient-one" aria-hidden="true" />
      <div className="ambient ambient-two" aria-hidden="true" />
      <Sidebar
        currentView={currentView}
        connectionReady={connectionReady}
        version={state?.version}
        onNavigate={setCurrentView}
      />

      <main className="main-content">
        <header className="topbar">
          <div>
            <p className="section-kicker">CODEX × QQ</p>
            <h1>{viewTitles[currentView]}</h1>
          </div>
          <div className="topbar-actions">
            <span>{error ? "状态读取失败" : state ? "本机状态实时同步" : "正在读取本机状态…"}</span>
            <button className={`icon-button ${loading ? "is-spinning" : ""}`} type="button" onClick={() => void refresh()} aria-label="刷新本机状态" title="刷新本机状态">
              <Icon name="refresh" />
            </button>
            <span className={`health-pill ${connectionReady ? "is-ready" : "is-pending"}`}>
              <i aria-hidden="true" />
              {connectionReady ? "连接正常" : !state?.integration_ready ? "需要安装" : state.credentials_configured ? "等待连接" : "需要配置"}
            </span>
          </div>
        </header>

        {error && !state ? (
          <div className="error-banner"><Icon name="alert" /><span>{error}</span><button type="button" onClick={() => void refresh()}>重试</button></div>
        ) : null}

        {currentView === "overview" ? (
          <OverviewView
            state={state}
            activities={activities}
            pendingAction={pendingAction}
            onNavigate={setCurrentView}
            onInstallIntegration={setupIntegration}
            onOpenCredentials={() => setCredentialDialogOpen(true)}
            onToggleBridge={toggleBridge}
            onGeneratePair={generatePairing}
          />
        ) : null}
        {currentView === "connect" ? (
          <ConnectionView
            state={state}
            pairing={pairing}
            pendingAction={pendingAction}
            onInstallIntegration={setupIntegration}
            onOpenCredentials={() => setCredentialDialogOpen(true)}
            onToggleBridge={toggleBridge}
            onGeneratePair={generatePairing}
            onRefresh={refresh}
          />
        ) : null}
        {currentView === "notifications" ? (
          <NotificationsView state={state} busy={pendingAction === "mute"} onSetMuted={setMuted} />
        ) : null}
        {currentView === "activity" ? (
          <ActivityView activities={activities} onRefresh={refresh} />
        ) : null}
      </main>

      <CredentialDialog
        open={credentialDialogOpen}
        busy={pendingAction === "credentials"}
        configured={state?.credentials_configured === true}
        onClose={closeCredentialDialog}
        onSave={updateCredentials}
      />

      {toast ? (
        <button className={`toast ${toast.error ? "is-error" : ""}`} type="button" onClick={dismissToast} aria-live="polite">
          <span>{toast.error ? "!" : "✓"}</span>
          {toast.message}
        </button>
      ) : null}
    </div>
  );
}
