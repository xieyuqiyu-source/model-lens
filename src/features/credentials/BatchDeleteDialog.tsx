import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { Credential } from "./data";
import type { CredentialsState } from "./useCredentials";

export function BatchDeleteDialog({ credentials, state: s, onClose }: { credentials: Credential[]; state: CredentialsState; onClose: () => void }) {
  const [pending, setPending] = useState(false);
  const submitted = useRef(false);
  const changed = credentials.some(c => !s.credentials.some(row => row.id === c.id && row.deleteToken === c.deleteToken && row.deletable));
  const blocked = pending || s.busy || s.stale || !s.connected || changed;
  return <Modal title="批量删除凭证" onClose={onClose}>{dismiss => <>
    <p className="notice">将从 CLIProxy 删除以下 {credentials.length} 个凭证文件，无法在应用内撤销。</p>
    <ul className="batch-delete-list" tabIndex={0} aria-label="待删除凭证">{credentials.map(c => <li key={c.id}><span>{c.name}</span><span className="muted">{c.fileName}</span></li>)}</ul>
    {changed && !pending && <p className="notice" role="alert">凭证已变化，请关闭后重新选择。</p>}
    {s.error && <p className="notice" role="alert">{s.error}</p>}
    <div className="modal-actions">
      <button className="button" disabled={pending} onClick={dismiss}>取消</button>
      <button className="button credential-delete" disabled={blocked} onClick={async () => {
        if (blocked || submitted.current) return;
        submitted.current = true; setPending(true);
        try { if (await s.deleteMany(credentials)) dismiss(); }
        finally { submitted.current = false; setPending(false); }
      }}>{pending ? "删除中…" : `确认删除 ${credentials.length} 个凭证`}</button>
    </div>
  </>}</Modal>;
}
