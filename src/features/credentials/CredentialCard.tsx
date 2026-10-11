import { ArrowUpRight, RefreshCw, Trash2 } from "lucide-react";
import { Brand, Toggle } from "../../components/Common";
import { credentialQuotas, resetLabel, type Credential } from "./data";
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
export function CredentialCard({ c, state: s, preview, onDelete }: {
  c: Credential; state: CredentialsState; preview: (id: string) => void; onDelete: (c: Credential) => void;
}) {
  return (
              <article
                className={`credential-card ${!c.enabled ? "off" : ""}`}

              >
                <header>
                  <Brand name={c.brand} />
                  <span title={c.name}>{c.name}</span>
                  <Toggle
                    on={c.enabled}
                    label={`${c.name} 凭证开关`}
                    disabled={s.busy || s.stale || !c.editable}
                    onChange={() => void s.toggle(c.id)}
                  />
                </header>
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
                        ? new Date(c.quotaAt * 1000).toLocaleTimeString(
                            "zh-CN",
                            { hour: "2-digit", minute: "2-digit" },
                          )
                        : "尚未查询"}
                    </p>
                  </div>
                </div>
                <p
                  className={`quota-message ${c.quotaError ? "has-error" : ""}`}
                >
                  {c.quotaError
                    ? `${c.quotaError}${c.quotaAt ? "（仍显示上次数据）" : ""}`
                    : !c.supported
                      ? "此凭证类型暂不支持额度查询"
                      : !c.enabled
                        ? "已停用，仍可查询额度"
                        : c.quotaAt
                          ? "额度以服务商返回为准"
                          : "点击刷新获取真实额度"}
                </p>
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
                  <button className="icon-button small credential-delete" aria-label={`删除 ${c.name}`} title={c.deletable ? "删除凭证" : "此凭证没有可删除的独立文件"}
                    disabled={s.busy || s.stale || !s.connected || !c.deletable} onClick={() => onDelete(c)}>
                    <Trash2 size={13} />
                  </button>
                </footer>
              </article>
  );
}
