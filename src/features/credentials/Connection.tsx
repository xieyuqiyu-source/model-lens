import { useRef, useState } from "react";
import { Modal } from "../../components/Common";
import type { CredentialsState } from "./useCredentials";
export function Connection({
  state,
  onClose,
  onInspect,
}: {
  state: CredentialsState;
  onClose: () => void;
  onInspect: () => void;
}) {
  const form = useRef<HTMLFormElement>(null);
  const [port, setPort] = useState(String(state.port || 8317));
  return (
    <Modal title="连接 CLIProxy" onClose={onClose}>
      {(dismiss) => (
        <>
          <div className="saved-connection">
            <p className="muted">{state.saved ? "已保存本机连接。" : "连接成功后自动记住。"}</p>
            {(state.saved || state.error) && <div className="actions">
              {!state.connected && <button type="button" className="text-button" disabled={state.busy} onClick={async () => { if (await state.restore()) dismiss(); }}>使用本地记录重连</button>}
              <button type="button" className="text-button" disabled={state.busy} onClick={() => void state.forget()}>忘记密钥并断开</button>
            </div>}
          </div>
          <form
            ref={form}
            className="connection-form"
            onSubmit={async (e) => {
              e.preventDefault();
              const input = form.current?.elements.namedItem(
                "key",
              ) as HTMLInputElement;
              const key = input.value;
              input.value = "";
              if (await state.connect(Number(port), key)) dismiss();
            }}
          >
            <label>
              本机管理端口
              <div className="endpoint">
                <span>http://127.0.0.1:</span>
                <input
                  aria-label="管理端口"
                  type="number"
                  min="1"
                  max="65535"
                  required
                  value={port}
                  disabled={state.busy}
                  onChange={(e) => setPort(e.target.value)}
                />
              </div>
            </label>
            <label>
              {state.saved ? "更换管理密钥" : "管理密钥"}
              <input
                name="key"
                aria-label="管理密钥"
                type="password"
                autoComplete="off"
                spellCheck={false}
                required
                maxLength={8192}
                disabled={state.busy}
                placeholder="输入 CLIProxy 管理密钥"
              />
            </label>
            <p className="muted">
              密钥仅存本机文件，仅当前用户可读写；断开连接保留记录。
            </p>
            {state.error && (
              <p className="notice" role="alert">
                {state.error}
              </p>
            )}
            <div className="modal-actions">
              <button type="button" className="text-button" onClick={onInspect}>
                查看服务位置
              </button>
              <div className="actions">
                {state.connected && (
                  <button
                    type="button"
                    className="button"
                    disabled={state.busy}
                    onClick={() => void state.disconnect()}
                  >
                    断开连接
                  </button>
                )}
                <button className="button primary" disabled={state.busy}>
                  {state.busy ? "处理中…" : "连接并读取列表"}
                </button>
              </div>
            </div>
          </form>
        </>
      )}
    </Modal>
  );
}
