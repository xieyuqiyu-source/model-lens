import { useState } from "react";
import { cpaApi } from "./api";
import { useCpaJob } from "./useCpaJob";
import { Folder, RefreshCw, Check, Circle, Power, Download } from "lucide-react";
import { Modal } from "../../components/Common";
import type { ServiceState } from "./useServices";
export function ServicePanel({
  state,
  onClose, port: initialPort,
}: {
  state: ServiceState;
  port: number;
  onClose: () => void;
}) {
  const job=useCpaJob();
  const [selected,setSelected]=useState("");
  const [installing,setInstalling]=useState(false);
  const [directory,setDirectory]=useState("");
  const [port,setPort]=useState(initialPort||8317);
  const [notice,setNotice]=useState("");
  const chosen=state.snapshot.services.find(s=>s.executable===selected) ?? (state.snapshot.services.length===1?state.snapshot.services[0]:undefined);
  const blocked=job.busy||state.busy||!state.known;
  return (
    <Modal title="本机 CLIProxy" onClose={onClose}>
      <div className="service-toolbar">
        <span className="status">
          <i className={state.running ? "online" : ""} />
          {!state.known
            ? "当前状态无法确认"
            : state.running
              ? "检测到运行中的进程"
              : "未发现运行中的进程"}
        </span>
        <button className="button" disabled={blocked||!chosen||port<1||port>65535} title="重启会中断当前请求；首次接管会停止原 dev 启动器和 CPM"
          onClick={() => {setNotice("");if(chosen)void job.run(()=>cpaApi.start(chosen.executable,port));}}>
          <Power size={13} />{chosen?.pid?"重启":"启动"}
        </button>
        <button
          className="button"
          disabled={job.busy||state.busy}
          onClick={() => void state.refresh()}
        >
          <RefreshCw size={13} className={state.busy ? "spin" : ""} />
          {state.busy ? "检查中…" : "重新检测"}
        </button>
        <button className="button" disabled={job.busy||state.busy} onClick={async()=>{
          const next=await state.refresh();
          if(!next?.statusKnown){setNotice("请先完成本机检测");return;}
          setNotice(next.services.length?"已发现进程或记住的目录，安装前会再次核实，已有安装将提示复用":"");
          setInstalling(true);
        }}><Download size={13} />安装 CLIProxy</button>
      </div>
      <label className="service-port">CPA 端口<input aria-label="CPA 端口" type="number" min={1} max={65535} value={port} disabled={job.busy} onChange={e=>setPort(Number(e.target.value))} /></label>
      {(job.message||notice) && <p className="notice" role="status">{job.busy?job.message:notice||job.message}</p>}
      {installing && <form className="service-install" onSubmit={e=>{e.preventDefault();setNotice("");void job.run(()=>cpaApi.install(directory,port));}}>
        <label>安装到新目录<input aria-label="CLIProxy 安装路径" value={directory} disabled={job.busy} placeholder="~/Applications/CLIProxy" onChange={e=>setDirectory(e.target.value)} /></label>
        <button type="button" className="button" disabled={job.busy} onClick={()=>void cpaApi.chooseDirectory().then(path=>{if(path)setDirectory(path);}).catch(()=>setNotice("目录选择未完成"))}>选择目录</button>
        <p className="muted">官方源码 · 安装并启动 · 自动生成本机密钥</p>
        <div className="modal-actions"><button type="button" className="button" disabled={job.busy} onClick={()=>setInstalling(false)}>收起</button><button className="button" type="submit" disabled={blocked||!directory.trim()||port<1||port>65535}>安装并启动</button></div>
      </form>}
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
            {state.snapshot.services.length>1 && <label><input type="radio" name="cpa-directory" checked={chosen?.executable===service.executable} onChange={()=>setSelected(service.executable)} />使用此目录</label>}
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
      {!installing && !state.snapshot.services.length && !state.busy && (
        <div className="empty">
          <Circle size={24} />
          <p>等待发现 CLIProxy</p>
          <span>选择安装目录以安装新的 CLIProxy。</span>
        </div>
      )}
      <footer className="modal-note">
        重启会中断当前请求。关闭面板后，后台任务仍会继续。
      </footer>
    </Modal>
  );
}
