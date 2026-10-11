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
