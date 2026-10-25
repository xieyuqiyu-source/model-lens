# 0.2.24：第三方 API / 中转站探查

2026-10-26，基于 0.2.23 `e30ca3c`。

## 变更

- 模型探查新增“CLIProxy / 第三方 API”来源切换，记住选择。第三方模式填写 OpenAI 兼容 Base URL 与 API Key，保存前读取 `/models` 验证，探查经 `/chat/completions` 直连采样，分析流程不变。
- Key 和地址存于 `external-api.json`（目录 0700、文件 0600），仅 Rust 后端读取，不回传界面；“忘记 Key”删除该文件。
- 远程地址须为 HTTPS，回环地址可 HTTP；拒绝含账号、查询参数、片段的地址；不跟随重定向，错误不回显响应体。此通道启用 macOS 原生 TLS 与系统代理，管理接口保持回环、禁用代理。
- 本地 JSON 文件读写复用连接记录的安全实现。

## 验证

- `pnpm check` 通过；98 项 Rust 测试（新增地址校验、路径与认证头、错误脱敏与不跟随重定向、0600 文件读写）、25 项界面逻辑测试通过。
- 新增 `tests/browser-external-probe.py`：WebKit / Chromium 合成 IPC 验证保存失败提示、保存后 Key 不出现在页面、三次采样仅走第三方命令、结果标注来源主机、重载记住来源、忘记 Key 与切回 CLIProxy。原有 `browser-probe`、`browser-focus`、`browser-account-probe` 双浏览器回归通过。
- 未调用任何真实第三方 API 或中转站；各中转站兼容性需用户实测。
- `pnpm package` 完成，构建与安装副本通过 `codesign --verify --deep --strict`，安装至 `~/Applications/ModelLens.app`（版本 0.2.24），旧版备份至 `~/Downloads/ModelLens-backup-before-0.2.24.app`。ad-hoc 签名，未 Apple 公证。
