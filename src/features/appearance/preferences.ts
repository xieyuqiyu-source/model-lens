export type AppearanceMode = "system" | "light" | "dark";
export type Palette = "jade" | "ocean" | "iris";
export type AppearancePreference = { mode: AppearanceMode; palette: Palette };
export const appearanceKey = "modellens-appearance";
export function parseAppearance(raw: string | null): AppearancePreference {
  try {
    const value = JSON.parse(raw ?? "null");
    return {
      mode: ["system", "light", "dark"].includes(value?.mode) ? value.mode : "system",
      palette: ["jade", "ocean", "iris"].includes(value?.palette) ? value.palette : "jade",
    };
  } catch { return { mode: "system", palette: "jade" }; }
}
export function resolveAppearance(mode: AppearanceMode, systemDark: boolean) {
  return mode === "system" ? (systemDark ? "dark" : "light") : mode;
}
