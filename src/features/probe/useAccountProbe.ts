import { useRef, useState } from "react";
import type { ManagementSnapshot } from "../credentials/data";
import { generateChallenges } from "./vendor/challenge.js";
import { analyze, probeApi } from "./api";
import { accountVerdict } from "./accountMatch";
import { accountActive, accountStatusLabel } from "./accountStatus";

export function useAccountProbe(state: ManagementSnapshot, model: string, notify: (message: string) => void, inspect: (runId: number) => void) {
  const pending = useRef(new Set<string>());
  const [, render] = useState(0);
  const active = Object.values(state.accountTests ?? {}).filter(accountActive);
  const occupied = new Set([...active.map(t => t.credentialId), ...pending.current]);
  const blocked = state.busy || state.stale || !state.connected || !model;
  return {
    model, activeCount: occupied.size, inspect,
    canRetry: (id: string, runId: number) => state.accountTests?.[id]?.runId === runId,
    blocked: (id: string) => blocked || occupied.has(id) || occupied.size >= 3,
    run: async (id: string, previousRunId?: number) => {
      if (blocked || pending.current.has(id) || occupied.has(id) || new Set([...occupied, ...pending.current]).size >= 3) return;
      pending.current.add(id); render(n => n + 1);
      let runId: number | undefined;
      const name = state.credentials.find(c => c.id === id)?.name ?? "账号";
      try {
        const batch = await probeApi.accountSamples(id, model, generateChallenges(3), previousRunId);
        runId = batch.runId;
        if (batch.error) { notify(`${name}：${batch.error}`); return; }
        const detection = await analyze(batch.samples, new AbortController().signal);
        const verdict = accountVerdict(model, detection);
        await probeApi.accountFinish(id, runId, verdict.status, verdict.message);
        notify(`${name}：${accountStatusLabel(verdict.status)}，可查看历史详情`);
      } catch (e) {
        const message = typeof e === "string" ? e : e instanceof Error ? e.message : "账号探查未完成";
        notify(message);
        if (runId !== undefined) await probeApi.accountFinish(id, runId, "error", message.slice(0, 1000)).catch(() => {});
      } finally { pending.current.delete(id); render(n => n + 1); }
    },
  };
}
export type AccountProbeState = ReturnType<typeof useAccountProbe>;
