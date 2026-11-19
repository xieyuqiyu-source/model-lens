# ModelLens Windows x64

本目录放置 Windows 专用 Rust 和 React 适配、Tauri 打包配置、图标、验收与发布脚本。公共功能继续使用项目根目录的 `src/` 和 `src-tauri/`。编译中间文件保存到 `windows/target/`，发布文件保存到 `windows/artifacts/<版本>/`。

## 安装与使用

1. 下载并运行 [ModelLens 0.2.30 Windows x64 安装包](https://www.ccoos.cn/modellens/windows/releases/0.2.30/ModelLens_0.2.30_x64-setup.exe)。安装程序会检查 Microsoft Edge WebView2 Runtime，有需要时联网安装。
2. 需要 CLIProxy 时，先在 Windows 上启动它，在 ModelLens 点击「连接 CLIProxy」，填写本机管理端口和管理密钥。第三方 API 和手动检测可独立使用。
3. 点击任务栏通知区域的 ModelLens 图标查看额度和快捷探查。右键菜单可打开主窗口或退出应用。

Windows 版包括模型探查、指纹库更新、凭证管理、额度与外观设置。CPA 的本机进程发现、安装、启停与更新需要独立操作。管理连接和第三方 API 记录保存到 `%LOCALAPPDATA%\io.modellens.desktop`。

安装包有用于应用内自动更新的 Tauri 签名。首次运行安装程序时，Windows 可能显示 SmartScreen 提示，因为安装包当前没有 Authenticode 签名。可核对 [SHA-256](https://www.ccoos.cn/modellens/windows/releases/0.2.30/SHA256SUMS)。

## 应用自动更新

Windows 版启动时检查更新，持续运行时每 6 小时复查。发现新版本后，主窗口工具栏显示更新入口。在设置中也可点击「检查应用更新」。点击「安装并重启」后，应用下载安装包、验证签名、安装并重新启动，保留已有连接记录和偏好。

安装包和更新清单由「超级 Linux」的 HTTPS 静态站点提供。[更新源](https://www.ccoos.cn/modellens/windows/latest.json)

## 本机开发和构建

使用 x64 Windows、Microsoft C++ Build Tools（使用 C++ 的桌面开发）、WebView2、Node.js 24、pnpm 10.10.0 和 Rust 1.89。以下 PowerShell 命令在项目根目录运行。

```powershell
pnpm install --frozen-lockfile
rustup target add x86_64-pc-windows-msvc
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'windows/target'
pnpm exec tauri dev --config windows/tauri.conf.json
```

构建与测试使用相同的 Windows 配置。

```powershell
node node_modules/@tauri-apps/cli/tauri.js build --ci --target x86_64-pc-windows-msvc --config windows/tauri.conf.json -- --locked
$env:TAURI_CONFIG = Get-Content windows/tauri.conf.json -Raw
cargo test --locked --release --features tauri/custom-protocol --target x86_64-pc-windows-msvc --manifest-path src-tauri/Cargo.toml
pnpm test:ui
```

NSIS 安装包位于 `windows/target/x86_64-pc-windows-msvc/release/bundle/nsis/`。GitHub Actions 也会构建 Windows x64 安装包并运行合成 CLIProxy 验收，可从通过的运行下载 `modellens-windows-x64` 产物。安装验收脚本仅在可丢弃的 Windows 测试环境运行。

## 发布新版到超级 Linux

1. 升级 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 的版本，并更新 Cargo 锁文件。提交后等待 Windows 构建及验收成功。
2. 从成功的运行下载对应提交的产物到 `windows/artifacts/<版本>/`。在持有发布密钥的本机设置 `TAURI_SIGNING_PRIVATE_KEY_PATH` 为签名私钥文件的绝对路径，然后执行发布脚本。需要 Python 3.11+、pnpm、SSH 和对「超级 Linux」的部署访问权限。

```sh
gh run download <运行编号> --name modellens-windows-x64 --dir windows/artifacts/<版本>
python3 windows/publish.py "windows/artifacts/<版本>/ModelLens_<版本>_x64-setup.exe" --notes "本版本更新说明"
```

脚本在本机完成签名，上传安装包、签名和 SHA-256，核对暂存文件后原子替换更新清单，再通过 HTTPS 回读验证。更新私钥只保存在签名机器，并保留安全备份。GitHub Actions 和服务器均不需要更新私钥。遗失私钥后，现有用户需要手动安装携带新公钥的版本。

3. 在 GitHub Actions 手动运行「ModelLens Windows x64」，打开 `verify_update`，验证旧测试版通过正式 HTTPS 源更新、重新启动并恢复连接。
