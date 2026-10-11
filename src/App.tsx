import { useEffect, useState } from "react";
import {
  History,
  Settings,
  RefreshCw,
  ArrowUpRight,
  Scan,
  X,
} from "lucide-react";
import { Logo, Modal } from "./components/Common";
import { useServices } from "./features/service/useServices";
import { ServicePanel } from "./features/service/ServicePanel";
import {
  Credentials,
  CredentialRows,
} from "./features/credentials/Credentials";
import { initialCredentials } from "./features/credentials/data";
import { ImportDialog } from "./features/credentials/ImportDialog";
import { Probe, ModelSelect } from "./features/probe/Probe";
import { openMain, openTray, native } from "./features/service/api";
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
  const [credentials, setCredentials] = useState(initialCredentials);
  const [modal, setModal] = useState("");
  const [toast, setToast] = useState("");
  const isTray =
    new URLSearchParams(window.location.search).get("view") === "tray";
  const preview = () =>
    setToast("当前是界面预览，此操作未请求模型或修改真实凭证。");
  const toggle = (id: string) =>
    setCredentials((cs) =>
      cs.map((c) => (c.id === id ? { ...c, enabled: !c.enabled } : c)),
    );
  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(""), 4000);
      return () => clearTimeout(timer);
    }
  }, [toast]);
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
  const rows = (
    <CredentialRows
      credentials={credentials}
      toggle={toggle}
      preview={preview}
    />
  );
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
              凭证示例 · {services.running ? "已识别本机进程" : "等待发现服务"}
            </span>
            <button className="text-button" onClick={preview}>
              批量检测
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
            <div className="segment focus-switch">
              <button
                aria-pressed={focus === "probe"}
                onClick={() => setFocus("probe")}
              >
                探查优先
              </button>
              <button
                aria-pressed={focus === "credentials"}
                onClick={() => setFocus("credentials")}
              >
                凭证优先
              </button>
            </div>
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
                aria-label="设置与服务发现"
                onClick={() => setModal("service")}
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
              credentials={credentials}
              expanded={focus === "credentials"}
              toggle={toggle}
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
              <span>凭证为示例</span>
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
        <div className="toast" role="status">
          <span>{toast}</span>
          <button
            className="icon-button"
            aria-label="关闭提示"
            onClick={() => setToast("")}
          >
            <X size={13} />
          </button>
        </div>
      )}
    </div>
  );
}
