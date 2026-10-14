import { useState } from "react";
import { ArrowUpRight, Download } from "lucide-react";
import { cpaApi } from "./api";
import { useCpaJob } from "./useCpaJob";
export function CpaActions({ connected }: { connected: boolean }) {
  const job=useCpaJob();
  const [notice,setNotice]=useState("");
  return <>
    <button className="text-button proxy-url" disabled={!connected || job.busy} onClick={() => void cpaApi.open().catch(e=>setNotice(typeof e==="string"?e:"管理页未能打开"))}>CPA 管理<ArrowUpRight size={12} /></button>
    <button className="text-button proxy-url" disabled={!connected || job.busy} title="拉取、编译并重启 CPA，会中断当前请求" onClick={() => {setNotice("");void job.run(cpaApi.pull);}}><Download size={12} />{job.busy ? "处理中…" : "更新并重启"}</button>
    {(notice||job.message) && <span className="muted cpa-status" role="status">{notice||job.message}</span>}
  </>;
}
