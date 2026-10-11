import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { analyzeSamples, sampleIssue } from "../src/features/probe/analysis.ts";
import { collectSamples } from "../src/features/probe/run.ts";
import { generateChallenges } from "../src/features/probe/vendor/challenge.js";
const base = new URL("../src/features/probe/vendor/", import.meta.url);
const bank = JSON.parse(readFileSync(new URL("unified_bank.json", base)));
const artifact = JSON.parse(
  readFileSync(new URL("shared_detector.json", base)),
);
const samples = Array.from({ length: 3 }, (_, s) => ({
  id: `${s}`,
  slot: s + 1,
  expected_count: 300 + s,
  prompt: `challenge ${s}`,
  finish: "stop",
  text: Array.from(
    { length: 300 },
    (_, i) => ((i * (17 + s * 6) + s * 41) % 355) + 1,
  ).join(", "),
}));
test("DSH 锁定的六份上游文件逐字一致", () => {
  const hashes = {
    "challenge.js":
      "65884da44d29b7d5337993609414596272010cb07cffe98b0c5546b1fe9728e4",
    "fingerprint-core.js":
      "5a096c0e5638b9b7e266714725660549273b018e0b5eddbec3302ef96af21f16",
    "shared-detector.ts":
      "15c4b40b1d7d1efdfb72901bfcb4181b539d4d7ba69f2a1ea645094869685c21",
    "unified_bank.json":
      "61dade060fed71948b723f0dcde77bf5317c1e3b0bf5862fe08464d0e6d9d537",
    "shared_detector.json":
      "d5290bbaf02c1906191a40ef745c7b2fbc865a2feea5dae38e3d049985639926",
    LICENSE: "238afb57e498990742f84b9ce6167b4e256ac06104a9938ce953bc41c14bea51",
  };
  for (const [file, hash] of Object.entries(hashes))
    assert.equal(
      createHash("sha256")
        .update(readFileSync(new URL(file, base)))
        .digest("hex"),
      hash,
    );
});
test("同 DSH 固定样本排名与概率；部分样本绝不显示置信度", () => {
  const golden = [
    ["claude-fable-5.1", 1.6035275049253455],
    ["kimi-k3", 1.4526688419624127],
    ["kimi-k3", 1.5011732680064112],
  ];
  for (const count of [1, 2, 3]) {
    const { analysis: r } = analyzeSamples(
      samples.slice(0, count),
      bank,
      artifact,
    );
    assert.equal(r.prediction, golden[count - 1][0]);
    assert.ok(Math.abs(r.results[0].score - golden[count - 1][1]) < 1e-12);
    if (count < 3) assert.ok(r.results.every((x) => x.probability === null));
    else assert.ok(Math.abs(r.probability - 0.36420091160579576) < 1e-12);
  }
});
test("拒绝不配套库、无效样本、重复数量；校准失配只排名", () => {
  assert.throws(() =>
    analyzeSamples(samples, { ...bank, reference_sha256: "bad" }, artifact),
  );
  assert.throws(() => analyzeSamples([samples[0], samples[0]], bank, artifact));
  for (const text of [
    "",
    "1,2,3",
    Array(300).fill(234).join(","),
    "x".repeat(20001),
  ])
    assert.throws(() =>
      analyzeSamples([{ ...samples[0], text }], bank, artifact),
    );
  assert.ok(sampleIssue({ ...samples[0], finish: "incomplete" }));
  const broken = {
    ...artifact,
    calibration: {
      ...artifact.calibration,
      binding: { ...artifact.calibration.binding, base_sha256: "bad" },
    },
  };
  assert.equal(
    analyzeSamples(samples, bank, broken).analysis.probability,
    null,
  );
  const partial = analyzeSamples(
    [...samples.slice(0, 2), { ...samples[2], text: "bad" }],
    bank,
    artifact,
  );
  assert.equal(partial.analysis.used_outputs, 2);
  assert.equal(partial.warnings.length, 1);
});
test("挑战原文采样、错误停止后续请求并保留已完成回答", async () => {
  const challenges = generateChallenges(3),
    calls = [];
  const result = await collectSamples(
    challenges,
    async (prompt) => {
      calls.push(prompt);
      if (calls.length === 2) throw Error("模拟额度耗尽");
      return { ...samples[0], reportedModel: "test", elapsedMs: 1 };
    },
    new AbortController().signal,
    () => {},
  );
  assert.deepEqual(
    calls,
    challenges.slice(0, 2).map((c) => c.prompt),
  );
  assert.equal(result.samples.length, 1);
  assert.equal(result.warnings.length, 1);
});
test("取消后不再继续采样；无效回答不充作有效样本，不自动重试", async () => {
  const abort = new AbortController();
  let calls = 0;
  await assert.rejects(
    collectSamples(
      generateChallenges(3),
      async () => {
        calls++;
        abort.abort();
        return { ...samples[0], reportedModel: "test", elapsedMs: 1 };
      },
      abort.signal,
      () => {},
    ),
  );
  assert.equal(calls, 1);
  calls = 0;
  const r = await collectSamples(
    generateChallenges(3),
    async () => {
      calls++;
      return {
        ...samples[0],
        text: "1,2,3",
        reportedModel: "test",
        elapsedMs: 1,
      };
    },
    new AbortController().signal,
    () => {},
  );
  assert.equal(calls, 3);
  assert.equal(r.samples.length, 0);
  assert.equal(r.attempts.length, 3);
});
