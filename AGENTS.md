# 星镜协作约定

- 产品为「星镜 Starshot」，独立于星笺；中文、星空主题、本地优先，macOS 与 Windows 同等重要。新产品不重复开发星伴。
- 工作前先读 .agents/skills/starshot/SKILL.md，再按需读 README.md、docs/product.md 与 docs/verification.md。先检查工作区，保留用户改动。分支默认 codex/，提交说明中文；没有用户授权不提交、推送、发布或安装。
- Rust / Tauri 2 / Vue 3 / TypeScript。截图、标注、美化、贴图都在本机完成；不自动截图、不读取或监听剪贴板、不上传图片。
- 不把真实屏幕、剪贴内容、图片、日志、凭据加入 Git。测试只用 public/demo-workspace.svg 或测试代码生成的原创固定像素。不要为测试捕获真实桌面。
- 所有保存、复制、导出给出真实结果；浏览器下载仅能确认请求，不能宣称知道用户下载目录。
- 区分源码实现、自动检查、浏览器验证、平台构建与原生实测。不能把原生窗口边界/像素边缘启发式称为系统控件语义识别。
- 行为变化运行 npm test、npm run build、cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features。桌面修改还需cargo check、desktop lib tests、cargo fmt检查和平台构建。
- 更新已确认决策和限制到文档；有新事实主动记录，没有新结论不堆叠。截图仅保存在Git忽略的artifacts/，公开文档只引用原创例图。
- 磁盘不足不能清理其他项目或个人文件，改用已授权的CI构建产物。
