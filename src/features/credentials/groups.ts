import type { Credential } from "./data";
export const credentialGroups = [
  ["all", "全部"], ["codex", "Codex"], ["gemini", "Gemini"],
  ["claude", "Claude"], ["grok", "Grok"], ["antigravity", "Antigravity"], ["other", "其他"],
] as const;
export function credentialGroup(provider: string): string {
  if (provider === "gemini-cli") return "gemini";
  if (provider === "xai") return "grok";
  return credentialGroups.some(([id]) => id !== "all" && id === provider) ? provider : "other";
}
export function inGroup(rows: Credential[], group: string) {
  return group === "all" ? rows : rows.filter(c => credentialGroup(c.provider) === group);
}
