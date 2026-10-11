import "./credentials.css";
import { useState, type CSSProperties } from "react";
import {
  ArrowUpRight,
  PanelTop,
  RefreshCw,
  CircleAlert,
  Plug,
  ListRestart,
} from "lucide-react";
import { Brand, Toggle } from "../../components/Common";
import { activeCredentialsFirst, type Credential } from "./data";
import type { CredentialsState } from "./useCredentials";
import { CredentialCard } from "./CredentialCard";
import { DeleteDialog } from "./DeleteDialog";
type Props = {
  state: CredentialsState;
  expanded: boolean;
  preview: (id: string) => void;
  onConnect: () => void;
  onImport: () => void;
  onExpand: () => void;
  onTray: () => void;
};
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
    <div className="credential-rows" tabIndex={0} role="region" aria-label="凭证列表，可滚动">
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
            style={{ "--quota-width": `${c.five ?? c.week ?? 0}%` } as CSSProperties}
            title={`${c.name} · ${c.provider} · ${c.quotaError || (c.five !== null ? `5h 剩余 ${c.five}%` : c.week !== null ? `1w 剩余 ${c.week}%` : !c.enabled ? "已停用，仍可查询额度" : !c.supported ? "此厂商额度暂未适配" : "额度尚未查询")}`}
          >
            <span className="account-fill" />
            <span className="account-label">{c.name}</span>
            <span className="account-label account-label-on-fill" aria-hidden="true">{c.name}</span>
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
            disabled={state.busy || !c.supported || state.stale}
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
  const [deleting, setDeleting] = useState<Credential | null>(null);
  return (
    <section
      className={`credentials ${p.expanded ? "expanded" : ""}`}
      aria-label="凭证与额度"
      tabIndex={0}
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
              <button className="button" disabled={!s.connected || s.busy || s.stale} onClick={p.onImport}>
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
              <CredentialCard key={c.id} c={c} state={s} preview={p.preview} onDelete={setDeleting} />
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
            <span className="muted">刷新所有支持的凭证，包含已停用账号</span>
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
            账号开关立即生效
          </span>
        </>
      )}
      {deleting && <DeleteDialog credential={deleting} state={s} onClose={() => setDeleting(null)} />}
    </section>
  );
}
