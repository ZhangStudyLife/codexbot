import { useEffect, useRef, useState, type FormEvent } from "react";
import type { CredentialsInput } from "../types";
import { Icon } from "./Icon";

interface CredentialDialogProps {
  open: boolean;
  busy: boolean;
  configured: boolean;
  onClose: () => void;
  onSave: (credentials: CredentialsInput) => Promise<boolean>;
}

export function CredentialDialog({
  open,
  busy,
  configured,
  onClose,
  onSave,
}: CredentialDialogProps) {
  const [appId, setAppId] = useState("");
  const [appSecret, setAppSecret] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const appIdRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) return;
    setAppId("");
    setAppSecret("");
    setShowSecret(false);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const focusTimer = window.setTimeout(() => appIdRef.current?.focus(), 40);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [busy, onClose, open]);

  if (!open) return null;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const saved = await onSave({ app_id: appId.trim(), app_secret: appSecret.trim() });
    if (!saved) return;
    setAppSecret("");
    onClose();
  };

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target && !busy) onClose();
      }}
    >
      <section className="credential-dialog" role="dialog" aria-modal="true" aria-labelledby="credential-dialog-title">
        <button className="dialog-close" type="button" onClick={onClose} disabled={busy} aria-label="关闭凭据窗口">
          <Icon name="close" />
        </button>
        <span className="dialog-icon"><Icon name="lock" /></span>
        <p className="section-kicker">QQ BOT SANDBOX</p>
        <h2 id="credential-dialog-title">{configured ? "更新机器人凭据" : "配置机器人凭据"}</h2>
        <p className="dialog-description">
          从 QQ 开放平台复制沙箱应用凭据。AppSecret 会直接写入 Windows Credential Manager，不会写入项目文件或日志。
        </p>
        <form onSubmit={handleSubmit} autoComplete="off">
          <label>
            <span>AppID</span>
            <input
              ref={appIdRef}
              value={appId}
              onChange={(event) => setAppId(event.target.value)}
              inputMode="numeric"
              maxLength={128}
              placeholder="例如 1020••••••"
              autoComplete="off"
              disabled={busy}
              required
            />
          </label>
          <label>
            <span>AppSecret</span>
            <span className="secret-field">
              <input
                value={appSecret}
                onChange={(event) => setAppSecret(event.target.value)}
                type={showSecret ? "text" : "password"}
                maxLength={512}
                placeholder="粘贴 AppSecret"
                autoComplete="new-password"
                disabled={busy}
                required
              />
              <button type="button" onClick={() => setShowSecret((visible) => !visible)} aria-label={showSecret ? "隐藏 AppSecret" : "显示 AppSecret"}>
                <Icon name="eye" />
              </button>
            </span>
          </label>
          <div className="form-security-note">
            <Icon name="shield" />
            <span>Tauri IPC 仅连接当前桌面窗口，密钥不会经过浏览器服务器。</span>
          </div>
          <button className="primary-button full" type="submit" disabled={busy}>
            {busy ? "正在安全保存…" : "安全保存凭据"}
          </button>
        </form>
      </section>
    </div>
  );
}
