import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import { Select } from "../../components/Select";
import type { ExternalEntry, ExternalProtocol } from "./api";
import type { ProbeState } from "./useProbe";
/** Adds a new third-party API when `entry` is null, otherwise edits it. */
export function ExternalApi({ state, entry, onClose, notify }: { state: ProbeState; entry: ExternalEntry | null; onClose: () => void; notify: (text: string) => void }) {
  const form = useRef<HTMLFormElement>(null);
  const [name, setName] = useState(entry?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(entry?.baseUrl ?? "");
  const [protocol, setProtocol] = useState<ExternalProtocol>(entry?.protocol ?? "chat");
  const [modelId, setModelId] = useState(entry?.modelId ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Modal title={entry ? "编辑第三方 API" : "添加第三方 API"} onClose={onClose} dismissible={!busy}>
      {(dismiss) => (
        <form
          ref={form}
          className="connection-form"
          onSubmit={async (e) => {
            e.preventDefault();
            if (busy) return;
            const input = form.current?.elements.namedItem("key") as HTMLInputElement;
            setBusy(true);
            setError("");
            try {
              const rows = await state.saveExternal(entry?.id ?? null, {name, baseUrl, protocol, modelId}, input.value);
              input.value = "";
              notify(modelId.trim() ? "已保存模型配置" : `已保存，读取到 ${rows.length} 个模型`);
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
            接口协议
            <Select label="接口协议" value={protocol} onChange={(value) => { if (!busy) setProtocol(value as ExternalProtocol); }} options={[
              {value: "chat", label: "Chat Completions"},
              {value: "responses", label: "Responses"},
              {value: "messages", label: "Anthropic Messages"},
            ]} />
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
            {entry ? "更换 API Key（留空沿用原设置）" : "API Key（可选）"}
            <input
              name="key"
              aria-label="API Key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              maxLength={8192}
              disabled={busy}
              placeholder="sk-…"
            />
          </label>
          <label>
            模型 ID（可选）
            <input aria-label="模型 ID" value={modelId} maxLength={512} disabled={busy}
              placeholder="接口无模型列表时填写" onChange={(e) => setModelId(e.target.value)} />
          </label>
          <p className="muted">
            Key 仅存本机。填写模型 ID 时，连接将在检测时验证。默认采样两次，补测另发一次请求。
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
                  setBusy(true);
                  setError("");
                  try {
                    await state.forgetExternal(entry.id);
                    notify(`已删除 ${entry.name}`);
                    dismiss();
                  } catch (err) {
                    setError(typeof err === "string" ? err : "删除失败");
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                删除此 API
              </button>
            ) : (
              <span />
            )}
            <button className="button primary" disabled={busy}>
              {busy ? "保存中…" : modelId.trim() ? "保存配置" : "保存并读取模型"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}
