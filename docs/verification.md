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
- 本机Rust core10/10、desktop16/16通过，新增生产Pending状态转移测试覆盖未下载/空包拒绝安装、失败缓存重试、候选版本固定、重查清空与成功释放缓存，以及原生确认期禁止编辑/退出、稍后解锁、陈旧版本不能解锁、安装中不能撤销确认；生成窗口ACL包含仅主窗口可调用的新下载与确认命令。cargo check、fmt通过。
- 原生确认门控修正前，真实Vue/Canvas配模拟IPC的更新/恢复回归6组通过：下载只发版本、不保存会话；失败不弹安装框；ready先于命令返回时先禁用操作；过期进度被忽略；重复ready/本机刷新不重弹；Escape及稍后安装解锁，Tab留在确认框；稍后修改主题和留白后，安装传递最新值；备份失败保留编辑与缓存，重试不重下；安装成功展示自动重启状态，没有再要求常规二次重启确认。更新说明HTML仍以纯文字呈现。
- 原导入异步/设置弹窗5项回归重新通过；普通浏览器预览只显示桌面更新不可用，没有检查/下载按钮。并行操作两个测试页面时曾有动画帧等待超时，重新加载测试页后按顺序执行通过，未改变产品代码。
- 原创测试数据与结果在Git忽略的 `artifacts/update-regression/`、`artifacts/import-regression/`；`install-and-restart.png`为模拟0.2.3候选版本的确认框截图，不代表该版本已经发布。
- 核对了当前Tauri/updater/single-instance源码契约：macOS安装完成由app.restart自动重启；Windows显式启用安装后重启，清理后的失败继续走缓存Env的进程级重启。下载成功唤起窗口为尽力操作，不因聚焦失败丢弃已验签包。
- 追加审查发现仅前端确认锁不足：全局/托盘入口可绕过前端触发截图，结果会被busy回调丢弃。已用原生confirmationPending修复，ready原子锁定；稍后安装通过固定版本IPC解锁，重新确认检查无活动截图再上锁，安装只能从锁定确认状态开始。初次构建run40321882938全部成功，但未包含此修正，不能作为最终包。
- 确认门控后的前端单元与构建通过，浏览器脚本新增取消失败/延迟响应仍锁、重复点击抑制、活动截图拒绝重开等第7组覆盖。补测时TaskSpace24已消失，未接管用户空间或另建替代；因此最终门控版浏览器回归尚未执行，保留的6组结果与截图只作为修正前证据，不宣称新增联动已在浏览器验证。

以上没有运行真实更新下载、安装或重启；旧0.1.0进程保持原状，没有读取真实屏幕或剪贴板。首次正式更新源尚未公开，0.1.0需先手动升级。两平台完整原生升级、安装器失败后重启、托盘隐藏后的唤起与会话恢复仍待实机验收。

### 最终修正版构建

[CI run40329996324](https://github.com/cpython666/starshot/actions/runs/40329996324) 对应最终应用源码 `c346878580b2691a78d3b39ec01c9f45d3ebebf0`，已包含原生确认门控。macOS Apple Silicon、Windows x64、Ubuntu真实签名校验三个任务全部成功，两平台普通测试安装包均已生成；没有创建发行标签或公开Release。

两份下载ZIP的SHA-256与GitHub API digest一致。最终macOS DMG `Starshot_0.2.2_aarch64.dmg` 为4,787,243字节，SHA-256 `c70ecc7395daa32110eb5106b6fce887a3991d1e0c90b3f6c025ef4f7ba73efe`；只读挂载核对版本/构建号0.2.2、com.starshot.desktop、arm64，codesign严格校验通过，已卸载；ad-hoc签名不代表Apple公证。Windows NSIS `Starshot_0.2.2_x64-setup.exe` 为3,770,713字节，SHA-256 `6c0fcfcce71fc32c8b1b0528b828e1e4846202ed5b6811a4e940dd6357500bef`，只保存与校验，未执行。

最终包与汇总/分平台核验JSON在Git忽略的 `artifacts/ci-0.2.2/run-40329996324/`，旧run40321882938产物保留作过程记录。没有启动或安装最终包，原生实机和最终门控浏览器补测边界保持如上。末次提交仅同步文档与知识库，不重复构建。

## 2026-10-27 · 0.3.0 编辑、敏感信息遮盖与桌面基础体验

本轮按用户要求新增本地敏感信息识别、对象编辑与PNG贴纸、偏好/预设、退出备份、桌面设置、备份管理、分享介绍图及Homebrew分发；明确不做项目文件导入导出。以下记录区分已执行检查与待验收项。

- 本机 Node 44项：43通过、1项真实minisign测试因普通PATH没有工具而跳过；Vue/TypeScript/Vite生产构建通过。Rust core17/17，desktop31通过、1项离线模型手动测试忽略；cargo check及fmt通过。
- macOS Vision默认测试使用仓库原创固定像素图，识别数字/邮箱及坐标通过。Windows使用的官方固定模型已在macOS单独实际运行并识别同一原创图；这项不等同于Windows原生worker验收。模型共12,226,852字节，固定来源、SHA-256和CC-BY-SA-4.0许可见models目录。
- 真实Vue/Canvas浏览器回归：功能8组通过（对象选择移动/颜色/文字/删除撤销、PNG贴纸旋转缩放、偏好恢复、宣传图下载、零面积遮盖、跨图片贴纸预算）。实心遮盖在1×/2×及裁剪后真实PNG解码检查覆盖像素全黑，源像素不变。2×边缘插值问题已修复；预览辅助线不进入导出。介绍图1080×1440、352,389字节，下载与预览编码一致。
- 真实App配模拟IPC：隐私/退出/空窗释放5组通过；更新/恢复7组通过，补齐0.2.2最终原生确认门控的浏览器回归，并验证schema1恢复后新写schema2。原导入异步/弹窗5组也重新通过。模拟IPC不代表系统安装、退出或实际OCR链路通过。
- 构建独立标识com.starshot.verify-030的macOS验收应用，codesign严格检查通过，无安装到Applications或替换旧应用。空白窗口关闭后关联WebContent进程退出，应用重新唤起重新创建编辑器。一次内存样本：就绪67.9MiB、关闭后52.5MiB（含主进程及关联WebKit，GPU/Networking仍保留）；不是长期稳定内存或全场景上限。后续窗口状态被用户操作改变，未完成原生UI的OCR→应用遮盖→备份退出闭环；没有读取或保存截图浮层画面。
- 可复用回归在tests；日志/截图/下载/内存证据仅保存在忽略的artifacts。全部主动测试使用原创例图与固定生成像素，没有读取真实剪贴板。

### 最终CI与发行包

最终应用源码/标签为 `cd7a69107c6473d1262d67d8666268756f754e52`。普通验证[40365269047](https://github.com/cpython666/starshot/actions/runs/40365269047)和签名发行[40365419560](https://github.com/cpython666/starshot/actions/runs/40365419560)全部任务成功；后续文档同步不改变这些安装包的源码。

- Ubuntu签名任务Node44/44通过，含真实minisign及损坏文件/篡改版本拒绝测试；macOS Node43通过1跳过、Rust core17通过、desktop31通过1忽略；Windows Node43通过1跳过、Rust core15/desktop29通过，包含实际离线模型识别。前端生产构建、Rust格式检查及双平台安装包构建通过。
- Windows最终发布EXE及安装后的EXE分别运行离线OCR worker，原创固定像素上的数字/邮箱与坐标通过；测试不启动GUI、不截图、不读剪贴板。NSIS实际静默安装/卸载均返回0；校验x64、版本0.3.0、发布者starshot、当前用户范围、卸载登记Starshot与许可资源，卸载登记/主程序已移除。CI已有WebView2；不覆盖缺运行时或旧版本升级场景。
- Windows安装证据 `artifacts/ci-0.3.0/release-windows-installer/2d026cacb0654d9ba87d7dedede6af15/result.json`：success=true、errors为空，安装/元数据/安装后OCR/卸载各阶段通过；其中安装包SHA-256等于下表。
- GitHub artifact ZIP的下载digest与API一致；发行附件的7项SHA256SUMS逐项通过，latest.json两平台URL和签名文本匹配。Mac DMG只读挂载核对版本/构建号0.3.0、com.starshot.desktop、arm64和许可资源；codesign严格检查通过并已卸载。公开DMG匿名下载字节与最终包一致。

| 最终安装包 | 字节数 | SHA-256 |
| --- | ---: | --- |
| Starshot_0.3.0_aarch64.dmg | 4,976,926 | `546824f9ebf3bcda889fc7ac841b23d6ea530aa473bb01bf9b25863b1e11aa4d` |
| Starshot_0.3.0_x64-setup.exe | 16,335,455 | `e437c0ac62f125c0947d9b7192a1e9d6c2500bb9d37437a526b8e399df39b0a1` |

模型不再作为Git二进制提交；构建前 `npm run models:prepare`下载官方固定文件，按大小和SHA-256校验，验证通过才替换目标；同哈希缓存跳过。新空目录下载与两平台CI均通过。Windows仍将12,226,852字节模型编译嵌入，不在使用OCR时联网。证据和许可来源见 `scripts/prepare-ocr-models.mjs`、`src-tauri/models/ocrs/NOTICE.md` 与忽略的 `artifacts/model-download-verification.log`。

### 公开预览与Homebrew

[v0.3.0](https://github.com/cpython666/starshot/releases/tag/v0.3.0)已于2026-10-26 17:23:11 UTC（北京时间10月27日）公开，isDraft=false、isPrerelease=true，未设为最新稳定版。独立Tauri更新签名已经验证；macOS仍是ad-hoc且未公证，Windows没有平台发行签名。

Homebrew Tap的公开提交为[0b2d188](https://github.com/cpython666/homebrew-tap/commit/0b2d18875071fa37f3df82bff423cca3acdafdbc)，最终Cask采用上表DMG哈希。Ruby语法、brew style、公开下载和brew fetch通过。online audit唯一失败为 `v0.3.0 is a GitHub pre-release.`，如实保留预览标记，不宣称audit全部通过。

Homebrew真实安装到仓库忽略的隔离目录；brew info的安装版本、应用Info.plist和codesign严格校验通过，卸载后应用及Cask安装登记均不存在。未启动该发行应用或替换已有应用。首轮安装验证器误用只适用于formulae的 `brew list --versions`，后来使用 `brew info --cask --json=v2`完整重跑并记录成功；使用 `HOMEBREW_NO_INSTALL_CLEANUP=1`避免后续安装清理缓存。结果在 `artifacts/release-0.3.0/brew-install-verification.json`。安装说明与边界见[Homebrew指南](homebrew.md)。

仍未完整验收：旧版真实更新安装及失败恢复、原生GUI的OCR→应用遮盖→备份退出全链路、混合DPI/多显示器、系统权限、全局热键、开机启动和平台信任提示。浏览器、单元、真实worker与隔离安装各自覆盖不同层级，不能合并表述为完整原生使用已通过；本轮交付为开发预览。

## 2026-10-27 · 轻量截图改进（发布前验证，后续收入0.3.1）

根据用户提供的竞品视频核对交互，改为选区底部工具条，按需进入更多编辑；明确星镜不做星伴。参考视频只用于观察流程，没有作为测试像素或加入Git。以下检查针对本次本地源码，不能用上文0.3.0发行CI代替。

- Node 52项：51通过、1项真实minisign测试因普通PATH缺工具而跳过；Vue/TypeScript/Vite生产构建通过。Rust core18通过，desktop33通过、1项离线模型手动测试忽略，cargo check和fmt检查通过。新增原生测试覆盖操作枚举、标注PNG实际格式/精确裁切尺寸，窗口ACL检查使用新的捕获标签格式。
- 浏览器截图回归7组通过：真实Worker从原创示例识别560×320圆角卡片；Tab正反切换候选并保留点击选择；就地文字/箭头/实心遮盖、撤销/重做、零面积草稿和真实PNG下载；只在更多编辑后进入主页面；模拟IPC保存取消/复制失败保留选区和标注；忙碌状态阻止重复操作；Enter复制及未标注操作不回传图片。200次同帧pointermove只产生1次小画布取色。模拟IPC没有调用系统截图或剪贴板。
- 原有功能浏览器回归8组重新通过，包含对象/贴纸/预算、偏好、宣传图与真实导出像素。新增预览回归检查100×70预览的150个遮盖像素均为不透明黑色，1×/2×导出仍为200×140/400×280，并保持遮盖像素；预览不替代导出精度。
- Chromium固定4K原创图、20个箭头、1280×800显示区域，预热后交替测量7次：单次完整渲染中位数10.1ms，按显示尺寸渲染2.8ms；该次临时canvas分配像素总数由26,652,672降至2,748,869（约减少89.7%）。这不是整机内存、原生帧率或截图启动耗时。
- 矩形算法使用连续方向边缘与四边支撑，并移到Worker。固定1920×1080细框图旧算法候选0、新算法201，新算法中位21.53ms；密集棋盘噪声中位50.23ms且候选0。新算法改善薄边识别，计算本身并未更快；不会在每次鼠标移动重算。系统窗口保持前后顺序，遮挡窗口命中和跨窗口边缘过滤已有单元覆盖。
- 可复用脚本为tests/browser-capture-regression.mjs与tests/browser-preview-regression.mjs。日志、原创截图、下载与性能记录仅在忽略的artifacts/capture-toolbar、preview-performance、region-detection和capture-final-*.log；没有真实屏幕或剪贴内容进入Git。
- 最终macOS arm64 app/DMG构建成功，独立名称Starshot Capture Preview、标识com.starshot.capture-preview，沿用源码版本0.3.0，仅供本地验收，不是新的发行版本。DMG 4,977,110字节，SHA-256 `a67374b4378f7d1591c923726186ff2a29418b90eb9242bb6824b81fbdabbaab`；codesign严格校验通过，仍为ad-hoc、未公证。产物位于src-tauri/target/release/bundle，未安装或替换现有应用。
- 已运行这个独立应用，在tauri://localhost的打包WebView打开「体验区域选择（原创示例）」：真实Worker识别560×320卡片，点击出现就地工具条，Esc回到空白编辑器，随后退出测试应用。原创例图的原生工具条截图在artifacts/capture-toolbar/macos-original-demo.png。这验证了打包后的Worker和工具条，不涉及系统捕获、剪贴板或真实图片。

仍待验收：本轮Windows平台构建与原生运行；真实截图的启动耗时、系统复制/保存/贴图完整链路、多屏和混合DPI。表格相连格线、严重缺边和极低对比可能漏检，窗口边界/像素启发式不等同于系统控件语义识别。本轮没有提交、推送、发布或更新Homebrew，公开0.3.0包不包含这些修改。

## 2026-10-28 · 更新源排查与0.3.1发布前准备

用户反馈检查更新后仍是旧交互。只读GitHub API核对公开列表只有v0.3.0，draft=false、prerelease=true；固定tag的latest.json返回HTTP200且version=0.3.0。客户端内置的`releases/latest/download/latest.json`和GitHub latest API均返回HTTP404，尚无稳定更新入口。新交互仍在本地工作区，未进入公开0.3.0包，不能由旧包的检查更新获得。

本机运行的应用经进程路径与Info.plist核对为Starshot Verify 0.3.0（com.starshot.verify-030）。只读取原生设置页，实际状态为「暂时无法检查更新……Could not fetch a valid release JSON from the remote」。已核对所用Tauri updater源码：HTTP404最终返回ReleaseNotFound，当前代码进入error，不是up-to-date；因此没有为假想的错误状态转换增加补丁。查看后返回原编辑页，未截图、读取剪贴板、替换应用或关闭现有会话。

使用scripts/version.mjs将六处版本同步为0.3.1，轻量截图变更整理到对应CHANGELOG条目，仍标待发布。本机重新运行Node52项（51通过、1项minisign跳过）、前端生产构建、Rust core18项、desktop33项（另1项手动测试忽略）、cargo check与fmt检查均通过。发布说明提取为3条实际改动，没有将待发布状态混入发行说明。检查日志为忽略的artifacts/release-031-*.log。

0.3.1本地macOS app/DMG构建成功，使用官方应用标识com.starshot.desktop；核对版本0.3.1、arm64和codesign严格检查通过，仍为ad-hoc、未公证。DMG位于src-tauri/target/release/bundle/dmg/Starshot_0.3.1_aarch64.dmg，4,981,929字节，SHA-256 `edf8e41c8ceabd26b3407153527c941548e35e37d7b4e229ea772b8c45130d58`。这是本地普通安装包，没有安装/启动或用于更新现有验收应用；最终远端签名发行包需另行核验。

截至该准备阶段，尚未提交、推送、创建标签或公开发行。0.3.1双平台CI、签名发行包和公开更新源验证随后在下节授权发布流程中完成；前一轮0.3.0的CI不能作为本轮构建证据。

## 2026-10-28 · 0.3.1发行验证

用户在完成更新源排查后明确授权提交、推送、发布和同步Homebrew。候选1bef233的CI 40680770025在两平台及签名任务的模型准备阶段均因上游HTTP 403失败，未进入安装升级验证。将此前已核验的原模型与来源、许可、SHA256SUMS发布到独立的 `ocr-models-v1` 构建依赖预览Release，保持两个模型共12,226,852字节及原SHA不变；修改下载地址后的候选为fe609e8。本机空目录匿名下载与大小/SHA验证通过，Node52项为51通过、1项本机minisign缺失而跳过。镜像不属于应用更新，不进入稳定latest，图片仍不上传，模型仍只在构建时下载。

最终源码与`v0.3.1`标签为 `fe609e8171b759cb027ab6ff434e7c3d95f50071`。普通验证[40681522684](https://github.com/cpython666/starshot/actions/runs/40681522684)与签名发行[40683246836](https://github.com/cpython666/starshot/actions/runs/40683246836)全部成功，覆盖版本同步、前端生产构建、Rust核心/桌面测试、格式检查及双平台安装包。发行validate的Node52项全部通过，包括真实minisign、篡改包及版本绑定拒绝测试。

| 最终公开文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| Starshot_0.3.1_aarch64.dmg | 5,019,285 | `bbd926e229248afa8533dafa31755060ed216029d45b1b2d4a7a7810b12a86ee` |
| Starshot_0.3.1_aarch64.app.tar.gz | 5,133,238 | `b054668ce7a73d035868bb3d8428c6645a1a2121f05e4096b5470fc6a9b98271` |
| Starshot_0.3.1_x64-setup.exe | 16,342,664 | `5c4ffebb23cc4ee3fbd64d7513ab20379ae1b53b62b249a5b2e3faf65eacf2fa` |

- 完整CI artifact为26,466,890字节，GitHub提供的SHA-256 `bcbb1d1060b3d89e99c3b09993695f63bf295f119df2d38b8602440f0609ed4b`与下载文件一致；ZIP完整性、8个文件集合、SHA256SUMS、清单版本/平台/URL/签名及GitHub草稿资产digest全部匹配。下载中断后只续传缺失字节，再校验完整哈希。证据`artifacts/release-0.3.1/release-verification.json`。
- macOS最终DMG只读挂载，版本/构建号0.3.1、正式标识com.starshot.desktop、arm64、模型许可与严格codesign检查通过。`macos-updater-smoke`实际运行1项通过：用公开0.3.0临时副本，损坏签名包被Tauri拒绝且`diff -qr`确认旧包不变；真实已签名0.3.1更新包安装成功、版本及codesign通过。使用mock app runtime，但执行的验签和应用包替换是真实插件操作；没有启动、重启或替换用户应用。日志`artifacts/release-031-macos-upgrade.log`。
- Windows最终安装器的实际SHA与隔离升级报告一致。公开0.3.0安装、旧版OCR、`/UPDATE`升级至0.3.1、新版元数据及OCR、卸载均通过；6项阶段passed，退出码均0，`errors=[]`，无意外GUI进程。证据为[最终安装检查附件](https://github.com/cpython666/starshot/actions/runs/40683246836/artifacts/12813848071)，本地`artifacts/windows-release-40683246836/`。这验证安装载荷升级，不等同于GUI下载/重启验收。
- [v0.3.1](https://github.com/cpython666/starshot/releases/tag/v0.3.1)于2026-10-28 12:15:48 UTC公开，draft=false、prerelease=false并设为latest。匿名获取内置稳定更新地址成功，latest.json与CI逐字节一致；GitHub latest API返回v0.3.1。正在运行的Starshot Verify 0.3.0原生设置页实际显示「发现新版本0.3.1」及更新按钮。本次保留其当前编辑，未替换或重启；独立验收标识与正式包的数据目录不同，不能假定恢复点自动迁移。
- Homebrew最终Tap提交为[98abcab](https://github.com/cpython666/homebrew-tap/commit/98abcab34c228aa64d6e4faf68165b37a88c6bcb)。首次audit因默认livecheck将模型依赖标签读成版本1而失败；生成器与Cask改用github_latest后，Ruby/style/livecheck/在线严格audit/fetch均通过。独立目录实际安装版本与签名核对、卸载和清理登记均通过，未启动应用或移除quarantine，公开DMG缓存与上表一致。详情见[Homebrew指南](homebrew.md)。生成器调整后的本机Node、前端生产构建、Rust core和fmt再次通过；该调整不改变已发布应用二进制。

仍未全部覆盖：应用GUI完整下载、安装后自动重启及会话恢复，Windows安装器启动失败后的旧版重启，多显示器/混合DPI、真实截图启动与各原生交互矩阵。macOS仍为ad-hoc且未Apple公证，Windows尚无平台发行者证书；更新签名及稳定latest发布不改变这些边界。

## 2026-10-28 · 旧验收包安装前会话校验失败

用户提供的安装确认截图显示0.3.1已下载验签，但报「恢复点内容无效：贴纸和遮挡区域尺寸必须大于零」。源码调用链为UpdateSettings.install先getSession，再调用原生monitor.install；getSession序列化当前及最近最多6张图片，session.ts对sticker/redact宽高≤0拒绝。因此此次阻塞在生成会话备份之前，原生安装未开始；不能据此前安装载荷测试通过推断用户旧会话已能升级。

正式v0.3.0及v0.3.1均已有遮盖宽高同时大于2才提交、零面积草稿不渲染及反向拖动归一化。该保护所在提交为cd7a691（10月27日01:04），当时Starshot Verify二进制mtime为10月26日23:52，早于修复；这支持旧验收构建残留异常标注的判断，但未读取真实图片或用户对象尺寸，无法确认哪张图片、哪个对象及零或负值。初步排查阶段没有改代码或操作用户会话；后续处理如下。

需要保留进度时应先稍后安装，确认异常对象后单独撤销/删除，并先验证备份成功；未经授权不能清空会话。旧前端在调用安装前已经失败，单纯发布新包不能热修复该进程。

## 2026-10-28 · 空遮盖兼容修复与旧会话解阻（未发布源码补丁）

- 用户明确回答「已经保存，当前会话可以清空」后，通过原生UI退出旧Verify并重开，确认空白工作台且错误消失。重新检查、下载和验签公开0.3.1，点击安装并重启后旧进程结束，新进程从原Verify.app路径启动；该包Info.plist已为0.3.1、com.starshot.desktop。操作工具当时仍把该路径绑定旧verify标识，对正式标识访问超时；进程采样主线程在正常AppKit事件等待，无死锁证据。用户随后明确确认「能看到，界面正常」，因此窗口正常已有用户实测反馈；这次只保存空会话，不能证明真实图片跨标识恢复成功。
- 工作区`editor.ts`仅在原1000对象上限检查后，统一过滤有限坐标、非负尺寸且宽或高恰为0的旧`redact`，覆盖所有遮盖绘制阶段，避免输出floor/ceil把空框变成1px黑线。正面积小遮盖保留；负尺寸、非有限值、损坏贴纸继续拒绝。
- 内部`serializeSession`对所有最近文档的标注副本做同一过滤，不修改运行状态及undo/redo；原2000对象/100000路径点限制仍先检查。外部`parseSession`与Rust恢复校验保持严格，不接收外部零尺寸对象。
- `App.vue`独立记录`renderError`，重绘成功或图像为空时清除；保存/复制等操作失败仍保留，避免旧预览异常一直盖住后续状态。
- Node54项为53通过、1项真实minisign因本机PATH缺工具跳过；Rust core18通过、desktop33通过（1项Windows引擎手动测试忽略），版本检查、前端生产构建、cargo check/fmt、diff检查通过。没有复用此前CI作为新补丁的双平台证据。
- Chromium功能回归10组通过，包含新增8种裁剪/1×/2×/预览像素完全一致、旧空遮盖备份与undo/redo不改变有效对象、预览错误恢复且保留保存失败、空会话清除旧错误。更新/恢复模拟IPC回归7组也重新通过，覆盖备份失败保护、确认门控及最新会话采集；不替代原生安装验收。仅使用原创示例和生成像素；记录在忽略的artifacts/feature-qa/results.json、update-regression/results.json及empty-mask-*-browser.log。
- 本地macOS app/DMG构建及严格codesign校验通过，仍用源码版本0.3.1；这个未发布构建不等于公开0.3.1。DMG为4,982,064字节，SHA-256 `e50857a3b4d620056beb010cf2c1099f24ec38293b0175242a22596835751e03`，日志artifacts/empty-mask-macos-build.log。未将本地补丁安装到用户应用，未提交、推送、创建新版本或覆盖公开发行资产；Windows新补丁构建未执行。

## 2026-10-28 · 权限提示名称核对（未发布文案修复）

- 用户反馈应用提示允许「星镜」，但系统搜索显示Starshot/Starshot Verify。正式包Info.plist的CFBundleName/CFBundleDisplayName均为Starshot；本次实际运行路径为target/release/bundle/macos/Starshot.app。原生系统设置「录屏与系统录音」列表实际显示Starshot.app且开关为on；应用内「重新检查截图权限」与重开后的检查仍报告不可用，不能将系统列表开关等同于当前程序已取得权限。具体授权不一致原因未确认，没有读取TCC数据库、重置权限或为测试截取桌面。
- 工作区desktop.rs以同一CAPTURE_PERMISSION_HELP统一首次预检与截图失败的提示，DesktopSettings.vue备用文案同步，明确查找Starshot（星镜）。本次只修文案，不自动授予权限、不改变应用标识或更新公开发行包。用户开始操作系统设置后停止窗口操作，未替用户完成新授权；截图权限是否生效仍待实际预检或用户反馈。
- 本机Node54项为53通过/1项minisign跳过，前端生产构建、Rust core18项、desktop33项（1项手动测试忽略）、cargo check/fmt和diff检查通过。macOS以独立名称Starshot Permission Preview与com.starshot.permission-preview构建app/DMG并通过严格codesign，避免打包覆盖当前正在使用的Starshot.app；未安装或运行该验收包。核验后仅移除本轮生成的临时app，避免增加同名搜索项，保留DMG（4,982,065字节、SHA-256 `35a90b6f8e32e4c422aef02a94851afc5444865ea06ef38b11e526a9bad7d592`）。日志在忽略的artifacts/permission-name-*.log。未执行Windows新构建，未新增模拟文本实现的测试。

## 2026-10-29 · 清理重复应用并统一本机入口

用户看到多个Starshot/Verify/Capture Preview后要求「旧的都删了」。清理前确认没有starshot运行进程，/Applications/Starshot.app尚不存在；索引共发现5个星镜app。最新工作区重新执行前端生产构建与macOS app打包，严格codesign校验通过，二进制包含新Starshot（星镜）权限提示；连同前轮空遮盖修复一起移到`/Applications/Starshot.app`。这是本地修复构建，版本仍为0.3.1，不能视作公开v0.3.1资产或新发行。主二进制SHA-256为`fd549585c1edfe7d9dc5da5e00955f525b8c93a932da9124c781837ac287186a`，移动后签名再次通过。

以下4个旧副本通过Foundation FileManager.trashItem移到系统废纸篓，可恢复：target下Starshot Verify.app（外层旧验收名、内部为公开0.3.1）、Starshot Capture Preview.app（0.3.0）、artifacts内macos-updater-smoke/Starshot.app（0.3.0测试基线），以及旧Verify数据目录application-backups下0.3.0应用备份。只移除app包，恢复会话与图片目录未读取或修改；Starclip/星笺不在本次清理范围。

旧路径逐一注销LaunchServices注册，固定入口重新注册和导入索引后，`mdfind`按Starshot.app名称查询仅返回`/Applications/Starshot.app`。原生UI实际打开正常，运行进程也来自该固定路径，设置中的版本为0.3.1；「重新检查截图权限」仍返回新的权限说明，故权限未通过，不曾为验证捕获真实桌面或代用户改变授权。返回工作台并保留运行。清理路径映射和构建日志仅位于忽略的artifacts/canonical-starshot-cleanup.json及canonical-starshot-build.log。没有提交、推送、发布或修改Homebrew。

## 2026-10-29 · 录屏授权失配定位与修复

用户再次反馈系统已开启权限但仍无法截图。针对星镜的系统tccd诊断确认：现有ScreenCapture授权保存的代码要求与`/Applications/Starshot.app`当前代码签名不匹配；`codesign`核对当前包为ad-hoc，指定要求绑定代码哈希。原生系统设置同时显示Starshot.app开关已开启。源码`desktop.rs::capture_permission`每次实时调用CoreGraphics，无前端或Rust权限缓存，拒绝发生在xcap读取屏幕前；此前「原因未确认」在本次排查中得到解决。原始系统日志未加入Git。

用户明确回答「允许，修复星镜授权」后，确认应用为空会话并退出，仅执行`tccutil reset ScreenCapture com.starshot.desktop`，命令成功；系统设置中的旧Starshot条目消失。随后通过系统原生添加按钮选择应用程序目录的Starshot.app，条目恢复且开关开启，重新启动固定入口。应用内「重新检查截图权限」明确显示「截图权限已可用」，底部旧权限错误消失，已返回工作台。没有重置其他应用授权、修改TCC数据库或为验证截取真实桌面；本次确认权限预检通过，不宣称完成截图/复制/保存全流程。未更改代码、重新签名或发布；未来ad-hoc重建仍需留意授权身份变化，稳定平台签名属于后续发行事项。

## 2026-10-29 · 长截图与贴图交互（未发布工作区）

- 源码新增手动逐屏纵向长截图：选区内入口→小控制窗→用户滚动后主动采集→重叠去重与缩略预览→复制/保存/贴图/更多编辑。简单固定头尾、重复帧、歧义/无重叠拒绝、显示器配置变化、取消和失败保留均有明确路径；像素匹配是启发式，不宣称任意动态页面或复杂悬浮层正确。
- 长图上限100屏、单边16384/40MP，首帧及追加提交前按真实PNG编码检查64MiB；超限保持图像、上一屏与屏数，避免已有长图无法完成。新增7个原创固定像素Rust测试，含精确拼接、固定头尾、稀疏文档、动态重叠拒绝、重复/反向/无重叠/尺寸变化/上限与编码预算回滚重试。
- 贴图改为双击关闭并增加右键/Shift+F10菜单；macOS按下立即起拖，使用同屏幕坐标连续按下兜底双击；菜单不误触拖动/关闭，支持键盘、边界夹取、小窗滚动和关闭失败重试。截图辅助线为独立DOM横竖线，H显隐、随现有rAF更新，选区确定后隐藏，不参与成图。
- Node54项：53通过、1项本机minisign缺失跳过；前端生产构建通过；Rust no-default25通过、desktop lib40通过/1项Windows专用测试忽略；cargo check、fmt与版本一致性检查通过。源码版本仍0.3.1，与公开发行包区分。
- Ego Chromium串行回归最终通过：截图9组（含原有就地标注/撤销/裁切像素、辅助线、模拟IPC长截图开始失败/重复/重叠失败/保存取消/完成路由/380×300布局）；贴图5组（菜单/键盘/180×130布局/双击兜底/失败重试）。预览截图已人工查看。证据在Git忽略的`artifacts/capture-toolbar/`、`artifacts/pin-interactions/`；使用原创demo或生成像素、模拟原生命令，没有读取真实屏幕或剪贴板。浏览器前台焦点需串行使用，后台rAF暂停不作为产品性能结论。
- 竞品研究基于QQ/Snipaste官方资料及明确降级的微信媒体转述，未运行或逆向竞品；REA skill已按用户授权安装，未安装分析引擎/MCP。来源与范围见[截图交互研究](screenshot-research.md)。
- 本地macOS app/DMG构建与严格codesign校验通过，仍为ad-hoc、未公证。DMG为5,040,861字节，SHA-256 `aa73c2ffa0df356d6985079692fab1e458c651449bb73d9892828f23dd42d78b`；以明确未发布名称保存在`artifacts/scroll-interactions-20261029/`，同目录含app归档及`build-verification.json`。归档中的可执行文件哈希与构建app一致后，仅移除本轮生成的可运行app副本，避免重复搜索入口；日用应用未修改。构建日志`artifacts/scroll-interactions-macos-build.log`。
- 本轮未执行Windows新构建，也未实测真实滚动页面、原生双击/拖动、控制窗隐藏后采集、多显示器/混合DPI。未提交、推送、发布或替换`/Applications/Starshot.app`，不能用旧0.3.1发行CI代替本轮证据。

## 2026-10-29 · 悬浮预选修复与Snipaste REA调查（未发布工作区）

- 用户反馈悬浮快捷选区框消失。代码检查发现xcap 0.9.8在macOS把可选窗口名称/共享状态当作窗口入选条件，以及全屏候选向外outline被stage裁切的路径；没有捕获用户真实桌面，因此不能把这些可复现代码缺陷说成已唯一确认其实际场景根因。macOS改为一次CoreGraphics几何快照，保留有效无标题窗口；前端边框向内、启动按实际指针预选，初次检测不依赖非活动屏的双rAF。
- 新macOS AX点查询仅在已有辅助功能授权时启用，仅读位置/尺寸；捕获会话和宿主PID受后端约束，每个AX对象设置剩余80ms预算，未授权/超时/错误保留窗口/像素候选。前端最多一个在途请求，过期回复不覆盖新位置、Tab手选或确定后的选区。该路径是真实原生源码，但本轮没有执行跨应用AX或修改系统权限。
- Node55项：54通过、1项本机minisign缺失跳过；Vue/TypeScript/Vite生产构建通过。Rust no-default30通过、desktop lib45通过/1项Windows专用测试忽略；cargo check、fmt及git diff --check通过。新增纯测试覆盖无标题几何快照条件、叠放过滤、初始指针缩放/负坐标、控件包含点/宿主夹取及权限门控，ACL检查覆盖新捕获命令。
- Ego Chromium悬浮专项5组通过：初始指针/全屏内侧框；Worker构造及转移buffer后错误回退；迟到Worker和Shift+Tab松键；模拟控件优先/单请求/陈旧回复；Shift手动与静止点击。已有截图回归9组亦通过，覆盖就地标注、导出像素、辅助线、长截图模拟IPC与失败保留。新测试为`tests/browser-hover-regression.mjs`，结果`artifacts/hover-detection/browser-results.json`，全屏候选原创预览已查看。
- 新`tests/macos-capture-webview-smoke.swift`在真实WKWebView用当前dist、现有CSP和`tauri://localhost`加载生产module Worker，6/6通过、stderr为空：实际指针、Worker构造失败/运行失败回退、模拟AX控件优先、迟到AX与Worker不覆盖Tab。另有探针确认同源module Worker可用，故没有加入blob Worker或放宽CSP。测试只有原创固定像素与模拟原生IPC，不等于真实屏幕捕获或控件命中率验收；日志`artifacts/hover-detection/macos-capture-webview-smoke.jsonl`。
- 按用户要求实际使用REA CLI 6.7.0的Mach-O/plist检查，并以系统otool只读反汇编补充核对本机Snipaste 2.9.2 arm64窗口→AX元素→位置/尺寸→宿主求交调用链。深度反编译返回provider_unavailable，没有安装分析引擎、改MCP配置或运行竞品；不声称完整还原其算法。版本、文件哈希、Evidence和地址关系见[调查报告](snipaste-detection-research.md)，原始证据仅在忽略的artifacts中。
- 最终macOS arm64 app/DMG构建、codesign严格校验与DMG校验和验证通过，版本仍0.3.1、ad-hoc且未公证。DMG 5,049,770字节，SHA-256 `10562e97c5bbfb0d962a71cec39ed33d25404896bab13d99b5e0b842f92ffba1`；app可执行文件SHA-256 `75ce3c32a7c649c4f9fd44a231f9c34a8551b29e3d3486c38d748ec8476df73d`。明确未发布名称的DMG/app归档、构建日志与`build-verification.json`保存于`artifacts/hover-detection/`；核对归档可执行文件哈希后移除本轮生成的可运行app副本，避免新增搜索入口。
- 日用`/Applications/Starshot.app`可执行文件哈希仍为`fd549585c1edfe7d9dc5da5e00955f525b8c93a932da9124c781837ac287186a`，未替换、启动测试副本或重置权限。未提交、推送或发布；Windows本轮构建/UI Automation、真实跨应用AX、多显示器/混合DPI和真实捕获全流程仍待验收，旧版CI不能代替。

## 2026-10-29 · 用户授权安装最新本地构建

- 用户要求「替换下，我要体验最新版的」后，将上一节已核验的app归档安装到`/Applications/Starshot.app`。解包前核对归档SHA-256，替换前保留旧app归档并核对其中可执行文件；安装后再次校验可执行文件SHA-256为`75ce3c32a7c649c4f9fd44a231f9c34a8551b29e3d3486c38d748ec8476df73d`、严格codesign通过，并核实新进程来自固定路径。版本仍0.3.1，本地补丁未公开发行。
- 新应用已原生打开，设置页可见新增的控件预选权限提示。初次录屏预检拒绝、辅助功能未授权；用户随后明确同意「允许修复录屏权限并开启辅助功能」，并自行完成系统解锁。仅重置`ScreenCapture com.starshot.desktop`，在系统界面重新添加固定app；辅助功能仅重新登记该app。新进程内「重新检查截图权限」返回「截图权限已可用」，同页显示「精准控件预选已可用」，随后返回工作台留待用户体验。没有捕获真实屏幕、读取剪贴板或执行跨应用控件命中测试。
- 旧应用备份与安装记录在忽略的`artifacts/hover-detection/installation-20261029-124911/`，临时可运行旧副本已在核对归档后移除。系统设置一度缓存旧临时文件名previous.app；权限有效性最终以固定路径新进程的原生API预检为准，不把界面开关单独作为证据。
- 本次未先手动备份图片会话：退出前主窗显示一张图片，Cmd+Q直接退出且未显示备份提示；新版为空会话、恢复点列表为空。保留的是旧程序归档，不包含那张内存图片。已向用户说明，后续替换须先手动备份并核实成功。只读核对当前锁定依赖发现默认macOS Quit由muda的terminate:经Tao WillTerminate/LoopDestroyed到RunEvent::Exit，绕过项目仅监听ExitRequested的备份门控；未运行跟踪旧二进制，不能将匹配的源码调用链说成已唯一证明本次事故根因。此问题列为高优先级待修，应用菜单应复用begin_exit并另验Dock Quit。没有宣称完整编辑会话迁移或恢复通过。
- 本轮没有修改产品代码、重建、提交、推送或发布，复用上一节已验证构建。Windows UIA仅核实官方实现路线，尚未写入Windows分支；其平台构建和原生验收仍待后续工作。


## 2026-10-29 · 对照Starclip补齐操作细节（未安装工作区）

- 只读核对本地Starclip源码后，补齐星镜实际快捷键提示、可撤销的「恢复默认美化」、贴图复制/另存为/送到编辑器。Starclip的图片OCR仍未实现，独立全文提取、快捷键录制、HEX输入、最近6张键盘选择和恢复点定位明确列为后续项，未改动Starclip及其用户数据。具体来源见产品说明的Starclip对照表。
- 默认美化重置保留原图、裁剪和标注，清除背景图片和自定义装饰；一次撤销可完整恢复。生成双色PNG及背景的浏览器回归验证重置/撤销/重做的完整输出像素SHA-256一致，迟到背景导入不覆盖重置；初始、保存成功、保存失败回读后的顶部和帮助提示均跟随实际快捷键。这6组测试通过，证据为忽略目录`artifacts/starclip-details-20261029/browser-results.json`；模拟IPC不等于系统热键注册实测。
- 贴图8组Chromium回归通过：沿用原菜单/小窗/双击/失败重试，并增加复制方向参数、预览缩放/透明度不改变输出请求、保存取消/失败/真实路径、在途请求防重入和送编辑器保留贴图。Rust原创2×3不同RGBA像素测试覆盖所有90°旋转/水平翻转组合、尺寸及alpha、非法角度；实际变换在Rust执行。结果在`artifacts/pin-interactions/results.json`，预览图片已查看；未读真实剪贴板、未实测原生复制或保存对话框。
- 进一步审查发现原先仅发capture-complete事件会在编辑器同时进入备份时被忽略，却返回发送成功。工作区现改为单待处理贴图请求、一次领取、实际导入后回执；窗口创建失败、忙碌、解码失败、超时或被新导入取代均拒绝且保留贴图，等待不占用operation锁且不写普通截图current。新增5组浏览器回归通过（细节脚本共11组），Rust验证重复领取、旧ID、失败清理及迟到回执；更新/恢复7组模拟IPC回归亦通过。
- 最终Node55项为54通过、1项真实minisign因本机PATH缺工具跳过；Rust no-default31通过、desktop lib47通过/1项Windows手动测试忽略；Vue/TypeScript生产构建、cargo check/fmt、版本一致性及diff检查通过。这里只记录本轮重新执行结果。
- macOS应用菜单使用普通MenuItem接管⌘Q并复用begin_exit，保留File/Edit/View/Window/Help菜单。独立标识`com.starshot.details-verify-20261029`的原生应用载入内置原创1200×750示例后，⌘Q实际显示「退出前保留编辑进度？」；取消回到完整示例，再次⌘Q选择备份并退出，进程结束，独立数据目录产生schema2恢复点且session SHA-256与清单一致。证据`artifacts/starclip-details-20261029/native-quit-results.json`记录对应构建哈希；没有运行真实屏幕捕获。Dock菜单退出尚未接管，不把该路径算作通过。
- 最终macOS arm64 app/DMG构建、严格codesign和DMG校验通过，仍为ad-hoc、未公证。独立验证包DMG为5,081,290字节，SHA-256 `010fbda722ad7f1b835434eb34283946dae38939cc1c3cad7e122ababc185def`；可执行文件SHA-256 `0aa8821536c4bcb5241b2da89436ce4bb6267b35c1f0ed4c276e70db43cc00b4`。产物、app归档及build-verification.json保存在忽略目录`artifacts/starclip-details-20261029/`，核对归档后仅移除本轮生成的可运行验证app，避免增加重复搜索入口。
- 最终原生验证另从内置示例生成1344×894美化贴图，按R旋转后用⌘E发送，主编辑器实际显示894×1344新图。再次验证⌘Q提示、取消保留、备份并退出，恢复点包含原始1200×750示例和894×1344贴图，清单SHA-256一致；证据native-final-results.json。这里只验证作为新图片导入，不还原贴图生成前的标注对象；没有实测原生复制、另存为或真实屏幕截图。
- 日用`/Applications/Starshot.app`保持运行，本轮未替换、重启或修改其权限；本轮功能尚未安装。未提交、推送、发布或变更Homebrew，源码版本仍0.3.1。本轮Windows构建与原生GUI未执行，不复用旧发行CI作为新代码证据。


## 2026-10-29 · Windows同伴调查包复核与集成交接

- 用户交付`starshot-windows-investigation.zip`，SHA-256 `53f51c44b2e8063765b32d465bce230e8c91a22b3b6dd9e694c1657ab1ba778b`，25个清单文件的哈希全部核对一致；原始包内容留在忽略目录`artifacts/windows-investigation-20261029/`。没有在macOS执行Windows脚本或重跑其窗口测试，没有把原始日志、个人路径、截图或竞品二进制加入Git。
- 两项独立离线复核：1008条探针记录与42个helper组合完整，四边误差、216次可访问控件零误差、B/C覆盖层命中及耗时统计均与原始数据一致；8段静态摘录长度/节偏移与10个RIP相对目标一致，MSAA接口槽位与官方头文件吻合。完整EXE不在包内，因此无法在本机重新验证整个Snipaste原文件哈希及摘录归属；这是同伴静态证据的一致性核对，不能称为Snipaste运行实测。
- 216次结果来自两屏共12个固定控件位置的场景重复；全局UIA在不透明覆盖层上命中自身，保存目标HWND的局部查询可取得底层控件。普通Win32不能给无子HWND的WPF按钮返回独立矩形；无provider的自绘矩形仍只返回面板。150ms预算是合作式检查，最长158.17ms；未做阻塞provider故障注入。真实Tauri/WebView2、带文字/重叠候选、滚动和重建失效、硬超时恢复仍待测试。详细证据见[Snipaste调查](snipaste-detection-research.md#windows同伴交付与离线复核2026-10-29)。
- 本轮只更新调查结论与接入边界；星镜Windows控件识别仍返回不可用，未把探针冒充产品实现。已核实接入缺口为原生HWND保留及operation锁外的受限查询；准备包含当前未提交代码的独立源码快照与Windows实现/验收任务，用于用户手动转交，未提交、推送、安装或发布。


## 2026-10-29 · 平台依赖与资源裁剪（未发布工作区）

- 用户确认不同平台仅编译和打包对应特性。现有macOS AX/Vision及Windows OCR worker原本已有条件编译，Mac生产包本来不含Windows worker或12,226,852字节模型；源码快照和Cargo.lock包含两平台信息不等于安装包包含两套实现。
- 本轮将ocrs/rten移到Cargo的Windows目标依赖，将offline/worker及其模型测试限制为Windows；Windows保留模型SHA-256、原创像素识别和最终exe worker测试，Mac不再为测试额外编译Windows引擎。三份模型许可/说明（合计25,085字节）移入`tauri.windows.conf.json`，Mac不再附带；CI的模型下载只在Windows desktop job执行，Ubuntu校验及Mac无需下载。
- 两平台`cargo tree --locked --offline -e normal`实际解析：Mac树无ocrs/rten系列，保留objc2-vision；Windows树保留ocrs/rten，无objc2系列。这里是目标依赖图验证，不是本轮Windows编译或打包。GitHub工作流YAML语法检查通过；没有推送/触发CI。
- 本机Node55项为54通过/1项minisign缺工具跳过，前端构建通过；Rust core31通过、desktop46通过/0忽略，cargo check/fmt、版本及diff检查通过。相对前轮47通过/1忽略的减少来自Windows模型测试改由Windows执行，不是删除这些测试。
- 独立`Starshot Platform Verify` / `com.starshot.platform-verify-20261029`的macOS arm64 app/DMG构建、严格codesign及DMG校验通过，未启动或安装。实际app Resources只含`icon.icns`，无模型许可或.exe/.dll/.rten文件；产物继续ad-hoc签名、未公证。DMG 5,051,406字节，SHA-256 `8ad9dbee2cbf2f6ae66719e679108be3ec0daee707e4905f9037c5749267d7d6`；可执行文件SHA-256 `ae9dc8f120da881d7fc9c8f416903b62f73575083ea3ca2455b93c5c60475c2c`。日志、依赖图、app归档和build-verification.json位于忽略的`artifacts/platform-packaging-20261029/`；归档核验后移除本轮生成的可运行副本。
- 日用应用未替换，哈希仍为`75ce3c32a7c649c4f9fd44a231f9c34a8551b29e3d3486c38d748ec8476df73d`。本轮未新增界面行为，未重复原生截图/交互测试；Windows新配置的实际资源与构建由同伴后续验收。源码交接包另生成v2，保留旧快照以便区分基线；未提交、推送、发布或安装。

## 2026-10-29 · 贴图焦点光圈（未发布工作区）

- 根据用户提供的Snipaste参考图实现默认开启的活跃蓝色/失焦灰色光圈。设置页和右键菜单共用本机持久开关，旧设置缺字段默认开启；关闭不改变图片位置或尺寸。原生窗口四周增加12逻辑像素透明空间，光圈框住旋转后的实际图片矩形，不进入原始图片导出。原生焦点与透明度通知均限定当前窗口，光圈偏好向所有贴图同步。
- 浏览器贴图回归13组通过，涵盖原有菜单/输出行为、默认开启、目标窗口焦点隔离、关闭后重载记忆、保存失败保留、全局同步、小窗四种旋转和边距，以及监听注册期间/初始请求期间的事件先后顺序。使用原创固定像素和模拟IPC，已查看活跃/失焦预览；不能当作原生截图或Windows验收。
- 设置与编辑回归13组通过，新增迟到get/set设置响应不能覆盖较新光圈事件，且事件不改快捷键草稿。回归还发现原有设置对象通过emit共享给主页面，导致未保存草稿提前修改已生效快捷键提示；现改为仅复制两个快捷键字符串，修复后全部通过。上述结果及图片复制到`artifacts/pin-halo-20261029/`，与之前验证批次区分。
- Node55项为54通过、1项minisign因缺工具跳过；Rust no-default31通过、desktop lib47通过；Vue/TypeScript生产构建、cargo check/fmt、版本一致性和diff检查通过。
- 独立`Starshot Halo Verify` / `com.starshot.halo-verify-20261029`原生应用载入内置原创示例，检查新贴图可访问状态为「活跃」、菜单关闭写入pinHalo=false、设置页读取关闭状态、第二张贴图继承关闭，以及重新开启后已有贴图同步。只读AX与该验证应用设置文件，没有捕获真实桌面或读取剪贴板。原生失焦视觉效果、跨应用切换、混合DPI、Windows原生仍待验收。此轮AX未取得退出提示文字，Escape可取消，故不新增退出对话框验收结论；最终只终止独立测试实例。
- 原生检查对应二进制SHA-256为`e7380d72a42ff391431c2a8279f6902801cad5fcbe6d5d8d45f6404ec41f8a07`；随后仅补快捷键字符串独立快照修正，并通过相关浏览器回归。最终macOS arm64 app重新构建、严格codesign与归档内二进制校验通过，未重复启动，仍为ad-hoc、未公证；最终二进制SHA-256为`568786507f80469cb3a4302bd17be27335d41bf5df8cd17e131e563be5d9a4e5`。app归档、构建日志、native-verification.json与build-verification.json保存在`artifacts/pin-halo-20261029/`，归档后移除可运行验证副本，避免重复入口。
- 日用应用PID和二进制哈希保持不变；这批代码未安装、未提交、未推送或发布，版本仍为0.3.1。没有把既有Windows调查或旧发行构建当作本轮Windows验证。

## 2026-10-29 · 贴图与截图悬浮提示（未发布工作区）

- 按用户提供的Snipaste悬浮说明参考图，补齐贴图图片区多行title及可访问说明，涵盖双击关闭、拖动、滚轮、修饰键透明度、右键菜单、空格和光圈设置入口。工具栏与右键菜单补充准确的重置动作、穿透恢复方法和全局光圈偏好说明；截图工具条补重做、复制快捷键及贴图后的操作提示。未新增自定义浮层、计时器、双击设置或导出处理。
- 独立复核文案与现有handler一致：Mac透明度支持⌘或Ctrl，Windows提示Ctrl；重做为⌘/Ctrl+Shift+Z，复制支持Enter与⌘/Ctrl+C；光圈和托盘恢复入口均已存在。双击行为目前不可配置，未照抄竞品的设置路径。
- Node55项为54通过/1项minisign缺工具跳过；Rust no-default31通过、desktop lib47通过；前端生产构建、cargo check/fmt、版本及diff检查通过。本轮为低影响提示属性修改，未新增只匹配文案的测试，也未把前轮浏览器回归算作本轮重新执行。
- 独立`Starshot Hint Verify` / `com.starshot.hint-verify-20261029`的macOS arm64 app构建、严格codesign与归档内二进制校验通过，未启动、未安装，仍为ad-hoc、未公证。最终二进制SHA-256为`22d32fd0b6e4485a5151f29d3705b8c428fd98aad29c1953eac49eef55abb515`。归档及build-verification.json保存在`artifacts/hover-hints-20261029/`，归档后移除本轮可运行验证副本。
- 原生悬浮气泡的出现时机、换行及双平台实际显示未在本轮实测；这里确认的是title/可访问属性接入和构建，不能等同原生视觉验收。日用应用PID及哈希保持不变；未截图、未读剪贴板、未提交、推送、发布或替换应用。

## 2026-10-29 · Windows选区v2交付合入与本机回归（未发布工作区）

- 用户交付`starshot-selection-rules-v2.zip`，SHA-256 `2615cdebedca3478e0b831d36736620c964d1580aa0710cf75ba34c701dd0796`；14个文件、清单中13个内容文件的哈希均匹配。累计补丁基于第一版116文件源码交接包，早于本机平台分包/光圈/提示修订。重建基线与候选后逐文件三方合并，保留后续本机改动，Windows依赖并入既有target表，没有覆盖整个工作区。
- 合入Windows目标HWND局部UIA、可终止helper、代数取消和会话复核；公共候选改为严格包含的父级路径，Tab向外、Shift+Tab返回、两端停止，并在首次Tab冻结路径。合入时另修复±2px去重吞掉精确窗口/全屏、全屏候选带入Frame字段，以及Shift事件遇排队鼠标移动的竞态。Windows全局槽忙时由直接返回改为最多250ms有界等待，保持5ms取消轮询、单helper和其750ms运行期限；新增3个Windows专用槽争用测试，Mac不能执行这些测试。
- 同伴v2的46/46已从原始数据复算，含两屏各23个断言、20次固定语义位置零边误差；这是原Windows开发二进制的原创夹具结果，非46个真实应用，也非本机修订版的Windows原生验收。真实WebView2/UIA但手工构造Frame/单宿主，逐屏运行；未经过生产begin_capture/窗口枚举/同时双屏，recovery重启了probe父进程。Snipaste静态摘录只做内部一致性复核，未执行目标；边界与来源见[Windows集成](windows-uia-integration.md)和[选区规则](snipaste-selection-rules.md)。
- 最终Node59项：58通过、1项minisign缺工具跳过；Rust no-default31通过、desktop lib47通过；Vue/TypeScript生产构建、cargo check、cargo fmt通过。新几何用例包含近似宿主/全屏保留及带额外Frame字段的全屏候选。日志仅在忽略的`artifacts/windows-selection-rules-v2-20261029/`。
- Ego Chromium悬浮8组与截图9组通过。新增非指针屏不查询、迟到候选下固定双向层级路径/端点、排队同像素事件与Shift+Tab回归；原有就地标注/导出像素、辅助线和长截图模拟IPC继续通过。首次回归捕获全屏Region多余字段问题，修复后完整重跑通过，没有降低断言。测试只使用原创固定像素和模拟IPC。
- 最终dist在真实WKWebView复跑6/6，stderr为空：module Worker、构造失败/运行失败回退、模拟原生控件优先、迟到控件及Worker不覆盖Tab。脚本适配新的invalidate命令与候选数组，仍不是macOS AX、真实屏幕或Windows原生实测。
- 两目标离线依赖图验证：macOS不含windows/ocrs/rten而含objc2-vision，Windows包含前者而无objc2-vision；仅图解析不等于Windows编译。本机仅安装aarch64-apple-darwin目标，未安装工具链或执行Windows程序。
- 独立`Starshot Selection Verify` / `com.starshot.selection-verify-20261029`的macOS arm64 app构建、严格codesign与归档内二进制校验通过；未启动/安装，ad-hoc且未公证。二进制SHA-256 `6756845198da7e0e569bb176b58c56406a3b8992fa2ef634107198cdb46772e4`；Resources只有icon.icns。归档`Starshot-selection-verify-0.3.1-unreleased-20261029.app.tar.gz`为4,981,240字节，SHA-256 `2588be7ad0e969616495e408485ffd212abe3148c2ec4cdffd3a4baf2b85f694`，核验后移除本轮可运行验证副本，避免新增重复入口。
- 日用`/Applications/Starshot.app`二进制哈希仍为`75ce3c32a7c649c4f9fd44a231f9c34a8551b29e3d3486c38d748ec8476df73d`，未替换、重启或改权限；源码版本仍0.3.1，未提交、推送或发布。Windows同进程故障恢复、同时双屏槽交接、生产窗口快照和合入后平台构建须后续复测；真实跨应用、跨PID provider与高权限/自绘界面仍是明确限制。


## 2026-10-29 · Windows v3生产初始化、同进程恢复与release/NSIS（同伴执行）

- 基于Mac回传的starshot-selection-integrated-v3-20261029独立目录工作，126文件哈希一致。新增只在验收feature编译的生产begin_capture固定像素入口、真实IPC采样与同进程诊断，修复同坐标重新进入覆盖层不重新查询及离屏遗留rAF事件；未修改同PID限制和250/750ms预算。
- 最终原生旧矩阵46/46、新矩阵19/19；新矩阵同一Tauri父进程PID46728完成20次双屏静止启动、普通/150ms延迟回收注入各20轮跨屏、各10次超时/取消后健康查询，以及窗口遮挡与句柄重建。186次物理移动校对、48次原创像素替换，helper峰值1且结束后0。恢复组直接调用同一父进程内的生产UIA执行层；不冒称每次恢复均经前端触发。详见[完整方法与边界](windows-native-v3.md)。
- 最终Node59项：58通过/1项minisign缺失跳过；Rust core31/desktop50通过；Windows槽争用筛选4/4（含3个新增案例）；前端/版本/cargo check/fmt通过。存在原有unused path及兼容element_at警告。原生性能检查与构建串行运行。
- 验收dev exe SHA-256为7047e398145f4deaac909d6582b6e32e8a2c49fa66df6e490d5887236cd71194；普通Windows x64 release exe为0114a174d5c72a9ab7d1fba0b88c345be042a10ce441cf448976a9eacc9c2c2a。NSIS为Starshot_0.3.1_x64-setup.exe，16469536字节，SHA-256 b5a8c681864f87154d09148eeff2b1fc98970aa17147d5f2901d2318c2a1f2d5。只构建，不运行安装器；Tauri打包器恢复了NSIS3.13和tauri-utils0.5.3的便携构建缓存，未安装系统工具或应用。
- 普通release运行三个验收CLI参数均返回64，发生于应用初始化前；编译条件/普通handler及二进制诊断标记检查确认测试实现排除。两份OCR模型完整字节在exe内，生成的NSIS脚本包含NOTICE和两份许可证；Windows目标依赖图含ocrs/rten且无objc2-vision/objc2-foundation。未实测安装、升级、卸载或正式UI中的测试IPC调用。
- 首次静止点在较早批次读到与测试预设不同的坐标，失败保留；测试改用物理坐标并补齐移动后和Frame读取时的DPI/指针记录，最终20/20通过，不宣称统一解决所有历史无框问题。驱动互等/只读invoke拦截/文件共享失败单列，不计通过。Mac公共Vue回归、真实跨应用/跨PID/高权限/自绘provider、负Y/旋转/热插拔和长期稳定性仍未验证。
- 未截取真实桌面、读剪贴板、改权限、安装日用应用、提交、推送或发布。源码仍0.3.1未发布，交付仅相对v3的增量补丁和精简证据；本机完整日志留在忽略的artifacts。

## 2026-10-29 · Windows v3接回、源码完整性与本机收尾

- 接回`starshot-windows-selection-v3-result.zip`（71,750字节，SHA-256 `6ac5de3bf88040a9bd28d70c3af020edaed6d61d9d0fcb93af39df9fde8a3c65`）。8个文件、清单7项哈希匹配；17个变更文件的before哈希全部与当前工作区一致，备份后直接应用增量补丁，after哈希逐文件通过，没有覆盖已有本机改动。
- 离线复核原始记录：旧矩阵两屏各23项共46项，新矩阵19项；199条控制命令和726条helper事件均为同一父PID，样本数、矩形、指针及spawn/reap记录吻合。147次前端查询加40次恢复直调共187次查询。接回审查未发现阻碍合入的问题，本轮收尾，不再要求用户转发第四轮。真实像素采集、复杂真实应用和长期稳定性仍按既有边界记录。
- 合入后本机Node59项为58通过/1项minisign缺失跳过，Rust core31/desktop47通过；前端构建、版本、cargo check/fmt与diff检查通过。真实WKWebView载入最终dist，6/6通过、stderr为空；仍使用原创像素与模拟IPC，不当作macOS真实AX或屏幕捕获验收。Windows原生结果来自同伴执行，本机没有运行Windows程序。
- 用户明确要求Windows实现及交接验收保留源码。核查130个源码/资源文件，UIA/helper、生产截图、Rust验收入口、Python/JavaScript驱动、C#原创夹具及构建配置均齐全，没有自制DLL/EXE/LIB/PDB作为源码替代。首次源码包126文件，v3增加4个源码/文档文件；当前完整快照另存本轮忽略的artifacts，含逐文件SHA清单。
- 只读查询公开GitHub：main仍为`c69c87aed506c015ddf267e0d4aa22ea8ec47c30`，与本地HEAD一致；新Windows实现及本轮其他功能尚未提交/推送，不能称已在公开仓库开放。本地源码、交接快照与公开发行状态分别记录；README列出实现和验收源码入口。
- 本轮所有日志、输入包和源码审计位于`artifacts/windows-selection-v3-result-20261029/`。未安装、重启日用应用、改变权限、读取剪贴板或捕获真实桌面。
- 最终独立`Starshot V3 Verify` / `com.starshot.v3-verify-20261029`的macOS arm64 app构建、严格codesign及归档二进制校验通过，未启动/安装，ad-hoc且未公证。二进制SHA-256 `ee87cef6d126a84775a8ab56e31e772cfc82411c962bee7a69874dcef44ba8f7`；Resources仅icon.icns。归档4,981,324字节，SHA-256 `8017f12c4a0bd9f273d15a6f9b81a942ff0cb418ac042989b4867de62952e09b`；核验后移除本轮可运行副本。日用应用PID87441及二进制哈希保持不变。

## 2026-10-29 · 中文分批提交与公开源码推送

- 用户明确要求推送源码并使用中文分批提交。只读核对远端main仍为c69c87a且无新增冲突后，在`codex/screenshot-interactions-20261029`按模块提交：`a8147fb`修复零面积遮盖与会话备份，`a4a1a17`隔离平台依赖/资源，`d7a3a15`提交截图/贴图功能及完整验收源码，`eea18d1`补充研究、验证与知识库文档。
- 提交前公开范围审查未发现真实图片、凭据、原始日志、个人绝对路径或闭源DLL替代实现。99个实现/测试/构建输入文件与上一轮已验证的完整源码快照逐文件哈希一致；本轮仅整理提交及文档，未改变产品代码或重复跑平台测试。此前已通过的检查边界保持不变，不能把推送等同于新一轮CI通过。
- 首次HTTP/2推送因连接中断失败；仅对重试命令使用HTTP/1.1，随后远端明确确认`c69c87a..eea18d1 HEAD -> main`。无强制推送、重写历史或覆盖远端提交。源码已公开；前节的“尚未提交/推送”保留为当时状态。
- 本次只推送源码，没有创建版本标签、Release或替换安装包，也没有安装/重启日用应用；稳定发行仍为v0.3.1。原始证据、源码快照、构建产物与日志继续保留在Git忽略的artifacts中。

## 2026-10-29 · 取消截图后保持主窗口收起

- 用户反馈从主窗口点击开始截图后，点选区工具条叉号会再次弹出软件。源码确认`begin_capture`记录`capture_restore_main`，`cancel_capture`及原生`CloseRequested`据此显示主窗口，属于此前明确实现的恢复策略。
- 移除该状态、赋值和两处恢复分支。叉号、Esc、右键、长截图取消及系统关闭仅调用`clear_capture`，撤销原生控件查询并销毁本次覆盖层，不改`current`、编辑会话或贴图。保留旧会话标签检查、截图启动失败显示错误，以及主动“更多编辑”显示主窗口；快速复制/保存/贴图原行为不变。
- 本机Node59项为58通过/1项minisign缺失跳过，Rust core31/desktop47、前端构建、cargo check/fmt通过。此次为取消入口删除恢复分支，没有增加镜像源码的测试，也未重复无关浏览器矩阵或让用户转发Windows测试；没有捕获真实桌面，尚未进行安装版原生取消实测。
- 检查日志在忽略的`artifacts/capture-cancel-20261029/`。此修复需安装含新代码的应用后才改变日用行为，不能将源码推送当作已替换本机程序。
- 独立`Starshot Cancel Verify` / `com.starshot.cancel-verify-20261029`的macOS arm64 app构建、严格codesign及归档二进制校验通过；未启动/安装，ad-hoc且未公证。二进制SHA-256 `923b730527cf44a5d6b1538845147f457f84caaf8e86e5e18bdf2da52174ed8b`，归档SHA-256 `d0991589a9561123038b88d1dcd562af109323d5dcd3b69d21489a32ebe71b56`。归档后移除本轮验证副本，日用应用哈希保持不变；本轮未执行Windows新构建。
