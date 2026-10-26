# 星镜 Starshot

**把灵感，定格成星。** 本地优先的桌面截图、标注、置顶贴图与图片美化工具，使用 Rust / Tauri 2 / Vue 3 / TypeScript。名称、中文界面和五套外观沿用用户的星空系列偏好，作为独立产品开发，不包含桌面宠物。

当前版本 **0.2.0 开发版**，macOS / Windows 同为目标平台。macOS Apple Silicon DMG与Windows x64 NSIS已通过GitHub Actions构建；macOS原创示例的原生导出和部分贴图交互已有实测，Windows实机及真实更新安装仍待验收。逐项记录见 [验证记录](docs/verification.md)。

## 当前功能

- 按需冻结多显示器画面；系统窗口边界吸附与本地矩形边缘预判，支持手动框选、8向调整、拖动、1px / 10px键盘微调、放大镜和HEX读数。
- 区域截图后进入编辑器；PNG / JPEG / WebP 导入；延时3 / 5 / 10秒截图。
- 箭头、矩形、椭圆、画笔、荧光笔、文字、序号、马赛克；非破坏裁剪、撤销重做。
- 渐变、纯色、透明、本地图片背景；留白、圆角、边框、阴影；自适应、1:1、4:3、16:9、9:16画布。
- 同一Canvas管线预览并导出PNG / JPEG，1× / 2×；原生保存对话框、复制图片、置顶贴图。
- 贴图拖动、缩放、旋转、翻转、透明度、隐藏工具条；托盘和全局快捷键。
- 星夜、月白、素白、青苔、暮樱外观。最近图片平时仅保留在内存；手动备份或安装更新前可保存完整编辑恢复点。

- 设置内手动检查签名更新、查看版本说明和下载进度；安装前保存编辑会话与旧应用副本，支持恢复本机会话。

源码仓库：[cpython666/starshot](https://github.com/cpython666/starshot)。版本同步、签名和 Release 草稿流程见 [发布指南](docs/releases.md)；尚未公开发行安装包，0.1.0需手动升级才能获得应用内更新。

浏览器可以完整体验导入、原创示例区域选择、标注、美化与下载；浏览器中的截图为明确标示的原创示例，真正桌面截图及置顶贴图仅原生版提供。

## 运行

需要 Node.js 22.12+、Rust 1.90、Tauri 对应系统构建环境。

```sh
npm ci
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

当前仍缺少系统控件树语义识别、滚动长截图、OCR、跨屏连续选区、持久截图历史、可配置全局快捷键、贴图分组、GIF等。完整范围、限制和后续验收见 [产品说明](docs/product.md)。

交互研究参考 [Snipaste官网](https://www.snipaste.com/)（2026-10-26访问）；品牌、UI、图标、演示素材和实现独立制作。没有复用竞品代码、素材或产品名称。星镜不是已达到竞品全部功能的成品。
