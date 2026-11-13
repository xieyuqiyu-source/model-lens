import { invoke, isTauri } from "@tauri-apps/api/core";
import { builtinBank, prepareSamples, type BankInfo, type Challenge, type Detection, type Sample } from "./analysis";
export type ExternalEntry = { id: string; name: string; baseUrl: string };
export type ProbeModel = { id: string; name: string; brand: string };
export type Answer = {
  text: string;
  finish: string;
  reportedModel: string;
  elapsedMs: number;
};
export const probeApi = {
  accountSamples: (id: string, model: string, challenges: Challenge[], bankVersion: string, previousRunId?: number) => invoke<{runId: number; samples: Sample[]; error: string | null}>("probe_account_samples", {id, model, challenges, bankVersion, previousRunId}),
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
export type BankStatus = { bank: BankInfo; busy: boolean; message: string; revision: number };
export const referenceApi = {
  status: () => invoke<BankStatus>("probe_reference_status"),
  update: () => invoke<void>("probe_reference_update"),
};
export async function currentBank(): Promise<BankInfo> {
  return isTauri() ? (await referenceApi.status()).bank : builtinBank;
}
async function analyzeUpdated(samples: Sample[], version: string, signal: AbortSignal): Promise<Detection> {
  signal.throwIfAborted();
  const prepared = prepareSamples(samples);
  const outputs = prepared.samples.map(({ text, expected_count }) => ({ text, expected_count }));
  const scoring = await invoke<Pick<Detection, "analysis" | "referenceBank">>("probe_reference_analyze", { version, outputs });
  signal.throwIfAborted();
  const { analysis } = scoring;
  if (analysis.decision === "not_model") {
    analysis.prediction_name = "样本分布异常";
    analysis.evidence.label = "无法评分";
    analysis.evidence.reason = "数字的相邻变化与库内模型回答不同。请核对回答或换一组提示词重测。";
  } else if (analysis.decision === "unscorable") {
    analysis.prediction_name = "暂不可评分";
    analysis.evidence.label = "有效样本不足";
    analysis.evidence.reason = "请补齐有效回答后重新检测。";
  } else if (analysis.decision === "partial") {
    analysis.evidence.label = "部分样本排名";
    analysis.evidence.reason = "补齐三条有效回答后可计算库内置信度。";
  } else {
    analysis.evidence.label = analysis.probability === null ? "未校准排名" : "库内校准排名";
    analysis.evidence.reason = analysis.probability === null
      ? "当前指纹库只提供候选排名。"
      : "置信度只在参考库收录的模型之间计算。";
  }
  return { ...prepared, ...scoring };
}
export function analyze(
  samples: Sample[],
  signal: AbortSignal,
  version = builtinBank.version,
): Promise<Detection> {
  if (version !== builtinBank.version) return analyzeUpdated(samples, version, signal);
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
