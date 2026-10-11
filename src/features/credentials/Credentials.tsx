import {
  ArrowUpRight,
  ChevronDown,
  PanelTop,
  RefreshCw,
  Check,
  Minus,
} from "lucide-react";
import { Brand, Toggle } from "../../components/Common";
import type { Credential } from "./data";
type Props = {
  credentials: Credential[];
  expanded: boolean;
  toggle: (id: string) => void;
  preview: () => void;
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
        aria-label={`${label} ${value === null ? "额度未提供" : "示例剩余额度"}`}
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
      <span className="quota-reset">{reset}</span>
    </div>
  );
}
export function CredentialRows({
  credentials,
  toggle,
  preview,
}: {
  credentials: Credential[];
  toggle: Props["toggle"];
  preview: Props["preview"];
}) {
  return (
    <div className="credential-rows">
      {credentials.map((c) => (
        <div className={`credential-row ${!c.enabled ? "off" : ""}`} key={c.id}>
          <Brand name={c.brand} />
          <div className={`account-bar ${c.five === null ? "weekly" : ""}`}>
            <span
              className="account-fill"
              style={{ width: `${c.five ?? c.week ?? 0}%` }}
            />
            <span>{c.id}</span>
          </div>
          <span className="period">{c.five !== null ? "5h" : "1w"}</span>
          {c.id === "work" ? (
            <Check size={12} className="success" />
          ) : (
            <Minus size={12} />
          )}
          <button
            className="icon-button small"
            title="单凭证检测（预览）"
            aria-label={`检测 ${c.id}（预览）`}
            onClick={preview}
          >
            <RefreshCw />
          </button>
          <Toggle
            on={c.enabled}
            label={`${c.id} 示例凭证开关`}
            onChange={() => toggle(c.id)}
          />
        </div>
      ))}
    </div>
  );
}
export function Credentials(p: Props) {
  return (
    <section
      className={`credentials ${p.expanded ? "expanded" : ""}`}
      aria-label="凭证与额度"
    >
      <header className="section-heading">
        <div>
          <h2>凭证与额度</h2>
          <span className="muted detailed">
            {p.credentials.length} 个示例凭证 ·{" "}
            {p.credentials.filter((c) => c.enabled).length} 个启用
          </span>
        </div>
        <div className="actions">
          {p.expanded ? (
            <>
              <button className="button" onClick={p.preview}>
                <RefreshCw size={13} />
                刷新全部额度
              </button>
              <button className="button primary" onClick={p.onImport}>
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
      {p.expanded ? (
        <>
          <div className="credential-grid">
            {p.credentials.map((c) => (
              <article
                className={`credential-card ${!c.enabled ? "off" : ""}`}
                key={c.id}
              >
                <header>
                  <Brand name={c.brand} />
                  <span>{c.id}</span>
                  <Toggle
                    on={c.enabled}
                    label={`${c.id} 示例凭证开关`}
                    onChange={() => p.toggle(c.id)}
                  />
                </header>
                <Quota label="5h" value={c.five} reset={c.resetFive} />
                <Quota label="1w" value={c.week} reset={c.resetWeek} />
                <div className="card-meta">
                  <div>
                    <span>重置次数</span>
                    <p>暂无记录</p>
                  </div>
                  <div>
                    <span>凭证到期</span>
                    <p>{c.expires}</p>
                  </div>
                </div>
                <footer>
                  <button className="text-button" onClick={p.preview}>
                    <RefreshCw size={13} />
                    刷新额度
                  </button>
                  <button className="text-button" onClick={p.preview}>
                    单独检测
                    <ChevronDown size={12} />
                  </button>
                </footer>
              </article>
            ))}
          </div>
          <div className="section-foot">
            <span>界面预览 · 所有凭证数据均为示例</span>
            <span>额度与到期信息以服务商返回为准</span>
          </div>
        </>
      ) : (
        <>
          <CredentialRows
            credentials={p.credentials}
            toggle={p.toggle}
            preview={p.preview}
          />
          <div className="batch">
            <span className="muted">按所选模型逐个检测</span>
            <button className="button" onClick={p.preview}>
              批量检测
            </button>
          </div>
          <div className="session-info">
            <h3>
              本次检测 <span className="muted">· 示例</span>
            </h3>
            <dl>
              <dt>使用凭证</dt>
              <dd>work</dd>
              <dt>采样次数</dt>
              <dd>3 次</dd>
              <dt>分析方式</dt>
              <dd>模型指纹</dd>
            </dl>
          </div>
          <button className="text-button tray-link" onClick={p.onTray}>
            <PanelTop size={16} />
            菜单栏快捷检测
            <ArrowUpRight size={14} />
          </button>
          <span className="preview-caption">凭证与检测结果为界面示例</span>
        </>
      )}
    </section>
  );
}
