import type { Detection } from "./analysis";
export type AccountVerdict = { status: "match" | "mismatch" | "unknown"; message: string };
function canonical(model: string, known: Set<string>): string | undefined {
  let id = model.toLowerCase();
  if (id === "gpt-6.1-sol") id = "gpt-6-astra"; // The reference library explicitly groups these two names.
  if (known.has(id)) return id;
  id = id.replace(/\((?:high|medium|low|minimal|none)\)$/, "").replace(/-(?:high|medium|low|minimal|none)$/, "");
  id = id.replace(/^(claude-(?:haiku|sonnet|opus)-\d+)-(\d+)(?=-|$)/, "$1.$2").replace(/-\d{8}$/, "");
  return known.has(id) ? id : undefined;
}
export function accountVerdict(model: string, detection: Detection): AccountVerdict {
  const a = detection.analysis;
  const known = new Set(a.results.map(r => r.model));
  const expected = canonical(model, known);
  if (!expected) return { status: "unknown", message: `${model}：参考库没有可确认的对应型号` };
  if (a.used_outputs !== 3 || a.probability_status !== "reference_calibrated" || (a.probability ?? 0) < 0.8)
    return { status: "unknown", message: `${model}：证据不足，尚不能判断是否匹配` };
  const matches = expected === a.prediction;
  return { status: matches ? "match" : "mismatch", message: `${model} · ${matches ? "库内匹配" : "库内不匹配"}：${a.prediction_name}（${((a.probability ?? 0) * 100).toFixed(1)}%）；不代表身份认证` };
}
