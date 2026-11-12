# 0.2.26 第三方 API 支持无 Key 探查

2026-11-12，基于 0.2.25 `ea8fef5`。

- 新增第三方 API 时可以留空 Key。读取 `/models` 和调用 `/chat/completions` 时，仅在该组有 Key 时发送 Bearer 鉴权。
- 编辑时留空沿用该组原设置。保存前仍需成功读取模型列表。
- `pnpm check`、101 项 Rust 测试和 25 项界面逻辑测试通过。回环 HTTP 测试核实无 Key 时能保存、重新读取配置和采样，两种请求均无 `Authorization` 头。
- WebKit 和 Chromium 的合成 IPC 回归通过，覆盖无 Key 添加、切换、编辑留空沿用原 Key、记住选择、删除和采样。未访问真实第三方 API。
- `pnpm package` 完成。已安装到 `~/Applications/ModelLens.app`，核对版本 0.2.26、arm64 架构和 `codesign --verify --deep --strict`。旧版备份到 `~/Downloads/ModelLens-backup-before-0.2.26.app`。签名为 ad-hoc，未 Apple 公证。
