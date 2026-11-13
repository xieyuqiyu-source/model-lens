import { useEffect, useRef, useState } from "react";
import type { ManagementSnapshot } from "../credentials/data";
import { native } from "../service/api";
import { generateChallenges } from "./vendor/challenge.js";
import { analyze, currentBank, probeApi, type ExternalEntry, type ProbeModel } from "./api";
import { collectSamples } from "./run";
import type { Challenge, ProbeRecord, Sample } from "./analysis";
export function useProbe(credentials: ManagementSnapshot) {
  const [challenges, setChallenges] = useState<Challenge[]>(() =>
    generateChallenges(3),
  );
  const [manual, setManual] = useState(false);
  const [answers, setAnswers] = useState(["", "", ""]);
  const [credentialId, setCredentialId] = useState("");
  const available = credentials.credentials.filter(
    (c) => c.enabled,
  );
  const credential =
    available.find((c) => c.id === credentialId);
  const [models, setModels] = useState<ProbeModel[]>([]);
  const [modelId, setModelId] = useState(() => {
    try {
      return localStorage.getItem("model") ?? "";
    } catch {
      return "";
    }
  });
  const [source, setSourceState] = useState(() => {
    try {
      return localStorage.getItem("probeSource") === "external" ? "external" : "cliproxy";
    } catch {
      return "cliproxy";
    }
  });
  const external = source === "external";
  const [entries, setEntries] = useState<ExternalEntry[]>([]);
  const [entryId, setEntryIdState] = useState(() => readStored("externalId"));
  const entry = entries.find((e) => e.id === entryId) ?? entries[0];
  const loadEntries = () =>
    native ? probeApi.externalList().then((rows) => setEntries(rows ?? []), () => {}) : Promise.resolve();
  useEffect(() => {
    void loadEntries();
  }, []);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === "model") setModelId(event.newValue ?? "");
      // Keep main window and tray on the same source and saved endpoint.
      if (event.key === "probeSource") setSourceState(event.newValue === "external" ? "external" : "cliproxy");
      if (event.key === "externalId") setEntryIdState(event.newValue ?? "");
      if (event.key === "externalApi") void loadEntries();
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const ready = external ? !!entry : credentials.connected && !credentials.busy && !credentials.stale;
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
  const enabledKey = available.map((c) => c.id).sort().join("\n");
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
    if (!native || (external ? !entry : !credentials.connected)) {
      setCatalogBusy(false);
      return;
    }
    setCatalogBusy(true);
    void (external ? probeApi.externalModels(entry!.id) : probeApi.models(id))
      .then((rows) => {
        if (!cancelled) {
          setModels(rows);
          if (!rows.length)
            setCatalogError(external ? "第三方 API 未返回可用模型" : "CLIProxy 未返回可用模型");
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
  }, [credentials.connected, credentials.port, enabledKey, id, refresh, external, entry?.id, entry?.baseUrl]);
  async function run() {
    if (active.current) return;
    const pasted: Sample[] = manual
      ? challenges
          .map((challenge, i) => ({ ...challenge, slot: i + 1, text: answers[i] }))
          .filter((sample) => sample.text.trim())
      : [];
    if (manual && !pasted.length) {
      setError("请粘贴至少一份模型回答");
      return;
    }
    if (!manual && (!native || !ready || !model || catalogBusy)) {
      setError(external ? "请先填写第三方 API 并选择可用模型" : "请先连接管理接口并选择可用模型");
      return;
    }
    const controller = new AbortController();
    active.current = controller;
    setBusy(true);
    setStopping(false);
    setError("");
    setResult(null);
    setAttempts(pasted);
    try {
      const bank = await currentBank();
      controller.signal.throwIfAborted();
      const collected = manual
        ? { samples: pasted, attempts: pasted, warnings: [] }
        : await collectSamples(
            challenges,
            (prompt) =>
              external
                ? probeApi.externalSample(entry!.id, model!.id, prompt)
                : probeApi.sample(id, model!.id, prompt),
            controller.signal,
            (current, rows) => {
              if (mounted.current) {
                setPhase(`采样 ${current} / 3`);
                setAttempts(rows);
              }
            },
          );
      const outputs = collected.samples;
      const all = collected.attempts;
      const warnings = collected.warnings;
      if (!outputs.length)
        throw new Error(warnings.join("；") || "未取得有效回答");
      controller.signal.throwIfAborted();
      setPhase("本地分析中…");
      const detection = await analyze(outputs, controller.signal, bank.version);
      if (!mounted.current || controller.signal.aborted) return;
      const record: ProbeRecord = {
        ...detection,
        warnings: [...warnings, ...detection.warnings],
        id: crypto.randomUUID(),
        at: Date.now(),
        source: manual ? "manual" : "auto",
        model: manual ? "手动回答" : model!.id,
        credential: manual ? "" : external ? `第三方 API · ${entry!.name}` : "CLIProxy 自动路由",
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
    manual,
    setMode: (mode: string) => {
      const next = mode === "manual";
      if (active.current || manual === next) return;
      setManual(next);
      setError("");
      setResult(null);
      setAttempts([]);
    },
    answers,
    setAnswer: (index: number, text: string) => {
      if (active.current) return;
      setAnswers((old) => old.map((answer, i) => i === index ? text : answer));
      setError("");
      setResult(null);
      setAttempts([]);
    },
    source,
    external,
    ready,
    entries,
    entry,
    setEntryId: (value: string) => {
      if (active.current) return;
      setEntryIdState(value);
      store("externalId", value);
    },
    setSource: (value: string) => {
      if (active.current) return;
      setSourceState(value);
      try {
        localStorage.setItem("probeSource", value);
      } catch {
        /* Optional preference. */
      }
    },
    saveExternal: async (id: string | null, name: string, baseUrl: string, key: string) => {
      const saved = await probeApi.externalSave(id, name, baseUrl, key);
      await loadEntries();
      setEntryIdState(saved.id);
      store("externalId", saved.id);
      // Same entry edited: force the model list to reload from the new address.
      setRefresh((n) => n + 1);
      store("externalApi", String(Date.now()));
      return saved.models;
    },
    forgetExternal: async (id: string) => {
      await probeApi.externalForget(id);
      await loadEntries();
      store("externalApi", String(Date.now()));
    },
    challenges,
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
        setChallenges(generateChallenges(3, challenges.map((challenge) => challenge.expected_count)));
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
function readStored(key: string) {
  try {
    return localStorage.getItem(key) ?? "";
  } catch {
    return "";
  }
}
function store(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Optional preference; other window syncs on next launch. */
  }
}
export type ProbeState = ReturnType<typeof useProbe>;
