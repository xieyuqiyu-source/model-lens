import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { Credential } from "./data";
import type { CredentialsState } from "./useCredentials";

export function DeleteDialog({ credential, state, onClose }: {
  credential: Credential; state: CredentialsState; onClose: () => void;
}) {
  const [pending, setPending] = useState(false);
  const submitted = useRef(false);
  const current = state.credentials.find((row) => row.id === credential.id);
  const changed = !current || current.deleteToken !== credential.deleteToken;
  const blocked = pending || state.busy || !state.connected || state.stale || changed;
  return (
    <Modal title="删除凭证" onClose={onClose}>
      {(dismiss) => <>
        <p className="delete-target">{credential.name}</p>
        <p className="muted delete-target">{credential.fileName}</p>
        <p className="notice">将从 CLIProxy 删除此凭证文件，账号将不再参与路由。此操作无法在应用内撤销。</p>
        {changed && <p className="notice" role="alert">凭证已变化或不存在，请关闭后重新确认。</p>}
        {state.error && <p className="notice" role="alert">{state.error}</p>}
        <div className="modal-actions">
          <button type="button" className="button" disabled={pending} onClick={dismiss}>取消</button>
          <button type="button" className="button credential-delete" disabled={blocked} onClick={async () => {
            if (blocked || submitted.current) return;
            submitted.current = true;
            setPending(true);
            try { if (await state.deleteCredential(credential)) dismiss(); }
            finally { submitted.current = false; setPending(false); }
          }}>{pending ? "删除中…" : "确认删除"}</button>
        </div>
      </>}
    </Modal>
  );
}
