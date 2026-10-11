import { useRef, useState } from "react";
import { Toggle } from "../../components/Common";
import type { Credential } from "./data";
import type { CredentialsState } from "./useCredentials";

export function CredentialTools({ state: s, selected, onSelect, onDelete }: {
  state: CredentialsState; selected: string[]; onSelect: (ids: string[]) => void; onDelete: (rows: Credential[]) => void;
}) {
  const [notice, setNotice] = useState("");
  const copying = useRef(false);
  const blocked = s.busy || s.stale || !s.connected;
  const deletable = s.credentials.filter(c => c.deletable);
  const checked = deletable.filter(c => selected.includes(c.id));
  const all = deletable.length > 0 && checked.length === deletable.length;
  const editable = s.credentials.filter(c => c.editable);
  const anyEnabled = editable.some(c => c.enabled);
  const url = `http://127.0.0.1:${s.port}/v1`;
  const copy = async (key: boolean) => {
    if (blocked || copying.current) return;
    copying.current = true;
    setNotice("");
    try {
      if (key) { if (await s.copyClientKey()) setNotice("API Key 已复制"); }
      else { await navigator.clipboard.writeText(url); setNotice("代理地址已复制"); }
    } catch { setNotice("复制失败，请重试"); }
    finally { copying.current = false; }
  };
  return <div className="credential-tools">
    {s.connected && <div className="proxy-access">
      <button className="text-button proxy-url" disabled={blocked} onClick={() => void copy(false)}>{url}</button>
      <button className="text-button proxy-url" disabled={blocked} aria-label="复制 API Key" onClick={() => void copy(true)}>API Key</button>
      <span className="muted copy-status" role="status">{notice}</span>
    </div>}
    {s.credentials.length > 0 && <div className="credential-batch-actions">
      <label className="credential-select-all"><input type="checkbox" aria-label="全选可删除凭证" checked={all} ref={e => {if(e) e.indeterminate = checked.length > 0 && !all;}} disabled={blocked || !deletable.length} onChange={() => onSelect(all ? [] : deletable.map(c => c.id))} />全选{checked.length ? ` · ${checked.length}` : ""}</label>
      <button className="text-button credential-delete" disabled={blocked || !checked.length} onClick={() => onDelete(checked)}>删除所选{checked.length ? ` (${checked.length})` : ""}</button>
      <div className="credential-enable-actions">
        <Toggle on={anyEnabled} label="全部凭证开关" disabled={blocked || !editable.length}
          onChange={() => void s.toggleMany(editable.filter(c => c.enabled === anyEnabled), !anyEnabled)} />
      </div>
    </div>}
  </div>;
}
