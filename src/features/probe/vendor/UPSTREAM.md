# ModelLens 模型指纹内核来源

沿用 DSH `dsh-probe-kit-0.3.0/src/dsh-modeltrace/vendor` 已锁定的 [Ikaleio/lm-detector](https://github.com/Ikaleio/lm-detector/tree/4ffda94e5e24a05c4f55d6f14a1e27f4ef23c8f3) 快照。以下六份文件逐字节复制，数学逻辑、提示词、数据及校准参数不作修改；MIT 声明见 `LICENSE`。

提交：`4ffda94e5e24a05c4f55d6f14a1e27f4ef23c8f3`。参考库 58 个模型，构建于 `2026-10-07T20:04:35.858118+00:00`。

| 文件 | 上游路径 | SHA-256 |
| --- | --- | --- |
| challenge.js | shared/challenge-browser.js | 65884da44d29b7d5337993609414596272010cb07cffe98b0c5546b1fe9728e4 |
| fingerprint-core.js | shared/fingerprint-core.js | 5a096c0e5638b9b7e266714725660549273b018e0b5eddbec3302ef96af21f16 |
| shared-detector.ts | shared/shared-detector.ts | 15c4b40b1d7d1efdfb72901bfcb4181b539d4d7ba69f2a1ea645094869685c21 |
| unified_bank.json | data/unified_bank.json | 61dade060fed71948b723f0dcde77bf5317c1e3b0bf5862fe08464d0e6d9d537 |
| shared_detector.json | data/shared_detector.json | d5290bbaf02c1906191a40ef745c7b2fbc865a2feea5dae38e3d049985639926 |
| LICENSE | LICENSE | 238afb57e498990742f84b9ce6167b4e256ac06104a9938ce953bc41c14bea51 |

`types.ts` 是同一提交 `shared/types.ts` 中 BankModel、Bank、Analysis、Output 的纯类型子集；两个 `.d.ts` 为 ModelLens 的 TypeScript 接入声明，不参与计算。

Vite 直接编译 TypeScript，两个 JSON 原样作为内置库打包。使用内置库分析时创建 Web Worker、加载并校验 JSON 哈希，计算结束即释放 Worker。三份有效样本且校准匹配时显示库内闭集置信度，一两份只排名。

更新内置库时从同一个上游提交整组更新六份文件，同步此表、Worker 哈希与来源记录，运行 `pnpm test:ui` 与浏览器探查验收。

从 0.2.29 起，运行时通过 [上游数据接口](https://lm.ikale.io/docs/sdk/data/) 下载配套的模型目录和二进制评分参数，使用 `lmfpd 0.2.0` 本地评分。独立更新的数据格式为 `lmfpd-data-v1` / `shared-detector-v2`。本页哈希对应安装包内置库。
