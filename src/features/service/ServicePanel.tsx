import { useState } from "react";
import { cpaApi } from "./api";
import { useCpaJob } from "./useCpaJob";
import { Folder, RefreshCw, Power, Download, ChevronDown } from "lucide-react";
import { Modal } from "../../components/Common";
import { CpaVersions } from "./CpaVersions";
import { CommandLog } from "./CommandLog";
import type { ServiceState } from "./useServices";

export function ServicePanel({ state, onClose, port: initialPort }: {
  state: ServiceState;
  port: number;
  onClose: () => void;
}) {
  const job = useCpaJob();
  const [selected, setSelected] = useState("");
  const [installing, setInstalling] = useState(false);
  const [directory, setDirectory] = useState("");
  const [port, setPort] = useState(initialPort || 8317);
  const [notice, setNotice] = useState("");
  const chosen = state.snapshot.services.find(s => s.executable === selected)
    ?? (state.snapshot.services.length === 1 ? state.snapshot.services[0] : undefined);
  const blocked = job.busy || state.busy || !state.known;
  const running = chosen?.pid != null;
  return (
    <Modal title="本机 CLIProxy" onClose={onClose}>
      <div className="service-panel">
        <div className="service-toolbar">
          <span className="status">
            <i className={state.running ? "online" : ""} />
            {!state.known ? "状态未知" : state.running ? "CPA 运行中" : "CPA 未运行"}
          </span>
          <div className="service-buttons">
            <button className="button" disabled={blocked || !chosen || port < 1 || port > 65535}
              aria-label={running ? "关闭 CPA" : "启动 CPA"}
              title={running ? "关闭会中断当前请求；原 dev 启动器托管时也会停止 CPM" : "启动已选择的 CPA"}
              onClick={() => {
                setNotice("");
                if (chosen) void job.run(() => cpaApi.setRunning(chosen.executable, port, !running, chosen.pid));
              }}><Power size={13} />{running ? "关闭" : "启动"}</button>
            <button className="button" disabled={job.busy || state.busy} onClick={() => void state.refresh()}>
              <RefreshCw size={13} className={state.busy ? "spin" : ""} />{state.busy ? "检查中…" : "重新检测"}
            </button>
            <button className="button" disabled={job.busy || state.busy} onClick={async () => {
              const next = await state.refresh();
              if (!next?.statusKnown) { setNotice("请先完成本机检测"); return; }
              setNotice(next.services.length ? "已发现安装，优先使用已有目录" : "");
              setInstalling(true);
            }}><Download size={13} />安装 CLIProxy</button>
          </div>
        </div>
        <label className="service-port">CPA 端口
          <input aria-label="CPA 端口" type="number" min={1} max={65535} value={port} disabled={job.busy} onChange={e => setPort(Number(e.target.value))} />
        </label>
        {(notice || state.error || state.snapshot.warning) && <p className="notice" role="status">{notice || state.error || state.snapshot.warning}</p>}
        {installing && <form className="service-install" onSubmit={e => {
          e.preventDefault(); setNotice(""); void job.run(() => cpaApi.install(directory, port));
        }}>
          <label>安装到新目录
            <input aria-label="CLIProxy 安装路径" value={directory} disabled={job.busy} placeholder="~/Applications/CLIProxy" onChange={e => setDirectory(e.target.value)} />
          </label>
          <button type="button" className="button" disabled={job.busy} onClick={() => void cpaApi.chooseDirectory().then(path => {
            if (path) setDirectory(path);
          }).catch(() => setNotice("目录选择未完成"))}>选择目录</button>
          <div className="service-buttons">
            <button type="button" className="button" disabled={job.busy} onClick={() => setInstalling(false)}>收起</button>
            <button className="button" type="submit" disabled={blocked || !directory.trim() || port < 1 || port > 65535}>安装并启动</button>
          </div>
        </form>}
        {state.snapshot.services.length > 0 && <div className="service-list">
          {state.snapshot.services.map(service => (
            <article className="service-location" key={service.executable}>
              {state.snapshot.services.length > 1 && <label className="service-select">
                <input type="radio" name="cpa-directory" checked={chosen?.executable === service.executable} disabled={job.busy} onChange={() => setSelected(service.executable)} />使用此目录
              </label>}
              <details>
                <summary className="service-name" aria-label={`CPA 地址：${service.executable}`}>
                  <Folder size={16} /><strong>{service.executable.split("/").pop()}</strong>
                  <span className="muted">{!state.known ? "状态未知" : service.pid ? `运行中 · PID ${service.pid}` : "未运行"}</span>
                  <ChevronDown size={14} />
                </summary>
                <dl>
                  <dt>程序路径</dt><dd>{service.executable}</dd>
                  <dt>所在目录</dt><dd>{service.directory}</dd>
                  <dt>工作目录</dt><dd>{service.workingDirectory ?? "未能读取"}</dd>
                </dl>
              </details>
            </article>
          ))}
        </div>}
        {chosen && <CpaVersions executable={chosen.executable} busy={job.busy} update={job.update} />}
        <CommandLog logs={job.logs} feedback={job.feedback} busy={job.busy} />
      </div>
    </Modal>
  );
}
