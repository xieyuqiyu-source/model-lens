export type Credential = {
  id: string;
  brand: string;
  enabled: boolean;
  five: number | null;
  week: number | null;
  resetFive: string;
  resetWeek: string;
  expires: string;
};
export const initialCredentials: Credential[] = [
  {
    id: "personal",
    brand: "openai",
    enabled: true,
    five: 82,
    week: 46,
    resetFive: "2 小时 14 分钟后重置",
    resetWeek: "3 天 6 小时后重置",
    expires: "10 月 28 日 14:30",
  },
  {
    id: "work",
    brand: "claude",
    enabled: true,
    five: 54,
    week: 72,
    resetFive: "1 小时 32 分钟后重置",
    resetWeek: "5 天 2 小时后重置",
    expires: "未提供",
  },
  {
    id: "backup",
    brand: "gemini",
    enabled: true,
    five: null,
    week: 67,
    resetFive: "此窗口未提供",
    resetWeek: "2 天 9 小时后重置",
    expires: "11 月 02 日 09:00",
  },
  {
    id: "testing",
    brand: "deepseek",
    enabled: false,
    five: 28,
    week: null,
    resetFive: "4 小时 08 分钟后重置",
    resetWeek: "此窗口未提供",
    expires: "未提供",
  },
];
