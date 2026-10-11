export type Credential = {
  id: string;
  name: string;
  brand: string;
  provider: string;
  enabled: boolean;
  editable: boolean;
  supported: boolean;
  five: number | null;
  week: number | null;
  resetFive: string;
  resetWeek: string;
  quotaAt: number | null;
  quotaError: string | null;
};
export type ManagementSnapshot = {
  connected: boolean;
  port: number;
  busy: boolean;
  stale: boolean;
  credentials: Credential[];
  error: string | null;
  revision: number;
};
export function resetLabel(value: string): string {
  if (!value) return "重置时间未提供";
  const date = new Date(/^\d+$/.test(value) ? Number(value) * 1000 : value);
  return Number.isNaN(date.getTime())
    ? "重置时间未提供"
    : `${date.toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" })} 重置`;
}

export function traySummary(state: Pick<ManagementSnapshot, "connected" | "busy" | "stale" | "error" | "credentials">): string {
  if (!state.connected) return state.error ? "管理连接失败" : "尚未连接";
  if (state.stale) return "列表待核实，请刷新";
  if (state.busy) return "正在刷新…";
  const failed = state.credentials.filter((row) => row.quotaError).length;
  const count = `${state.credentials.length} 个凭证`;
  if (failed) return `${count} · ${failed} 项额度异常`;
  return state.error ? `${count} · 刷新失败` : count;
}

export function activeCredentialsFirst(rows: Credential[]): Credential[] {
  return [...rows.filter((row) => row.enabled), ...rows.filter((row) => !row.enabled)];
}
