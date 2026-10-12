import { ArrowUpRight, RefreshCw, Trash2 } from "lucide-react";
import { Brand, Toggle } from "../../components/Common";
import { credentialQuotas, resetLabel, type Credential } from "./data";
import type { AccountProbeState } from "../probe/useAccountProbe";
import { AccountTestButton, accountTestState } from "./AccountTestButton";
import { PlanBadge } from "./PlanBadge";
import type { CredentialsState } from "./useCredentials";
function Quota({
  label,
  value,
  reset,
  detail = "",
  period = "",
}: {
  label: string;
  value: number | null;
  reset: string;
  detail?: string;
  period?: string;
}) {
  return (
    <div className={`quota ${period === "1w" ? "weekly" : ""}`}>
      <div className="quota-label">
        <span>{label}</span>
        <span>{value === null ? detail || "未提供" : `${value}% 剩余`}</span>
      </div>
      <div
        className="track"
        role={value === null ? "img" : "meter"}
        aria-label={`${label} 剩余额度`}
        {...(value === null
          ? {}
          : {
              "aria-valuenow": value,
              "aria-valuemin": 0,
              "aria-valuemax": 100,
            })}
      >
        <span style={{ width: `${value ?? 0}%` }} />
      </div>
      {value !== null && detail && <span className="quota-reset">{detail}</span>}
      <span className="quota-reset">{resetLabel(reset)}</span>
    </div>
  );
}
export function CredentialCard({ c, state: s, accountProbe, preview, onDelete, selected, onSelect }: {
  accountProbe: AccountProbeState;
  c: Credential; state: CredentialsState; preview: (id: string) => void; onDelete: (c: Credential) => void; selected: boolean; onSelect: () => void;
}) {
  return (
              <article
                data-test={accountTestState(s.accountTests?.[c.id], accountProbe.model)?.status}
                className={`credential-card ${!c.enabled ? "off" : ""}`}

              >
                <header>
                  <input className="credential-check" type="checkbox" aria-label={`选择 ${c.name}`} checked={selected} disabled={s.busy || s.stale || !c.deletable} onChange={onSelect} />
                  <Brand name={c.brand} />
                  <span title={c.name}>{c.name}</span>
                  <Toggle
                    on={c.enabled}
                    label={`${c.name} 凭证开关`}
                    disabled={s.busy || s.stale || !c.editable}
                    onChange={() => void s.toggle(c.id)}
                  />
                </header>
                {c.provider === "codex" && <div className="credential-tags">
                  <PlanBadge credential={c} />
                  <span className="reset-count" title={c.resetCountError || undefined}>
                    {c.resetCountError ? (c.resetAvailable == null ? "重置次数未获取" : `上次剩余 ${c.resetAvailable} 次 · 更新失败`) : c.resetAvailable == null ? "重置次数未查询" : `重置剩余 ${c.resetAvailable} 次`}
                  </span>
                </div>}
                {credentialQuotas(c).map((q, index) => (
                  <Quota key={`${q.label}:${index}`} label={q.label} value={q.remaining} reset={q.reset} detail={q.detail} period={q.period} />
                ))}
                <div className="card-meta">
                  <div>
                    <span>厂商</span>
                    <p>{c.provider || "未知"}</p>
                  </div>
                  <div>
                    <span>额度更新</span>
                    <p>
                      {c.quotaAt
                        ? new Date(c.quotaAt * 1000).toLocaleString(
                            "zh-CN",
                            { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" },
                          )
                        : "尚未查询"}
                    </p>
                  </div>
                </div>
                {(c.quotaError || !c.supported) && <p className={`quota-message ${c.quotaError ? "has-error" : ""}`}>
                  {c.quotaError ? `${c.quotaError}${c.quotaAt ? "（仍显示上次数据）" : ""}` : "此凭证类型暂不支持额度查询"}
                </p>}
                <footer>
                  <button
                    className="text-button"
                    disabled={s.busy || s.stale || !c.supported}
                    onClick={() => void s.refreshQuota(c.id)}
                  >
                    <RefreshCw size={13} />
                    刷新额度
                  </button>
                  <button className="text-button" disabled={s.busy || s.stale || !c.enabled} onClick={() => preview(c.id)}>
                    探查模型
                    <ArrowUpRight size={12} />
                  </button>
                  <AccountTestButton credential={c} probe={accountProbe} test={s.accountTests?.[c.id]} />
                  <button className="icon-button small credential-delete" aria-label={`删除 ${c.name}`} title={c.deletable ? "删除凭证" : "此凭证没有可删除的独立文件"}
                    disabled={s.busy || s.stale || !s.connected || !c.deletable} onClick={() => onDelete(c)}>
                    <Trash2 size={13} />
                  </button>
                </footer>
              </article>
  );
}
