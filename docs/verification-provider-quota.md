# 新厂商额度与图标：独立分支验证

> 历史分支记录：下述探查范围和安装版本是当时状态。后续已与全厂商自动探查合并至 0.2.13，当前交付见 [0.2.13 验证](verification-0.2.13.md)。

日期：2026-10-11。工作区：`model-lens-isolated`，分支：`codex/isolated-dev`。基线 `ed4fab5`（0.2.11），保留已合并的模型探查、凭证管理和本地连接保存。未操作原 `model-lens` 工作树。

## 实现

- xAI / Grok：固定 credits 账单 GET，缺少总额度比例或失败时回退固定月度账单 GET。周额度存在时不额外拉取月度账单；成功显示对应真实周期，未知周期不会推断为周。产品使用占比仅表示共享额度消耗贡献，不能变成各模型独立余额。不会调用模型做付费健康探测。
- Gemini / Gemini CLI：使用 `retrieveUserQuota` 读取模型额度桶，保留 0%、100%、缺失百分比与仅剩余数量的区别；展示实际重置时间。
- Antigravity：使用 `retrieveUserQuotaSummary` 展示分组及窗口。
- Gemini 项目标识来自列表白名单字段；缺失时仅 `loadCodeAssist` 查询。没有项目则停止并提示；不调用 onboard、不下载认证文件。
- `quotaItems` 与原有 `five/week` 并存；刷新失败和列表重载保留旧数据。支持停用账号查询，自动模型探查的支持范围保持 Codex / Claude OAuth。
- Grok 添加 Lobe Icons 单色资源，Gemini CLI 修复已有 Gemini 图标映射；不改变主题令牌。卡片按实际模型／分组展示，紧凑列表依次优先 5h、1w、其他实际周期，同优先级多个百分比标“最低”，未知项不参与计算。

## 接口依据（公开源码）

- [CLIProxy 管理端 xAI 查询](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/xai/data.ts)、[接口常量](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/constants.ts)、[账单与分组解析](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/builders.ts)、[xAI 共享额度测试](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/tests/xaiQuotaUnavailable.test.ts)。只参考契约并自行实现必要逻辑，未引入其付费模型健康检查。
- [官方 Gemini CLI 服务](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/code_assist/server.ts)、[响应类型](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/code_assist/types.ts)。
- 图标来源与许可见 [THIRD_PARTY.md](THIRD_PARTY.md)。

## 已验证

- `pnpm test`：45 项 Rust 测试通过，其中 10 项新增厂商测试覆盖真实周期枚举、共享贡献语义、月度回退、固定请求目标、停用查询、项目发现、数据投影、错误脱敏与旧数据保留。
- `pnpm test:ui`：21 项界面逻辑测试通过，包括新厂商紧凑额度选择、0 值、未知值、月度标签以及已有主题与探查逻辑。
- `pnpm check` 与 `pnpm build`：通过。
- `uv run --with playwright python tests/browser-provider-quota.py`：WebKit / Chromium 通过，新厂商卡片、真实明暗主题、图标加载、0/100/数量/未知值、失败后保留数据、400×270 紧凑条与无横向溢出。截图仅含合成账号，保存在 `/tmp`，未纳入仓库；已查看 WebKit 深色截图。
- `uv run --with playwright python tests/browser-credentials.py`：WebKit / Chromium 原有停用查询、文件/粘贴导入、删除、冲突与错误、菜单栏回归通过。
- `MODELLENS_TEST_URL=http://127.0.0.1:1421 uv run --with playwright python tests/browser-probe.py`：WebKit / Chromium 原有自动采样、取消、部分结果、手动分析与历史回归通过。
- 代码审查指出的产品贡献误算和周期枚举问题均已修复，复核 10 项 provider 测试通过，无新增阻断项。

## 尚未验证／交付范围

仅使用合成 HTTP / IPC，没有调用真实额度接口、模型、账号开关、导入或删除；未读取本机连接记录、认证文件或钥匙串。厂商 OAuth 订阅、地区、服务端版本差异仍需实际账号验证；API Key 不声称可用。

本轮未修改版本号、未合并主分支、未打包或替换本机应用，本机正式版仍为此前交付的 0.2.11。本轮先交付独立开发分支供合并。用户随后明确结束 Figma 设计阶段，Grok 图标与新厂商额度条目无需同步设计稿，不再列为交付缺项。
