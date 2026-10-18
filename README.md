# 璇玑录（Xuanji Lu）

《璇玑录》是《星骰工作室》的 Windows 本地版。当前源码基准：**v261018A（UTC+8）**，内部版本 **0.1.18**。

## 项目结构

- `src/`：React + TypeScript 前端
- `src-tauri/`：Tauri 2 / Rust 桌面端、SQLite 存储与本地命令
- `assets/`、`public/`：应用视觉资源
- `release/`：版本更新说明、产品文档、研究注记、发布清单及完整源码 ZIP
- `patch/`：Windows 差别升级包构建脚本

## 文档索引

发布历史、操作指南、产品介绍、宣传文案、版本与补丁约定、训练场设计说明及 Edge TTS 技术资料均位于 `release/`。最新版为 `更新说明-璇玑录-v261018A.md` 与 `manifest-v261018A.json`。本版使用 `edge-tts-rust 0.1.7` 的 Sec-MS-GEC v2 实现。

## v261018A 完整安装包

Windows x64 完整安装包为 `release/璇玑录-v261018A-x64-完整安装包.exe`，同目录 `.sha256` 文件用于校验；发布说明和验证清单也在 `release/`。安装包不包含用户数据或服务凭据。

## 完整源码归档

`release/璇玑录-v261018A-Full-Source.zip` 为 v261018A 完整源码包，旁边的 `.sha256` 文件可用于完整性校验。在 Linux/macOS 上，可于 `release/` 目录执行：

```sh
sha256sum -c '璇玑录-v261018A-Full-Source.zip.sha256'
```

源码 ZIP 本身保留必要源码、配置、依赖锁文件、发布文档和补丁脚本；不含用户数据库／图片／日志、密钥、`.git` 历史、依赖安装缓存或构建缓存。仓库 `release/` 还包含 v261018A 完整安装包及此前版本的已发布 Windows 安装包与升级包。完整源码 ZIP 已随 v261018A 更新。

## 技术栈

Tauri 2、React、TypeScript、Vite、Rust、SQLite。具体依赖与构建方式见 `package.json`、`pnpm-lock.yaml`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`。

> 本仓库用于源码与项目文档归档，不包含用户个人档案或任何服务凭据。
