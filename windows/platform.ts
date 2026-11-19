import { isTauri } from "@tauri-apps/api/core";

export const windowsDesktop = isTauri() && /Windows NT/.test(navigator.userAgent);
