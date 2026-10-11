# 凭证管理独立开发验收

> 此文件保留独立开发阶段记录。两侧已在 0.2.11 统一交付，最终合并、安装与验证状态以 [0.2.11 验证记录](verification-0.2.11.md) 为准。

日期：2026-10-11。基于已合并的 `330abe8` / 0.2.10，在独立工作树 `model-lens-isolated`、分支 `codex/isolated-dev` 开发。模型探查由另一进程负责；本轮不修改探查目录或原始 `model-lens` 工作目录，不统一新的发布版本号。

## 三项交付

1. **启用或停用均能查询额度**：移除 Rust 与主窗口/菜单栏的停用限制。手动“全部刷新”查询所有支持的凭证，保持开关原状态；不增加自动刷新，不临时启用账号。当前厂商支持范围仍为 Codex / Claude。
2. **JSON 导入**：连接后从“凭证优先”打开导入弹窗，选择单个文件或粘贴 JSON。后端检查文件名、1 MiB 上限和含有效 `type` 的单个对象；导入前刷新列表，保守比较 Unicode 规范化与大小写等价名称，发现同名拒绝覆盖。导入后重新读取列表确认文件出现。导入内容不写入 ModelLens 本地数据或日志，不进入广播快照。
3. **删除凭证**：卡片垃圾桶打开确认弹窗，显示账号与文件名。后端重新获取列表，对身份、开关、文件元数据的变更标记进行核实，再删除单个文件。拒绝运行时/配置来源/不安全文件名以及同一文件对应多个账号的删除；写后核实列表，不把仅收到成功 HTTP 当作已删除。

请求文件名通过 URL query 编码；支持已协商的 v8 `/credentials` 和 v0 `/auth-files`。HTTP 204 空响应也可处理。任何写操作只尝试一次，关闭请求库自动重试，不在写入失败时切换版本重发。写后读取失败会标记列表待核实；后续写入按钮禁用，先手动刷新列表。

## 实现边界

- 新逻辑位于 `src-tauri/src/management/mutations.rs` 及其测试；沿用现有内存会话和跨窗口广播。
- 前端请求仍在 feature API，状态通过 `useCredentials` 管理，增加即时互斥避免双击提交。
- 现有卡片抽出为 `CredentialCard.tsx`，删除确认与导入为独立组件；复用原 Modal、字体、间距、主题令牌和动效。
- `src/App.tsx` 仅向 ImportDialog 传入凭证状态；`src-tauri/src/main.rs` 仅注册两个新命令。未修改 `src/features/probe/`。
- 版本仍为 0.2.10 开发基线，正式版本号留待两条功能线整合。本轮产物不覆盖本机已安装应用，不退出或重启用户服务。

## 验证

- `pnpm test`：28 项通过，0 失败、0 跳过。
- 新增 8 项 Rust 测试（含多个分支与循环场景）：非法/超大 JSON、路径名、重复名称、五类 Unicode 等价冲突、外部变更、运行时/配置来源/共享文件保护、v8/v0 文件名 query 编码、HTTP 200/204、写失败后核实、不自动重试、HTTP 成功但结果未落实、读取失败标记 stale，以及停用查询不发开关写入。
- `pnpm test:ui`：15 项通过。
- `pnpm check`、`pnpm package`：通过。
- `tests/browser-credentials.py`：WebKit 与 Chromium 通过，使用 `tests/fixtures/credentials-ipc.js` 合成 IPC。覆盖停用单账号/批量查询、文件/粘贴导入、文件过大、重名/错误保留弹窗、导入内容不持久化、删除取消/确认、待核实禁用、外部状态变化、菜单栏停用查询、导入弹窗键盘打开/Esc/焦点恢复；无未捕获页面错误。
- 本轮独立开发服务器使用 1421，未操作其他端口进程。已有焦点回归在同一测试地址的 WebKit / Chromium × 主窗口 / 菜单栏四组通过；断开状态下导入入口现在断言为禁用，连接后的弹窗焦点由新的凭证测试覆盖。
- 已查看 WebKit 的合成主窗口及导入弹窗截图，卡片新增操作与弹窗内容可见，沿用现有布局与主题；截图未提交。
- 独立审查发现的 Unicode 文件名冲突问题已修复并复核通过，无剩余阻断项。
- 构建产物 `codesign --verify --deep --strict` 通过；仍为 ad-hoc 签名、未公证。

## 已知限制及未验收项

- 官方上传/删除接口无原子 no-clobber 或条件版本写入。客户端最新列表预检查无法消除检查后另一客户端修改同名文件的竞态；导入/删除时应避免其他客户端并行修改相同文件。保守名称比较可拒绝部分在区分大小写系统上本可并存的名称。
- 一次导入一个 JSON 文件，不支持目录扫描或 JSON 数组批量导入。只验证结构，不据此声称 Token 有效。
- 全部测试使用合成 JSON、HTTP 和 IPC；未读取真实凭证、管理密钥或本地连接文件，未导入/删除真实凭证，未查询真实额度或调用模型。真实厂商与本机安装后的流程未验收。
- 本轮无可调用的 Figma 编辑工具，未同步新增删除按钮/弹窗与导入状态，不声称 Figma 已一致。

## 公开接口依据

- [CLIProxy v8 路由](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/api/server_management_v8.go)
- [上传与删除处理](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/api/handlers/management/auth_files_crud.go)
- [官方管理接口说明](https://github.com/router-for-me/CLIProxyAPIDocs/blob/main/docs/en/management/api.md)

产物：`src-tauri/target/release/bundle/macos/ModelLens.app`（独立工作树）。仅提交源码、合成测试和文档，不提交构建产物、截图或真实账号数据。
