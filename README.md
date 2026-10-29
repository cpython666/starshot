# 星镜 Starshot

**把灵感，定格成星。** 本地优先的桌面截图、标注、置顶贴图与图片美化工具，使用 Rust / Tauri 2 / Vue 3 / TypeScript。名称、中文界面和五套外观沿用用户的星空系列偏好，作为独立产品开发，不包含桌面宠物。

当前版本 **0.3.1**，已进入应用内更新通道，macOS / Windows 同为目标平台。原生实测、自动检查、双平台构建和安装结果分别记录在 [验证记录](docs/verification.md)。

## 当前功能

- 按需冻结多显示器画面；系统窗口边界吸附与本地矩形边缘预判，支持手动框选、8向调整、拖动、1px / 10px键盘微调、放大镜和HEX读数。
- 截图后在选区下方直接标注、撤销/重做、复制、保存和贴图；主动选择「更多编辑」才进入完整页面。PNG / JPEG / WebP 导入；延时3 / 5 / 10秒截图。
- 箭头、矩形、椭圆、画笔、荧光笔、文字、序号、马赛克、实心遮盖；对象选择/移动/单独修改删除，PNG贴纸旋转缩放、文字水印；非破坏裁剪、撤销重做。
- 主动本地识别手机号、邮箱、证件、银行卡和常见凭据，预览勾选后实心打码。macOS支持中英文，Windows主要支持拉丁文字和数字；仍需复核漏检，扫描不含装饰背景。
- 渐变、纯色、透明、本地图片背景；留白、圆角、边框、阴影；自适应、1:1、4:3、16:9、9:16画布。
- 同一Canvas管线预览并导出PNG / JPEG，1× / 2×；原生保存对话框、复制图片、置顶贴图。
- 贴图拖动、缩放、旋转、翻转、透明度、隐藏工具条；托盘和全局快捷键。
- 未发布工作区新增：手动逐屏滚动长截图（本地去重拼接/预览）、贴图双击关闭及右键菜单、截图横竖辅助线（H切换）。2026-10-29已按用户授权安装到本机日用入口；公开0.3.1尚不包含这些改动。
- 未发布悬浮预选修复：启动按实际鼠标位置预选、无标题窗口仍可命中、全屏框向内显示；macOS已有辅助功能授权时优先查询真实控件边界，未授权或查询失败保留窗口/像素候选。Windows控件识别及真实跨应用命中率仍待验收或实现；[调查依据](docs/snipaste-detection-research.md)。
- 后续工作区细节（尚未安装）：贴图复制/另存为/送到编辑器，美化一键恢复默认且可撤销，帮助显示实际快捷键；macOS应用菜单和⌘Q复用退出前备份检查，Dock菜单退出仍待处理。[Starclip对照与后续清单](docs/product.md#对照星笺-starclip-的细节补齐2026-10-29)。
- 星夜、月白、素白、青苔、暮樱外观和自定义配色；记住延时/导出选项，可保存不含图片的美化预设。最近图片平时仅保留在内存；手动、退出前选择备份或安装更新前可保存完整恢复点，并查看占用/删除。
- 可配置全局热键、可选开机启动/启动到托盘、空白窗口释放后重新唤起、退出保护；产品介绍图与文案预览/保存/明确复制。

- 设置内手动检查更新、下载并校验签名；完成后弹出「安装并重启」或「稍后安装」。确认安装时备份最新编辑会话与旧应用副本，安装成功后自动重启，支持恢复本机会话。

源码仓库：[cpython666/starshot](https://github.com/cpython666/starshot)。版本同步与签名流程见 [发布指南](docs/releases.md)，[0.3.1 已公开](https://github.com/cpython666/starshot/releases/tag/v0.3.1)，Homebrew安装与验收边界见 [分发说明](docs/homebrew.md)；0.1.0需手动升级才能获得应用内更新。

Windows控件识别及helper源码位于[`src-tauri/src/window_regions/windows.rs`](src-tauri/src/window_regions/windows.rs)，公共截图界面为[`CaptureOverlay.vue`](src/components/CaptureOverlay.vue)。原生验收源码包含[`desktop/windows_integration.rs`](src-tauri/src/desktop/windows_integration.rs)、[`windows_smoke.rs`](src-tauri/src/desktop/windows_smoke.rs)和`tests/windows-*.py/js/cs`，不以自制闭源DLL代替实现。构建、原创夹具及复现入口见[Windows v3记录](docs/windows-native-v3.md)；本地源码状态与公开版本须以各次验证记录区分。

2026-10-29新增功能及完整验收源码已分批提交至`main`，尚未发行对应的新安装版。下方v0.3.1下载仍是此前公开的稳定包；源码更新不代表日用应用或安装包已同步更新。

浏览器可以完整体验导入、原创示例区域选择、标注、美化与下载；浏览器中的截图为明确标示的原创示例，真正桌面截图及置顶贴图仅原生版提供。

0.3.1新增就地截图工具条、细边/圆角候选识别与显示尺寸预览优化。旧版需下载更新并确认「安装并重启」才会启用新交互，仅检查更新不会替换正在运行的软件。

## 安装与更新

Apple Silicon Mac（macOS 12+）：

```sh
brew install --cask cpython666/tap/starshot
```

Windows x64 安装包及 macOS DMG 见 [0.3.1 下载](https://github.com/cpython666/starshot/releases/tag/v0.3.1)。macOS 尚未 Apple 公证，Windows 尚无平台发行者签名；两平台更新包均有独立Tauri更新签名。Windows 离线 OCR 模型合计约 12.2 MB，随安装包内置，主要识别拉丁文字和数字；macOS 使用系统 Vision，无需下载模型。

## 运行

需要 Node.js 22.12+、Rust 1.90、Tauri 对应系统构建环境。

```sh
npm ci
npm run models:prepare # 仅构建 Windows 时需要；macOS 使用系统 Vision，无需模型
npm run dev
# 浏览器： http://127.0.0.1:1430
npm run tauri dev
```

macOS首次截图需在系统设置的屏幕录制权限中允许应用，可能需要重新打开。首启不发起截图，不读取剪贴板。关闭主窗口后留在托盘；从托盘选择「退出星镜」彻底退出。

快捷键：`⌘/Ctrl + Shift + 2` 开始截图，`⌘/Ctrl + Alt + 3` 贴出最近截图原图。编辑器支持 `⌘/Ctrl + S` 导出、`⌘/Ctrl + Z` 撤销、`⌘/Ctrl + Shift + Z` 重做。全局快捷键发生系统冲突时会显示错误，仍可用按钮与托盘操作。

## 检查与打包

```sh
npm run version:check
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run tauri -- build --bundles app,dmg  # macOS
npm run tauri -- build --bundles nsis    # Windows
```

[CI配置](.github/workflows/verify.yml)提供macOS Apple Silicon和Windows x64 NSIS构建与产物上传，不发布版本、不使用真实屏幕做测试。macOS本地ad-hoc签名不是Apple公证；Windows未设置发行签名。

## 范围与参考

星镜专注截图、标注、贴图与图片美化；用户已确认当前会话最近6张图片足够，不规划持久截图历史，长期剪贴板历史由星笺负责。

当前仍缺少系统控件树语义识别、长截图自动滚动与复杂页面支持、独立全文OCR/Windows中文模型、跨屏连续选区、贴图分组、GIF等；不做可编辑项目文件导入导出。首版手动长截图已在工作区实现，尚待原生矩阵验收。完整范围、限制和后续验收见 [产品说明](docs/product.md)，竞品资料与采用决策见 [截图交互研究](docs/screenshot-research.md)。

交互研究参考 [Snipaste官网](https://www.snipaste.com/)（2026-10-26访问）；品牌、UI、图标、演示素材和实现独立制作。没有复用竞品代码、素材或产品名称。星镜不是已达到竞品全部功能的成品。
