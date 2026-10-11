import {
  ArrowUpRight,
  PanelTop,
  RefreshCw,
  CircleAlert,
  Plug,
  ListRestart,
} from "lucide-react";
import { Brand, Toggle } from "../../components/Common";
import { activeCredentialsFirst, resetLabel, type Credential } from "./data";
import type { CredentialsState } from "./useCredentials";
type Props = {
  state: CredentialsState;
  expanded: boolean;
  preview: () => void;
  onConnect: () => void;
  onImport: () => void;
  onExpand: () => void;
  onTray: () => void;
};
function Quota({
  label,
  value,
  reset,
}: {
  label: string;
  value: number | null;
  reset: string;
}) {
  return (
    <div className={`quota ${label === "1w" ? "weekly" : ""}`}>
      <div className="quota-label">
        <span>{label}</span>
        <span>{value === null ? "未提供" : `${value}% 剩余`}</span>
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
      <span className="quota-reset">{resetLabel(reset)}</span>
    </div>
  );
}
function disabled(state: CredentialsState, c: Credential) {
  return state.busy || state.stale || !c.editable;
}
export function CredentialRows({
  state,
  onConnect,
}: {
  state: CredentialsState;
  onConnect: () => void;
}) {
  return (
    <div className="credential-rows">
      {!state.credentials.length && (
        <div className="credentials-empty">
          <p>{state.connected ? "没有可显示的凭证" : "尚未连接管理接口"}</p>
          <button
            className="text-button"
            disabled={state.busy}
            onClick={state.connected ? () => void state.refresh() : onConnect}
          >
            {state.connected ? "刷新列表" : "连接 CLIProxy"}
          </button>
        </div>
      )}
      {activeCredentialsFirst(state.credentials).map((c) => (
        <div className={`credential-row ${!c.enabled ? "off" : ""}`} key={c.id}>
          <Brand name={c.brand} />
          <div
            className={`account-bar ${c.five === null ? "weekly" : ""}`}
            data-quota={c.five === null && c.week === null ? "missing" : "known"}
            data-error={Boolean(c.quotaError)}
            title={`${c.name} · ${c.provider} · ${c.quotaError || (c.five !== null ? `5h 剩余 ${c.five}%` : c.week !== null ? `1w 剩余 ${c.week}%` : !c.enabled ? "已停用，额度未刷新" : !c.supported ? "此厂商额度暂未适配" : "额度尚未查询")}`}
          >
            <span
              className="account-fill"
              style={{ width: `${c.five ?? c.week ?? 0}%` }}
            />
            <span>{c.name}</span>
          </div>
          <span className="period">
            {c.five !== null ? "5h" : c.week !== null ? "1w" : ""}
          </span>
          <span className="row-status" title={c.quotaError || undefined}>
            {c.quotaError && <CircleAlert size={12} aria-label={c.quotaError} />}
          </span>
          <button
            className="icon-button small"
            title={
              c.quotaError || (!c.supported ? "此厂商额度暂未适配" : "刷新额度")
            }
            aria-label={`刷新 ${c.name} 额度`}
            disabled={state.busy || !c.supported || !c.enabled || state.stale}
            onClick={() => void state.refreshQuota(c.id)}
          >
            <RefreshCw className={state.busy ? "spin" : ""} />
          </button>
          <Toggle
            on={c.enabled}
            label={`${c.name} 凭证开关`}
            disabled={disabled(state, c)}
            onChange={() => void state.toggle(c.id)}
          />
        </div>
      ))}
    </div>
  );
}
export function Credentials(p: Props) {
  const s = p.state;
  return (
    <section
      className={`credentials ${p.expanded ? "expanded" : ""}`}
      aria-label="凭证与额度"
    >
      <header className="section-heading">
        <div>
          <h2>凭证与额度</h2>
          <span className="muted detailed">
            {s.connected
              ? `${s.credentials.length} 个凭证 · ${s.credentials.filter((c) => c.enabled).length} 个启用`
              : "连接本机 CLIProxy 后显示"}
          </span>
        </div>
        <div className="actions">
          <button
            className="icon-button"
            title="管理连接"
            aria-label="管理连接"
            onClick={p.onConnect}
          >
            <Plug size={15} />
          </button>
          {s.connected && (
            <button
              className="icon-button"
              title="刷新凭证列表"
              aria-label="刷新凭证列表"
              disabled={s.busy}
              onClick={() => void s.refresh()}
            >
              <ListRestart size={15} />
            </button>
          )}
          {p.expanded ? (
            <>
              <button
                className="button"
                disabled={!s.connected || s.busy || s.stale}
                onClick={() => void s.refreshQuota()}
              >
                <RefreshCw size={13} className={s.busy ? "spin" : ""} />
                {s.busy ? "处理中…" : "刷新全部额度"}
              </button>
              <button className="button" onClick={p.onImport}>
                导入 JSON
              </button>
            </>
          ) : (
            <button className="text-button" onClick={p.onExpand}>
              管理
              <ArrowUpRight size={13} />
            </button>
          )}
        </div>
      </header>
      {s.error && (
        <p className="notice credentials-notice" role="alert">
          {s.error}
          {s.stale ? " · 当前列表待核实" : ""}
        </p>
      )}
      {p.expanded && s.credentials.length ? (
        <>
          <div className="credential-grid">
            {s.credentials.map((c) => (
              <article
                className={`credential-card ${!c.enabled ? "off" : ""}`}
                key={c.id}
              >
                <header>
                  <Brand name={c.brand} />
                  <span title={c.name}>{c.name}</span>
                  <Toggle
                    on={c.enabled}
                    label={`${c.name} 凭证开关`}
                    disabled={disabled(s, c)}
                    onChange={() => void s.toggle(c.id)}
                  />
                </header>
                <Quota label="5h" value={c.five} reset={c.resetFive} />
                <Quota label="1w" value={c.week} reset={c.resetWeek} />
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
                      ? "此厂商额度暂未适配"
                      : !c.enabled
                        ? "已停用，额度未自动刷新"
                        : c.quotaAt
                          ? "额度以服务商返回为准"
                          : "点击刷新获取真实额度"}
                </p>
                <footer>
                  <button
                    className="text-button"
                    disabled={s.busy || s.stale || !c.supported || !c.enabled}
                    onClick={() => void s.refreshQuota(c.id)}
                  >
                    <RefreshCw size={13} />
                    刷新额度
                  </button>
                  <button className="text-button" onClick={p.preview}>
                    单独检测
                    <ArrowUpRight size={12} />
                  </button>
                </footer>
              </article>
            ))}
          </div>
          <div className="section-foot">
            <span>已连接 127.0.0.1:{s.port}</span>
            <span>缺失额度不会显示为 0%</span>
          </div>
        </>
      ) : (
        <>
          <CredentialRows state={s} onConnect={p.onConnect} />
          <div className="batch">
            <span className="muted">仅刷新启用且支持的凭证</span>
            <button
              className="button"
              disabled={!s.connected || s.busy || s.stale}
              onClick={() => void s.refreshQuota()}
            >
              刷新全部
            </button>
          </div>
          <button className="text-button tray-link" onClick={p.onTray}>
            <PanelTop size={16} />
            菜单栏快捷入口
            <ArrowUpRight size={14} />
          </button>
          <span className="preview-caption">
            模型检测尚未接入 · 账号开关立即生效
          </span>
        </>
      )}
    </section>
  );
}
