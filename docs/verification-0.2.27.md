# 0.2.27 第三方 API 兼容 SSE 采样响应

2026-11-12，基于 0.2.26 `3ddab9e`。

- 第三方 API 收到 `stream: false` 仍可能返回 `text/event-stream`。应用从 SSE 事件中拼接第一项回答的正文分片，检查结束原因，继续沿用现有采样校验。错误事件、缺少结束状态和空正文不能参与评分。
- `pnpm check`、103 项 Rust 测试和 25 项界面逻辑测试通过。回环 HTTP 回归用分片响应复现 0.2.26 的 JSON 报错，核对新版返回正文、结束状态和模型字段。单元测试覆盖截断、错误事件和达到长度上限。
- WebKit 和 Chromium 的生产前端合成 IPC 回归通过，覆盖第三方 API 保存、切换与采样。事件格式参考 [HTML Standard](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream)。
- 对用户提供的回环接口另行诊断。实测两个模型返回 HTTP 200 和 SSE，但均缺少可评分正文。对其中一个模型显式请求流式输出，仍得到空正文。
- 已打包安装 arm64 版本，核对 `~/Applications/ModelLens.app` 为 0.2.27，且通过 `codesign --verify --deep --strict`。旧版备份在 `~/Downloads/ModelLens-backup-before-0.2.27.app`。签名为 ad-hoc，未 Apple 公证。
