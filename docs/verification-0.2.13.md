# 0.2.13：模型探查与新厂商额度合并交付

日期：2026-10-11。Apple Silicon Mac，Tauri 2 + Rust + React。合并父提交：主线 `d860ec3` 与 `codex/isolated-dev` 的 `437842d`（包含额度实现 `cbe7acb`）。

## 合并范围

- 保留 0.2.12 全厂商 `/v1/models` 目录、别名与前缀、标准 `/v1/chat/completions` 自动采样、仅后端客户端 API Key、目录筛选、错误与取消、指纹分析和窗口内历史。手动检测界面仍移除，凭证优先窄栏保持正常间距。
- 合入 Grok / xAI、Gemini CLI、Antigravity 额度查询、真实周期与模型分组、缺失值处理、失败保留旧数据，以及 Grok 图标和 Gemini 类型映射。保留 Codex / Claude、停用账号查询、导入删除、开关核实和本地连接持久化。
- 冲突处保留新额度支持与项目字段，同时移除分支旧的 `probe_supported`：额度支持范围不限制模型探查。凭证“探查模型”按启用状态控制，实际执行账号由 CLIProxy 路由。
- `package.json`、Tauri、Cargo、锁文件与界面统一 0.2.13；中文 README、接续规则和设计约定更新。按最新决定不再同步 Figma，既有设计仅作历史存档。

## 验证

- `pnpm test:ui`：21 项通过。
- `pnpm test`：47 项 Rust 测试通过，包括 9 项探查和 10 项新增厂商额度测试。
- `pnpm check`：TypeScript / Rust 检查通过。
- WebKit / Chromium：`browser-provider-quota.py`、`browser-credentials.py`、`browser-probe.py` 全部通过；覆盖真实周期语义、图标与明暗、未知值、停用查询、导入删除、全厂商目录刷新、自动采样/部分结果/取消/历史、920/1120/1280 宽度无按钮重叠。补充合成 Grok 启用状态决定“探查模型”可用性的回归。
- `browser-focus.py`：双浏览器 × 主窗口/菜单栏四组焦点回归通过。
- 生产预览加载 Tauri CSP 的探查回归：WebKit / Chromium 通过，worker 正常分析合成回答。查看 WebKit 深色合成额度卡片截图，无横向溢出。
- `pnpm package`：arm64 Release `.app` 构建成功；本机安装版本 0.2.13 与 `codesign --verify --deep --strict` 通过。
- 独立只读代码审查：核对两父版本，探查内核、请求路径及状态管理未被回退；新额度实现、错误处理、旧数据保留与字段投影完整保留，无新增合并缺陷。审查另行运行 21 项界面逻辑测试通过。

## 备份与安装

- 合并前两个工作树均干净；已 fetch 核对远端。
- 全分支 Git bundle 与引用快照：`~/Downloads/ModelLens-merge-backup-20261011-234608/`。
- 正常退出旧版 ModelLens 后备份至 `~/Downloads/ModelLens-backup-before-0.2.13.app`，原应用版本 0.2.12。
- 新版安装于 `~/Applications/ModelLens.app` 并重新启动，界面显示 0.2.13。本机观察到管理连接与已选模型恢复、自动检测入口，以及 xAI / Antigravity 的额度查询入口；没有点击真实额度刷新或模型采样。

## 边界

开发测试仅使用合成 HTTP / IPC；没有调用真实模型、刷新真实额度、切换账号、导入或删除凭证，也没有读取本机连接文件或密钥。厂商订阅、地区及上游版本差异尚未逐项实测。安装仅退出并重开 ModelLens，不重启 CLIProxy 或 DSH。当前为 Apple Silicon ad-hoc 签名，未经 Apple 公证。
