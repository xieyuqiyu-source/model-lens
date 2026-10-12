# 0.2.17：凭证开关保持排序

基于 0.2.16 `7f502a5`，2026-10-12。

删除紧凑列表按启用状态分组的逻辑及旧规则测试。卡片、探查优先右侧列表、菜单栏均直接使用后端按账号名称及 ID 排列的顺序；开关只改变状态，不移动条目。没有新增排序偏好或本地列表副本。

- `pnpm test:ui`：23 项通过，删除已废弃的“启用账号优先”测试。
- `pnpm test`：74 项通过；`pnpm check`、`pnpm build` 通过。
- `browser-credentials.py`：WebKit / Chromium 通过。补齐单个开关的合成 IPC，在混合启停状态下验证卡片、紧凑列表和菜单栏逐项开/关后位置不变；原有额度、导入、删除、错误及外部变更流程同时通过。
- 开发只使用合成 IPC，未操作真实账号开关。`pnpm package` 通过，Apple Silicon 安装到 `~/Applications/ModelLens.app`，原 0.2.16 备份至 `~/Downloads/ModelLens-backup-before-0.2.17.app`。构建产物及安装副本签名校验通过，本机启动显示 0.2.17。沿用 ad-hoc 签名，未公证。
