import { useState } from "react";
import { Copy, Pencil, RefreshCw } from "lucide-react";
import { Segment } from "../../components/Segment";
import { ExternalApi } from "./ExternalApi";
import { PlanBadge } from "../credentials/PlanBadge";
import { Select } from "../../components/Select";
import { Disclosure } from "../../components/Disclosure";
import { ManualProbe } from "./ManualProbe";
import { ProbeResult } from "./ProbeResult";
import { ReferenceBank } from "./ReferenceBank";
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
              brand: m.brand,
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
  state: ProbeState;
  notify: (text: string) => void;
  onConnect: () => void;
  connected: boolean;
  managementBusy: boolean;
  stale: boolean;
};
const ADD = "\u0000add";
export function Probe(p: Props) {
  const s = p.state;
  const [editing, setEditing] = useState<"" | "new" | "edit">("");
  const formBusy = s.busy || (!s.external && p.managementBusy);
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
        <fieldset className="probe-mode-controls" disabled={s.busy}>
          <Segment
            label="检测模式"
            value={s.manual ? "manual" : "auto"}
            onChange={(mode) => {
              s.setMode(mode);
              if (mode === "manual" && p.compact) p.onExpand();
            }}
            options={[
              ["auto", "自动检测"],
              ["manual", "手动检测"],
            ]}
          />
          {!s.manual && (
            <Segment
              label="探查来源"
              value={s.source}
              onChange={s.setSource}
              options={[
                ["cliproxy", "CLIProxy"],
                ["external", "第三方 API"],
              ]}
            />
          )}
        </fieldset>
      </header>
      {!s.manual && (
        <fieldset className="probe-form" disabled={formBusy}>
          <label>
            <span>模型</span>
            <ModelSelect state={s} />
          </label>
          {s.external ? (
            <div className="credential-choice probe-field">
              <span>第三方 API</span>
              {s.entry ? (
                <div className="external-pick">
                  <Select
                    label="选择第三方 API"
                    value={s.entry.id}
                    onChange={(v) => (v === ADD ? setEditing("new") : s.setEntryId(v))}
                    options={[
                      ...s.entries.map((e) => ({ value: e.id, label: e.name, detail: e.baseUrl })),
                      { value: ADD, label: "＋ 添加第三方 API" },
                    ]}
                  />
                  <button type="button" className="icon-button" aria-label="编辑第三方 API" title="编辑或删除" onClick={() => setEditing("edit")}>
                    <Pencil size={14} />
                  </button>
                </div>
              ) : (
                <button type="button" className="button" onClick={() => setEditing("new")}>
                  添加第三方 API
                </button>
              )}
            </div>
          ) : (
            <label className="credential-choice">
              <span>模型范围</span>
              <Select
                label="选择模型范围"
                value={s.credential?.id ?? ""}
                onChange={s.setCredentialId}
                options={[
                  { value: "", label: "全部已启用凭证" },
                  ...s.available.map((c) => ({ value: c.id, label: c.name, brand: c.brand, detail: c.provider, badge: <PlanBadge credential={c} /> })),
                ]}
              />
            </label>
          )}
          <button
            className="button primary start"
            disabled={!s.ready || !s.model || s.catalogBusy}
            onClick={() => void s.run()}
          >
            开始检测
          </button>
        </fieldset>
      )}

      {s.manual && !p.compact && (
        <ManualProbe key={s.challenges[0].id} state={s} copy={copy} />
      )}
      {!s.manual && !s.busy && (
        <div className="probe-status">
          <span className="muted">
            {s.external
              ? !s.entry
                ? "添加第三方 API 后可探查"
                : s.catalogError || "3 次采样 · 消耗额度 · 第三方 API 直连"
              : !p.connected
                ? "连接管理接口后可自动探查"
                : s.catalogError ||
                  "3 次采样 · 消耗额度 · CLIProxy 路由"}
          </span>
          <button
            className="text-button"
            disabled={formBusy || s.catalogBusy}
            onClick={
              s.external
                ? s.entry ? s.refreshModels : () => setEditing("new")
                : p.connected ? s.refreshModels : p.onConnect
            }
          >
            {(s.external ? s.entry : p.connected) ? (
              <>
                <RefreshCw size={12} />
                刷新模型
              </>
            ) : (
              s.external ? "添加" : "连接"
            )}
          </button>
        </div>
      )}
      {s.busy && (
        <div className="probe-status" role="status">
          <span>
            {s.stopping && !s.manual ? "等待当前请求结束，停止后续采样…" : s.phase}
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
                : s.manual ? "展开后粘贴模型回答" : "选择模型后开始采样"}
            </small>
          </div>
          <button className="button expand-probe" onClick={p.onExpand}>
            展开探查详情
          </button>
        </>
      ) : (
        <>
          {s.result ? (
            <ProbeResult record={s.result} notify={p.notify} />
          ) : (
            !s.busy && !s.manual &&
            (
              <article className="result">
                <p className="muted">尚无检测结果</p>
              </article>
            )
          )}
          <Disclosure title="检测提示词">
            {!s.manual && (
              <div className="probe-status">
                <button
                  className="text-button"
                  disabled={s.busy}
                  onClick={s.newRound}
                >
                  {s.answers.some((answer) => answer.trim()) ? "更换提示词并清空回答" : "新一组提示词"}
                </button>
              </div>
            )}
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
      {editing && (
        <ExternalApi
          state={s}
          entry={editing === "edit" ? s.entry ?? null : null}
          notify={p.notify}
          onClose={() => setEditing("")}
        />
      )}
      <footer className="probe-foot">
        <ReferenceBank notify={p.notify} />
      </footer>
    </section>
  );
}
