import { useState } from "react";
import { Copy } from "lucide-react";
import { Segment } from "../../components/Segment";
import type { ProbeState } from "./useProbe";

export function ManualProbe({ state, copy }: { state: ProbeState; copy: (text: string) => void }) {
  const [slot, setSlot] = useState("0");
  const index = Number(slot);
  const hasAnswers = state.answers.some((answer) => answer.trim());

  return (
    <fieldset className="manual-probe" disabled={state.busy} aria-label="手动评分">
      <p className="muted">分别在新对话发送提示词，再粘贴对应的原始回答。</p>
      <div className="manual-probe-tools">
        <Segment
          label="回答序号"
          value={slot}
          onChange={setSlot}
          options={[
            ["0", "回答 1"],
            ["1", "回答 2"],
            ["2", "回答 3"],
          ]}
        />
        <button type="button" className="text-button" onClick={() => copy(state.challenges[index].prompt)}>
          <Copy size={13} />
          复制对应提示词
        </button>
      </div>
      <textarea
        aria-label={`模型回答 ${index + 1}`}
        rows={2}
        value={state.answers[index]}
        onChange={(event) => state.setAnswer(index, event.target.value)}
        placeholder={`粘贴第 ${index + 1} 条提示词的模型回答`}
        spellCheck={false}
      />
      <div className="manual-probe-tools">
        <button type="button" className="text-button" onClick={state.newRound}>
          {hasAnswers ? "更换提示词并清空回答" : "新一组提示词"}
        </button>
        <button type="button" className="button primary" disabled={!hasAnswers} onClick={() => void state.run()}>
          开始评分
        </button>
      </div>
    </fieldset>
  );
}
