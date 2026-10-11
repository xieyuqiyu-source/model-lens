import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { discoverServices, native, type Snapshot } from "./api";
export function useServices() {
  const [snapshot, setSnapshot] = useState<Snapshot>({
    services: [],
    checkedAt: 0,
    statusKnown: false,
    warning: null,
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const refresh = useCallback(async () => {
    setBusy(true);
    setError("");
    try {
      setSnapshot(await discoverServices());
    } catch (e) {
      setError(typeof e === "string" ? e : "检查未完成，请重试");
    } finally {
      setBusy(false);
    }
  }, []);
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 30000);
    const listener = native
      ? listen("refresh-service", () => void refresh())
      : null;
    return () => {
      window.clearInterval(timer);
      void listener?.then((unlisten) => unlisten());
    };
  }, [refresh]);
  return {
    snapshot,
    busy,
    error,
    refresh,
    known: snapshot.statusKnown && !error,
    running:
      snapshot.statusKnown &&
      !error &&
      snapshot.services.some((s) => s.pid !== null),
  };
}
export type ServiceState = ReturnType<typeof useServices>;
