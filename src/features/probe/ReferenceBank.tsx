import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { native } from "../service/api";
import { builtinBank } from "./analysis";
import { referenceApi, type BankStatus } from "./api";

export function ReferenceBank({ notify }: { notify: (message: string) => void }) {
  const [status, setStatus] = useState<BankStatus>({
    bank: builtinBank, busy: false, message: "", revision: -1,
  });
  useEffect(() => {
    if (!native) return;
    let stopped = false;
    let dispose: (() => void) | undefined;
    const apply = (next: BankStatus) => {
      if (!stopped) setStatus(old => next.revision >= old.revision ? next : old);
    };
    void (async () => {
      const stop = await listen<BankStatus>("probe-reference-changed", event => apply(event.payload));
      if (stopped) { stop(); return; }
      dispose = stop;
      apply(await referenceApi.status());
    })().catch(() => {
      if (!stopped) setStatus(old => ({ ...old, message: "无法读取指纹库状态" }));
    });
    return () => { stopped = true; dispose?.(); };
  }, []);
  const { bank, busy, message } = status;
  return (
    <div className="probe-bank">
      <span title={bank.version === builtinBank.version ? "随应用提供的内置库" : bank.version}>
        指纹库 {bank.builtAt.slice(0, 10)} · {bank.modelCount} 个模型
      </span>
      {native && (
        <button
          type="button"
          className="text-button"
          aria-label="立即更新指纹库"
          disabled={busy}
          onClick={() => void referenceApi.update().catch(() => notify("无法启动指纹库更新"))}
        >
          {busy ? "检查中…" : "立即更新"}
        </button>
      )}
      {message && <span className="probe-bank-message" role="status">{message}</span>}
    </div>
  );
}
