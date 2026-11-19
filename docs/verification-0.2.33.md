# 0.2.33 中转站模型检测

- 第三方 API 默认采样两次，手动补测第三次时沿用原模型、API 配置和参考库版本。补测更新同一条历史。CLIProxy 和手动检测继续使用原流程。
- 支持 Chat Completions、Responses 和 Anthropic Messages。普通 JSON 和标准 SSE 都解析正文与完成状态。已有配置沿用 Chat Completions。接口没有模型列表时可填写模型 ID。
- ModelTrace 使用固定的参考库快照，在本地独立分析同一批有效回答。来源和授权记录见 [内核来源](../src/features/probe/vendor/UPSTREAM.md)。
- 109 项 Rust 测试、25 项界面逻辑测试通过，覆盖普通响应、流式分片、截断、模型目录、手填 ID、不同协议的鉴权及旧配置升级。开发测试使用合成端点及凭证。
- WebKit 和 Chromium 合成测试通过。核对默认两次、第三次需用户点击、三条探针互异、ModelTrace 对照、配置编辑与保留选择，以及既有检测和指纹库更新流程。
