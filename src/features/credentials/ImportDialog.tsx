import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { CredentialsState } from "./useCredentials";

const maxBytes = 1024 * 1024;
export function ImportDialog({ state, onClose }: { state: CredentialsState; onClose: () => void }) {
  const [file, setFile] = useState<File | null>(null);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  const paste = useRef<HTMLTextAreaElement>(null);
  const submitting = useRef(false);
  const blocked = reading || state.busy || !state.connected || state.stale;
  return (
    <Modal title="导入 JSON 凭证" onClose={onClose}>
      {(dismiss) => (
        <form className="connection-form" onSubmit={async (event) => {
          event.preventDefault();
          if (blocked || submitting.current) return;
          submitting.current = true;
          setReading(true);
          setError("");
          try {
            const contents = file ? await file.text() : paste.current?.value || "";
            if (!contents || new TextEncoder().encode(contents).length > maxBytes) {
              setError("JSON 不能为空，且不能超过 1 MiB");
              return;
            }
            if (await state.importCredential(file?.name ?? "", contents)) {
              if (paste.current) paste.current.value = "";
              setFile(null);
              dismiss();
            }
          } catch {
            setError("无法读取所选文件，请重新选择");
          } finally {
            submitting.current = false;
            setReading(false);
          }
        }}>
          <p className="muted">选择文件或粘贴凭证 JSON，粘贴内容自动命名，不覆盖同名文件。</p>
          <input ref={fileInput} type="file" accept=".json,application/json" aria-label="选择 JSON 文件" hidden disabled={blocked} onChange={(e) => {
            const selected = e.target.files?.[0];
            e.target.value = "";
            if (!selected) return;
            if (selected.size > maxBytes || !selected.size) {
              setFile(null);
              setError("文件不能为空，且不能超过 1 MiB");
              return;
            }
            setError("");
            setFile(selected);
            if (paste.current) paste.current.value = "";
          }} />
          {file ? <div className="saved-connection import-file">
            <p>已选择：{file.name}（{Math.ceil(file.size / 1024)} KiB）</p>
            <button type="button" className="text-button" disabled={blocked} onClick={() => setFile(null)}>移除文件，改为粘贴</button>
          </div> : <textarea ref={paste} className="json-input" aria-label="凭证 JSON" placeholder="粘贴完整凭证 JSON，需包含 type 字段" maxLength={maxBytes} required disabled={blocked} autoComplete="off" spellCheck={false} />}
          {!state.connected && <p className="notice">请先连接本机管理接口</p>}
          {state.stale && <p className="notice">当前列表待核实，请关闭弹窗并刷新列表</p>}
          {(error || state.error) && <p className="notice" role="alert">{error || state.error}</p>}
          <div className="modal-actions">
            <button type="button" className="button" disabled={blocked} onClick={() => fileInput.current?.click()}>选择文件</button>
            <button type="submit" className="button primary" disabled={blocked}>{reading ? "导入中…" : "校验并导入"}</button>
          </div>
        </form>
      )}
    </Modal>
  );
}
