import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { ExternalEntry } from "./api";
import type { ProbeState } from "./useProbe";
/** Adds a new third-party API when `entry` is null, otherwise edits it. */
export function ExternalApi({ state, entry, onClose, notify }: { state: ProbeState; entry: ExternalEntry | null; onClose: () => void; notify: (text: string) => void }) {
  const form = useRef<HTMLFormElement>(null);
  const [name, setName] = useState(entry?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(entry?.baseUrl ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Modal title={entry ? "编辑第三方 API" : "添加第三方 API"} onClose={onClose}>
      {(dismiss) => (
        <form
          ref={form}
          className="connection-form"
          onSubmit={async (e) => {
            e.preventDefault();
            const input = form.current?.elements.namedItem("key") as HTMLInputElement;
            setBusy(true);
            setError("");
            try {
              const rows = await state.saveExternal(entry?.id ?? null, name, baseUrl, input.value);
              input.value = "";
              notify(`已保存，读取到 ${rows.length} 个模型`);
              dismiss();
            } catch (err) {
              setError(typeof err === "string" ? err : "保存失败");
            } finally {
              setBusy(false);
            }
          }}
        >
          <label>
            名称（可选）
            <input
              aria-label="名称"
              maxLength={60}
              value={name}
              disabled={busy}
              placeholder="留空使用域名"
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label>
            Base URL
            <input
              aria-label="Base URL"
              type="url"
              required
              spellCheck={false}
              maxLength={2048}
              value={baseUrl}
              disabled={busy}
              placeholder="https://api.example.com/v1"
              onChange={(e) => setBaseUrl(e.target.value)}
            />
          </label>
          <label>
            {entry ? "更换 API Key（留空沿用已保存）" : "API Key"}
            <input
              name="key"
              aria-label="API Key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              required={!entry}
              maxLength={8192}
              disabled={busy}
              placeholder="sk-…"
            />
          </label>
          <p className="muted">
            兼容 OpenAI 接口（/models、/chat/completions）。Key 仅存本机文件，仅当前用户可读写，不回显。每次检测采样 3 次，消耗该服务额度。
          </p>
          {error && (
            <p className="notice" role="alert">
              {error}
            </p>
          )}
          <div className="modal-actions">
            {entry ? (
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={async () => {
                  try {
                    await state.forgetExternal(entry.id);
                    notify(`已删除 ${entry.name}`);
                    dismiss();
                  } catch (err) {
                    setError(typeof err === "string" ? err : "删除失败");
                  }
                }}
              >
                删除此 API
              </button>
            ) : (
              <span />
            )}
            <button className="button primary" disabled={busy}>
              {busy ? "读取中…" : "保存并读取模型"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}
