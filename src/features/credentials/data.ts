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
