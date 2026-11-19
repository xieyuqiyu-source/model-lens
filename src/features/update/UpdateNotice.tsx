import { useCallback, useEffect, useRef, useState } from "react";
import { Download } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";

type Status = "idle" | "checking" | "current" | "available" | "installing" | "error" | "install-error";

export function useAppUpdate(enabled: boolean, macDesktop: boolean) {
  const [status, setStatus] = useState<Status>("idle");
  const [version, setVersion] = useState("");
  const [installError, setInstallError] = useState("");
  const update = useRef<Update | null>(null);
  const busy = useRef(false);

  const refresh = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setStatus("checking");
    try {
      const previous = update.current;
      update.current = null;
      setVersion("");
      await previous?.close();
      const next = await check({ timeout: 15000 });
      update.current = next;
      setVersion(next?.version ?? "");
      setStatus(next ? "available" : "current");
    } catch {
      setStatus("error");
    } finally {
      busy.current = false;
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 6 * 60 * 60 * 1000);
    return () => window.clearInterval(timer);
  }, [enabled, refresh]);

  const install = async () => {
    if (busy.current || !update.current) return;
    busy.current = true;
    setStatus("installing");
    try {
      if (macDesktop) await invoke("install_macos_update", { rid: update.current.rid });
      else await update.current.downloadAndInstall(undefined, { timeout: 300000 });
    } catch (error) {
      setInstallError(macDesktop && typeof error === "string" ? error : "更新下载、验证或安装失败，请重试。");
      setStatus("install-error");
      busy.current = false;
    }
  };

  return { status, version, installError, refresh, install };
}

export function UpdateNotice({ update, appBusy }: {
  update: ReturnType<typeof useAppUpdate>;
  appBusy: boolean;
}) {
  const { status, version, installError, refresh, install } = update;
  const text = {
    idle: "",
    checking: "正在检查应用更新…",
    current: "ModelLens 已是最新版本",
    available: `发现 ModelLens ${version}，安装完成后应用将重新打开。`,
    installing: `正在下载并验证 ModelLens ${version}…`,
    error: "应用更新检查失败，请重试。",
    "install-error": installError,
  }[status];
  return (
    <div>
      {text && <p className="muted" role="status">{text}</p>}
      <div className="actions">
        <button className="text-button" disabled={status === "checking" || status === "installing"} onClick={() => void refresh()}>
          <Download size={14} />检查应用更新
        </button>
        {(status === "available" || status === "install-error") &&
          <button className="text-button" disabled={appBusy} onClick={() => void install()}>安装并重启</button>}
      </div>
    </div>
  );
}
