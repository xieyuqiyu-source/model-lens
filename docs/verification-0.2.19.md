# 0.2.19：凭证分组与 CPA 源码拉取

2026-10-14，基于 0.2.18 与文档提交 `0ddc326`。

## 变更

- 凭证优先添加分组及数量、记忆选择、切组清勾选。手动额度和批量启停/删除仅当前组；全部页、后台额度、紧凑列表及菜单栏保留原范围与排序。
- CPA 管理入口打开当前回环端口的 management.html，不带密钥。
- 按当前端口、本用户进程、程序位置和 Go module/Git 根目录核实源码；拉取只允许快进。保护未提交及 ignored 文件，拒绝分叉，禁用 hook/子模块递归；不编译、重启、改配置。
- 原 Git pull 快进会覆盖 ignored 文件的风险已通过独立复核发现并修复：fetch 后使用 merge --ff-only --no-overwrite-ignore，并复核工作区未变化。

## 验证

- 25 项界面逻辑测试通过，包含厂商别名、独立 Antigravity、未知组及稳定顺序。
- `pnpm test`：83 项全部通过。Rust CPA 三项临时仓库测试通过：快进、无新提交、脏工作区、游离 HEAD、分叉、源码路径类型、ignored 配置碰撞、禁止 hooks；分组额度目标测试通过。
- `pnpm check` 与生产前端构建通过。
- Playwright 1.63.0 + WebKit/Chromium：新分组流程及现有凭证回归通过，覆盖批量范围、删除确认、手动额度、选择清理/记忆、布局切换、空组退回全部、CPA 管理和更新结果/失败反馈、920/1120 宽度无横向溢出。
- 独立代码复核完成；仅使用合成 IPC 和临时 Git 仓库。没有点击真实管理入口、读取配置或拉取真实 CPA 仓库，没有编译/重启 CPA。

## 本机交付

`pnpm package` 通过，Apple Silicon 本机安装到 `~/Applications/ModelLens.app`，旧 0.2.18 备份到 `~/Downloads/ModelLens-backup-before-0.2.19.app`。构建产物及安装副本通过 `codesign --verify --deep --strict`；原生启动确认 0.2.19、凭证分组、CPA 管理与拉取更新入口。仅退出/重开 ModelLens，未触发真实 CPA 拉取；保持 ad-hoc 签名、未公证。
