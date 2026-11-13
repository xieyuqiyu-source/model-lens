import {
  analyzeSharedOutputs,
  supportsSharedDetector,
  type SharedDetector,
} from "./vendor/shared-detector.ts";
import { parseNumbers } from "./vendor/fingerprint-core.js";
import type { Analysis, Bank } from "./vendor/types.ts";
export type Challenge = { id: string; prompt: string; expected_count: number };
export type Sample = Challenge & {
  slot: number;
  text: string;
  finish?: string;
  reportedModel?: string;
  elapsedMs?: number;
};
export const detectorCommit = "4ffda94e5e24a05c4f55d6f14a1e27f4ef23c8f3";
export type BankInfo = { builtAt: string; modelCount: number; version: string };
export const builtinBank: BankInfo = {
  builtAt: "2026-10-07T20:04:35.858118+00:00",
  modelCount: 58,
  version: "builtin",
};
export function sampleIssue(sample: Sample): string {
  if (sample.text.length > 20000) return "回答超过 20000 字符";
  if (sample.finish && sample.finish !== "stop") return "回答未正常结束";
  const numbers: number[] = parseNumbers(sample.text);
  const minimum = Math.max(80, Math.ceil(sample.expected_count * 0.55));
  if (numbers.length < minimum)
    return `有效整数不足（${numbers.length}/${minimum}）`;
  if (numbers.every((n) => n >= 200)) return "数字分布异常（全部不小于 200）";
  return "";
}
export function prepareSamples(samples: Sample[]) {
  if (
    samples.length > 3 ||
    !samples.length ||
    new Set(samples.map((s) => s.expected_count)).size !== samples.length ||
    samples.some(
      (s) =>
        !Number.isInteger(s.expected_count) ||
        s.expected_count < 292 ||
        s.expected_count > 332 ||
        !s.prompt.trim() ||
        s.prompt.length > 4000,
    )
  )
    throw new Error("请使用本轮三条提示词分别采样");
  const warnings: string[] = [];
  const accepted = samples.filter((s) => {
    if (!s.text.trim()) return false;
    const issue = sampleIssue(s);
    if (issue) warnings.push(`回答 ${s.slot}：${issue}`);
    return !issue;
  });
  if (!accepted.length)
    throw new Error(warnings.join("；") || "未取得有效模型回答");
  return { warnings, samples: accepted };
}
export function analyzeSamples(samples: Sample[], bank: Bank, artifact: SharedDetector) {
  if (
    !supportsSharedDetector(bank, artifact) ||
    bank.reference_sha256 !== artifact.source_reference_sha256
  )
    throw new Error("参考库与检测器不匹配");
  const prepared = prepareSamples(samples);
  return {
    ...prepared,
    analysis: analyzeSharedOutputs(prepared.samples, bank, artifact, {
      allowPartial: true,
    }),
    referenceBank: builtinBank,
  };
}
export type Detection = {
  warnings: string[];
  samples: Sample[];
  analysis: Pick<Analysis, "prediction" | "prediction_name" | "probability" |
    "evidence" | "used_outputs" | "decision"> & {
    results: Omit<Analysis["results"][number], "absolute_match">[];
    probability_status?: string;
  };
  referenceBank: BankInfo;
};
export type ProbeRecord = Detection & {
  id: string;
  at: number;
  source: "auto" | "manual" | "account";
  model: string;
  credential: string;
  attempts: Sample[];
};
