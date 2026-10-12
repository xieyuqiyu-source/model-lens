import type { AccountTest } from "../credentials/data";
export function accountActive(test: AccountTest) {
  return test.status === "running" || (test.status === "analyzing" && Date.now() / 1000 - test.at < 420);
}
export function accountStatusLabel(status: string) {
  return ({ running: "采样中", analyzing: "分析中", match: "库内匹配", mismatch: "库内不匹配", unknown: "证据不足", error: "探查失败" } as Record<string, string>)[status] ?? "待核实";
}

export function accountDisplay<T extends AccountTest>(test: T): T {
  return test.status === "analyzing" && !accountActive(test)
    ? { ...test, status: "error", message: "分析已超时，请重新探查" } : test;
}
