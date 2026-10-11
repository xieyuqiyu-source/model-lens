export type QuotaItem = { label: string; remaining: number | null; detail: string; reset: string; period: string };
export type Credential = {
  id: string;
  name: string;
  brand: string;
  provider: string;
  enabled: boolean;
  editable: boolean;
  deletable: boolean;
  fileName: string;
  deleteToken: string;
  supported: boolean;
  probeSupported: boolean;
  five: number | null;
  week: number | null;
  resetFive: string;
  resetWeek: string;
  quotaItems?: QuotaItem[];
  quotaAt: number | null;
  quotaError: string | null;
};
export type ManagementSnapshot = {
  connected: boolean;
  saved: boolean;
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

export function credentialQuotas(c: Credential): QuotaItem[] {
  if (c.quotaItems?.length) return c.quotaItems;
  if (["codex", "claude"].includes(c.provider)) return [
    { label: "5h", remaining: c.five, detail: "", reset: c.resetFive, period: "5h" },
    { label: "1w", remaining: c.week, detail: "", reset: c.resetWeek, period: "1w" },
  ];
  return [{ label: "额度", remaining: null, detail: "", reset: "", period: "" }];
}
export function compactQuota(c: Credential) {
  const items = credentialQuotas(c);
  const known = items.filter(q => q.remaining !== null);
  const five = known.filter(q => q.period === "5h");
  const week = known.filter(q => q.period === "1w");
  const candidates = five.length ? five : week.length ? week : known;
  const chosen = candidates.reduce<QuotaItem | undefined>((min, q) => !min || q.remaining! < min.remaining! ? q : min, undefined);
  const description = items.map(q => `${q.label}：${q.remaining === null ? "百分比未提供" : `剩余 ${q.remaining}%`}${q.detail ? ` · ${q.detail}` : ""}`).join("；");
  return {
    value: chosen?.remaining ?? null,
    period: chosen ? (candidates.length > 1 ? "最低" : chosen.period) : "",
    description,
    weekly: chosen?.period === "1w",
  };
}
