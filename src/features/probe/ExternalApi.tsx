import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { ProbeState } from "./useProbe";
export function ExternalApi({ state, onClose, notify }: { state: ProbeState; onClose: () => void; notify: (text: string) => void }) {
  const form = useRef<HTMLFormElement>(null);
  const [baseUrl, setBaseUrl] = useState(state.externalApi.baseUrl);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const saved = state.externalApi.saved;
  return (
    <Modal title="第三方 API" onClose={onClose}>
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
              const rows = await state.saveExternal(baseUrl, input.value);
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
            {saved ? "更换 API Key（留空沿用已保存）" : "API Key"}
            <input
              name="key"
              aria-label="API Key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              required={!saved}
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
            {saved ? (
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={async () => {
                  try {
                    await state.forgetExternal();
                    notify("已删除第三方 API 记录");
                    dismiss();
                  } catch (err) {
                    setError(typeof err === "string" ? err : "删除失败");
                  }
                }}
              >
                忘记 Key
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
