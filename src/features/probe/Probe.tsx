import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { Select } from "../../components/Select";
import { Segment } from "../../components/Segment";
import { Disclosure } from "../../components/Disclosure";
import { Brand } from "../../components/Common";
type Props = {
  compact: boolean;
  onExpand: () => void;
  preview: () => void;
  mode: string;
  setMode: (mode: string) => void;
  model: string;
  setModel: (model: string) => void;
};
export function ModelSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <Select
      label="选择模型（预览）"
      value={value}
      onChange={onChange}
      options={[
        {
          value: "Sonnet 4.5",
          label: "Sonnet 4.5",
          brand: "claude",
          detail: "Anthropic",
        },
        { value: "GPT-5", label: "GPT-5", brand: "openai", detail: "OpenAI" },
        {
          value: "Gemini 2.5 Pro",
          label: "Gemini 2.5 Pro",
          brand: "gemini",
          detail: "Google",
        },
        {
          value: "DeepSeek V3",
          label: "DeepSeek V3",
          brand: "deepseek",
          detail: "DeepSeek",
        },
      ]}
    />
  );
}

export function Probe(p: Props) {
  const [answer, setAnswer] = useState("");
  const [example, setExample] = useState("work");
  return (
    <section
      className={`probe ${p.compact ? "compact" : ""}`}
      aria-label="模型探查"
    >
      <header className="section-heading">
        <div>
          <h1>模型探查</h1>
          <p className="muted">
            {p.compact
              ? "当前配置与最近结果"
              : "选择一个模型与凭证，查看参考指纹的匹配结果。"}
          </p>
        </div>
        {!p.compact && (
          <Segment
            label="检测模式"
            value={p.mode}
            onChange={p.setMode}
            options={[
              ["auto", "自动检测"],
              ["manual", "手动检测"],
            ]}
          />
        )}
      </header>
      <div className="probe-form">
        <label>
          <span>当前模型</span>
          <ModelSelect value={p.model} onChange={p.setModel} />
        </label>
        <label className="credential-choice">
          <span>凭证</span>
          <Select
            label="选择示例凭证"
            value={example}
            onChange={setExample}
            options={[
              {
                value: "work",
                label: "work",
                brand: "claude",
                detail: "探查界面示例",
              },
              {
                value: "personal",
                label: "personal",
                brand: "openai",
                detail: "探查界面示例",
              },
              {
                value: "backup",
                label: "backup",
                brand: "gemini",
                detail: "探查界面示例",
              },
            ]}
          />
        </label>
        {p.compact && (
          <div className="compact-account">
            <span>凭证</span>
            <span>
              <Brand name="claude" />
              work
            </span>
          </div>
        )}
        <button className="button primary start" onClick={p.preview}>
          {p.mode === "manual" ? "分析输入" : "开始检测"}
        </button>
      </div>
      {p.compact ? (
        <>
          <div className="recent-result">
            <span className="meta">最近一次 · 示例</span>
            <p>
              <Check size={14} />
              参考指纹匹配
            </p>
            <span>Claude Sonnet 4.5</span>
            <small>3 / 3 组采样完成 · 示例</small>
          </div>
          <button className="button expand-probe" onClick={p.onExpand}>
            展开探查详情
          </button>
        </>
      ) : (
        <>
          {p.mode === "manual" ? (
            <div className="manual-input">
              <div>
                <h3>粘贴模型回答</h3>
                <span className="muted">可来自任意外部模型</span>
              </div>
              <textarea
                aria-label="外部模型回答"
                value={answer}
                onChange={(e) => setAnswer(e.target.value)}
                placeholder="在这里粘贴回答，体验输入界面…"
              />
              <span className="meta">
                内容仅保留在当前窗口，分析内核将在下一阶段接入。
              </span>
            </div>
          ) : (
            <article className="result">
              <header>
                <span>检测结果</span>
                <span className="meta">界面示例</span>
              </header>
              <div className="result-title">
                <span className="result-check">
                  <Check />
                </span>
                <div>
                  <h2>参考指纹匹配</h2>
                  <p className="muted">Claude Sonnet 4.5 · work</p>
                </div>
              </div>
              <div className="rankings">
                <span className="meta">参考库内排序</span>
                {[
                  "Claude Sonnet 4.5",
                  "Claude Sonnet 4",
                  "Claude Haiku 4.5",
                ].map((name, i) => (
                  <div key={name}>
                    <span className="rank-index">0{i + 1}</span>
                    <span>{name}</span>
                    {i === 0 && <small>最接近</small>}
                  </div>
                ))}
              </div>
              <footer>
                <span className="meta">3 / 3 组采样完成 · 示例</span>
                <button
                  className="text-button"
                  onClick={() => {
                    void navigator.clipboard
                      .writeText(
                        "ModelLens 界面示例：Claude Sonnet 4.5 参考指纹匹配（非真实检测结果）",
                      )
                      .then(p.preview, p.preview);
                  }}
                >
                  <Copy size={13} />
                  复制示例结果
                </button>
              </footer>
            </article>
          )}
          <Disclosure title="检测提示词" hint="3 条 · 待接入">
            <p className="muted">
              接入检测内核后，这里将展示三条可展开、复制的检测提示词。
            </p>
          </Disclosure>
          <Disclosure title="原始回答与采样详情" hint="查看">
            <p className="muted">当前为界面预览，尚未发起真实采样。</p>
          </Disclosure>
        </>
      )}
      <footer className="probe-foot">
        上次使用：{p.mode === "auto" ? "自动检测" : "手动检测"}
        <span>本机预览版 0.2.3</span>
      </footer>
    </section>
  );
}
