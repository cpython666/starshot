# 实现与验证要点

以下均对应仓库源码；验收结果仅在 `docs/verification.md` 维护。

| 入口 | 职责 |
| --- | --- |
| `src/App.vue` | 主编辑器、原图与标注、美化偏好、撤销/重做、最近会话、导入/导出。`?view=capture` / `?view=pin` 分别挂载独立界面 |
| `src/editor.ts` | 预览和PNG/JPEG共用Canvas管线；标注原图坐标，裁剪只改变可见窗口；马赛克实际替换像素 |
| `src/{privacy.ts,objects.ts,preferences.ts,share.ts}` | 本地敏感规则、对象/贴纸预算、非图片偏好与原创宣传图；OCR原文不存入候选状态 |
| `src-tauri/src/{ocr.rs,ocr/,desktop_settings.rs}` | macOS Vision / Windows离线子进程识别；快捷键与开机启动事务保存 |
| `src/geometry.ts` | 选区夹取、候选命中、连续方向边缘/四边支撑启发式；`capture-regions.worker.ts`一次分析受限探测图，并非系统控件树 |
| `src/components/CaptureOverlay.vue` | 原生冻结图/明确demo分支，选择、8向调整、键盘微调、放大镜、就地标注/撤销及copy/save/pin/edit路由 |
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
- 用户反馈菜单栏原图标难辨认（2026-10-26）。托盘不再复用带底板的应用图标：`public/tray-template.svg`生成36px黑色透明模板，macOS启用icon_as_template；`public/tray-contrast.svg`生成32px紫色/白边图标供Windows。源码接入desktop.rs，已有image依赖负责解码，无新增依赖。生成命令为 `npm run tauri -- icon public/tray-template.svg --output src-tauri/icons/tray-template --png 36`，另一份替换为tray-contrast和32。此修复自0.2.1引入；系统实机效果与设计预览分开记录。

## 轻量截图改进（0.3.1已发布）

- `finish_capture`接受可选action（edit/copy/save/pin，缺省edit兼容旧调用）及仅标注时传入的PNG；原生核对活跃窗口、矩形、实际PNG格式和精确裁切尺寸。保存取消返回completed=false保留选区；失败不关闭。quick成功更新current/已有main会话但不创建或唤起main，edit才唤起。
- 每次捕获窗口使用唯一`capture-{id}-{index}`，旧请求/关闭事件不得影响后续捕获；取消只恢复捕获前原本可见的主窗。主窗原本隐藏时不等待180ms移除动画。保存/贴图使用已持有operation锁的内部helper，不能重入外层加锁命令。
- 原生未标注选区直接裁切RGBA用于copy/save，仍编码一份PNG供最近图片和已有主窗状态；不能声称完全没有PNG编码。前端仅显示图片，不为放大镜再常驻完整CPU截图canvas；小画布每帧最多一次读色，隐藏时不读。
- 区域候选Worker在界面可交互后分析一次，探测图长边1920、算法硬上限4MP，最多256边缘候选；不再二次480跳采样。四边支撑拒绝文字/单线/密集纹理，圆角与短断边可保留；表格相连格线、严重缺边/极低对比仍可能漏检。演示卡片候选也走真实算法。
- 系统窗口保留xcap返回的前后顺序，指针处只采用最前窗口及其内部像素边缘候选，避免后台小窗抢选；全屏作为末级候选。Tab按矩形坐标匹配当前候选（不能用Vue代理对象的indexOf），同位置点击/抬起保留键盘选择，真实移动后重新命中。
- `renderPreview`复用同一标注管线但按显示尺寸/DPR（最多2）限制4MP；坐标和导出`renderImage`保持原图像素，遮盖仍最后重绘不透明边缘。预览不能作为导出分辨率。

## 回归方式

0.3.0关键边界：

- `main.ts`按窗口动态加载界面；空白主窗关闭使用前后端握手，先锁前端输入再释放WebView。任何图片、异步导入、OCR、更新或退出工作均阻止释放；macOS Reopen重建主窗。已有编辑关闭主窗仍保留会话。
- 退出请求总是向存活编辑器取最新状态，备份失败不得退出，备份成功后先flush最新原生状态再确认。安装器重启使用专用路径，不被普通退出门控拦截。
- OCR只在用户点击后运行，输入为当前裁剪与已绘制对象；异步结果校验图片ID和编辑revision。Windows worker嵌入固定模型，标准输入/输出传图与结果、90秒超时、2线程，结束后释放进程。模型来源/哈希/独立许可见 `src-tauri/models/ocrs/NOTICE.md`。源码构建先运行 `npm run models:prepare`，固定字节数与SHA校验后写入Git忽略的模型路径；CI执行同一步骤，安装版仍完全内置离线模型。0.3.1因原S3地址403改从独立`ocr-models-v1`预览Release下载原版镜像，原哈希及两模型合计12,226,852字节不变；该构建依赖不进入应用稳定更新通道。
- `redact`使用不透明黑色，覆盖所有对象后再绘制；2×渲染后按物理像素floor/ceil补绘防止插值暴露边缘。零面积草稿不进入渲染或会话。候选预览框和选中辅助线不进入导出。
- PNG贴纸单个2MiB/2MP，全会话按对象实例合计8MiB/8MP；导入、移动恢复和undo/redo均先校验预算。预解码缓存按当前会话及撤销/重做可达对象回收；拖动首次实际位移才创建撤销点。
- 快捷键注册、OS开机启动和设置文件采用事务与回滚；插件autostart直通权限不开放给WebView。启动到托盘与开机启动分别保存/读取实际状态。


`npm test`、`npm run build`、Rust `--no-default-features` 检查固定数据；桌面改动另跑 `cargo check --locked`、`cargo test --locked --lib`、fmt和平台打包，具体命令见README。

浏览器可验证Canvas真实像素、裁切/取消/撤销、图片背景/边框和下载；不能据此宣称原生截图、录屏权限、置顶透显、快捷键和多屏通过。所有测试素材为原创，导出的截图与临时验证脚本放Git忽略的 `artifacts/`，可复用回归脚本放 `tests/`。

`tests/browser-update-regression.mjs` 在已授权的Ego space/page运行原创像素与模拟IPC，覆盖备份/恢复失败保护、确认锁、固定安装版本和陈旧进度；脚本覆盖范围不等于已经执行通过，不能替代真实安装器验收。各版本构建和回归结果见 [验证记录](../../../../docs/verification.md)。

`src-tauri/tests/macos-updater-smoke.rs`默认忽略，需`updater-smoke` feature及`STARSHOT_UPDATER_SMOKE_DIR`指定真实发行产物；以mock runtime驱动真实Tauri updater，仅替换临时应用副本，检查损坏包拒绝、旧包保留和新版codesign。`tests/windows-installer-smoke.ps1`以最终NSIS做隔离升级、固定像素OCR和卸载检查；两者均不等于原生GUI下载、自动重启与会话恢复全流程，0.3.1实际执行结果见验证记录。
