import { useEffect, useLayoutEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import tokens from "./tokens.json";
import { appearanceKey, parseAppearance, resolveAppearance, type AppearancePreference } from "./preferences";
function readAppearance() {
  try { return parseAppearance(localStorage.getItem(appearanceKey)); }
  catch { return parseAppearance(null); }
}
export function applyAppearance(preference: AppearancePreference, systemDark: boolean) {
  const mode = resolveAppearance(preference.mode, systemDark);
  const root = document.documentElement;
  root.dataset.appearance = mode;
  root.dataset.palette = preference.palette;
  root.style.colorScheme = mode;
  for (const [key, value] of Object.entries(tokens[preference.palette][mode]))
    root.style.setProperty(`--${key}`, value);
}
export function initializeAppearance() {
  applyAppearance(readAppearance(), matchMedia("(prefers-color-scheme: dark)").matches);
}
export function useAppearance() {
  const [preference, setPreference] = useState(readAppearance);
  const [error, setError] = useState("");
  useLayoutEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => applyAppearance(preference, media.matches);
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [preference]);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === appearanceKey || event.key === null) setPreference(readAppearance());
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    invoke("set_appearance", { mode: preference.mode }).then(
      () => { if (active) setError(""); },
      () => { if (active) setError("系统窗口外观未能同步，请重新选择明暗模式。"); },
    );
    return () => { active = false; };
  }, [preference.mode]);
  function update(next: AppearancePreference) {
    setPreference(next);
    try { localStorage.setItem(appearanceKey, JSON.stringify(next)); setError(""); }
    catch { setError("外观已应用，但偏好无法保存；下次启动会恢复默认。"); }
  }
  return { preference, update, error };
}
