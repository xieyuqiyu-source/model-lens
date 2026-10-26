# 0.2.25：保存多个第三方 API

2026-10-26，基于 0.2.24 `7bff594`。

## 变更

- `external-api.json` 由单条改为列表，最多 50 组，每组含随机 id、名称（可选，默认域名）、Base URL 与 API Key；仍为目录 0700、文件 0600、原子替换，Key 只在 Rust 后端使用。
- 0.2.24 的单条记录读取时自动作为第一组保留，下次保存时写成新格式，无需重新输入。
- 界面：第三方模式用下拉切换已保存的 API，末项新增；铅笔按钮编辑或删除当前一组。编辑时 Key 留空沿用该组；所选组在主窗口与菜单栏间同步并记住。
- 命令改为按 id 读取模型和采样；`external_status` 由 `external_list` 取代，只返回 id、名称和地址。

## 验证

- `pnpm check` 通过；100 项 Rust 测试（新增多组增改删、留空沿用各自 Key、校验失败不保存、0.2.24 单条迁移）、25 项界面逻辑测试通过。
- `tests/browser-external-probe.py` 扩展为 WebKit / Chromium 多组场景：添加失败提示、添加两组、下拉切换后模型列表跟随、采样携带所选 id、编辑改名沿用 Key、重载记住选择、逐组删除至空状态、页面不出现 Key。原有 `browser-probe`、`browser-focus`、`browser-account-probe` 双浏览器回归通过。
- 未调用任何真实第三方 API 或中转站。
- `pnpm package` 完成，构建与安装副本通过 `codesign --verify --deep --strict`，安装至 `~/Applications/ModelLens.app`（0.2.25），旧版备份至 `~/Downloads/ModelLens-backup-before-0.2.25.app`。ad-hoc 签名，未 Apple 公证。
