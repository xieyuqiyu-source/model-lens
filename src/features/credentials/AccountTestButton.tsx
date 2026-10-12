import { FlaskConical, LoaderCircle } from "lucide-react";
import type { AccountProbeState } from "../probe/useAccountProbe";
import type { AccountTest, Credential } from "./data";
export function accountTestState(test: AccountTest | undefined, model: string) {
  if (!test || test.model !== model) return undefined;
  return ["running", "analyzing"].includes(test.status) && Date.now() / 1000 - test.at >= 420
    ? { ...test, status: "error", message: "测试已超时，请重试" } : test;
}
export function AccountTestButton({ credential: c, probe, test }: { credential: Credential; probe: AccountProbeState; test?: AccountTest }) {
  const current = accountTestState(test, probe.model);
  const running = current && ["running", "analyzing"].includes(current.status);
  return <button className="icon-button small account-test" aria-label={`测试 ${c.name}`} title={current?.message || `独立测试 ${probe.model || "所选模型"} · 3 次采样`}
    disabled={probe.blocked} onClick={() => void probe.run(c.id)}>
    {running ? <LoaderCircle size={13} className="spin" /> : <FlaskConical size={13} />}
  </button>;
}
