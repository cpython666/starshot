# Snipaste 悬浮区域检测静态调查（2026-10-29）

本轮用户明确要求使用 REA 查看 Snipaste 的实现。调查对象是本机 `/Applications/Snipaste.app`，版本 **2.9.2**、标识 `com.Snipaste`。主程序是 x86_64/arm64 通用 Mach-O，本轮选取 arm64；主程序完整 SHA-256 为 `8ba614c85cacbb9d4673d25b648bb9e44ded2feae8c563db2cf258a5f77a5276`，arm64 UUID 为 `0D1DE9DA-40B5-3FFE-89E6-E44DEDB81CB5`。这些结论仅适用于这个已安装构建，不外推为 Snipaste 最新版或 Windows 实现。

没有运行 Snipaste、触发截图、读取用户配置/历史、读取剪贴板或改变系统授权；没有复制竞品实现到星镜。

## 实际使用的 REA 与限制

已安装的 skill 只是指令，并未注册 MCP 或安装深度分析引擎。本轮实际通过 **REA CLI 6.7.0** 的 `inspect-macho` 和 `inspect-plist` 取得 Evidence，不是仅阅读官网。

REA 6.8.0 要求 Node `^22.23.4 || ^24.22.1 || >=26.12.1`，本机 Node 25.2.1 与内置 24.19.0 不满足；停止首次调用后，使用 npm 元数据确认兼容内置 Node 24.19.0 的固定版本 6.7.0。没有升级 Node、改动 Codex 配置或替换现有 skill。CLI 支持范围以 6.7.0 实际帮助为准，不能把 skill v53 的全部接口当作该版本已经提供。

| 实际操作 | 证据与结果 |
| --- | --- |
| `inspect-macho /Applications/Snipaste.app` | `ev_6c54633c2dc2e584d33adb4c0aa2372a67320e88715ca155768c51b3969a08c6`；取得 arm64 分片、依赖、导入/导出与段信息 |
| `inspect-plist /Applications/Snipaste.app` | `ev_7c85146848f3629c439b3ede294a0261817428d85bb5ae077b57deae657f5293`；确认标识和版本 |
| `function /Applications/Snipaste.app --procedure=0x1002bc008` | 返回 `provider_unavailable`。Hopper/Ghidra/IDA/Kuna 均不可用；Ghidra 也没有可用 Java 运行时。没有自动安装这些额外引擎或反复重试 |

完整 REA Evidence、错误与补充检查结果保存在 Git 忽略的 `artifacts/snipaste-rea-20261029/`。REA 导入/导出结果声明 `exhaustive: false`，隐藏或剥离符号可能缺失。CLI 操作结束后进程退出，没有留下打开的原生分析会话。

## 已观察到的实现事实

**该 macOS 构建有真正的系统辅助功能元素定位路径。** REA 显示它导入 `AXUIElementCopyElementAtPosition`、`AXUIElementCopyAttributeValue`、`AXUIElementCreateApplication`、`AXValueGetValue`、辅助功能授权检查，以及 CoreGraphics 窗口枚举函数；同时保留了 `WindowDetector_mac`、`WindowDetector`、`ThreadedWindowDetector` 等检测类的导出符号。

单独看到导入符号不足以断言它用于悬浮选区。本轮进一步使用本机 `otool -arch arm64 -tvV` 只读反汇编，并将指令地址与 REA 的导出范围对应。以下是**普通系统工具与原始文件字节的补充观察，不冒充新的 REA Evidence，也不称为已反编译的原始源码**：

| 代码位置 | 可直接核对的调用关系 |
| --- | --- |
| `WindowDetector_mac::collectTopLevelWindows()`，`0x1002bb5dc` | 在 `0x1002bb670` / `0x1002bb678` 调用 `CGWindowListCreate` / `CGWindowListCreateDescriptionFromArray`；读取 `kCGWindowAlpha`、`kCGWindowIsOnscreen`、`kCGWindowBounds`、`kCGWindowLayer`、名称和所属 PID |
| `WindowDetector_mac::elementRectUnderPos(...)`，`0x1002bc008` | `0x1002bc088` 调用 `AXUIElementCopyElementAtPosition`；成功后在 `0x1002bc0c0` 调用取矩形 helper |
| 取矩形 helper，`0x1002bb4e8` | 在 `0x1002bb514` / `0x1002bb544` 读取属性，再调用 `AXValueGetValue`；按 Mach-O 段位置解码对应 CFString 数据指针，属性分别确认为 `AXPosition`、`AXSize` |
| `elementRectUnderPos` 后半段 | `0x1002bc134` 调用 `QRect::contains(point)` 检查矩形包含鼠标点；`0x1002bc150` 调用矩形求交，将有效元素框约束在宿主窗口内；不包含点的路径回到窗口矩形 |
| 备用 helper，`0x1002bc168` | 一条特定错误分支进入这里；读取 `AXChildren`（原始 CFString 已确认），逐个检查包含点的子元素并在 `0x1002bc238` 递归处理 |

这条「窗口→鼠标处元素→元素位置/尺寸→包含点检查→与窗口求交」调用链有二进制静态证据，比“根据外观猜它做了边缘检测”更明确。但本轮没有运行时观测，不能断言每次悬浮都会进入该分支、哪个应用会返回正确的 AX 元素，或它在所有权限状态下如何退化。

REA 还观察到下列符号：`detectRectInternal`、`findNextLevelRectByNeighbors`、`getParentRect`、`getChildRect`、`queryRectQueued`、`consumeQueries`、`clearQueuedQueries`、`onElementDetectionStarted/Ended`、`Snipper::handleDetectedRect`。它们支持继续调查层级选择与异步检测的方向；仅凭符号名不能声称已确认它们的全部排序、缓存、超时或陈旧请求处理算法。

## 对星镜的启发与尚未回答的问题

调查前星镜采用xcap窗口边界加截图像素边缘候选，和上述AX控件定位是不同证据层级。仅调边缘阈值不能保证获得同样的系统控件边界。本轮星镜独立实现了macOS CoreGraphics几何快照，以及已有授权下的AX点查询：仅取位置/尺寸，检查包含点并与宿主窗口求交；失败保留窗口/像素候选，过期异步结果不能覆盖新位置或Tab手选。实现入口为`src-tauri/src/window_regions.rs`、`desktop.rs::get_capture_element`和`CaptureOverlay.vue`。没有复制竞品代码，亦未实现上述递归子元素备用分支。此macOS调查阶段尚未实现Windows UIA；同日后续已接入Windows v2源码，见[Windows集成记录](windows-uia-integration.md)。真实跨应用命中率、生产多屏流程与合入版Windows原生补测仍未完成，见[验证记录](verification.md)。

本轮macOS调查未回答：Snipaste Windows是否采用UI Automation（后续Windows证据见下，UIA仍未确认）；完整多层候选排序与邻域扫描策略；所有特殊窗口/辅助功能错误的分支；是否另有像素检测及其占比；原生性能和实际控件命中率。后续Windows v2只补强局部父子包含、邻域几何探测和层级历史的静态证据，不能将调查称为“完整还原Snipaste算法”，也不能把星镜修复预选框显示等同于已实现Snipaste同等级控件识别。


## Windows同伴交付与离线复核（2026-10-29）

用户提供`starshot-windows-investigation.zip`，SHA-256为`53f51c44b2e8063765b32d465bce230e8c91a22b3b6dd9e694c1657ab1ba778b`。本机解包到Git忽略的`artifacts/windows-investigation-20261029/`，交付清单25个文件逐字节哈希全部匹配；不将原始日志、窗口身份、个人路径或反汇编摘录加入Git。该包是报告、原创探针源码与测量结果，不含星镜补丁或Snipaste程序。本机只做静态审查和离线数据核算，没有在macOS重新执行Windows探针。

### 静态结论及归属

同伴调查对象是Windows **Snipaste 2.11.3 x64**，主程序7,008,768字节，SHA-256 `29aac14eca91aa8fe3d44207212078599800509d3381123fd0def34d0b19b33a`，首选ImageBase `0x140000000`。不能与上文macOS 2.9.2的地址或算法混用。

- REA MCP 6.5.0的`inspect_pe_resources` Evidence为`ev_ccd1d5063083d91e026578d689b33673c77af8e1eb6a256c02eefa19b6af6932`，证明工件身份和PE资源结构，**不证明控件检测调用链**。
- 同伴使用已有Iced 1.21解码形成独立补充证据：`WindowDetector_win`的RTTI/vtable关联到RVA `0x25dba0`，其`0x25ddae`调用`AccessibleObjectFromWindow`并传入IID_IAccessible；后续helper包含`accHitTest`、`accLocation`与`get_accParent`。本机离线核算摘录长度、节偏移与RIP相对目标一致，IAccessible槽位与Microsoft接口定义一致。没有原始EXE，不能在本机再次证明摘录与整份EXE字节相同；原始文件匹配属于同伴交付声明。
- 可确认的是**MSAA局部静态调用链**。命中循环有elapsed大于100ms、计数大于60时跳过下一次调用的分支；这是调用间检查，不是可强行中断COM调用的截止时间。线程/重启字符串只支持异步检测与恢复方向，完整调度和缓存策略未还原。
- 没有Snipaste运行时调用证据。主EXE的UIA导入、字符串及GUID有界搜索未命中，不能推出没有UIA；Qt qwindows.dll的UIA provider符号也不能当作检测外部控件的UIA client证据。

### 原创探针结果

同伴在Windows 11 25H2 x64、两屏100%/150%缩放及负X坐标上运行WinForms/WPF原创控件；A无覆盖，B不透明覆盖，C为指定分层/鼠标穿透样式组合。每个点有6次查询，4种方法，共1008条记录、42个helper。本机独立重算唯一键、真值矩形、四边误差和统计，与报告一致：

| 观察 | 本机离线复核结果与适用范围 |
| --- | --- |
| 保存目标HWND后在该UIA子树按点查询 | 六类可访问控件216/216次边误差0px；是2屏×6个固定控件位置×3场景×6次重复，不是216个独立控件或跨应用成功率 |
| 覆盖层上的全局UIA点查询 | B中84/84命中覆盖层；C中84/84命中底层目标。C依赖本探针的样式组合，不能推断Tauri/WebView2窗口或单个穿透标志同样有效 |
| 没有子HWND的WPF按钮 | UIA返回独立按钮矩形、NativeWindowHandle=0；Win32只返回较大宿主，最大边误差110px，说明UIA确实补足窗口边界能力 |
| 没有无障碍元素的自绘矩形 | UIA与Win32返回承载面板，误差140px；仍需窗口/像素候选或手动选区 |
| 局部UIA查询耗时 | A/B/C重复中位数约61–69ms，重复P95约98–120ms；不含进程创建/文件写入，且全局UIA先执行，不是公平冷启动排名 |
| 限时与异常 | 深度8/节点128/调用间150ms预算；最长局部查询158.17ms，证实合作式预算不是硬截止。每helper整批24次查询另有5秒进程看门狗；本轮没有人为挂起provider，0超时不能证明故障恢复可靠 |

这些是首轮独立原创探针的Windows运行记录，不能冒充Snipaste运行证据或星镜集成验收。该轮未覆盖真实Tauri/WebView2、滚动/关闭/重建或卡住的provider；后续v2夹具的新增证据见下。Chromium等真实应用、快速跨窗口切换、提升权限窗口、负Y/旋转/RDP仍未形成原生矩阵。

### 首轮接入缺口与后续进展

首轮调查时Windows控件识别尚未实现，宿主仅存PID/矩形，`get_capture_element`持有operation锁期间查询；这些是当时的源码状态，不是当前待办。后续v2已保存HWND/PID/前后顺序及身份信息，从目标HWND建立UIA根，通过独立、受Job Object管理的COM MTA helper查询；读取快照后先释放operation和frames锁，返回再复查会话、请求代数、点位与宿主。只读几何、类型和身份，不读取Name/Value/Text，失败保留窗口/像素候选，不提权、不终止目标应用。

当前Windows源码已有128节点/12层/8候选上限、300ms合作式遍历预算及750ms helper运行期限；共享前端已有按完整包含关系前进/后退的父子路径。普通任务超时或丢弃Future不能中断COM，因此不能把300ms预算称为硬截止；独立helper的启动、调度、回收也在750ms之外。不因Snipaste的MSAA静态链就新增MSAA或复杂缓存。实现与剩余验收见[Windows集成记录](windows-uia-integration.md)。

平台约束已与Microsoft文档核对：[UIA线程要求](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading)、[物理坐标与DPI](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-screenscaling)、[IUIAutomation2超时配置](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomation2)、[AccessibleObjectFromWindow](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-accessibleobjectfromwindow)。

## Windows v2选区规则与集成复核（2026-10-29）

用户交回 `starshot-selection-rules-v2.zip`，SHA-256为 `2615cdebedca3478e0b831d36736620c964d1580aa0710cf75ba34c701dd0796`。本机复核14项文件清单与累计/增量补丁，两种回放结果一致；原始数据留在Git忽略的 `artifacts/windows-selection-rules-v2-20261029/`。补丁现已合入本地工作区并保留后续的平台分包、光圈及提示，不等于已安装或发布。

### 静态证据新增范围

本轮仍是Snipaste 2.11.3 x64同一构建。REA函数分析因provider不可用而未完成，已有PE资源Evidence只证明身份，不为Iced函数调用链背书。本机复核18段摘录的3,200字节、720条指令跨度、49处RIP相对引用及126处相对调用/跳转目标，内部一致；无原始EXE，不能独立证明摘录与声明构建一致，也没有重新执行Snipaste。

新增证据将Qt元数据中的 `queryParentRect` / `queryChildRect` 经信号和 `connectImpl` 连到具体函数：父层要求完整包含且不同框，再取较小面积；四边外一像素中点是几何探测；子层读取局部保存的矩形序列。MSAA父链同样要求宿主求交、包含点及合格祖先。QThread创建与 `moveToThread` 是静态安排，不是运行观测。位置、Evidence归属及未知项见[选区规则](snipaste-selection-rules.md)。

星镜采用“完整包含、最近外层、保留返回路径”，Tab向外、Shift+Tab向内、两端停止。按钮类型优先、重叠兄弟拒绝和这些快捷键是星镜自身选择，没有证实Snipaste也使用相同规则；未移植其四点探测或MSAA父链。

### 原生记录的复核与边界

v2实际46/46为两屏各23个布尔断言，本机重算与汇总一致；原补丁文档中的40项属于首轮。基线为每屏7个位置各2次：其中5个可访问语义位置共20次边误差0，重叠位置保守拒绝，自绘无provider位置返回面板且误差35px，没有算进语义成功。150%负X屏及100%主屏分别记录按钮→面板→宿主→全屏路径，连续Tab/Shift+Tab验证端点和逆序，CSS回读最大约0.5物理像素。

这是原创WinForms/WPF、真实UIA/Tauri/WebView2/IPC和固定棋盘像素的运行证据；入口手工建立Frame和单个宿主，两屏分别测试，未经过生产 `begin_capture`、真实窗口枚举/Z顺序、同时双屏覆盖层或真实截屏。provider恢复阶段重新启动了Starshot probe父进程；重复6次超时/取消后的清理有证据，同一存活应用恢复正常命中尚无证据。当前拒绝跨宿主PID的元素，合法跨进程provider也未验收。

本地合入另修正近似候选去重吞掉精确父层、全屏Region字段及Shift前待处理移动，并补共享查询槽争用处理；这些不属于原46项对应二进制，Windows需重新构建验收。后续优先补生产入口、同时双屏切换、同进程故障恢复和反复启动无框问题；不需要在个人桌面触发竞品截图来填充证据。最新自动检查和平台构建状态见[验证记录](verification.md)。
