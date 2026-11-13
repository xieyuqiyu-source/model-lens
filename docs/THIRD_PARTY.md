# 第三方资源

- Tauri：MIT / Apache-2.0，版本见 src-tauri/Cargo.lock。
- React：MIT，版本见 pnpm-lock.yaml。
- Inter：SIL Open Font License 1.1，通过 @fontsource/inter 本地打包，许可见 licenses/Inter-OFL.txt。
- Lucide：ISC，许可见 licenses/Lucide-ISC.txt。
- OpenAI、Claude、Gemini、DeepSeek、Grok 图标：来自 LobeHub Lobe Icons 的单色 SVG，与 09A 设计稿所用图标一致。来源 https://github.com/lobehub/lobe-icons ，MIT 许可见 licenses/Lobe-Icons-MIT.txt。Grok 新增资源取自 `@lobehub/icons-static-svg@1.95.1/icons/grok.svg`，复用现有单色处理。品牌商标权归各厂商，展示不表示合作或背书。
- ModelLens 应用和菜单栏图标为本项目几何矢量，源文件位于 src-tauri/icons/。

- LM Fingerpoint Detector：MIT。内置指纹库与挑战沿用 `Ikaleio/lm-detector` 提交 `4ffda94e5e24a05c4f55d6f14a1e27f4ef23c8f3`，文件哈希见 [UPSTREAM.md](../src/features/probe/vendor/UPSTREAM.md)。运行时更新库使用同一项目的 [lmfpd Rust SDK 0.2.0](https://crates.io/crates/lmfpd/0.2.0) 和 [公开数据包](https://lm.ikale.io/docs/sdk/data/)。完整许可见 [LICENSE](../src/features/probe/vendor/LICENSE)。

本项目不包含 CLIProxy 二进制。其他依赖许可参见对应包；依赖锁文件用于重现构建。
