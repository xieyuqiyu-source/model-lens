# 0.2.31 Windows CLIProxy 源码安装和管理

2026-11-18，基于 0.2.30。

- Windows 复用桌面端的本机服务面板。支持发现已有服务、克隆 CLIProxyAPI 官方仓库、使用 Go 编译、启动并保存本机连接，以及版本核对、快进更新和正常启停。
- Git 仓库位于所选安装目录下的 `CLIProxyApi`，运行文件位于 `.dev`。新安装仅监听回环地址。由 CPSwitch 或 EasyCLIProxyAPI 托管的程序保留原有控制方式。共享 Windows 控制台的进程使用原启动入口关闭。
- Windows Server 2025 x64 上通过 93 项 Rust 测试和 25 项界面逻辑测试。真实原生目录选择器可填入默认安装路径。应用通过临时 Git 仓库和合成 Go 服务完成克隆、编译、自动连接、目录发现、快进更新、收到正常关闭信号并退出、重新启动，以及新版启动失败后恢复原程序与配置。验收还核对新配置文件的 Windows ACL。[端到端验收](https://github.com/xieyuqiyu-source/model-lens/actions/runs/44464730503)
- 在临时目录克隆 CLIProxyAPI 官方源码 `db7f177`，使用 Go 1.26.1 交叉编译为 Windows amd64 可执行文件，并核对其中的 Go 模块和 VCS 元数据。
- Apple Silicon 上通过 `pnpm check`、105 项串行 Rust 测试、WebKit 和 Chromium 服务面板合成回归、`pnpm package` 与 `codesign --verify --deep --strict`。

Windows 验收使用合成凭证与服务。安装步骤和已有服务边界见 [Windows 开发说明](../windows/README.md)。
