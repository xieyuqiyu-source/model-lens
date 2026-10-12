import { useRef } from "react";
import type { ManagementSnapshot } from "../credentials/data";
import { generateChallenges } from "./vendor/challenge.js";
import { analyze, probeApi } from "./api";
import { accountVerdict } from "./accountMatch";

export function useAccountProbe(state: ManagementSnapshot, model: string, notify: (message: string) => void) {
  const active = useRef(false);
  return {
    model,
    blocked: state.busy || state.stale || !state.connected || !model || Object.values(state.accountTests ?? {}).some(t =>
      ["running", "analyzing"].includes(t.status) && Date.now() / 1000 - t.at < 420),
    run: async (id: string) => {
      if (active.current || state.busy || state.stale || !state.connected || !model) return;
      active.current = true;
      let runId: number | undefined;
      try {
        const challenges = generateChallenges(3);
        const batch = await probeApi.accountSamples(id, model, challenges.map(c => c.prompt));
        runId = batch.runId;
        if (batch.error) { notify(batch.error); return; }
        const samples = batch.answers.map((answer, i) => ({ ...challenges[i], ...answer, slot: i + 1 }));
        const detection = await analyze(samples, new AbortController().signal);
        const verdict = accountVerdict(model, detection);
        await probeApi.accountFinish(id, runId, verdict.status, verdict.message);
      } catch (e) {
        const message = typeof e === "string" ? e : e instanceof Error ? e.message : "账号测试未完成";
        notify(message);
        if (runId !== undefined) await probeApi.accountFinish(id, runId, "error", "测试未完成，请重试").catch(() => {});
      } finally { active.current = false; }
    },
  };
}
export type AccountProbeState = ReturnType<typeof useAccountProbe>;
