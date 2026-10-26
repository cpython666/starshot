# 更新、恢复与版本

用户于2026-10-26明确要求迁入星笺的更新与版本机制、关联远端，并确认公开仓库提供更新下载。仓库为 https://github.com/cpython666/starshot。详细操作和验收边界在 docs/releases.md / docs/verification.md。

- 版本由 scripts/version.mjs 校验/递增，同步 npm/Cargo/Tauri 六个版本字段；标签必须 vX.Y.Z。当前开发版本0.2.0；旧0.1.0无updater，须手动升级。
- src-tauri/src/updater.rs 与 desktop.rs 使用官方 Tauri updater，限制本仓库下载地址，要求更新签名绑定版本。检查手动触发，无启动联网。先验签，再写会话恢复点与旧应用副本，最后安装；失败不伪报成功。
- 更新/recovery IPC 仅主窗口；捕获、贴图窗口不授予 updater 插件直通权限。忙时后端拒绝图像操作；Windows on_before_exit 先释放单实例锁再 cleanup。清理后安装器启动失败必须使用预缓存Env做进程级重启，不能再用AppHandle状态或事件，否则应用可能隐藏且无托盘。基于当前Tauri/插件源码核对与编译，此失败路径仍待Windows实机验收。
- src/update-api.ts 单订阅、revision防陈旧响应、延迟订阅销毁；UpdateSettings.vue 固定确认版本、持有编辑锁，展示阶段/进度/错误；App.vue阻断导入/截图/编辑及慢回调。
- session.ts 与 recovery.rs 协议schema1，原图/标注/裁剪/样式/背景/主题；6张/48MP原图、每图40MP、背景24MP、边长16384、JSON256MiB，标注2000/文档、路径100000点/文档。字符串按UTF-8字节限额；解析及全部图片预解码后整体替换。
- 恢复点原子创建，SHA-256校验，只接受内部生成ID，拒绝任意路径、符号链接/损坏清单/未来schema。恢复前先备份当前会话；不持久化撤销历史或贴图窗口。启动只列元数据并提示，不自动载入图片。
- macOS保留旧app包，Windows只保留主exe；不宣称一键降级或完整安装回滚。图片恢复点和应用备份不入Git。
- 星镜独立更新私钥保存在仓库外及Actions Secret，不与星笺共用；公钥在tauri.conf。用户已授权创建公开远端与首版同步，不等于授权公开发行安装包或安装到Applications。
- verify工作流双平台构建；release工作流集齐macOS/Windows完整文件并经真实minisign验签、版本绑定后生成latest.json与SHA256SUMS，停在草稿。平台证书/公证仍需配置，更新签名不能替代平台信任。

发布流程不覆盖已有草稿资产；重跑前需人工检查或删除旧草稿。恢复点列表的 compatible 仅指 schema 兼容；读取时才强制校验 SHA-256、字段和图片内容。

自动测试、浏览器模拟IPC、平台构建与真实升级验收分别记录，不把模拟安装或编译通过写成实机升级成功。

源码3ce733f已推送远端main，Actions run40315358657的双平台构建与真实签名校验全部成功；macOS最终DMG版本/arm64/ad-hoc完整性已核验，验收详情及SHA-256见docs/verification.md。Windows真实安装升级、失败后重启和平台证书仍待验收；没有创建tag或正式Release。
