# 0.2.14：凭证管理增强统一交付

日期：2026-10-12。基于 0.2.13 `e649584`，按用户最新要求将此前两轮本地修改统一提交、推送、打包并安装。

## 变化

- 粘贴凭证由后端根据厂商与 email 自动命名，缺少 email 使用时间标识；去掉文件名输入框，选择文件保留原名，重名不覆盖。
- 凭证优先顶部提供代理 `/v1` 地址高亮复制、客户端 API Key 按钮复制。Key 仅由后端写入剪贴板，不返回前端或写入日志。
- 全部开启/关闭、全选与批量确认删除。逐项核实，遇到外部变化或失败停止，不自动重试；确认弹窗展示所选账号与文件名。
- GPT/Codex 等级标签（Free/Plus/Pro 颜色区分）、Free 实际额度窗口、剩余可重置次数。未知数据不推算；重置次数仅查询展示，不消费次数。
- 保留模型自动探查、DSH 锁定的指纹内核、其他厂商额度、主题、连接持久化以及焦点与窄栏修复。

实现与接口范围、前一轮审查详见 [凭证增强开发验证](verification-credential-tools.md)。

## 发布检查

- 版本统一：package.json、Cargo.toml/Cargo.lock、Tauri 配置与界面均为 0.2.14。
- `pnpm test:ui`：22 项通过。
- `pnpm test`：58 项 Rust 测试通过。
- `pnpm check`：TypeScript / Rust 检查通过。
- 生产资源 WebKit / Chromium：新凭证工具和原有凭证流程回归均通过（`browser-credential-tools.py`、`browser-credentials.py`）；合成 IPC，不访问真实账号。
- `pnpm package`：Apple Silicon Release `.app` 构建通过；构建产物和已安装应用的 `codesign --verify --deep --strict` 均通过，Info.plist 版本为 0.2.14。

## 备份

- 未提交修改和新增文件备份：`~/Downloads/ModelLens-source-backup-20261012-004350/`。
- 原 0.2.13 应用备份：`~/Downloads/ModelLens-backup-before-0.2.14.app`。
- 正常退出 ModelLens 后安装至 `~/Applications/ModelLens.app`，重新启动显示 0.2.14。保留应用数据和偏好。本机观察到连接与用户原有模型选择恢复，新复制按钮、全部开关和全选入口可见；没有触发真实业务操作。

## 验证边界

没有操作真实账号开关/导入/删除、查询真实额度、调用模型或复制真实 API Key。开发和浏览器测试均为合成数据；新账号信息及真实剪贴板操作留给用户实测。仅交付 Apple Silicon Mac，ad-hoc 签名，未 Apple 公证。升级不修改应用数据、CLIProxy 或 DSH。
