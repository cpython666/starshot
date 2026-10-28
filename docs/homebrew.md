# 星镜 Homebrew 分发

星镜使用已有的公开安装源 [cpython666/homebrew-tap](https://github.com/cpython666/homebrew-tap)，目标文件为 `Casks/starshot.rb`；现有 `Casks/starclip.rb` 独立保留。2026-10-26 核对远端默认分支为 `main`，基线提交为 `e11f8f06a43e4754855a8f101a39bc712aebdf08`，仓库没有额外 AGENTS 或发布工作流。

[v0.3.1](https://github.com/cpython666/starshot/releases/tag/v0.3.1)已于2026-10-28北京时间公开并成为应用内最新更新。Tap的 `Casks/starshot.rb` 与安装说明已同步，最终提交为 [`98abcab`](https://github.com/cpython666/homebrew-tap/commit/98abcab34c228aa64d6e4faf68165b37a88c6bcb)，可执行 `brew install --cask cpython666/tap/starshot`。仅提供Apple Silicon、macOS 12+；Windows使用独立NSIS安装包。

## 生成 Cask

使用 `scripts/homebrew-cask.mjs` 从最终 DMG 计算真实 SHA-256，再输出安装描述。没有 DMG 时不生成占位 Cask，不使用 `sha256 :no_check`。命令会核对仓库六处版本、DMG 文件名、非空文件和 UDIF 标识；这些检查不能替代 DMG 挂载、应用版本/架构、签名完整性或安装实测。

```sh
node scripts/homebrew-cask.mjs \
  artifacts/release-0.3.1/Starshot_0.3.1_aarch64.dmg \
  artifacts/homebrew-tap/Casks/starshot.rb
```

路径中的 `artifacts/release-0.3.1` 是示例，替换为最终核验包所在目录。生成器输出版本、字节数、SHA-256 和文件位置；如果项目最低 macOS 版本不再是当前的 12.0，会停止并要求同步 Cask 的平台要求。

Cask 固定到该版本的 GitHub Release 附件，声明 Apple Silicon 和 macOS Monterey 12 起，不提供 Intel 包；保留尚未 Apple 公证的安装提示。不执行安装脚本，不移除 quarantine，不设置删除本地编辑恢复点的 `zap`。`livecheck`显式使用`github_latest`策略，与应用内稳定更新通道一致；不能使用默认Git标签检测，否则会把构建依赖`ocr-models-v1`误识别为应用版本1。

星镜可以在应用内检查、下载和安装更新，因此声明 `auto_updates true`。Homebrew 将这类应用视为具备自更新能力，即使检查更新由用户手动触发。[Cask 字段说明](https://docs.brew.sh/Cask-Cookbook)

## 发布与检查顺序

1. 完成最终 DMG 的版本、架构、SHA-256 和签名检查；只从这份文件生成 `starshot.rb`。
2. 在星镜仓库公开对应 Release 和同名附件，确认匿名下载可用且下载字节与本机核验一致。
3. 在 Tap 的工作副本加入生成的 Cask，并更新 Tap README 的星镜安装入口、版本与平台签名状态；保留星笺的说明与安装描述。
4. 执行 Ruby 语法及 Homebrew style、online audit、fetch；安装验收需在隔离目标目录进行，不能覆盖用户已有应用。预览 Release 可能触发 prerelease 审查提示，应如实记录。
5. 核对最终差异后提交、推送 Tap，核验远端 `Casks/starshot.rb` 与本地一致。未完成这一步不能宣称一行安装已上线。

```sh
ruby -c artifacts/homebrew-tap/Casks/starshot.rb
HOMEBREW_DEVELOPER=1 HOMEBREW_NO_AUTO_UPDATE=1 brew style artifacts/homebrew-tap/Casks/starshot.rb
```

将已审阅 Cask 同步到实际 Tap 后，再针对包名进行完整检查：

```sh
HOMEBREW_DEVELOPER=1 HOMEBREW_NO_AUTO_UPDATE=1 brew audit --cask --strict --online cpython666/tap/starshot
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --cask cpython666/tap/starshot
```

`HOMEBREW_DEVELOPER=1` 仅作用于该命令，避免 developer 命令自动保存全局开发者模式。当前系统已安装 Tap 的路径是 `/opt/homebrew/Library/Taps/cpython666/homebrew-tap`；准备工作使用本仓库忽略的 `artifacts/homebrew-tap` 克隆，origin 指向 GitHub，不能将本地克隆误当作已安装的 Tap。

Homebrew 自有 Tap 通过 Git 仓库维护；公开包及安装描述上线后，用户可执行下列命令。[Tap 维护与使用](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)

```sh
brew install --cask cpython666/tap/starshot
brew update
brew upgrade --cask --greedy cpython666/tap/starshot
brew uninstall --cask cpython666/tap/starshot
```

`--greedy` 将声明自更新的 Cask 纳入 Homebrew 升级。升级前应导出或备份当前编辑并退出星镜；不使用 `--force` 覆盖已有手动安装的版本。普通卸载只管理应用，不主动删除星镜恢复点。[Homebrew 命令说明](https://docs.brew.sh/Manpage)

GitHub 的 prerelease 可以通过固定 tag 下载，但不能成为 latest release。`v0.3.0`当时只提供固定tag预览下载；`v0.3.1`现已成为`/releases/latest/download/latest.json`的公开目标。[GitHub Release 规则](https://docs.github.com/en/rest/releases/releases#create-a-release)

## 0.3.1 已验证结果

- 最终DMG为5,019,285字节，SHA-256为 `bbd926e229248afa8533dafa31755060ed216029d45b1b2d4a7a7810b12a86ee`。CI产物、公开下载、生成的Cask及Homebrew实际缓存一致。
- Ruby语法、`brew style`、`brew livecheck`、在线严格`brew audit`、`brew fetch`均通过。初次audit发现默认livecheck误读构建依赖标签为版本1，改用`github_latest`后实际结果为`0.3.1 ==> 0.3.1`；不是跳过检查。
- Homebrew实际安装到忽略目录`artifacts/homebrew-install-0.3.1/`，确认登记版本0.3.1、正式应用标识、arm64和严格codesign完整性；卸载后该应用与Cask安装登记均已移除。设置`HOMEBREW_NO_INSTALL_CLEANUP=1`，未启动应用、移除quarantine或覆盖用户已有程序。
- Tap最终公开提交98abcab；本机实际Tap与工作克隆中的Cask一致。证据在`artifacts/release-0.3.1/`的brew日志及`brew-install-verification.json`，双平台发行和真实旧版更新检查见[验证记录](verification.md)。

## 0.3.0 已验证结果

- 最终包 `Starshot_0.3.0_aarch64.dmg` 为4,976,926字节，SHA-256为 `546824f9ebf3bcda889fc7ac841b23d6ea530aa473bb01bf9b25863b1e11aa4d`；生成Cask和匿名公开下载的文件均匹配该值。
- DMG只读挂载核对版本/构建号0.3.0、标识com.starshot.desktop、arm64、附带模型许可；`codesign --verify --deep --strict`通过。这是ad-hoc签名，不代表Apple公证。
- 最终Cask的Ruby语法、`brew style`、`brew fetch --cask cpython666/tap/starshot`通过。在线严格audit返回1，唯一审查项为 `v0.3.0 is a GitHub pre-release.`；因此不记为audit全部通过，也不为消除提示改成稳定发行。
- Homebrew实际安装到Git忽略的 `artifacts/homebrew-install-0.3.0/`，使用 `brew info --cask --json=v2`确认安装版本，读取应用Info.plist并严格校验签名；卸载后应用及Cask安装登记均已移除。没有启动应用、移除quarantine或替换用户已有应用。后续隔离测试设置 `HOMEBREW_NO_INSTALL_CLEANUP=1`，避免安装触发缓存清理。
- 本机实际Tap已快进到公开提交，Cask与生成文件逐字节一致。证据在忽略的 `artifacts/release-0.3.0/`，包括 `brew-install-verification.json`、下载及audit日志；双平台发行证据见[验证记录](verification.md)。

## Windows / WinGet 后续

WinGet 接入是单独的清单和审核流程，不由 Homebrew Tap 覆盖。Microsoft 要求清单具备准确的发布者、应用许可、版本、安装 URL、SHA-256 和安装类型；NSIS 对应 `nullsoft`。URL 应指向发布者自己的公开安装包。[清单格式](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)、[仓库政策](https://learn.microsoft.com/en-us/windows/package-manager/package/windows-package-manager-policies)

当前星镜根目录的 `LICENSE` 已声明应用使用 MIT，第三方依赖和 OCR 模型保留各自许可；应引用应用许可，不套用 Tap 自身的许可。0.3.0最终EXE已在Windows CI中实际隔离安装/卸载：发行者 `starshot`、版本0.3.0、x64、当前用户安装，卸载项及ProductCode为 `Starshot`；安装后的离线OCR worker也通过。后续可据这些实际证据用 `wingetcreate new` 或官方 YAMLCreate 生成本地清单，不选择自动提交。

生成后在 Windows 上执行 `winget validate <清单目录>`，并用官方 SandboxTest 检查静默安装、卸载和权限场景。提交 Microsoft 社区仓库的 PR 后还需自动及人工审核；本轮未创建外部 PR，也未将缺字段的模板标成可上架清单。[提交和验收流程](https://learn.microsoft.com/en-us/windows/package-manager/package/repository)
