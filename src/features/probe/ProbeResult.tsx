import { Fingerprint, Copy } from "lucide-react";
import type { ProbeRecord } from "./analysis";
export function ProbeResult({
  record,
  notify,
}: {
  record: ProbeRecord;
  notify: (text: string) => void;
}) {
  const r = record.analysis;
  return (
    <article className="result" aria-label="检测结果">
      <header>
        <span>检测结果</span>
        <span className="meta">{r.used_outputs} / 3 有效样本</span>
      </header>
      <div className="result-title">
        <span className="result-check">
          <Fingerprint />
        </span>
        <div>
          <h2>{r.prediction_name}</h2>
          <p className="muted">
            {r.evidence.label} · {record.model}
          </p>
        </div>
      </div>
      <div className="rankings">
        <span className="meta">
          参考库内排序{r.probability !== null ? " · 库内置信度" : ""}
        </span>
        {r.results.slice(0, 3).map((row, i) => (
          <div key={row.model}>
            <span className="rank-index">0{i + 1}</span>
            <span>{row.display_name}</span>
            <small>
              {row.probability === null
                ? i === 0
                  ? "最接近"
                  : ""
                : `${(row.probability * 100).toFixed(1)}%`}
            </small>
          </div>
        ))}
      </div>
      <p className="muted probe-caveat">{r.evidence.reason} 不代表身份认证。</p>
      {record.warnings.map((w, i) => (
        <p className="notice" key={i}>
          {w}
        </p>
      ))}
      <footer>
        <span className="meta">
          {record.source === "manual" ? "手动分析" : record.credential}
        </span>
        <button
          className="text-button"
          onClick={() =>
            void navigator.clipboard
              .writeText(
                JSON.stringify(
                  {
                    ...record,
                    detectorCommit: "4ffda94e5e24a05c4f55d6f14a1e27f4ef23c8f3",
                  },
                  null,
                  2,
                ),
              )
              .then(
                () => notify("检测结果已复制"),
                () => notify("复制失败，请重试"),
              )
          }
        >
          <Copy size={13} />
          复制结果
        </button>
      </footer>
    </article>
  );
}
