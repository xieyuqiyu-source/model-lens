import { invoke } from "@tauri-apps/api/core";
import type { ManagementSnapshot } from "./data";
export const managementApi = {
  snapshot: () => invoke<ManagementSnapshot>("management_snapshot"),
  connect: (port: number, key: string) =>
    invoke<ManagementSnapshot>("management_connect", { port, key }),
  disconnect: () => invoke<ManagementSnapshot>("management_disconnect"),
  restore: () => invoke<ManagementSnapshot>("management_restore"),
  forget: () => invoke<ManagementSnapshot>("management_forget"),
  list: () => invoke<ManagementSnapshot>("credentials_list"),
  toggle: (id: string, expected: boolean, enabled: boolean) =>
    invoke<ManagementSnapshot>("credential_toggle", { id, expected, enabled }),
  importCredential: (name: string, contents: string) =>
    invoke<ManagementSnapshot>("credential_import", { name, contents }),
  deleteCredential: (id: string, expected: string) =>
    invoke<ManagementSnapshot>("credential_delete", { id, expected }),
  quota: (id: string | null = null) =>
    invoke<ManagementSnapshot>("quota_refresh", { id }),
};
