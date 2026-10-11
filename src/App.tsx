import { useEffect, useState } from "react";
import { History, Settings, RefreshCw, ArrowUpRight, Scan } from "lucide-react";
import { Segment } from "./components/Segment";
import { Toast } from "./components/Toast";
import { Logo, Modal } from "./components/Common";
import { useServices } from "./features/service/useServices";
import { ServicePanel } from "./features/service/ServicePanel";
import {
  Credentials,
  CredentialRows,
} from "./features/credentials/Credentials";
import { useCredentials } from "./features/credentials/useCredentials";
import { Connection } from "./features/credentials/Connection";
import { ImportDialog } from "./features/credentials/ImportDialog";
import { Probe, ModelSelect } from "./features/probe/Probe";
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
  const services = useServices();
  const [focus, setFocus] = usePreference("focus", "probe");
  const [mode, setMode] = usePreference("mode", "auto");
  const [model, setModel] = usePreference("model", "Sonnet 4.5");
  const credentials = useCredentials();
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
  const preview = () => setToast("模型检测尚未接入，本次没有发起模型请求。");
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
    <div className={isTray ? "tray-app" : "app"}>
      {isTray ? (
        <>
          <header className="tray-header">
            {status}
            <ModelSelect value={model} onChange={setModel} />
          </header>
          {rows}
          <div className="tray-bottom">
            <span>
              {credentials.error
                ? "连接或刷新异常，请打开主窗口"
                : credentials.connected
                  ? `${credentials.credentials.length} 个凭证`
                  : "尚未连接"}
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
              onChange={setFocus}
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
                className="text-button"
                onClick={() => setModal("history")}
              >
                <History size={15} />
                历史
              </button>
              <button
                className="icon-button"
                aria-label="管理连接设置"
                onClick={() => setModal("connection")}
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
              onExpand={() => setFocus("probe")}
              preview={preview}
              mode={mode}
              setMode={setMode}
              model={model}
              setModel={setModel}
            />
            <Credentials
              state={credentials}
              expanded={focus === "credentials"}
              onConnect={connect}
              preview={preview}
              onImport={() => setModal("import")}
              onExpand={() => setFocus("credentials")}
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
      {modal === "import" && <ImportDialog onClose={() => setModal("")} />}
      {modal === "history" && (
        <Modal title="检测历史" onClose={() => setModal("")}>
          <div className="empty">
            <History size={28} />
            <p>还没有检测记录</p>
            <span>当前为第一阶段 UI 预览，真实检测将在后续版本接入。</span>
          </div>
        </Modal>
      )}
      {modal === "tray" && (
        <Modal title="菜单栏面板预览" onClose={() => setModal("")}>
          <div className="tray-preview">
            <header className="tray-header">
              {status}
              <ModelSelect value={model} onChange={setModel} />
            </header>
            {rows}
            <div className="tray-bottom">
              <span>真实凭证列表</span>
              <button className="text-button" onClick={preview}>
                批量检测
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
