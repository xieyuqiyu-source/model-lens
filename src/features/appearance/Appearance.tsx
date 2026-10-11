import { Check, Moon, Sun, Monitor, Plug } from "lucide-react";
import { Modal } from "../../components/Common";
import tokens from "./tokens.json";
import type { AppearanceMode, Palette, AppearancePreference } from "./preferences";
const palettes: { id: Palette; name: string; detail: string }[] = [
  { id: "jade", name: "翡翠绿", detail: "默认 · 清晰自然" },
  { id: "ocean", name: "海湾蓝", detail: "冷静 · 明快" },
  { id: "iris", name: "鸢尾紫", detail: "柔和 · 鲜明" },
];
const modes = [{ id: "light", name: "浅色", Icon: Sun }, { id: "dark", name: "深色", Icon: Moon }, { id: "system", name: "跟随系统", Icon: Monitor }] as const;
export function Appearance({ preference, update, error, onClose, onConnect, compact }: {
  preference: AppearancePreference; update: (next: AppearancePreference) => void;
  error: string; onClose: () => void; onConnect: () => void; compact: boolean;
}) {
  function mode(value: AppearanceMode) { update({ ...preference, mode: value }); }
  return <Modal title="外观设置" onClose={onClose}>
    <div className="appearance-settings" data-compact={compact}>
      <div className="appearance-modes" role="group" aria-label="明暗模式">
        {modes.map(({ id, name, Icon }) => <button key={id} aria-pressed={preference.mode === id} onClick={() => mode(id)}><Icon size={15} />{name}</button>)}
      </div>
      <div className="palette-options" role="group" aria-label="主题配色">
        {palettes.map(({ id, name, detail }) => <button key={id} aria-pressed={preference.palette === id} onClick={() => update({ ...preference, palette: id })}>
          <span className="palette-swatches" aria-hidden="true"><i style={{ background: tokens[id].light.green }} /><i style={{ background: tokens[id].light["dark-green"] }} /></span>
          <span><strong>{name}</strong>{!compact && <small>{detail}</small>}</span>
          {preference.palette === id && <Check size={14} />}
        </button>)}
      </div>
      {error && <p role="status" className="quota-message has-error">{error}</p>}
      {!compact && <button className="text-button" onClick={onConnect}><Plug size={14} />管理 CLIProxy 连接</button>}
    </div>
  </Modal>;
}
