# 0.2.32 Apple Silicon Mac 应用自动更新

- Mac 主窗口与 Windows 共用更新入口。启动时及运行期间每 6 小时检查，用户点击「安装并重启」后才下载和安装。Mac 更新源位于「超级 Linux」，使用独立的 Tauri 更新签名密钥和 `darwin-aarch64` 发布目录。旧版 0.2.31 及以下需先替换应用一次。
- Mac 按已检查的更新记录下载并验证签名。替换应用前锁住本机管理会话，已有凭证操作、账号探查或 CPA 任务时可稍后重试。替换期间拦截普通退出，成功后由 Tauri 正常重启。Windows 继续使用原有窗口权限和安装流程。
- 在 Apple Silicon、macOS 26.5.2 使用独立应用标识、临时更新签名和合成 CLIProxy。旧测试版 0.0.0 启动发现 0.2.32，篡改下载被拒绝且旧版仍可运行。修复下载后再次点击安装，应用替换并自动重启，恢复已保存的合成连接和深色偏好。重启后显示已是最新版本，应用包通过 `codesign --verify --deep --strict`。
- `pnpm check`、25 项界面测试及 105 项串行 Rust 测试通过。CPA 回归夹具选端口时同时排除已被其他地址族使用的监听端口，提高在本机多服务环境中的可重复性。
- 正式发布包为 Apple Silicon arm64，通过 `codesign --verify --deep --strict`。包已用专属更新密钥签名并发布，完整 HTTPS 下载的 SHA-256 与本地一致。单独验签通过，改动包内容后验签失败。[应用归档](https://www.ccoos.cn/modellens/macos/releases/0.2.32/ModelLens_0.2.32_aarch64.app.tar.gz) · [SHA-256](https://www.ccoos.cn/modellens/macos/releases/0.2.32/SHA256SUMS)
- 本机备份并正常退出 0.2.29，随后将正式包安装到 `~/Applications/ModelLens.app`。核对版本、代码签名，启动及手动在线检查均显示已是最新版本。Windows 线上更新清单维持 0.2.31。

应用使用 ad-hoc macOS 代码签名，未做 Apple 公证。端到端安装验收采用合成服务；线上验收包含正式包下载、验签与应用内检查。
