import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import { createWorkspaceMotion } from "./components/workspaceMotion";
import { History, Settings, RefreshCw, ArrowUpRight, Scan, Palette } from "lucide-react";
import { Segment } from "./components/Segment";
import { Toast } from "./components/Toast";
import { Logo, Modal } from "./components/Common";
import { useAppearance } from "./features/appearance/useAppearance";
import { Appearance } from "./features/appearance/Appearance";
import { useServices } from "./features/service/useServices";
import { ServicePanel } from "./features/service/ServicePanel";
import {
  Credentials,
  CredentialRows,
} from "./features/credentials/Credentials";
import { traySummary } from "./features/credentials/data";
import { useCredentials } from "./features/credentials/useCredentials";
import { Connection } from "./features/credentials/Connection";
import { ImportDialog } from "./features/credentials/ImportDialog";
import { Probe, ModelSelect } from "./features/probe/Probe";
import { useProbe } from "./features/probe/useProbe";
import { ProbeHistory } from "./features/probe/ProbeHistory";
import { openMain, openTray, closeTray, native } from "./features/service/api";
function readPreference(key: string, fallback: string) {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}
function usePreference(key: string, fallback: string) {
  const [v, set] = useState(() => readPreference(key, fallback));
  return [
    v,
    (value: string) => {
      set(value);
      try {
        localStorage.setItem(key, value);
      } catch {
        /* Preference storage is optional. */
      }
    },
  ] as const;
}
export default function App() {
  const [keyboardFocus, setKeyboardFocus] = useState(false);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Tab" || (!event.metaKey && !event.ctrlKey && !event.altKey)) setKeyboardFocus(true);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);
  const services = useServices();
  const appearance = useAppearance();
  const [focus, setFocus] = usePreference("focus", "probe");
  const [workspaceMotion] = useState(createWorkspaceMotion);
  useEffect(() => () => workspaceMotion.cancel(), [workspaceMotion]);
  const changeFocus = (value: string) =>
    workspaceMotion.run(() => flushSync(() => setFocus(value)));
  const credentials = useCredentials();
  const probe = useProbe(credentials);
  const [modal, setModal] = useState("");
  const [toast, setToast] = useState("");
  const isTray =
    new URLSearchParams(window.location.search).get("view") === "tray";
  useEffect(() => {
    if (!isTray) return;
    const dismiss = (event: KeyboardEvent) => {
      if (
        event.key === "Escape" &&
        !modal &&
        !document.querySelector('[popover][data-open="true"]')
      )
        void closeTray();
    };
    window.addEventListener("keydown", dismiss);
    return () => window.removeEventListener("keydown", dismiss);
  }, [isTray, modal]);
  const preview = (id: string) => {
    if (probe.busy) { setToast("请等待当前检测结束"); return; }
    probe.setCredentialId(id);
    changeFocus("probe");
  };
  const status = (
    <button
      className="service-status"
      onClick={() => setModal("service")}
      title="查看进程与记住的目录"
    >
      <i className={services.running ? "online" : ""} />
      {services.busy && !services.snapshot.checkedAt
        ? "正在发现 CLIProxy"
        : !services.known
          ? "CLIProxy 状态未知"
          : services.running
            ? "CLIProxy 运行中"
            : "CLIProxy 未运行"}
    </button>
  );
  const connect = () => {
    if (isTray) void openMain();
    else setModal("connection");
  };
  const rows = <CredentialRows state={credentials} onConnect={connect} />;
  return (
    <div
      className={isTray ? "tray-app" : "app"}
      data-keyboard-focus={keyboardFocus}
      onPointerDownCapture={() => setKeyboardFocus(false)}
    >
      {isTray ? (
        <>
          <header className="tray-header">
            {status}
            <ModelSelect state={probe} />
          </header>
          {rows}
          <div className="tray-bottom">
            <span>
              {traySummary(credentials)}
            </span>
            <button
              className="text-button"
              disabled={
                !credentials.connected || credentials.busy || credentials.stale
              }
              onClick={() => void credentials.refreshQuota()}
            >
              {credentials.busy ? "刷新中…" : "刷新额度"}
            </button>
            <button className="icon-button" aria-label="外观设置" title="外观设置" onClick={() => setModal("appearance")}><Palette size={14} /></button>
            <button
              className="icon-button"
              aria-label="打开主窗口"
              onClick={() => void openMain()}
            >
              <ArrowUpRight />
            </button>
          </div>
        </>
      ) : (
        <>
          <header className="titlebar" data-tauri-drag-region>
            <Logo />
            <Segment
              label="窗口重点"
              className="focus-switch"
              value={focus}
              onChange={changeFocus}
              options={[
                ["probe", "探查优先"],
                ["credentials", "凭证优先"],
              ]}
            />
            <div className="titlebar-right">
              {status}
              <button
                className="icon-button"
                aria-label="刷新进程状态"
                disabled={services.busy}
                onClick={() => void services.refresh()}
              >
                <RefreshCw className={services.busy ? "spin" : ""} />
              </button>
              <button
                className="icon-button"
                aria-label="历史"
                onClick={() => setModal("history")}
              >
                <History size={15} />
              </button>
              <button
                className="icon-button"
                aria-label="外观与连接设置"
                onClick={() => setModal("appearance")}
              >
                <Settings />
              </button>
            </div>
          </header>
          <main
            className={`workspace ${focus === "credentials" ? "credential-focus" : ""}`}
          >
            <Probe
              compact={focus === "credentials"}
              onExpand={() => changeFocus("probe")}
              state={probe}
              notify={setToast}
              onConnect={connect}
              connected={credentials.connected}
              managementBusy={credentials.busy}
              stale={credentials.stale}
            />
            <Credentials
              state={credentials}
              expanded={focus === "credentials"}
              onConnect={connect}
              preview={preview}
              onImport={() => setModal("import")}
              onExpand={() => changeFocus("credentials")}
              onTray={() => {
                if (native)
                  void openTray().catch(() =>
                    setToast("快捷面板打开失败，请从菜单栏重试。"),
                  );
                else setModal("tray");
              }}
            />
          </main>
        </>
      )}
      {modal === "appearance" && <Appearance {...appearance} compact={isTray} onClose={() => setModal("")} onConnect={() => setModal("connection")} />}
      {modal === "connection" && (
        <Connection
          state={credentials}
          onClose={() => setModal("")}
          onInspect={() => setModal("service")}
        />
      )}
      {modal === "service" && (
        <ServicePanel state={services} onClose={() => setModal("")} />
      )}
      {modal === "import" && <ImportDialog state={credentials} onClose={() => setModal("")} />}
      {modal === "history" && (
        <Modal title="检测历史" onClose={() => setModal("")}>
          <ProbeHistory state={probe} onChoose={() => { setModal(""); changeFocus("probe"); }} />
        </Modal>
      )}
      {modal === "tray" && (
        <Modal title="菜单栏面板预览" onClose={() => setModal("")}>
          <div className="tray-preview">
            <header className="tray-header">
              {status}
              <ModelSelect state={probe} />
            </header>
            {rows}
            <div className="tray-bottom">
              <span>真实凭证列表</span>
              <button className="text-button" onClick={() => { setModal(""); changeFocus("probe"); }}>
                模型探查
              </button>
            </div>
          </div>
          <p className="modal-note">
            也可以点击 Mac 菜单栏中的 <Scan size={13} /> 图标打开快捷面板。
          </p>
        </Modal>
      )}
      {toast && (
        <Toast key={toast} message={toast} onClose={() => setToast("")} />
      )}
    </div>
  );
}
