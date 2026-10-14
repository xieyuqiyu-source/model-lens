import { useRef, useState } from "react";
import { ArrowUpRight, Download } from "lucide-react";
import { cpaApi } from "./api";
export function CpaActions({ connected }: { connected: boolean }) {
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState("");
  const running = useRef(false);
  const run = async (pull: boolean) => {
    if (!connected || running.current) return;
    running.current = true; setPending(true); setNotice("");
    try {
      if (pull) {
        const result = await cpaApi.pull();
        setNotice(result.updated ? `源码已更新至 ${result.after.slice(0, 7)}，尚未编译或重启` : "没有可快进的新提交");
      } else await cpaApi.open();
    } catch (e) { setNotice(typeof e === "string" ? e : "操作未完成，请重试"); }
    finally { running.current = false; setPending(false); }
  };
  return <>
    <button className="text-button proxy-url" disabled={!connected || pending} onClick={() => void run(false)}>CPA 管理<ArrowUpRight size={12} /></button>
    <button className="text-button proxy-url" disabled={!connected || pending} onClick={() => void run(true)}><Download size={12} />{pending ? "处理中…" : "拉取更新"}</button>
    {notice && <span className="muted cpa-status" role="status">{notice}</span>}
  </>;
}
