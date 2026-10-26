# 星镜验证记录

## 2026-10-26 · 0.1.0开发版

测试使用原创 `public/demo-workspace.svg` 与程序生成的固定像素，没有读取真实剪贴板、没有用真实桌面截图做测试。新项目独立于星笺，未安装到Applications，未提交、推送或发布。

### 自动检查

| 检查 | 本轮结果 |
| --- | --- |
| `npm test` | 7项通过：裁切/比例布局、非法和超限尺寸、样式边界、反向拖动、候选命中与固定像素边缘识别 |
| `npm run build` | Vue strict TypeScript与Vite生产构建通过 |
| Rust `--no-default-features` | 6项通过：图片参数/解码、尺寸上限、坐标转换与原子保存等固定数据检查 |
| Rust `--lib`（desktop） | 7项通过，包括实际生成的三类窗口ACL隔离 |
| `cargo check --locked`、`cargo fmt --check` | 通过 |
| 项目skill `quick_validate.py` | 通过，使用临时uv环境提供PyYAML |

### 浏览器行为与真实像素

Ego Chromium，仅 `http://127.0.0.1:1430` 本机开发页。

- 编辑器：原创示例加载、箭头和文字绘制、撤销/重做、裁剪/恢复原图、比例切换、渐变切换、实际PNG下载。预览1344×1344，2×文件实际2688×2688；证据 `artifacts/qa/starshot-ui-export.png`。
- 13项Canvas像素检查通过：马赛克选区内1728像素改变，选区外0变化，原图不变；PNG透明与JPEG编码白底；裁剪保留源坐标与标注，2×不移动标注；方形居中不拉伸，边框落入实际输出；透明原图阴影没有不透明底片；64MP超限拒绝。结果与脚本在 `artifacts/image-engine-20261026/`。
- 输出尺寸检查：原创8300×8图可按1×预览8444×152；切换2×显示尺寸超限并禁用导出/复制，切回1×恢复。
- 区域选择：560×320卡片吸附、Tab切换990×702候选；右移1px、Shift下移10px、Alt加宽1px；Shift手选154×100。裁切原图对应区域逐RGBA对比零差异，含1440附近视口与1200×750图像坐标映射。
- 演示确认回编辑器、导出298×244、再截图后Esc取消能恢复原画布。证据 `artifacts/capture-crop-raw.png`、`capture-crop-export.png`。
- 历史与主题：月白切换后返回星夜，导入另一张原创图后从最近图片恢复，16:9构图与落日渐变、撤销栈保留。
- 异步/弹窗回归5项通过：慢先导入不覆盖后选图、旧背景不跨文档、较新渐变压过慢背景、过期坏图不覆盖成功状态、设置弹窗阻止底层撤销/保存/导入且退出后撤销可用。`tests/browser-import-regression.mjs`控制FileReader完成顺序，输入、PNG解码、Canvas与Vue界面走真路径，结果在`artifacts/import-regression/results.json`。
- 贴图演示：110%缩放、旋转90°、水平翻转、20%不透明度、隐藏工具条/空格恢复、0重置、Esc回主页。浏览器不能证明原生置顶/拖动/透显。

### 修复与边界

- 演示SVG先栅格化PNG，修复preserveAspectRatio导致的画面与选框映射差异；取消覆盖层后重新绑定画布观察器与渲染。
- 2×输出重新算上限，与原生64MP解码/保存一致；原生40MP截图和24MP导入分别检查，避免截图确认后才被导入阈值拒绝。
- 图片意图版本在读取前分配，背景绑定原文档/独立请求号；旧文件和旧背景不覆盖较新选择，错误反馈也丢弃过期结果。
- 弹窗先拦截底层编辑快捷键，保留Tab焦点循环和Esc关闭。
- 贴原图快捷键改为 `⌘/Ctrl + Alt + 3`，避开macOS系统 `⌘ + Shift + 3`。
- 原生保存采用同目录临时文件、fsync、原子替换；失败不会先截断已有目标。Windows混合DPI窗口先移动后设置最终物理尺寸。

### 平台与原生验收

本机为Apple Silicon macOS，最终0.1.0 app / DMG构建成功，包含最新前端 `index-BX5RX_Il.js` 和 `CommandOrControl+Alt+3`。app以及DMG内app均通过 `codesign --verify --deep --strict`，两处可执行文件SHA256一致；本地ad-hoc签名，未Apple公证。DMG在仓库外临时挂载点核验，已卸载。

- App总文件体积：7,531,606字节（约7.18MiB）。
- DMG：3,157,632字节（约3.01MiB）。
- DMG SHA256：`3859b8a262b5eb7b921c0c67fd0b60be10791fdc0398f6481e53457ce80f69c8`。
- 构建/签名证据：`artifacts/native-verification-20261026.json`。

直接从构建目录启动应用，未复制到Applications。CUA原生窗口实测：首次为空，提示录屏权限但未自动请求/捕获；原创示例1200×750可进入编辑器；原生保存对话框取消后内容保留；最终包成功保存 `artifacts/qa/starshot-native-qa.png`，UI显示完整真实路径，文件实际1344×894。

原生贴图成功创建，观察到100%到110%窗口缩放、90°旋转、透明度控件变化至0.6、工具条隐藏与空格恢复。没有以窗口截图的白底推断系统透明透显，也未独立测量原生拖动位置。原生贴图关闭流程在用户开始操作时停止自动验证，仍待补验；较早测试实例通过应用菜单正常退出后，最终包重启为空会话。

尚未验收：真实屏幕捕获与录屏授权流程、跨显示器/混合DPI/全屏窗口、系统全局热键实际触发、原生贴图实际拖动/透显/关闭、写图片剪贴板、Windows构建与Windows实机。系统控件级语义识别、滚动长截图、OCR等未实现，详见产品范围。

## 2026-10-26 · 0.2.0 更新与版本迁入

新增手动检查签名更新、会话恢复点、同步升版与双平台签名草稿工作流。用户确认公开仓库 cpython666/starshot 承载源码和后续更新下载，独立签名密钥已配置为 Actions Secret；未创建发布标签或公开 Release。

- 六处版本检查为0.2.0，Vue/TypeScript/Vite生产构建通过。
- Node检查30项：本机常规29通过，真实minisign集成项因PATH未安装而跳过；使用临时官方便携minisign单独运行发布脚本测试15/15全部通过，包括真实Tauri签名、损坏产物和篡改可信版本注释拒绝。测试密钥和便携工具已清理。
- 本机macOS的Rust core10项、desktop12项（含生成后的窗口ACL）通过；cargo check和fmt通过。
- 两套GitHub工作流经actionlint1.7.12检查通过；普通验证不使用发行密钥，Ubuntu安全任务强制真实验签测试。
- 会话测试覆盖原图与编辑状态往返、UTF-8限额、schema/像素/裁剪边界、全部图像预解码、备份失败停止恢复；更新监控覆盖陈旧结果、延迟订阅销毁、固定确认版本和未知下载总量。
- 浏览器普通预览正确提示桌面更新/恢复不可用，不调用原生更新。
- 浏览器真实App配模拟IPC：原创1200×750原图手动备份；恢复确认冻结，Esc/导入/截图不能覆盖；取消解锁；备份失败不读取旧恢复点，损坏JSON及坏PNG均保留当前会话；成功恢复后的原图、标注、裁剪、背景、主题与固定数据等价。安装传递固定确认版本和完整会话；20%到80%进度不被陈旧revision覆盖；安装完成保持冻结。更新说明中的HTML显示为纯文字。未调用真实安装/重启、录屏或剪贴板。
- 上述关键交互已整理为 `tests/browser-update-regression.mjs`，四组回归通过；结果在 `artifacts/update-regression/results.json`。沿用既有Ego space/page执行，不创建/结束额外浏览器空间。原导入异步/弹窗五项回归也在0.2.0重新通过。
- Windows异常分支审查发现：官方updater先调用on_before_exit，再启动安装器；Tauri cleanup会隐藏窗口/清空托盘。已修为清理前释放单实例锁、缓存Env，安装器启动失败或任务异常时直接进程级重启，不再发事件给已清理界面。这项是源码审查和编译验证，尚未实机触发失败分支。

### 最终远端构建与产物

[GitHub Actions run 40315358657](https://github.com/cpython666/starshot/actions/runs/40315358657) 对应最终源码 `3ce733fb72761e510d2833fefd341e769506f073`：macOS Apple Silicon、Windows x64、Ubuntu真实签名校验三个任务全部成功。Windows core9项、desktop11项通过（Unix专属符号链接测试不在Windows运行），普通NSIS安装包生成并上传；macOS生成DMG。

最终macOS DMG为 `Starshot_0.2.0_aarch64.dmg`，4,638,824字节，SHA-256：`1a137ec93785c57a24fa2bfaa12c57b82ba13085b3b4f5798089610d3a56b9e0`。只读挂载检查Info.plist版本和构建号均0.2.0、标识com.starshot.desktop、架构arm64；`codesign --verify --deep --strict`通过。挂载已卸载，未启动或安装该包，ad-hoc签名不等于Apple公证。

最终Windows NSIS包为 `Starshot_0.2.0_x64-setup.exe`，3,712,453字节，SHA-256：`c7395c9f51999dc2d19415b4e34a9ca515306962a61021caceed188480d971ab`。下载的artifact ZIP哈希与GitHub API digest一致；只存档和读取文件，未执行安装包。

产物及本地验证记录保存在Git忽略的 `artifacts/ci-0.2.0/run-40315358657/`。浏览器TaskSpace验收后已关闭。远端已关联并推送main，尚未创建发行标签或Release；最后的验证文档同步仅改文档/知识库，不重复构建。

未测试真实更新包安装、Windows安装器失败后的自动重启或Windows实机使用。平台发行签名/公证尚未配置；模拟和构建通过不代表这些原生验收完成。

## 2026-10-26 · 0.2.1 菜单栏/托盘图标

用户反馈macOS菜单栏图标难辨认。定位到托盘复用了带深色底板的应用图标，内部轮廓在18pt显示高度下过细。改为独立透明图标：macOS使用36px黑色alpha模板并启用系统模板着色，Windows使用32px紫色主体与白色外缘；四角框与中心星形放大加粗，应用主图标保持原品牌设计。

- 资产文件核验：macOS模板36×36，440个非透明像素全部为黑色，最外一圈透明，无裁边；Windows图标在16/20/24/32px浅深色背景预览中轮廓可辨。
- `artifacts/tray-icon/comparison.png` 和 `windows-size-review.png` 为原创背景离屏渲染预览，非系统截图；`review.json`记录像素结果。没有把用户提供的菜单栏截图加入Git。
- 本机Node29项通过，minisign集成项在普通PATH下跳过；Vue/TypeScript/Vite构建、Rust core10项/desktop12项、cargo check和fmt通过。版本已统一0.2.1。
- [CI run40317718698](https://github.com/cpython666/starshot/actions/runs/40317718698)对应源码6ee700a；macOS Apple Silicon、Windows x64与真实签名校验三个任务全部成功，两平台测试、图标生成和安装包构建通过。
- 下载产物的ZIP digest与GitHub API一致。macOS DMG为4,767,802字节，SHA-256 `1562e1c3fc46fd6dc4e264f467b34cf185cab9a2867c5485fda7f9aa293cd258`；只读挂载核对版本/构建号0.2.1、com.starshot.desktop、arm64，codesign严格校验通过，已卸载。Windows NSIS为3,758,309字节，SHA-256 `5749425fbdcd0be00642a10a0d78e54b71b9a47ab6fef584eaf34701ab7f87db`。本机产物/核验JSON位于 `artifacts/ci-0.2.1/run-40317718698/`。

未重启用户正在运行的旧应用，未安装或发布。当前菜单栏进程不会热替换编译内嵌图标，需要使用新版重开才生效；系统菜单栏随壁纸自动反色的实机表现、Windows真实托盘高DPI表现仍待原生目检。

## 2026-10-26 · 0.2.2 下载后确认安装并重启

按用户要求将更新拆为「检查 → 更新/下载验签 → 安装并重启确认」。下载完成后唤起主窗口，选择稍后安装可继续编辑；仅在最后确认时备份最新会话并安装。下载与安装IPC分离，已验签包仅缓存于本次运行；重查或退出清空。

- 六处版本统一0.2.2。Node测试30项通过，普通PATH下真实minisign项因工具缺失跳过；Vue/TypeScript/Vite构建通过。
- 本机Rust core10/10、desktop15/15通过，新增生产Pending状态转移测试覆盖未下载/空包拒绝安装、失败缓存重试、候选版本固定、重查清空与成功释放缓存；生成窗口ACL包含仅主窗口可调用的新下载命令。cargo check、fmt通过。
- 真实Vue/Canvas配模拟IPC的更新/恢复回归6组通过：下载只发版本、不保存会话；失败不弹安装框；ready先于命令返回时先禁用操作；过期进度被忽略；重复ready/本机刷新不重弹；Escape及稍后安装解锁，Tab留在确认框；稍后修改主题和留白后，安装传递最新值；备份失败保留编辑与缓存，重试不重下；安装成功展示自动重启状态，没有再要求常规二次重启确认。更新说明HTML仍以纯文字呈现。
- 原导入异步/设置弹窗5项回归重新通过；普通浏览器预览只显示桌面更新不可用，没有检查/下载按钮。并行操作两个测试页面时曾有动画帧等待超时，重新加载测试页后按顺序执行通过，未改变产品代码。
- 原创测试数据与结果在Git忽略的 `artifacts/update-regression/`、`artifacts/import-regression/`；`install-and-restart.png`为模拟0.2.3候选版本的确认框截图，不代表该版本已经发布。
- 核对了当前Tauri/updater/single-instance源码契约：macOS安装完成由app.restart自动重启；Windows显式启用安装后重启，清理后的失败继续走缓存Env的进程级重启。下载成功唤起窗口为尽力操作，不因聚焦失败丢弃已验签包。

以上没有运行真实更新下载、安装或重启；旧0.1.0进程保持原状，没有读取真实屏幕或剪贴板。首次正式更新源尚未公开，0.1.0需先手动升级。两平台完整原生升级、安装器失败后重启、托盘隐藏后的唤起与会话恢复仍待实机验收。
