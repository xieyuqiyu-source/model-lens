import { sampleIssue, type Challenge, type Sample } from "./analysis.ts";
export async function collectSamples(
  challenges: Challenge[],
  ask: (
    prompt: string,
  ) => Promise<{
    text: string;
    finish: string;
    reportedModel: string;
    elapsedMs: number;
  }>,
  signal: AbortSignal,
  progress: (slot: number, attempts: Sample[]) => void,
) {
  const samples: Sample[] = [],
    attempts: Sample[] = [],
    warnings: string[] = [];
  const started = Date.now();
  if (
    challenges.length !== 3 ||
    new Set(challenges.map((c) => c.expected_count)).size !== 3
  )
    throw new Error("检测提示词无效");
  for (const [index, challenge] of challenges.entries()) {
    signal.throwIfAborted();
    if (Date.now() - started >= 300000) {
      warnings.push("本轮已达到 5 分钟，停止后续采样");
      break;
    }
    progress(index + 1, [...attempts]);
    try {
      const answer = await ask(challenge.prompt);
      signal.throwIfAborted();
      const sample = { ...challenge, ...answer, slot: index + 1 };
      attempts.push(sample);
      const issue = sampleIssue(sample);
      if (issue) warnings.push(`回答 ${index + 1}：${issue}`);
      else samples.push(sample);
      progress(index + 1, [...attempts]);
    } catch (error) {
      signal.throwIfAborted();
      warnings.push(
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "模型请求失败",
      );
      break;
    }
  }
  return { samples, attempts, warnings };
}
