import { invoke } from "@tauri-apps/api/core";
import type { CredentialTarget, ManagementSnapshot } from "./data";
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
  copyClientKey: () => invoke<ManagementSnapshot>("copy_client_key"),
  toggleMany: (targets: CredentialTarget[], enabled: boolean) =>
    invoke<ManagementSnapshot>("credentials_toggle", { targets, enabled }),
  deleteMany: (targets: CredentialTarget[]) =>
    invoke<ManagementSnapshot>("credentials_delete", { targets }),
  quota: (id: string | null = null) =>
    invoke<ManagementSnapshot>("quota_refresh", { id }),
};
