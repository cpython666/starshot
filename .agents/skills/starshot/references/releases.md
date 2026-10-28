# 更新、恢复与版本

用户于2026-10-26明确要求迁入星笺的更新与版本机制、关联远端，并确认公开仓库提供更新下载。仓库为 https://github.com/cpython666/starshot。详细操作和验收边界在 docs/releases.md / docs/verification.md。

- 版本由 scripts/version.mjs 校验/递增，同步 npm/Cargo/Tauri 六个版本字段；标签必须 vX.Y.Z。当前0.3.1已公开为稳定latest，0.3.0保留为开发预览；0.2.2引入下载后安装确认，旧0.1.0无updater，须手动升级。
- src-tauri/src/updater.rs 与 desktop.rs 使用官方 Tauri updater，限制本仓库下载地址，要求更新签名绑定版本。检查和下载均手动触发，无启动联网。0.2.2 的 download_starshot_update 只下载验签，成功才缓存字节并进入 ready/downloadReady，再唤起主窗口显示「安装并重启」；下载阶段不保存会话、不安装、不重启。
- 选择「稍后安装」可继续编辑；已验签包只保留本次运行，重查或退出清空。install_starshot_update 只消费同一候选版本的缓存，点击安装时获取最新会话，先保存恢复点和旧应用副本，再安装并自动重启；备份失败保留编辑与缓存以供重试。macOS调用app.restart，Windows显式restart_after_install(true)。
- 更新/recovery IPC 仅主窗口；捕获、贴图窗口不授予 updater 插件直通权限。忙时后端拒绝图像操作；Windows on_before_exit 先释放单实例锁再 cleanup。清理后安装器启动失败必须使用预缓存Env做进程级重启，不能再用AppHandle状态或事件，否则应用可能隐藏且无托盘。基于当前Tauri/插件源码核对与编译，此失败路径仍待Windows实机验收。
- src/update-api.ts 单订阅、revision防陈旧响应、延迟订阅销毁；UpdateSettings.vue 固定下载/确认版本，ready后显示WebView内的HTML `<dialog>` 模态确认框。原生confirmationPending与ready原子设置，全局/托盘截图、贴图及退出入口同步受限；主窗口set_update_confirmation按候选版本切换，重新确认先检查没有活动截图。选择稍后成功才解锁，重复读取已暂缓状态不重弹；不能只锁前端，否则新截图可能被busy回调丢弃而遗漏于升级备份。App.vue阻断导入/编辑及慢回调；安装开始后不可取消，自动重启未完成时提供重试。
- session.ts 与 recovery.rs 新写schema2，读取schema1/2并在内存归一化；旧文件保持不变，schema1不能带sticker/redact新对象。原图/标注/贴纸/实心遮盖/裁剪/样式/背景/主题；6张/48MP原图、每图40MP、背景24MP、边长16384、JSON256MiB，标注2000/文档、路径100000点/文档。贴纸单个2MiB/2MP、全会话8MiB/8MP。字符串按UTF-8字节限额；解析及全部图片/贴纸预解码后整体替换。
- 恢复点原子创建，SHA-256校验，只接受内部生成ID，拒绝任意路径、符号链接/损坏清单/未来schema。恢复前先备份当前会话；不持久化撤销历史或贴图窗口。启动只列元数据并提示，不自动载入图片。
- 0.3.0可查看占用并明确删除一组恢复点及对应旧应用；删除前预检两棵目录树，拒绝符号链接，不自动清理。统计失败标未知并报告错误，不把0伪装成统计结果，也不因此禁止读取有效会话。
- macOS保留旧app包，Windows只保留主exe；不宣称一键降级或完整安装回滚。图片恢复点和应用备份不入Git。
- 星镜独立更新私钥保存在仓库外及Actions Secret，不与星笺共用；公钥在tauri.conf。用户本轮明确授权0.3.1提交、推送、公开发行及同步Homebrew；这不构成后续任务的默认操作授权。
- verify工作流双平台构建；release工作流集齐macOS/Windows完整文件并经真实minisign验签、版本绑定后生成latest.json与SHA256SUMS，停在草稿。平台证书/公证仍需配置，更新签名不能替代平台信任。

发布流程不覆盖已有草稿资产；重跑前需人工检查或删除旧草稿。恢复点列表的 compatible 仅指 schema 兼容；读取时才强制校验 SHA-256、字段和图片内容。

自动测试、浏览器模拟IPC、平台构建与真实升级验收分别记录，不把模拟安装或编译通过写成实机升级成功。

## 历史发行状态（0.3.0，2026-10-27）

- `v0.3.0`标签/应用源码为 `cd7a69107c6473d1262d67d8666268756f754e52`，普通验证40365269047与签名发行40365419560全部成功。Node44项含真实minisign全通过；macOS core17/desktop31通过（另1项手动模型测试忽略），Windows core15/desktop29通过；Windows实际NSIS隔离安装/卸载及安装后的离线worker通过。
- 2026-10-26 17:23:11 UTC公开为prerelease；固定tag下载可用，不成为稳定updater的latest。Homebrew Tap提交0b2d188已公开，真实最终DMG的下载、隔离安装、版本/签名核对与卸载通过。online audit仅拒绝GitHub prerelease政策，不宣称audit全绿。哈希与具体证据统一见docs/verification.md和docs/homebrew.md。
- 0.3.0构建时Windows模型由scripts/prepare-ocr-models.mjs从固定官方来源下载并校验大小/哈希；0.3.1因原S3地址返回403改用固定哈希镜像，见下。二进制模型Git忽略，Windows程序编译嵌入，无运行时下载。两模型共12,226,852字节；macOS使用系统Vision。
- 0.3.0已补跑0.2.2最终确认门控的7组浏览器回归；这仍是模拟IPC。旧版真实升级/恢复、完整原生GUI、多显示器/混合DPI、权限/全局热键/开机启动矩阵和平台发行证书仍未全部验收。预览发行不能作为正式稳定版验收记录。

## 当前发行状态（0.3.1，2026-10-28）

- 0.3.1于2026-10-28 12:15:48 UTC公开为稳定latest，包含轻量截图交互。最终源码/标签对应fe609e8，普通验证40681522684与签名发行40683246836全部成功，发行validate的52项测试含真实签名校验均通过；完整来源、哈希与产物证据统一见 [验证记录](../../../../docs/verification.md)。
- 发布前用户反馈检查更新后交互仍旧；当时公开列表仅有0.3.0 prerelease，固定tag的latest.json可用，但客户端稳定latest地址及GitHub latest API均返回404。Starshot Verify 0.3.0独立验收包（com.starshot.verify-030）的原生设置页实际显示无法读取清单，Tauri返回ReleaseNotFound。发布后公开latest.json已逐字节核对与CI一致，该旧验收包原生UI已发现0.3.1；没有改写更新逻辑或降低签名校验要求。
- `src-tauri/tests/macos-updater-smoke.rs`以Tauri mock runtime调用真实updater，在临时0.3.0应用副本上执行损坏包拒绝、旧包不变、签名0.3.1安装及codesign检查，实际执行1项通过。未启动或重启用户应用，应用数据路径隔离；这不覆盖原生GUI下载、自动重启与会话恢复全流程。Windows最终NSIS包的隔离0.3.0→0.3.1升级、固定像素OCR及卸载检查通过；仍不能据此宣称所有原生交互和失败后重启路径通过。
- 首次CI 40680770025因模型原S3地址HTTP 403失败。`scripts/prepare-ocr-models.mjs`现从本仓库独立的`ocr-models-v1`预览Release获取原版模型镜像，仍强制校验原大小和SHA-256，两模型合计12,226,852字节不变；来源、作者与许可保留。镜像仅用于构建，不进入应用稳定latest，也不改成运行时下载；后续双平台CI已通过。
- Homebrew Tap最终提交98abcab已公开0.3.1；Ruby/style/livecheck/在线严格audit/fetch及本轮隔离安装/卸载均通过。初次默认livecheck把模型依赖标签误读为版本1，生成器及Cask现显式使用github_latest，与应用稳定通道一致。具体哈希及证据以 [Homebrew指南](../../../../docs/homebrew.md) 为准。
- 独立验收包使用不同应用标识与数据目录；切换官方标识时需保留旧会话恢复点，不能假定数据目录自动迁移。用户正在编辑的旧应用不应为测试强制退出。
- 完整原生GUI、多显示器/混合DPI、权限/全局热键/开机启动矩阵仍未全部验收；macOS仍为ad-hoc签名且未公证，平台发行证书边界不因稳定latest发布而改变。

## 历史验证（仅适用0.2.2）

0.2.2本机Rust core10项、desktop16项（含实际Pending转移、确认门控与窗口ACL）通过。前端单元、浏览器模拟IPC、双平台构建与最终产物SHA-256见docs/verification.md；真实升级、失败后重启和平台证书仍待验收，未创建tag或正式Release。

最终源码c346878及Actions run40329996324包含原生确认门控，macOS、Windows和签名安全校验全部通过。门控前6组浏览器回归通过；门控后脚本扩为7组，但测试空间消失而未执行，不能把此前截图/结果作为最终门控联动验收。
