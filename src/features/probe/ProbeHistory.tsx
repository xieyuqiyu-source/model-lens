import type { ProbeState } from "./useProbe";
export function ProbeHistory({
  state,
  onChoose,
}: {
  state: ProbeState;
  onChoose: () => void;
}) {
  return (
    <div className="probe-history">
      {!state.history.length ? (
        <div className="empty">
          <p>还没有检测记录</p>
        </div>
      ) : (
        <>
          <p className="muted">仅保留本次窗口内最近 10 次结果</p>
          {state.history.map((r) => (
            <button
              className="button"
              key={r.id}
              disabled={state.busy}
              onClick={() => {
                state.showHistory(r);
                onChoose();
              }}
            >
              <span>{r.analysis.prediction_name}</span>
              <small>
                {r.source === "manual" ? "手动" : r.model} ·{" "}
                {new Date(r.at).toLocaleTimeString("zh-CN")} ·{" "}
                {r.analysis.used_outputs}/3
              </small>
            </button>
          ))}
        </>
      )}
    </div>
  );
}
