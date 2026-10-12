import { useEffect, useState } from "react";
import type { ProbeState } from "./useProbe";
import type { ManagementSnapshot } from "../credentials/data";
import type { AccountProbeState } from "./useAccountProbe";
import type { Detection } from "./analysis";
import { analyze } from "./api";
import { accountActive, accountStatusLabel, accountDisplay } from "./accountStatus";
import { ProbeResult } from "./ProbeResult";

type Entry = NonNullable<ManagementSnapshot["accountHistory"]>[number];
function AccountDetail({ entry, probe, notify, onRun }: { entry: Entry; probe: AccountProbeState; notify: (message: string) => void; onRun: () => void }) {
  const [detection, setDetection] = useState<Detection | null>(null);
  const [error, setError] = useState("");
  const running = accountActive(entry);
  useEffect(() => {
    setDetection(null); setError("");
    if (running || !entry.samples.length) return;
    const controller = new AbortController();
    void analyze(entry.samples, controller.signal).then(setDetection).catch(e => {
      if (!controller.signal.aborted) setError(e instanceof Error ? e.message : "本地分析失败");
    });
    return () => controller.abort();
  }, [entry.runId, entry.status, entry.samples.length, running]);
  return <div className="account-history-detail">
    <strong>{entry.credential} · {accountStatusLabel(entry.status)}</strong>
    <p className="muted">{entry.model} · {new Date(entry.at * 1000).toLocaleString("zh-CN")}</p>
    <p role="status">{entry.message}</p>
    {running && <p className="muted">可关闭此面板，探查继续在后台运行。</p>}
    {detection && <ProbeResult record={{...detection, id: String(entry.runId), at: entry.at * 1000, source: "account", credential: entry.credential, model: entry.model, attempts: entry.samples}} notify={notify} />}
    {error && <p className="notice">{error}</p>}
    {entry.samples.map(s => <details key={s.slot}><summary>采样 {s.slot} · {s.elapsedMs} ms · {s.finish}</summary><pre>{s.prompt}</pre><pre>{s.text}</pre></details>)}
    <button className="button" disabled={probe.blocked(entry.credentialId) || !probe.canRetry(entry.credentialId, entry.runId)} onClick={onRun}>重新探查当前模型</button>
  </div>;
}
export function ProbeHistory({state, accountHistory, selectedId, onSelect, accountProbe, notify, onChoose}: {
  state: ProbeState; accountHistory: Entry[]; selectedId: number | null; onSelect: (id: number | null) => void;
  accountProbe: AccountProbeState; notify: (message: string) => void; onChoose: () => void;
}) {
  accountHistory = accountHistory.map(accountDisplay);
  const selected = accountHistory.find(h => h.runId === selectedId);
  return <div className="probe-history">
    {selected ? <><button className="text-button" onClick={() => onSelect(null)}>返回记录</button><AccountDetail key={selected.runId} entry={selected} probe={accountProbe} notify={notify} onRun={() => { void accountProbe.run(selected.credentialId, selected.runId); onSelect(null); }} /></> : <>
      {!state.history.length && !accountHistory.length ? <div className="empty"><p>还没有检测记录</p></div> : <>
        <p className="muted">快捷探查保留本次应用内最近 10 条记录</p>
        {accountHistory.map(r => <button className="button" key={r.runId} onClick={() => onSelect(r.runId)}>
          <span>{r.credential} · {accountStatusLabel(r.status)}</span>
          <small>{r.model} · {new Date(r.at * 1000).toLocaleTimeString("zh-CN")} · {r.samples.length}/3</small>
        </button>)}
        {state.history.map(r => <button className="button" key={r.id} disabled={state.busy} onClick={() => {state.showHistory(r);onChoose();}}>
          <span>{r.analysis.prediction_name}</span><small>{r.model} · {new Date(r.at).toLocaleTimeString("zh-CN")} · {r.analysis.used_outputs}/3</small>
        </button>)}
      </>}
    </>}
  </div>;
}
