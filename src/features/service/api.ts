import { invoke, isTauri } from "@tauri-apps/api/core";
export type Service = {
  executable: string;
  directory: string;
  workingDirectory: string | null;
  lastSeen: number;
  pid: number | null;
};
export type Snapshot = {
  services: Service[];
  checkedAt: number;
  statusKnown: boolean;
  warning: string | null;
};
export const native = isTauri();
export async function discoverServices(): Promise<Snapshot> {
  if (!native)
    return {
      services: [],
      checkedAt: Date.now() / 1000,
      statusKnown: false,
      warning: "浏览器预览无法检查本机进程，请在 ModelLens 应用中查看。",
    };
  return invoke<Snapshot>("discover_services");
}
export async function openMain() {
  if (native) await invoke("open_main");
  else window.location.assign("/");
}

export async function openTray() {
  if (native) await invoke("open_tray");
}

export async function closeTray() {
  if (native) await invoke("close_tray");
}

export const cpaApi = {
  open: () => invoke<void>("cpa_open_management"),
  pull: () => invoke<string>("cpa_pull"),
  job: () => invoke<CpaJob>("cpa_job"),
  versions: (executable: string) => invoke<CpaVersions>("cpa_versions", {executable}),
  setRunning: (executable: string, port: number, running: boolean, expectedPid: number | null) => invoke<string>("cpa_set_running", {executable, port, running, expectedPid}),
  install: (directory: string, port: number) => invoke<string>("cpa_install", {directory, port}),
  chooseDirectory: () => invoke<string | null>("cpa_choose_directory"),
};

export type CpaUpdate = {executable: string; commit: string; revision: number};
export type CpaJob = {update?: CpaUpdate | null; busy: boolean; message: string; revision: number; logs: {id: number; at: number; message: string}[]};

export type CpaVersions = {
  source: string; local: string | null; remote: string | null; branch: string; upstream: string | null;
  ahead: number; behind: number; dirty: boolean; status: "current" | "behind" | "ahead" | "diverged" | "unbuilt" | "modified" | "unknown";
  remoteError: string | null; checkedAt: number;
};
