import { planLabel, type Credential } from "./data";

export function PlanBadge({ credential: c }: { credential: Credential }) {
  return c.provider === "codex" ? <span className="plan-badge" data-plan={c.plan || "unknown"}>{planLabel(c.plan)}</span> : null;
}
