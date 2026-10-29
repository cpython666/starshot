# Windows UIA 集成记录（2026-10-29，未发布）

用户交回的 Windows v2 补丁已于2026-10-29合入当前工作区，保留本机已有的平台分包、贴图光圈、悬浮提示、长截图、macOS AX及退出保护。该段描述v2合入时的状态；后续v3已完成下文生产初始化/同进程验收，并构建普通Windows release及NSIS，仍未安装或发布。Windows 原始验收包与本机合入后的代码有差异，不能把包内结果直接当作合入版的原生验收。

交付包 `starshot-selection-rules-v2.zip` 的 SHA-256 为 `2615cdebedca3478e0b831d36736620c964d1580aa0710cf75ba34c701dd0796`，共14个文件，SHA256SUMS中的13项文件哈希全部匹配。交付记录以先前源码快照的116个原始文件为基线；本机核对累计补丁和增量补丁的重建结果一致。该基线早于本机后续的平台分包、光圈和提示改动，接入时逐项保留这些改动，没有用整个快照覆盖工作区。原始数据仅存于Git忽略的 `artifacts/windows-selection-rules-v2-20261029/`。

## 源码行为

Windows 在覆盖层出现前保存顶层窗口 HWND、PID、Z 顺序、线程 ID、进程创建时间和类 atom。前端仅传本截图窗口中的点与请求代数，无法指定外部 HWND。目标关闭、窗口几何变化或身份不符时丢弃语义结果；权限不足仍保留可取得的窗口边界。

UIA 从 `ElementFromHandle` 建立目标根；不使用覆盖层上的全局 ElementFromPoint，不遍历桌面树。仅读 ProcessId、BoundingRectangle、ControlType 等几何/身份属性，不读 Name、Value、Text。枚举所有包含点的兄弟分支，优先语义控件而非按钮内文字；不相关重叠分支无法确认层次时回退，而非猜第一个元素。最多 128 节点、12 层、8 个去重候选；300 ms 是合作式遍历预算，不能中断 COM。

查询使用进程级唯一槽，在受 Job Object 管理的自建子进程中运行无窗口 COM MTA。750 ms 是 helper 启动后的运行截止，超时/撤销会终止并回收该 helper，进程启动、调度与回收另有开销。取消轮询间隔 5 ms；退出时 Job Object 的 kill-on-close 防止遗留。不会终止目标应用，不自动提权。没有增加 MSAA、全桌面扫描或复杂缓存。

`get_capture_element` 在读取会话快照后释放 `Context::operation` 和 frames 锁，返回时复核窗口标签、Arc 会话身份、请求代数、点位、宿主和显示器。鼠标移动、离开显示器、Tab、拖选和取消均撤销旧请求。窗口/像素候选立即可用，UIA 结果异步细化。

所有原生几何为带符号物理屏幕像素与半开矩形。Windows 初始指针使用已保存的显示器物理边界；CSS 视口在首次解码和鼠标事件时更新。Tab先处理待执行的rAF移动再冻结候选路径；Tab逐层向外、Shift+Tab沿原路径向内，两端停止。同像素旧修饰键事件和迟到查询不能改变键盘选择，真实移动后重新命中。层级规则见[选区规则](snipaste-selection-rules.md)。

入口为 `src-tauri/src/window_regions/windows.rs`、`desktop.rs::get_capture_element` 和 `CaptureOverlay.vue`；验收入口为 `desktop/windows_smoke.rs`。Windows UIA代码及 `windows` 依赖限定Windows编译；macOS AX继续使用自己的实现，共用候选数组、请求代数和前端交互。

## 验证层级与复现

Windows交付记录使用Rust 1.90.0、MSVC Build Tools 2022 17.14.43、Windows SDK 10.0.26100.0、Node 24.19.0。该端报告曾获用户授权安装开发工具；这是历史操作记录，不构成本线程安装工具、应用或改动系统权限的授权。本机此次只做离线复核和源码接入。

自动检查命令：

```powershell
npm ci --no-audit --no-fund
npm run models:prepare
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo build --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bin starshot
```

原生验收命令：

```powershell
./tests/build-windows-uia-fixture.ps1
cargo build --locked --manifest-path src-tauri/Cargo.toml --features windows-uia-smoke,tauri/custom-protocol
python tests/windows-uia-acceptance.py src-tauri/target/debug/starshot.exe
```

验收使用原生WinForms/WPF原创窗口、真实UIA、Tauri/WebView2、Vue覆盖层和Rust IPC。覆盖背景为生成的固定棋盘像素，不调用 `capture_image`，不读取真实桌面或剪贴板。`windows-uia-smoke` 是显式验收feature；普通构建不注册测试命令，也不接受验收入口。

验收入口手工建立 `Frame`，从夹具的 `describe` 结果指定单个 `CaptureHost`；两屏分别运行。它没有经过生产 `begin_capture`、真实窗口快照/Z顺序枚举、同时创建多个捕获覆盖层或真实截屏入口，不能称为完整生产截图流程通过。

Windows验收主机为Windows 11 25H2，左屏2560×1600 / 150% / X=-2560，主屏2560×1440 / 100%。验收检查静止初始指针、快速移动、Tab锁定及双向层级路径、按钮内部文字、重叠兄弟回退、滚动后新矩形、销毁重建旧句柄失效、无provider绘图退到面板、阻塞provider超时/取消与进程线程数量。目标夹具有明确PerMonitorV2清单，记录SetProcess/SetThread尝试与有效上下文；helper和overlay的有效DPI状态也记录。

注意：DPI 清单已生效时 SetProcessDpiAwarenessContext 再次设置返回 false 是正常的，判断依据是有效 PMv2 上下文和 GetDpiForWindow。早期无清单夹具产生了 DPI 虚拟化真值错误，修正后重新验收。重负载编译并行时也观察到预算超时并正常回退；性能验收应单独运行，不能把 750 ms 当作含 OS 调度的严格端到端上限。

首轮记录为40项检查、npm 55通过/1项minisign跳过；本次v2实际为 **46/46，即两屏各23个布尔断言**，npm 56通过/1项minisign跳过，Rust core 29及desktop lib 46通过。本机按 `native-results.json` 重算全部46项，与 `native-summary.json` 一致；未在Mac重跑Windows程序。两版数字不可混用，也不是46个独立应用或控件。

| v2实测项目 | 离线复核的结果与边界 |
| --- | --- |
| 固定语义控件 | 2屏×5个语义位置×2次，共20次四边误差0物理像素；完整基线每屏为7位置×2次。无provider绘图4次返回承载面板、边误差35px，不计入20次语义成功；重叠兄弟4次均保守拒绝 |
| 父子路径 | 两屏均为按钮→面板→宿主→全屏4层；每个方向连续10次Tab，末端停止，反向按原路径返回。150%屏的CSS回读允许1物理像素舍入，实际最大约0.5px |
| 查询耗时 | 两屏基线端到端中位数分别177.5/226.5ms，最大232/364ms；12次阻塞查询763–774ms，12次取消65–80ms。750ms不是含创建、调度和回收的严格端到端上限 |
| 取消与清理 | 每组6次阻塞/取消后helper数均0、采样线程数均4；阻塞时10ms前端计时器继续推进。覆盖层取消命令2/3ms返回、查询空结果，测试退出走生产退出保护 |
| 恢复 | recovery阶段重启夹具，也启动新的Starshot probe父进程；证明重新启动后恢复，不证明仍存活的同一应用在超时后下一次正常查询恢复。连续6次超时/取消支持重复故障时的清理结论 |

包内分别构建普通Windows开发exe和验收feature开发exe；并非release或安装器。产物哈希及完整检查结果保存在交付包中。原生记录只覆盖这些原创控件与固定位置，不是跨应用准确率，也不新增Snipaste运行证据。

最终复验前曾观察到一次WPF首次查询216ms触及原180ms遍历预算，故调整为300ms；750ms helper期限未变。同轮150%屏初始静止选框曾缺失，补录初始点位/视口后的最终矩阵通过，但该单次缺失的根因尚未确认，保留失败证据，后续应做更长时间重复启动回归。

## v2合入时的修订与后续状态

本机合入时另修正近似框去重吞掉精确外层、全屏 `Region` 混入仅属于 `Frame` 的字段，以及Shift事件前未处理待执行移动。原v2共享查询槽忙时直接回退，跨屏切换可能在旧查询退出前收到空结果、停住后不再细化；现以5ms间隔最多等待250ms，每轮先检查请求是否撤销，超限仍保守回退，拿到槽后才进入原helper的750ms流程。没有新增队列或并发helper。新增3个Windows专用单元测试覆盖释放后继续、等候时撤销及持续占用到截止；已在后续v3的Windows筛选测试中通过。

这些是原始v2验收后的变更，已由后续v3重跑Windows旧矩阵并补充生产入口矩阵，详见[后续记录](windows-native-v3.md)。当前Mac自动检查、WKWebView原创图模拟IPC及浏览器结果统一见[验证记录](verification.md)，不能由模拟IPC推断原生AX或Windows通过。

- 后续v3已用生产 `begin_capture` 补窗口快照、前后遮挡、同时双屏与共享查询槽争用；仅像素源注入原创图，未捕获真实桌面。
- 后续v3已验证同一应用进程在provider超时/取消后查询健康目标恢复，并重复20次静止启动。此前偶发缺框仍不能据此归因于单一原因。
- 未覆盖Chromium/Office/第三方复杂provider、提升权限窗口、RDP、负Y、旋转和显示器热插拔。当前worker拒绝与宿主PID不同的元素；合法跨进程provider可能回退，尚无跨PID验收。
- 重叠兄弟无通用 UIA Z 顺序，本版保守回退；自绘无 provider 的图形只识别承载面板，仍依赖像素/窗口候选。
- 不缓存目标树，每个查询启动新 helper，有约百毫秒级启动开销；移动期间继续使用即时候选。无响应/慢 provider 或 CPU 高负载可触发正常回退。
- 窗口身份组合显著降低句柄复用风险，但不声称消除极端同进程 HWND 计数回绕后、同线程/类/矩形完全重用的可能；需要更强保证时再引入窗口销毁事件跟踪。
- macOS与Windows共用的候选数组/代数和层级交互需分别回归；Windows包内检查不代表macOS原生验证。
- 普通 Windows exe 构建与验收 feature exe 分别记录。未制作/运行安装器，未验收升级、签名、自动更新或发布。


## v3后续原生验收

上文为v2历史范围。v3已补生产初始化、同一父进程双屏及恢复验收，并修复离屏后回到同像素不发新查询的问题。新增19组原生断言通过；普通/延迟回收注入各20轮分别记录，不能把注入延迟当作正常系统测量。此前静止点失败与测试驱动失败均保留，具体边界见[Windows v3原生验收](windows-native-v3.md)。
