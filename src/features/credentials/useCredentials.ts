import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { native } from "../service/api";
import { managementApi } from "./api";
import { credentialTargets, type Credential, type ManagementSnapshot } from "./data";
const empty: ManagementSnapshot = {
  connected: false,
  saved: false,
  port: 8317,
  busy: native,
  stale: false,
  credentials: [],
  error: null,
  revision: 0,
};
export function useCredentials() {
  const [snapshot, setSnapshot] = useState(empty);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const mounted = useRef(true);
  const pendingRef = useRef(false);
  const apply = useCallback((next: ManagementSnapshot) => {
    if (mounted.current)
      setSnapshot((old) => (next.revision >= old.revision ? next : old));
  }, []);
  useEffect(() => {
    mounted.current = true;
    let dispose: (() => void) | undefined;
    let cancelled = false;
    if (native)
      void (async () => {
        const stop = await listen<ManagementSnapshot>(
          "credentials-changed",
          (e) => apply(e.payload),
        );
        if (cancelled) {
          stop();
          return;
        }
        dispose = stop;
        const initial = await managementApi.snapshot();
        if (!cancelled) apply(initial);
      })().catch(() => {
        if (!cancelled && mounted.current) {
          setSnapshot((old) => ({ ...old, busy: false }));
          setError("无法读取管理连接状态，请重试连接");
        }
      });
    return () => {
      cancelled = true;
      mounted.current = false;
      dispose?.();
    };
  }, [apply]);
  const run = async (action: () => Promise<ManagementSnapshot>) => {
    if (!native) {
      setError("请在 ModelLens 应用内连接本机服务");
      return false;
    }
    if (pendingRef.current) return false;
    pendingRef.current = true;
    setPending(true);
    setError("");
    try {
      const result = await action();
      apply(result);
      return !result.error;
    } catch {
      if (mounted.current) setError("操作未完成，请刷新列表核实，不要重复提交");
      return false;
    } finally {
      pendingRef.current = false;
      if (mounted.current) setPending(false);
    }
  };
  return {
    ...snapshot,
    busy: snapshot.busy || pending,
    error: error || snapshot.error,
    connect: (port: number, key: string) =>
      run(() => managementApi.connect(port, key)),
    disconnect: () => run(managementApi.disconnect),
    restore: () => run(managementApi.restore),
    forget: () => run(managementApi.forget),
    refresh: () => run(managementApi.list),
    refreshQuota: (id: string | null = null) =>
      run(() => managementApi.quota(id)),
    importCredential: (name: string, contents: string) =>
      run(() => managementApi.importCredential(name, contents)),
    deleteCredential: (row: Credential) =>
      run(() => managementApi.deleteCredential(row.id, row.deleteToken)),
    copyClientKey: () => run(managementApi.copyClientKey),
    toggleMany: (rows: Credential[], enabled: boolean) =>
      run(() => managementApi.toggleMany(credentialTargets(rows), enabled)),
    deleteMany: (rows: Credential[]) =>
      run(() => managementApi.deleteMany(credentialTargets(rows))),
    toggle: (id: string) => {
      const row = snapshot.credentials.find((c) => c.id === id);
      if (row)
        return run(() => managementApi.toggle(id, row.enabled, !row.enabled));
    },
  };
}
export type CredentialsState = ReturnType<typeof useCredentials>;
