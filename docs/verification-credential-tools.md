# 凭证管理增强：开发验证

> 此文记录开发阶段验证；后续按用户要求纳入 0.2.14，发布与安装结果见 [0.2.14 交付记录](verification-0.2.14.md)。

日期：2026-10-12。基于 0.2.13 `e649584` 的本地修改，保留此前粘贴自动命名功能。本轮不提交推送、不升级版本、不打包或安装；本机正式版仍为 0.2.13。

## 功能与边界

- 凭证优先页面顶部：按当前连接端口显示高亮 `/v1` 代理地址并复制；客户端 API Key 只显示按钮。点击才通过专用管理字段读取，Rust 直接写 macOS 剪贴板，IPC 返回状态而非密钥。无 Key 和复制失败不报告成功；没有增加依赖。
- 全部开启/关闭针对可切换且需改变状态的凭证。批量先核对所有目标，随后逐项写入和核实；任意失败或外部变更停止，报告已核实数量，不重试或撤销已成功项。最终写前检查启用状态与凭证变更标记。
- 复选框、全选和删除所选；不可独立删除的凭证不能选择。确认弹窗列出全部账号和文件名，外部变更后禁用确认。逐个使用现有删除接口，不调用全目录删除。
- GPT/Codex 等级从元数据及用量响应白名单字段读取，未知等级不当作 Free；Free/Plus/Pro 标签以中性、蓝、紫色区分，新增颜色仍来自唯一 tokens.json，六套主题对比测试覆盖。
- Free 使用实际返回的周期；单周 primary window 不显示为 5h，缺失周期标“周期”。保留真实 0%、未知值及重置时间，不推断固定账号配额。
- 剩余重置次数只读：优先用量响应 `rate_limit_reset_credits`；缺少时最多增加一次固定 `GET /backend-api/wham/rate-limit-reset-credits`。使用明确可用次数或服务标为 available 的 Codex 重置项；不使用 applicable 数量冒充全部可用量。失败保留上次数值并标注；不请求 consume、不消费次数。
- 启动仍只恢复凭证列表与模型目录；上述真实额度查询、批量操作和复制只有用户点击才执行。

## 接口依据

仅参考公开接口字段，独立实现必要逻辑；没有复制上游重置消费或付费探测流程。

- [CLIProxy 管理端 Codex 数据层](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/codex/data.ts)
- [接口常量](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/constants.ts)
- [重置次数响应结构](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/resetCredits.ts)
- [账号元数据字段](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/resolvers.ts)

## 已验证

- `pnpm test`：58 项 Rust 测试通过。新增覆盖密钥只进入复制回调、空值与认证失败不写剪贴板；批量完整开启、重复目标、预检/中途变化、写后核实失败即停止、最终写前变更标记；Free 周期、未知等级、重置 0/缺失/非法值及查询失败保留旧值。
- `pnpm test:ui`：22 项通过，含等级白名单、批量请求投影、主题颜色对比和既有功能。
- `pnpm check`：TypeScript / Rust 检查通过。
- WebKit / Chromium：新增 `browser-credential-tools.py` 通过，覆盖地址复制、后端复制动作与错误、等级颜色、剩余次数、全开全关、全选删除确认/取消/变化、920/1120/1280 宽度布局；地址剪贴板使用测试替身，Key 复制仅验证命令与反馈。
- 双浏览器原有 `browser-credentials.py` 与 `browser-provider-quota.py` 回归通过，包括粘贴自动命名入口、导入删除、停用额度、新厂商和菜单栏。
- 独立审查发现的最终写前 token 漏检已修复，5 项批量测试独立复核通过。

## 未验证

全部 HTTP/IPC 与界面账号均为合成数据，没有读取真实密钥或连接文件，没有真实复制 API Key、启停/删除凭证或查询厂商。未构建安装版与原生剪贴板实测。实际账号的等级/Free额度/剩余重置次数仍取决于上游提供字段，不能将合成测试视为所有账号兼容验证。
