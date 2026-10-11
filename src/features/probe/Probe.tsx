import { Copy, RefreshCw } from "lucide-react";
import { Select } from "../../components/Select";
import { Segment } from "../../components/Segment";
import { Disclosure } from "../../components/Disclosure";
import { ProbeResult } from "./ProbeResult";
import type { ProbeState } from "./useProbe";
import "../../styles/probe.css";
export function ModelSelect({ state }: { state: ProbeState }) {
  return (
    <Select
      label="选择模型"
      value={state.model?.id ?? ""}
      onChange={state.setModelId}
      options={
        state.models.length
          ? state.models.map((m) => ({
              value: m.id,
              label: m.name,
              brand: state.credential?.brand,
            }))
          : [
              {
                value: "",
                label: state.catalogBusy ? "加载模型中…" : "暂无可用模型",
              },
            ]
      }
    />
  );
}
type Props = {
  compact: boolean;
  onExpand: () => void;
  mode: string;
  setMode: (mode: string) => void;
  state: ProbeState;
  notify: (text: string) => void;
  onConnect: () => void;
  connected: boolean;
  managementBusy: boolean;
  stale: boolean;
};
export function Probe(p: Props) {
  const s = p.state,
    manual = p.mode === "manual";
  const copy = (text: string) =>
    void navigator.clipboard.writeText(text).then(
      () => p.notify("提示词已复制"),
      () => p.notify("复制失败，请重试"),
    );
  return (
    <section
      className={`probe ${p.compact ? "compact" : ""}`}
      aria-label="模型探查"
      tabIndex={0}
    >
      <header className="section-heading">
        <div>
          <h1>模型探查</h1>
        </div>
        {!p.compact && (
          <fieldset disabled={s.busy} className="probe-controls">
            <Segment
              label="检测模式"
              value={p.mode}
              onChange={p.setMode}
              options={[
                ["auto", "自动检测"],
                ["manual", "手动检测"],
              ]}
            />
          </fieldset>
        )}
      </header>
      {!manual && (
        <fieldset className="probe-form" disabled={s.busy || p.managementBusy}>
          <label>
            <span>原始模型</span>
            <ModelSelect state={s} />
          </label>
          <label className="credential-choice">
            <span>凭证</span>
            <Select
              label="选择探查凭证"
              value={s.credential?.id ?? ""}
              onChange={s.setCredentialId}
              options={
                s.available.length
                  ? s.available.map((c) => ({
                      value: c.id,
                      label: c.name,
                      brand: c.brand,
                      detail: c.provider,
                    }))
                  : [{ value: "", label: "无可用凭证" }]
              }
            />
          </label>
          <button
            className="button primary start"
            disabled={!p.connected || p.stale || !s.model || s.catalogBusy}
            onClick={() => void s.run("auto")}
          >
            开始检测
          </button>
        </fieldset>
      )}
      {!manual && !s.busy && (
        <div className="probe-status">
          <span className="muted">
            {!p.connected
              ? "连接管理接口后可自动探查"
              : s.catalogError ||
                (!s.available.length
                  ? "自动探查支持 Codex / Claude OAuth，其他模型可手动分析"
                  : "3 次采样 · 消耗额度 · 不应用代理别名")}
          </span>
          <button
            className="text-button"
            disabled={p.managementBusy || s.catalogBusy}
            onClick={p.connected ? s.refreshModels : p.onConnect}
          >
            {p.connected ? (
              <>
                <RefreshCw size={12} />
                刷新模型
              </>
            ) : (
              "连接"
            )}
          </button>
        </div>
      )}
      {s.busy && (
        <div className="probe-status" role="status">
          <span>
            {s.stopping ? "等待当前请求结束，停止后续采样…" : s.phase}
          </span>
          <button
            className="text-button"
            disabled={s.stopping}
            onClick={s.stop}
          >
            停止
          </button>
        </div>
      )}
      {s.error && (
        <p role="alert" className="notice">
          {s.error}
        </p>
      )}
      {p.compact ? (
        <>
          <div className="recent-result">
            <span className="meta">最近一次</span>
            <p>{s.result?.analysis.prediction_name ?? "尚未检测"}</p>
            <small>
              {s.result
                ? `${s.result.analysis.used_outputs} / 3 有效样本 · ${s.result.analysis.evidence.label}`
                : "选择模型后开始采样"}
            </small>
          </div>
          <button className="button expand-probe" onClick={p.onExpand}>
            展开探查详情
          </button>
        </>
      ) : (
        <>
          {manual && (
            <div className="manual-input">
              <div>
                <h3>粘贴模型回答</h3>
                <fieldset className="probe-controls" disabled={s.busy}>
                  <Segment
                    label="回答序号"
                    value={s.slot}
                    onChange={s.setSlot}
                    options={[
                      ["0", "回答 1"],
                      ["1", "回答 2"],
                      ["2", "回答 3"],
                    ]}
                  />
                </fieldset>
              </div>
              <textarea
                aria-label={`模型回答 ${Number(s.slot) + 1}`}
                rows={2}
                maxLength={20000}
                disabled={s.busy}
                value={s.answers[Number(s.slot)]}
                onChange={(e) =>
                  s.setAnswers(
                    s.answers.map((a, i) =>
                      i === Number(s.slot) ? e.target.value : a,
                    ),
                  )
                }
                placeholder="分别在全新对话中发送对应提示词，再粘贴回答…"
              />
              <div>
                <button
                  className="text-button"
                  onClick={() => copy(s.challenges[Number(s.slot)].prompt)}
                >
                  <Copy size={13} />
                  复制对应提示词
                </button>
                <button
                  className="button primary"
                  disabled={s.busy || !s.answers.some((a) => a.trim())}
                  onClick={() => void s.run("manual")}
                >
                  分析输入
                </button>
              </div>
            </div>
          )}
          {s.result ? (
            <ProbeResult record={s.result} notify={p.notify} />
          ) : (
            !s.busy &&
            !manual && (
              <article className="result">
                <p className="muted">尚无检测结果</p>
              </article>
            )
          )}
          <Disclosure title="检测提示词">
            <div className="probe-status">
              <span className="muted">LM Detector · 58 个参考模型</span>
              <button
                className="text-button"
                disabled={s.busy}
                onClick={s.newRound}
              >
                新一组提示词
              </button>
            </div>
            {s.challenges.map((c, i) => (
              <div className="probe-sample" key={c.id}>
                <div className="probe-status">
                  <strong>
                    提示词 {i + 1} · {c.expected_count} 个整数
                  </strong>
                  <button
                    className="text-button"
                    onClick={() => copy(c.prompt)}
                  >
                    <Copy size={12} />
                    复制
                  </button>
                </div>
                <p>{c.prompt}</p>
              </div>
            ))}
          </Disclosure>
          <Disclosure title="原始回答与采样详情">
            {s.attempts.length ? (
              s.attempts.map((a) => (
                <div className="probe-sample" key={a.slot}>
                  <strong>
                    回答 {a.slot} ·{" "}
                    {a.finish === "stop"
                      ? "正常结束"
                      : a.finish
                        ? "未完整结束"
                        : "手动输入"}
                    {a.elapsedMs !== undefined
                      ? ` · ${(a.elapsedMs / 1000).toFixed(1)} 秒`
                      : ""}
                  </strong>
                  {a.reportedModel && (
                    <p className="muted">服务返回型号：{a.reportedModel}</p>
                  )}
                  <p className="muted">{a.prompt}</p>
                  <pre>{a.text || "没有文本回答"}</pre>
                </div>
              ))
            ) : (
              <p className="muted">暂无采样记录。</p>
            )}
          </Disclosure>
        </>
      )}
      <footer className="probe-foot">
        上次使用：{manual ? "手动检测" : "自动检测"}
        <span>本机预览版 0.2.11</span>
      </footer>
    </section>
  );
}
