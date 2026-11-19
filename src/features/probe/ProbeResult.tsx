import { Fingerprint, Copy } from "lucide-react";
import { Disclosure } from "../../components/Disclosure";
import { builtinBank, detectorCommit, type ProbeRecord } from "./analysis";
export function ProbeResult({
  record,
  notify,
  onSupplement,
}: {
  record: ProbeRecord;
  notify: (text: string) => void;
  onSupplement?: () => void;
}) {
  const r = record.analysis;
  const bank = record.referenceBank ?? builtinBank;
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
      {r.results.length > 0 && <div className="rankings">
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
      </div>}
      <p className="muted probe-caveat">{r.evidence.reason} 不代表身份认证。</p>
      {record.warnings.map((w, i) => (
        <p className="notice" key={i}>
          {w}
        </p>
      ))}
      {record.modelTrace && <Disclosure title="ModelTrace 对照">
        {record.modelTrace.length ? <div className="rankings">
          <span className="meta">同一批回答 · 独立排名</span>
          {record.modelTrace.map((name, i) => <div key={i}>
            <span className="rank-index">0{i + 1}</span><span>{name}</span>
          </div>)}
        </div> : <p className="muted">ModelTrace 对照暂不可用。</p>}
      </Disclosure>}
      <footer>
        <span className="meta">
          {record.credential && `${record.credential} · `}指纹库 {bank.builtAt.slice(0, 10)}
        </span>
        {onSupplement && <button className="text-button" onClick={onSupplement}>补测第 3 次</button>}
        <button
          className="text-button"
          onClick={() =>
            void navigator.clipboard
              .writeText(
                JSON.stringify(
                  {
                    ...record,
                    detectorCommit: bank.version === builtinBank.version ? detectorCommit : undefined,
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
