# 0.2.9 焦点阴影与界面精简

## 变化

- 检查全部现有弹窗与下拉入口，共用鼠标/键盘输入方式识别。移除硬焦点 outline；键盘与文本输入以主题色柔和阴影提示，120ms 过渡，失焦恢复。下拉浮层本身继续使用活动行表示键盘位置，不额外画整框。
- 下拉展开、配色选中不再改变描边颜色，使用原有主题 shadow；保留活动项、对勾及选中底色。
- 检测提示词和原始回答与采样详情去掉悬浮底色、按下缩放及重复右侧提示，保留折叠功能和箭头旋转。
- 手动回答默认两行，约 75px，保留拖高、滚动和键盘输入。
- 精简探查、连接、导入、服务与历史说明；仍明确模型分析/导入未接入，保留示例标记、失败信息、密钥本地保存与账号开关立即生效等必要说明。

## 验证

- `uv run --with playwright python tests/browser-focus.py`：WebKit、Chromium × 主窗口、400×270 菜单栏四组通过。
- 主窗口弹窗：外观、管理连接、服务位置、历史、JSON 导入、菜单栏预览。覆盖鼠标关闭、遮罩关闭、键盘打开、Esc 与入口焦点恢复；外观弹窗另测 Tab 导航及背景不可聚焦。
- 菜单栏：外观、服务位置弹窗及模型下拉。主窗口模型、示例凭证下拉另测鼠标选项、重复点击入口、外部关闭、方向键、End、Enter、Esc、Tab 收起及焦点恢复。
- 折叠栏鼠标悬浮与点击不改变背景、无焦点描边；输入框 rows=2、实际内容高度两行，多行合成输入可以滚动。六套明暗主题的输入阴影显示、失焦移除与过渡属性均通过。
- 检查本地浏览器浅色和深色截图：布局正常、回答框两行、折叠区无悬浮高亮。未捕获页面错误。
- `pnpm test:ui`：15 项通过；`pnpm test`：16 项通过，独立钥匙串测试默认跳过；`pnpm check`、`pnpm package` 通过。
- Impeccable 检查仅提示既有额度条 width 动画，属于已确认的额度填充反馈，本轮未改。
- 本机正常退出并备份 0.2.8 至 `~/Downloads/ModelLens-backup-before-0.2.9.app`；安装 `~/Applications/ModelLens.app`，版本与 `codesign --verify --deep --strict` 校验通过。
- 本机安装版实际查看两行回答框、点击输入框及折叠栏：阴影和展开正常，无硬焦点描边。随后用户开始操作，停止额外界面操作，保留用户状态。

## Figma 同步

保留组件关系、自动布局和变量：

- [折叠详情组件](https://www.figma.com/design/SORAmBaRKtmNnNMpmISYUg?node-id=50-647)：隐藏重复提示、记录无悬浮高亮规则，同步已应用的两个实例。
- [两行手动回答组件](https://www.figma.com/design/SORAmBaRKtmNnNMpmISYUg?node-id=50-654)：可编辑输入区域高度 75.2px，沿用字体与主题变量，精简标题和状态。
- [模型下拉框](https://www.figma.com/design/SORAmBaRKtmNnNMpmISYUg?node-id=36-105)：展开阴影与 120ms 状态切换，保留原四个状态及点击交互。
- [已应用主窗口](https://www.figma.com/design/SORAmBaRKtmNnNMpmISYUg?node-id=56-237)：精简标题说明、同步导入未开放状态。
- [主题与外观](https://www.figma.com/design/SORAmBaRKtmNnNMpmISYUg?node-id=69-3659)：六套主窗口标题说明与三套选中阴影。组件及对应区域截图已检查。

只同步上述改动，不宣称全部历史设计稿与程序完全一致。

## 边界

本轮不读取密钥、不测试真实账号开关、额度刷新或模型调用；应用启动仍按既有逻辑恢复本地连接，界面观察到列表恢复不等于重新验证全部持久化链路。未在 Intel Mac、Windows 验证。当前仍为 ad-hoc 签名，未 Apple 公证。
