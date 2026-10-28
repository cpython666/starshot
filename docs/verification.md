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
