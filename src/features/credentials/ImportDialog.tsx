import { Modal } from "../../components/Common";
export function ImportDialog({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="导入 JSON 凭证" onClose={onClose}>
      <p className="muted">粘贴 JSON，或选择本地文件</p>
      <textarea
        className="json-input"
        readOnly
        aria-label="JSON 导入预览"
        value={'{\n  "type": "example",\n  "account": "your-account"\n}'}
      />
      <p className="notice">
        凭证列表和开关已接入，JSON 导入尚未开放。请勿在这里输入真实凭证。
      </p>
      <div className="modal-actions">
        <button className="button" disabled>
          选择文件
        </button>
        <button className="button primary" disabled>
          校验并导入
        </button>
      </div>
    </Modal>
  );
}
