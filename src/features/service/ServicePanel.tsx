import { Folder, RefreshCw, Check, Circle } from "lucide-react";
import { Modal } from "../../components/Common";
import type { ServiceState } from "./useServices";
export function ServicePanel({
  state,
  onClose,
}: {
  state: ServiceState;
  onClose: () => void;
}) {
  return (
    <Modal title="本机 CLIProxy" onClose={onClose}>
      <p className="muted">自动识别正在运行的服务，并记住程序与目录。</p>
      <div className="service-toolbar">
        <span className="status">
          <i className={state.running ? "online" : ""} />
          {!state.known
            ? "当前状态无法确认"
            : state.running
              ? "检测到运行中的进程"
              : "未发现运行中的进程"}
        </span>
        <button
          className="button"
          disabled={state.busy}
          onClick={() => void state.refresh()}
        >
          <RefreshCw size={13} className={state.busy ? "spin" : ""} />
          {state.busy ? "检查中…" : "重新检测"}
        </button>
      </div>
      {(state.error || state.snapshot.warning) && (
        <p className="notice" role="status">
          {state.error || state.snapshot.warning}
        </p>
      )}
      <div className="service-list">
        {state.snapshot.services.map((service) => (
          <article
            className="service-location"
            key={`${service.executable}-${service.pid}`}
          >
            <div className="service-name">
              <Folder size={16} />
              <strong>{service.executable.split("/").pop()}</strong>
              <span className="muted">
                {!state.known
                  ? "已记住 · 状态未知"
                  : service.pid
                    ? `运行中 · PID ${service.pid}`
                    : "已记住 · 未运行"}
              </span>
            </div>
            <dl>
              <dt>程序路径</dt>
              <dd>{service.executable}</dd>
              <dt>所在目录</dt>
              <dd>{service.directory}</dd>
              <dt>工作目录</dt>
              <dd>{service.workingDirectory ?? "未能读取"}</dd>
            </dl>
            <span className="meta">
              <Check size={12} /> 上次发现{" "}
              {new Date(service.lastSeen * 1000).toLocaleString("zh-CN")}
            </span>
          </article>
        ))}
      </div>
      {!state.snapshot.services.length && !state.busy && (
        <div className="empty">
          <Circle size={24} />
          <p>等待发现 CLIProxy</p>
          <span>先用原来的方式启动服务，再点击重新检测。</span>
        </div>
      )}
      <footer className="modal-note">
        本阶段仅识别与记忆位置；已有服务继续由原来的方式管理。
      </footer>
    </Modal>
  );
}
