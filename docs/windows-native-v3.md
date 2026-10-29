# Windows v3 生产初始化与同进程恢复验收

基线为 `starshot-selection-integrated-v3-20261029`，126个文件哈希已核对。保留Mac端平台依赖/资源隔离、贴图光圈、提示、退出保护和长截图改动。仅交增量补丁，未安装或发布。

## 已定位并修复

`CaptureOverlay.vue`原先仅按坐标/Shift是否变化决定重新查询。离屏会取消请求，但返回同屏的同一像素时，`moved=false`，不会重新查询。失败原生记录显示代数继续递增、框仍可见，却没有当前代数的语义结果。现在单独记录指针是否位于覆盖层；重新进入也视为移动，清理键盘候选路径并重新查询。离开时取消尚未执行的rAF移动，避免旧屏重新启动查询。没有扩大同PID限制或调整250ms槽等待、750ms helper期限。

普通构建明确拒绝三个验收启动参数，避免旧验收命令意外打开日用应用；正常UIA/OCR worker入口保留。

## 实际覆盖的路径

新增 `desktop/windows_integration.rs` 只在Windows且启用 `windows-uia-smoke` 时编译，调用同一个生产 `begin_capture`。显示器枚举、Win32窗口快照/Z顺序、宿主选择、Frame、两个覆盖层的创建/定位和get_capture_element的取消/身份复核均复用生产代码。仅在 `capture_pixels` 边界注入原创棋盘像素，不调用真实capture_image。隔离WebView数据目录、采样IPC代理、限定夹具点位和诊断延迟只用于验收；生产handler没有测试命令。

测试IPC代理调用真实生产handler并记录几何/代数，不返回模拟UIA结果。仅允许原创夹具PID成为查询目标；意外鼠标移出夹具会使样本失败，不会探查个人应用控件。窗口快照只读几何/身份，不读标题、Name、Value或Text。

## 新矩阵结果

Windows 11 25H2 x64，左屏2560×1600/150%/X=-2560、主屏2560×1440/100%。最终同一Tauri父进程PID 46728完成199条控制命令，19组断言通过：

- 20次独立截图会话同时创建双屏覆盖层，20/20静止启动得到当前语义候选；非指针屏初始查询数为0。实际覆盖层位置/尺寸与显示器物理边界相符，宿主中自身PID数为0。
- 20轮普通跨屏往返，加20轮延迟回收故障注入；每轮4次真实物理指针跳转，最后静止后必须获得当前代数的新候选。两组均20/20，CSS回读四边最大误差约0.096物理像素。
- 普通组观察到41次取消，旧helper在新请求到达前均已回收，不能把这组声称为槽争用证明。第二组只在验收feature内让取消后的回收延迟150ms，实际观察到20次新请求先于旧helper回收、20次取消；全部恢复，未出现busy返回。这个注入不代表真实操作系统回收耗时，也不进入普通构建。
- 前后原创窗口遮挡按生产Z顺序选中前窗；隐藏前窗后命中后窗。目标RecreateHandle后旧Frame拒绝旧身份，新会话命中新句柄。夹具重建会把它提到覆盖层上方，测试明确重新提升自有覆盖层后再发指针事件；不声称覆盖层能自动压住所有新出现的置顶应用。
- 同一父进程10次阻塞provider超时后各查询健康控件，以及10次取消后各查询健康控件，20/20恢复，未重启父进程或健康夹具。恢复组由这个仍存活Tauri父进程的验收控制线程直接调用生产windows::query_reply，取消同样使用生产代数检查；没有另起probe父进程。前端IPC与Frame复核由双屏/旧矩阵分别覆盖，恢复组不宣称每次都经前端触发。故障provider是另一个原创进程；不要求已被故意永久阻塞的目标自行恢复。
- 超时761–774ms，取消67–83ms，随后健康请求100–122ms。全部已完成的原生前端查询最大约229.758ms。helper峰值1，每次恢复后0；采样线程25–27、句柄405–410，恢复组私有提交约10.60–11.27MB，未见持续累积。
- 186次预声明指针移动有实际物理坐标证据，48次像素源替换。夹具、主UI线程和查询helper记录有效DPI上下文。坐标为有符号物理像素/半开矩形，CSS回读允许1px舍入。

## 失败记录和限制

早期旧矩阵和新矩阵的首次静止样本曾读到与测试预设点不同的指针坐标（包括窗口上缘和屏幕原点）。测试定位改为SetPhysicalCursorPos，并记录移动后与Frame读取时的实际物理指针/DPI，最终20次通过；旧失败样本保留。之前没有移动后采样，不能由最终成功证明所有历史无框现象都具有同一个根因。

验收驱动开发时还修复了诊断持Frame锁调用UI线程getter的互等、试图覆盖只读Tauri invoke导致记录为空，以及本地控制文件的短暂Windows共享冲突。均为测试代码问题，失败过程未计为通过。

未验证真实屏幕像素采集、复制/保存/安装/升级、真实Chrome/Office/高权限或跨PID provider、负Y/旋转/热插拔、长时间稳定性。公共Vue改动仍需Mac原生回归。本轮不新增Snipaste逆向或运行结论。

## 复现

先执行项目基础检查并构建前端；完成编译后串行运行，避免编译负载污染耗时。原生测试会在原创覆盖层内移动物理鼠标，期间不要操作鼠标。

```powershell
./tests/build-windows-uia-fixture.ps1
cargo build --locked --manifest-path src-tauri/Cargo.toml --features windows-uia-smoke,tauri/custom-protocol --bin starshot
python tests/windows-uia-acceptance.py src-tauri/target/debug/starshot.exe
python tests/windows-integration-acceptance.py src-tauri/target/debug/starshot.exe
```

新脚本每次保存独立artifacts目录，记录父/helper PID、请求/回收时间线、预期/实际矩形、线程/句柄/内存和失败数；断言有明确样本数。最终报告包含去重后的精简JSON和各构建SHA。普通release/NSIS的具体结果见同批验证记录。
