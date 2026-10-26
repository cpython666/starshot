# 星镜版本、更新与发布

源码与下载仓库：[cpython666/starshot](https://github.com/cpython666/starshot)。用户已确认使用公开仓库承载更新下载。应用只在手动检查更新时访问 GitHub，不在启动时联网，不上传图片或编辑内容。

## 版本控制

采用稳定三段版本 `主版本.次版本.修订版本`，Git 标签为 `vX.Y.Z`。`scripts/version.mjs` 同时校验 package.json、package-lock.json 的两个版本位置、Cargo.toml、Cargo.lock 与 tauri.conf.json；升版必须严格递增，不修改依赖版本。

```sh
npm run version:check
npm run version:bump -- 0.2.1
npm run version:check -- v0.2.1
```

升版后更新 CHANGELOG.md，并完成测试、构建、提交和审核。版本号一致不代表版本已经发布。

## 应用内更新与恢复

「设置 → 更新与恢复」提供手动检查、版本说明、下载进度与安装确认。候选版本在确认时固定，过期状态不能覆盖更新的下载状态。正式包通过 Tauri 官方 updater 校验签名及签名绑定的版本，只接收本仓库 Release 下载地址。

安装前依次完成下载验签、当前编辑会话恢复点与旧应用副本保存，任何一步失败都会停止安装。安装过程中暂停截图、贴图和编辑操作，避免快照之后继续修改。Windows 安装器接管应用退出；若启动失败但应用资源已清理，将释放单实例锁并重新打开旧版，可从已保存恢复点继续。若系统连旧版进程都无法启动，需要手动重开；macOS 安装后由用户重启。

恢复点保存在本机应用数据目录 `recovery/`，包含当前最多6张原图、标注、裁剪、美化配置、本地背景和界面主题，并记录版本、校验和与时间。可手动创建或恢复，恢复前先备份当前会话；解析、校验和图片解码全部成功后才替换编辑器内容。不会保存撤销历史、置顶贴图窗口，也不会自动载入或上传恢复点。平时仍只保留内存会话；明确备份或安装更新才持久保存图片。恢复点可能包含完整原图，应由用户妥善保管。

旧应用副本保存在 `application-backups/`，macOS 为应用包，Windows 为主可执行文件。它是故障处理留存，不等于已实现一键降级；Windows 的完整安装回退仍应使用上一版本安装包。恢复编辑会话不会自动更换应用版本。

0.1.0 没有 updater，升级至0.2.0需要手动安装；0.2.0及之后的版本才具备应用内更新能力。没有已公开的更新包时，检查会显示服务不可用/未发布等错误，不会伪报“已是最新版”。

## 签名与 GitHub Actions

星镜使用独立的 Tauri 更新签名密钥，不与其他产品共用。公钥位于 tauri.conf.json，私钥只保存在仓库外的本机受限目录以及 GitHub Actions 的 `TAURI_SIGNING_PRIVATE_KEY` Secret；当前密钥无附加密码。需加密密码时另配置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。不得把私钥、备份、用户图片或日志放入 Git。

- `verify.yml`：每次推送/PR校验版本、前后端测试、格式与 macOS Apple Silicon / Windows x64 打包，不需要发行密钥；独立脚本任务执行真实 minisign 验签回归。
- `release.yml`：已有 `vX.Y.Z` 标签触发，或手动指定已有标签；版本、提交与标签匹配后构建两个平台，要求签名密钥。
- `release-manifest.mjs`：要求完整且唯一的平台产物，真实验签并核对版本绑定，然后生成 latest.json、SHA256SUMS 和发布说明。
- 流程只创建/更新 Release 草稿，不自动公开；拒绝覆盖已发布的同版本 Release 及已有草稿资产。已有资产的草稿需要重跑时，先人工检查并删除旧草稿，流程不会覆盖文件。

预期产物为 macOS 的 DMG / app.tar.gz / 签名，Windows 的 NSIS exe / 签名。`latest.json` 同时包含 `darwin-aarch64` 与 `windows-x86_64`。稳定更新地址为 `https://github.com/cpython666/starshot/releases/latest/download/latest.json`，Release 必须公开且成为最新稳定版本才能用于客户端更新。

Tauri 更新签名与平台代码签名不是同一件事。目前 macOS 默认 ad-hoc 签名，未 Apple 公证；Windows 未配置发行者证书。macOS 可配置 APPLE_SIGNING_IDENTITY、APPLE_CERTIFICATE、APPLE_CERTIFICATE_PASSWORD 以及 APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID。未取得平台证书不能声称已解决 Gatekeeper 或 SmartScreen 提示。

## 每次正式发布前

1. 升版并写变更记录；运行本地检查和双平台 CI。
2. 对候选安装包做两平台安装、旧版升级、签名损坏拒绝、备份失败停止、重启和会话恢复验收。开发构建或模拟 IPC 不能替代此项。
3. 创建并推送对应标签，让流程生成草稿；核对两平台文件、校验和、发布说明。
4. 确认验收完成后，手动发布草稿为最新稳定版。首次公开发布须由用户明确决定。

当前执行过的检查与尚未验证项以 docs/verification.md 为准。
