import bankUrl from "./vendor/unified_bank.json?url";
import artifactUrl from "./vendor/shared_detector.json?url";
import { analyzeSamples, type Sample } from "./analysis";
import type { Bank } from "./vendor/types";
import type { SharedDetector } from "./vendor/shared-detector";
async function read<T>(url: string, expected: string): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) throw new Error("本地参考库加载失败");
  const bytes = await response.arrayBuffer();
  const hash = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (n) => n.toString(16).padStart(2, "0"),
  ).join("");
  if (hash !== expected) throw new Error("本地参考库校验失败");
  return JSON.parse(new TextDecoder().decode(bytes));
}
self.onmessage = async (event: MessageEvent<Sample[]>) => {
  try {
    const [bank, artifact] = await Promise.all([
      read<Bank>(
        bankUrl,
        "61dade060fed71948b723f0dcde77bf5317c1e3b0bf5862fe08464d0e6d9d537",
      ),
      read<SharedDetector>(
        artifactUrl,
        "d5290bbaf02c1906191a40ef745c7b2fbc865a2feea5dae38e3d049985639926",
      ),
    ]);
    self.postMessage({ result: analyzeSamples(event.data, bank, artifact) });
  } catch (e) {
    self.postMessage({
      error: e instanceof Error ? e.message : "本地分析失败",
    });
  }
};
