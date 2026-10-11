import { useEffect, useRef, useState } from "react";
import type { ManagementSnapshot } from "../credentials/data";
import { native } from "../service/api";
import { generateChallenges } from "./vendor/challenge.js";
import { analyze, probeApi, type ProbeModel } from "./api";
import { collectSamples } from "./run";
import type { Challenge, ProbeRecord, Sample } from "./analysis";
export function useProbe(credentials: ManagementSnapshot) {
  const [challenges, setChallenges] = useState<Challenge[]>(() =>
    generateChallenges(3),
  );
  const [answers, setAnswers] = useState(["", "", ""]);
  const [slot, setSlot] = useState("0");
  const [credentialId, setCredentialId] = useState("");
  const available = credentials.credentials.filter(
    (c) => c.enabled && c.probeSupported,
  );
  const credential =
    available.find((c) => c.id === credentialId) ?? available[0];
  const [models, setModels] = useState<ProbeModel[]>([]);
  const [modelId, setModelId] = useState(() => {
    try {
      return localStorage.getItem("model") ?? "";
    } catch {
      return "";
    }
  });
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === "model") setModelId(event.newValue ?? "");
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const model = models.find((m) => m.id === modelId) ?? models[0];
  const [catalogBusy, setCatalogBusy] = useState(false);
  const [catalogError, setCatalogError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [phase, setPhase] = useState("");
  const [error, setError] = useState("");
  const [attempts, setAttempts] = useState<Sample[]>([]);
  const [result, setResult] = useState<ProbeRecord | null>(null);
  const [history, setHistory] = useState<ProbeRecord[]>([]);
  const active = useRef<AbortController | null>(null);
  const mounted = useRef(true);
  const id = credential?.id ?? "";
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      active.current?.abort();
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    setModels([]);
    setCatalogError("");
    if (!native || !credentials.connected || !id) {
      setCatalogBusy(false);
      return;
    }
    setCatalogBusy(true);
    // Both windows can ask for the catalog at startup; only retry a busy read.
    async function readCatalog() {
      for (let attempt = 0; ; attempt++) {
        if (cancelled) return [];
        try {
          return await probeApi.models(id);
        } catch (e) {
          if (e !== "管理操作正在进行，请稍后重试" || attempt >= 2) throw e;
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
      }
    }
    void readCatalog()
      .then((rows) => {
        if (!cancelled) {
          setModels(rows);
          if (!rows.length)
            setCatalogError("未找到可直连的原始模型（不支持别名或前缀）");
        }
      })
      .catch((e) => {
        if (!cancelled)
          setCatalogError(typeof e === "string" ? e : "读取模型列表失败");
      })
      .finally(() => {
        if (!cancelled) setCatalogBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [credentials.connected, id, refresh]);
  async function run(mode: string) {
    if (active.current) return;
    if (
      mode !== "manual" &&
      (!native ||
        !credentials.connected ||
        credentials.busy ||
        credentials.stale ||
        !credential ||
        !model ||
        catalogBusy)
    ) {
      setError("请先连接管理接口并选择可用凭证与模型");
      return;
    }
    const controller = new AbortController();
    active.current = controller;
    setBusy(true);
    setStopping(false);
    setError("");
    setResult(null);
    setAttempts([]);
    const source = mode === "manual" ? "manual" : "auto";
    let outputs: Sample[] = challenges.map((c, i) => ({
      ...c,
      slot: i + 1,
      text: answers[i],
    }));
    let all = outputs;
    let warnings: string[] = [];
    try {
      if (source === "auto") {
        const collected = await collectSamples(
          challenges,
          (prompt) => probeApi.sample(id, model!.id, prompt),
          controller.signal,
          (current, rows) => {
            if (mounted.current) {
              setPhase(`采样 ${current} / 3`);
              setAttempts(rows);
            }
          },
        );
        outputs = collected.samples;
        all = collected.attempts;
        warnings = collected.warnings;
        if (!outputs.length)
          throw new Error(warnings.join("；") || "未取得有效回答");
      }
      controller.signal.throwIfAborted();
      setPhase("本地分析中…");
      const detection = await analyze(outputs, controller.signal);
      if (!mounted.current || controller.signal.aborted) return;
      const record: ProbeRecord = {
        ...detection,
        warnings: [...warnings, ...detection.warnings],
        id: crypto.randomUUID(),
        at: Date.now(),
        source,
        model: source === "auto" ? model!.id : "手动回答",
        credential: source === "auto" ? credential!.name : "",
        attempts: all,
      };
      setAttempts(all);
      setResult(record);
      setHistory((old) => [record, ...old].slice(0, 10));
    } catch (e) {
      if (mounted.current)
        setError(
          controller.signal.aborted
            ? "已停止，未继续采样或评分"
            : typeof e === "string"
              ? e
              : e instanceof Error
                ? e.message
                : "检测未完成",
        );
    } finally {
      active.current = null;
      if (mounted.current) {
        setBusy(false);
        setStopping(false);
        setPhase("");
      }
    }
  }
  return {
    challenges,
    answers,
    setAnswers: (values: string[]) => {
      setAnswers(values);
      setResult(null);
      setError("");
    },
    slot,
    setSlot,
    available,
    credential,
    setCredentialId,
    models,
    model,
    setModelId: (value: string) => {
      setModelId(value);
      try {
        localStorage.setItem("model", value);
      } catch {
        /* Optional preference. */
      }
    },
    catalogBusy,
    catalogError,
    refreshModels: () => setRefresh((n) => n + 1),
    busy,
    stopping,
    phase,
    error,
    attempts,
    result,
    history,
    run,
    stop: () => {
      active.current?.abort();
      setStopping(true);
    },
    newRound: () => {
      if (!active.current) {
        setChallenges(generateChallenges(3));
        setAnswers(["", "", ""]);
        setResult(null);
        setAttempts([]);
        setError("");
      }
    },
    showHistory: (record: ProbeRecord) => {
      if (!active.current) {
        setResult(record);
        setAttempts(record.attempts);
      }
    },
  };
}
export type ProbeState = ReturnType<typeof useProbe>;
