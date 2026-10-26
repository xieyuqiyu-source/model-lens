import { invoke } from "@tauri-apps/api/core";
import type { Challenge, Detection, Sample } from "./analysis";
export type ExternalEntry = { id: string; name: string; baseUrl: string };
export type ProbeModel = { id: string; name: string; brand: string };
export type Answer = {
  text: string;
  finish: string;
  reportedModel: string;
  elapsedMs: number;
};
export const probeApi = {
  accountSamples: (id: string, model: string, challenges: Challenge[], previousRunId?: number) => invoke<{runId: number; samples: Sample[]; error: string | null}>("probe_account_samples", {id, model, challenges, previousRunId}),
  accountFinish: (id: string, runId: number, status: string, message: string) => invoke<void>("probe_account_finish", {id, runId, status, message}),
  models: (id: string) => invoke<ProbeModel[]>("probe_models", { id }),
  sample: (id: string, model: string, prompt: string) =>
    invoke<Answer>("probe_sample", { id, model, prompt }),
  externalList: () => invoke<ExternalEntry[]>("external_list"),
  externalSave: (id: string | null, name: string, baseUrl: string, key: string) =>
    invoke<{ id: string; models: ProbeModel[] }>("external_save", { id, name, baseUrl, key }),
  externalForget: (id: string) => invoke<void>("external_forget", { id }),
  externalModels: (id: string) => invoke<ProbeModel[]>("external_models", { id }),
  externalSample: (id: string, model: string, prompt: string) =>
    invoke<Answer>("external_sample", { id, model, prompt }),
};
export function analyze(
  samples: Sample[],
  signal: AbortSignal,
): Promise<Detection> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(new Error("分析已取消"));
      return;
    }
    const worker = new Worker(
      new URL("./detector.worker.ts", import.meta.url),
      { type: "module" },
    );
    const cleanup = () => {
      worker.terminate();
      signal.removeEventListener("abort", abort);
      clearTimeout(timer);
    };
    const abort = () => {
      cleanup();
      reject(new Error("分析已取消"));
    };
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error("本地分析超时，请重试"));
    }, 30000);
    signal.addEventListener("abort", abort, { once: true });
    worker.onerror = () => {
      cleanup();
      reject(new Error("本地检测器加载失败"));
    };
    worker.onmessage = (event) => {
      cleanup();
      event.data.error
        ? reject(new Error(event.data.error))
        : resolve(event.data.result);
    };
    worker.postMessage(samples);
  });
}
