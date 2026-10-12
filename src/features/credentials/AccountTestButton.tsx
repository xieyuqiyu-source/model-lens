import { FlaskConical, LoaderCircle, CircleCheck, CircleHelp, CircleAlert } from "lucide-react";
import type { AccountProbeState } from "../probe/useAccountProbe";
import { accountActive, accountStatusLabel, accountDisplay } from "../probe/accountStatus";
import type { AccountTest, Credential } from "./data";
export function accountTestState(test: AccountTest | undefined, model: string) {
  if (!test || test.model !== model) return undefined;
  return accountDisplay(test);
}
export function AccountTestButton({ credential: c, probe, test }: { credential: Credential; probe: AccountProbeState; test?: AccountTest }) {
  test = test && accountDisplay(test);
  const running = test && accountActive(test);
  const Icon = running ? LoaderCircle : test?.status === "match" ? CircleCheck : test?.status === "unknown" ? CircleHelp : test ? CircleAlert : FlaskConical;
  return <button className="icon-button small account-test" data-status={test?.status}
    aria-label={test ? `${accountStatusLabel(test.status)} ${c.name}，查看详情` : `测试 ${c.name}`}
    title={test ? `${test.model} · ${test.message} · 点击查看详情` : `独立测试 ${probe.model || "所选模型"} · 3 次采样`}
    disabled={!test && probe.blocked(c.id)} onClick={() => test ? probe.inspect(test.runId) : void probe.run(c.id)}>
    <Icon size={13} className={running ? "spin" : ""} />
  </button>;
}
