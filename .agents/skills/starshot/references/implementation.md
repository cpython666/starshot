# 实现与验证要点

以下均对应仓库源码；验收结果仅在 `docs/verification.md` 维护。

| 入口 | 职责 |
| --- | --- |
| `src/App.vue` | 主编辑器、原图与标注、美化偏好、撤销/重做、最近会话、导入/导出。`?view=capture` / `?view=pin` 分别挂载独立界面 |
| `src/editor.ts` | 预览和PNG/JPEG共用Canvas管线；标注原图坐标，裁剪只改变可见窗口；马赛克实际替换像素 |
| `src/geometry.ts` | 选区夹取、候选命中、受限矩形边缘启发式；并非系统控件树 |
| `src/components/CaptureOverlay.vue` | 原生冻结图/明确demo分支，选择、8向调整、键盘微调、放大镜与颜色读数 |
| `src/components/PinWindow.vue` | 原生贴图拖动、缩放队列、旋转翻转、透明度、工具条显隐 |
| `src-tauri/src/desktop.rs` | xcap捕获、窗口候选、多显示器覆盖层、快捷键、托盘、复制/保存/贴图和IPC |
| `src-tauri/src/imaging.rs` | 图片解码/上限、裁切参数、坐标转换、同目录临时文件原子替换 |
| `src-tauri/{build.rs,permissions,capabilities}` | 主编辑器/捕获/贴图三类窗口最小权限；lib.rs测试生成后的ACL |
| `.github/workflows/verify.yml` | macOS Apple Silicon app/DMG与Windows x64 NSIS检查和构建；配置存在不代表远端运行通过 |
| `src/{update-api.ts,session.ts}` 与 `src-tauri/src/{updater.rs,recovery.rs}` | 签名更新与完整会话恢复；协议、签名和失败边界见 [更新要点](releases.md) |

## 必须保持的关系

- 窗口/选区传输用物理图片像素；xcap macOS窗口和显示器边界是逻辑points，覆盖层用LogicalPosition/Size。Windows用PhysicalPosition/Size，先移动到目标屏，再设置最终尺寸，避免DPI切换二次缩放。此代码策略仍需混合DPI原生矩阵验收。
- 图片导入意图在FileReader前分配版本，所有await后与错误反馈前核对；背景另记请求版本和目标图片ID，切图片/选新背景后旧结果不能覆盖。源码：App.vue `importFile`、`loadImage`、`openDocument`。
- 设置/历史弹窗开启时先处理Tab/Esc，阻断底层编辑快捷键。取消截图返回需要重新绑定画布和ResizeObserver。
- 演示SVG先栅格化PNG供区域预览，避免SVG preserveAspectRatio与选框映射不一致。demo必须显式绑定，桌面设置里的原创示例不能访问原生捕获帧。
- 输入导入24MP/32MB，原生截图每屏40MP，渲染/原生导出64MP/64MB，最长边16384；不能在截图确认关闭覆盖层后才用更低导入阈值拒绝截图。2×应实际重新计算尺寸并预先禁用超限导出。
- 保存先完整编码，再在目标同目录创建临时文件、fsync、原子替换；失败保留旧目标。JPEG以白底合成透明像素。取消原生对话框不提交编辑或丢失当前图。
- macOS系统占用Cmd+Shift+3；贴原图使用 `CommandOrControl+Alt+3`，开始截图 `CommandOrControl+Shift+2`。实际冲突通过UI报错，不使启动失败。
- 新图最近会话最多6张、原图合计48MP；最多60个撤销步骤。预算不包含背景/临时渲染/快照，不代表进程硬内存上限。贴图最多12张。

## 回归方式

`npm test`、`npm run build`、Rust `--no-default-features` 检查固定数据；桌面改动另跑 `cargo check --locked`、`cargo test --locked --lib`、fmt和平台打包，具体命令见README。

浏览器可验证Canvas真实像素、裁切/取消/撤销、图片背景/边框和下载；不能据此宣称原生截图、录屏权限、置顶透显、快捷键和多屏通过。所有测试素材为原创，导出的截图与临时验证脚本放Git忽略的 `artifacts/`，可复用回归脚本放 `tests/`。

`tests/browser-update-regression.mjs` 在已授权的Ego space/page运行原创像素与模拟IPC，覆盖备份/恢复失败保护、确认锁、固定安装版本和陈旧进度；不能替代真实安装器验收。0.2.0已取得双平台CI构建记录，详见验证文档。
