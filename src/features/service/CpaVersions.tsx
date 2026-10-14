import { useCallback, useEffect, useRef, useState } from "react";
import { RefreshCw, Check } from "lucide-react";
import { cpaApi, type CpaUpdate, type CpaVersions as Versions } from "./api";

export function CpaVersions({ executable, busy, update }: { executable: string; busy: boolean; update?: CpaUpdate | null }) {
  const [result, setResult] = useState<{ executable: string; data: Versions }>();
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const request = useRef(0);
  const data = result?.executable === executable ? result.data : undefined;
  const refresh = useCallback(async () => {
    const id = ++request.current;
    setChecking(true); setError("");
    try {
      const data = await cpaApi.versions(executable);
      if (id === request.current) setResult({ executable, data });
    } catch (e) {
      if (id === request.current) setError(typeof e === "string" ? e : "版本检查失败，请重试");
    } finally {
      if (id === request.current) setChecking(false);
    }
  }, [executable]);
  useEffect(() => {
    if (!busy) void refresh();
    else setChecking(false);
    return () => { request.current++; };
  }, [refresh, busy, update?.revision]);
  const labels = data ? {
    current: "已同步到最新",
    behind: `有更新 · 落后 ${data.behind} 个提交`,
    ahead: `本地领先 ${data.ahead} 个提交`,
    diverged: `分支已分叉 · 领先 ${data.ahead} / 落后 ${data.behind}`,
    unbuilt: "源码已同步，程序尚未更新",
    modified: "存在本地修改，未完全同步",
    unknown: "无法确认是否最新",
  } : undefined;
  const message = busy ? "CPA 操作进行中…" : checking ? "正在检查远端…" : error || data?.remoteError || (data && labels?.[data.status]) || "尚未检查";
  const current = !busy && !checking && !error && data?.status === "current";
  return <section className="cpa-versions" aria-label="CPA 版本">
    <div className="cpa-version-heading">
      <span className={current ? "success" : "muted"} role="status">{message}</span>
      <button className="text-button" disabled={checking || busy} onClick={() => void refresh()} aria-label="检查 CPA 版本">
        <RefreshCw size={12} className={checking ? "spin" : ""} />检查更新
      </button>
    </div>
    <dl className="cpa-version-values">
      <div><dt>本地版本</dt><dd title={data?.local || undefined}>{data?.local?.slice(0, 8) || "未识别"}</dd></div>
      <div><dt>远端版本{data?.upstream ? ` · ${data.upstream}` : ""}</dt><dd title={data?.remote || undefined}>{data?.remote?.slice(0, 8) || "未获取"}</dd></div>
    </dl>
    {data && <p className="muted cpa-version-source">源码 {data.source.slice(0, 8)} · {data.branch}{data.checkedAt > 0 && ` · ${new Date(data.checkedAt * 1000).toLocaleTimeString("zh-CN", {hour12:false})} 核对`}</p>}
    {!busy && update?.executable === executable && <p className="success cpa-update-success" role="status"><Check size={13} />上次更新成功 · {update.commit.slice(0, 8)}，已编译并重启</p>}
  </section>;
}
