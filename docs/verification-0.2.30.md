# 0.2.30 Windows x64 与应用自动更新

2026-11-18，基于 0.2.29。

- Windows x64 复用探查、第三方 API、凭证、额度和指纹库代码，使用原生窗口及 Windows 系统托盘。连接已独立启动的 CLIProxy，进程发现、安装、启停和更新由 Windows 用户独立操作。
- Windows 启动时及运行期间每 6 小时检查客户端更新。用户确认后，Tauri 下载、验证安装包签名、安装并重启。发布目录位于「超级 Linux」现有 HTTPS 站点下，安装包按版本保存，更新清单原子替换。
- GitHub Actions 的 Windows Server 2025 x64 环境生成 NSIS 安装包，85 项 Rust 测试和 25 项界面逻辑测试通过。实际安装后核对了应用版本、合成 CLIProxy 连接与模型目录、系统剪贴板和托盘切换。发布包对应提交 `1efa7ab`。该次运行在验收完成后输出中文时遇到控制台编码错误，设置 Python UTF-8 后同一应用代码的完整流水线通过。[发布包构建](https://github.com/xieyuqiyu-source/model-lens/actions/runs/44455540794) · [完整安装验收](https://github.com/xieyuqiyu-source/model-lens/actions/runs/44456211825)
- HTTPS 回下载的安装包 SHA-256 与本机一致。使用 Tauri 更新器同款验签库验证通过，修改安装包一个字节后验签失败。[安装包](https://www.ccoos.cn/modellens/windows/releases/0.2.30/ModelLens_0.2.30_x64-setup.exe) · [SHA-256](https://www.ccoos.cn/modellens/windows/releases/0.2.30/SHA256SUMS)
- 旧测试版 `0.0.0` 从正式 HTTPS 更新源发现 `0.2.30`。点击「安装并重启」后完成下载、验签与升级，自动重启并恢复合成 CLIProxy 连接。重启后检查显示已是最新版本。[线上更新验收](https://github.com/xieyuqiyu-source/model-lens/actions/runs/44456974394)
- macOS 的 TypeScript 和 Rust 检查、105 项串行 Rust 测试、25 项界面逻辑测试通过。WebKit 和 Chromium 合成回归覆盖凭证、探查、历史、指纹库更新与失败回退。Apple Silicon 的 `pnpm package` 与 `codesign --verify --deep --strict` 均通过。

验收全程使用合成服务和账号。Windows 安装包通过 Tauri 应用更新签名验签，当前没有 Windows Authenticode 签名。Windows 配置、验收与发布命令见 [Windows 开发说明](../windows/README.md)。
