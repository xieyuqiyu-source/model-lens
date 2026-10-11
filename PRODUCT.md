# ModelLens
<!-- impeccable:product-schema 1 -->
## Platform
web
Tauri 容器内的桌面界面；本轮只交付本机 macOS Apple Silicon。
## Stack
已讨论并接受 Tauri 2、React、TypeScript、Rust。09A 的 CSS 变量方案沿用，不引入新视觉体系。
## Users
由维护者个人使用，查看本机 CLIProxy、模型探查及凭证额度。
## Capabilities and Constraints
第一阶段：只读发现已运行 CLIProxy 的程序路径和工作目录，持久记忆；实现第 11 版主窗口及菜单栏 UI。不会启动、停止、接管服务；不会读取凭证、调用模型或查询额度。凭证内容为明确标识的 UI 示例。
## Brand Commitments
保留已接受的 09A / 11B 风格，紧凑、轻量、中文界面与英文示例账号。
## Evidence on Hand
docs/images/main-window-11b.png 是用户认可的界面参考。
