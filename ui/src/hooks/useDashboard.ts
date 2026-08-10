import { useCallback, useEffect, useRef, useState } from "react";
import {
  createPairingCode,
  installIntegration,
  invokeBackend,
  saveCredentials,
  setNotificationsMuted,
  startBridge,
  stopBridge,
} from "../lib/platform";
import type {
  CredentialsInput,
  DashboardState,
  PairingCode,
  ToastMessage,
} from "../types";

const REFRESH_INTERVAL_MS = 10_000;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useDashboard() {
  const [state, setState] = useState<DashboardState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const [pairing, setPairing] = useState<PairingCode | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const refreshingRef = useRef(false);
  const toastIdRef = useRef(0);

  const showToast = useCallback((message: string, isError = false) => {
    toastIdRef.current += 1;
    setToast({ id: toastIdRef.current, message, error: isError });
  }, []);

  const refresh = useCallback(async (silent = false) => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    if (!silent) setLoading(true);

    try {
      const nextState = await invokeBackend<DashboardState>(
        "get_dashboard_state",
      );
      setState(nextState);
      setError(null);
    } catch (refreshError) {
      const message = errorMessage(refreshError);
      setError(message);
      if (!silent) showToast(message, true);
    } finally {
      refreshingRef.current = false;
      if (!silent) setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => {
      if (!document.hidden) void refresh(true);
    }, REFRESH_INTERVAL_MS);
    const handleVisibility = () => {
      if (!document.hidden) void refresh(true);
    };
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [refresh]);

  useEffect(() => {
    if (!toast) return;
    const timeout = window.setTimeout(() => setToast(null), 3600);
    return () => window.clearTimeout(timeout);
  }, [toast]);

  const runAction = useCallback(
    async <T,>(
      key: string,
      operation: () => Promise<T>,
      successMessage?: (result: T) => string,
    ): Promise<T | null> => {
      setPendingAction(key);
      try {
        const result = await operation();
        if (successMessage) showToast(successMessage(result));
        await refresh(true);
        return result;
      } catch (actionError) {
        showToast(errorMessage(actionError), true);
        return null;
      } finally {
        setPendingAction(null);
      }
    },
    [refresh, showToast],
  );

  const updateCredentials = useCallback(
    async (credentials: CredentialsInput) => {
      const result = await runAction(
        "credentials",
        () => saveCredentials(credentials),
        () => "QQ 凭据已安全保存到 Windows Credential Manager",
      );
      return result !== null;
    },
    [runAction],
  );

  const setupIntegration = useCallback(async () => {
    return runAction(
      "integration",
      installIntegration,
      (result) => result.message,
    );
  }, [runAction]);

  const toggleBridge = useCallback(async () => {
    const shouldStop = state?.daemon_running === true;
    await runAction(
      shouldStop ? "stop" : "start",
      shouldStop ? stopBridge : startBridge,
      (result) => result.message,
    );
  }, [runAction, state?.daemon_running]);

  const generatePairing = useCallback(async () => {
    const result = await runAction(
      "pair",
      createPairingCode,
      () => "配对码已生成，在 QQ 中发送即可完成绑定",
    );
    if (result) setPairing(result);
    return result;
  }, [runAction]);

  const setMuted = useCallback(
    async (muted: boolean) => {
      const result = await runAction(
        "mute",
        () => setNotificationsMuted(muted),
        () => (muted ? "主动通知已暂停" : "主动通知已恢复"),
      );
      return result !== null;
    },
    [runAction],
  );

  return {
    state,
    loading,
    error,
    pendingAction,
    pairing,
    toast,
    dismissToast: () => setToast(null),
    refresh,
    setupIntegration,
    updateCredentials,
    toggleBridge,
    generatePairing,
    setMuted,
  };
}
