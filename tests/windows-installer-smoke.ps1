#requires -Version 7.0
<#
Run only on an ephemeral GitHub Windows runner, after building the final NSIS:
  ./tests/windows-installer-smoke.ps1 -InstallerPath ./path/Starshot_0.3.0_x64-setup.exe

No GUI, screenshots, clipboard access, or user data are used. OCR uses the existing
original fixed-pixel fixture. Evidence stays in artifacts/, including on failure.

Verified against @tauri-apps/cli 2.12.4 (review these contracts on CLI upgrades):
https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi
https://nsis.sourceforge.io/Docs/Chapter3.html#installerusage
/S is silent; /NS skips shortcuts; omitting /R prevents Tauri's silent auto-launch.
/D= and _?= must be the final, unquoted absolute-path arguments, even with spaces.
_?= keeps the uninstaller in place so we can wait; we remove its own remaining
file after success, as in NSIS's documented example. Silent Tauri uninstall keeps
installer preferences; those are recorded and separately removed only if owned
by this run. This is not a test of interactive uninstall's delete-user-data option.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [string]$EvidenceDirectory = (Join-Path $PSScriptRoot '../artifacts/windows-installer-smoke')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'This install/uninstall check is restricted to an ephemeral GitHub Windows runner.'
}

$repo = Split-Path $PSScriptRoot -Parent
$runId = [Guid]::NewGuid().ToString('N')
$evidence = Join-Path ([IO.Path]::GetFullPath($EvidenceDirectory)) $runId
$sandbox = Join-Path ([IO.Path]::GetTempPath()) "starshot-installer-smoke-$runId"
$installDir = Join-Path $sandbox 'app'
$appExe = Join-Path $installDir 'starshot.exe'
$uninstaller = Join-Path $installDir 'uninstall.exe'
$errors = [Collections.Generic.List[string]]::new()
$report = [ordered]@{
    success = $false; runId = $runId; installDirectory = $installDir
    startedUtc = [DateTime]::UtcNow.ToString('o'); phases = [ordered]@{}
}
$installationStarted = $false
$uninstallSucceeded = $false
New-Item -ItemType Directory -Path $evidence | Out-Null

function Assert-Check([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Read-RegistryKey(
    [Microsoft.Win32.RegistryHive]$Hive,
    [Microsoft.Win32.RegistryView]$View,
    [string]$KeyPath
) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($Hive, $View)
    try {
        $key = $base.OpenSubKey($KeyPath)
        if ($null -eq $key) { return $null }
        try {
            $values = [ordered]@{}
            foreach ($name in $key.GetValueNames()) { $values[$name] = $key.GetValue($name) }
            return [pscustomobject]@{
                hive = "$Hive"; view = "$View"; path = $KeyPath
                values = $values; subKeys = @($key.GetSubKeyNames())
            }
        } finally { $key.Dispose() }
    } finally { $base.Dispose() }
}

function Find-RegistryKeys([string]$KeyPath) {
    foreach ($hive in @('CurrentUser', 'LocalMachine')) {
        foreach ($view in @('Registry64', 'Registry32')) {
            $entry = Read-RegistryKey $hive $view $KeyPath
            if ($null -ne $entry) { $entry }
        }
    }
}

function Stop-IsolatedAppProcesses {
    # Never terminate by product name alone, nor any process from another install.
    foreach ($process in @(Get-Process -Name starshot -ErrorAction SilentlyContinue)) {
        if ($process.Path -and [string]::Equals($process.Path, $appExe, [StringComparison]::OrdinalIgnoreCase)) {
            $entry = [pscustomobject]@{ id = $process.Id; path = $process.Path }
            $process.Kill()
            Assert-Check ($process.WaitForExit(10000)) 'Isolated app process did not stop.'
            $entry
        }
    }
}

function Invoke-Nsis([string]$Executable, [string]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new($Executable, $Arguments)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $process = [Diagnostics.Process]::Start($start)
    if (-not $process.WaitForExit(180000)) {
        $process.Kill()
        $process.WaitForExit()
        throw "NSIS timed out: $Executable"
    }
    Assert-Check ($process.ExitCode -eq 0) "NSIS returned $($process.ExitCode): $Executable"
    return $process.ExitCode
}

function Get-PeArchitecture([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        Assert-Check ($reader.ReadUInt16() -eq 0x5a4d) 'Installed executable is not an MZ binary.'
        $stream.Position = 0x3c
        $offset = $reader.ReadUInt32()
        Assert-Check ($offset -le $stream.Length - 6) 'Invalid PE header offset.'
        $stream.Position = $offset
        Assert-Check ($reader.ReadUInt32() -eq 0x00004550) 'Installed executable has no PE header.'
        switch ($reader.ReadUInt16()) {
            0x8664 { return 'x64' }
            0xaa64 { return 'arm64' }
            0x014c { return 'x86' }
            default { throw 'Unrecognized PE machine architecture.' }
        }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

try {
    $config = Get-Content (Join-Path $repo 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json -AsHashtable
    $cli = Get-Content (Join-Path $repo 'node_modules/@tauri-apps/cli/package.json') -Raw | ConvertFrom-Json
    Assert-Check ($cli.version -eq '2.12.4') 'Review the NSIS CLI contract before testing a different Tauri CLI version.'
    Assert-Check ($config.productName -eq 'Starshot') 'This smoke check is specific to Starshot.'
    $nsis = $config.bundle.windows.nsis
    Assert-Check (-not $nsis.ContainsKey('installMode') -or $nsis.installMode -eq 'currentUser') 'Only currentUser installations are allowed.'
    Assert-Check (-not $nsis.ContainsKey('template') -and -not $nsis.ContainsKey('installerHooks')) 'Custom NSIS templates/hooks need a new isolation review.'
    $publisher = if ($config.bundle.ContainsKey('publisher')) { $config.bundle.publisher } else { $config.identifier.Split('.')[1] }
    $version = $config.version
    $uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Starshot'
    $preferenceKey = "Software\$publisher\Starshot"
    $runKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
    $installer = (Resolve-Path -LiteralPath $InstallerPath).Path
    Assert-Check ((Get-Item -LiteralPath $installer).Name -like '*-setup.exe') 'Supply the final NSIS *-setup.exe.'
    Assert-Check (-not (Test-Path -LiteralPath $sandbox)) 'Unique temporary directory already exists.'
    Assert-Check (@(Find-RegistryKeys $uninstallKey).Count -eq 0) 'A Starshot uninstall entry already exists; refusing to touch it.'
    # The Tauri migration path can discover an older WiX installation under a
    # GUID key, not just the NSIS product-name key. Refuse either kind.
    foreach ($parent in @(Find-RegistryKeys 'Software\Microsoft\Windows\CurrentVersion\Uninstall')) {
        foreach ($child in $parent.subKeys) {
            $entry = Read-RegistryKey $parent.hive $parent.view "$($parent.path)\$child"
            Assert-Check ($null -eq $entry -or $entry.values['DisplayName'] -ne 'Starshot') 'An existing Starshot MSI/other uninstall entry exists; refusing installation.'
        }
    }
    Assert-Check (@(Find-RegistryKeys $preferenceKey).Count -eq 0) 'Starshot installer preferences already exist; refusing to touch them.'
    foreach ($entry in @(Find-RegistryKeys $runKey)) {
        Assert-Check (-not $entry.values.Contains('Starshot')) 'A Starshot autostart entry already exists; refusing to touch it.'
    }
    Assert-Check (@(Get-Process -Name starshot -ErrorAction SilentlyContinue).Count -eq 0) 'A Starshot process already exists; refusing installation.'
    # Match the template's x64 lookup. Never let the installer bootstrap WebView2
    # outside this test's TEMP installation directory.
    $webviewKey = 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $webview = Read-RegistryKey LocalMachine Registry32 $webviewKey
    if ($null -eq $webview -or -not $webview.values['pv']) {
        $webview = Read-RegistryKey CurrentUser Registry64 $webviewKey
    }
    Assert-Check ($null -ne $webview -and [bool]$webview.values['pv']) 'WebView2 must already exist on this runner; refusing an external runtime installation.'
    $report.installer = @{ path = $installer; sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash }
    $report.expected = @{ version = $version; publisher = $publisher; architecture = 'x64'; scope = 'user'; cli = $cli.version }
    $report.webview = $webview
    $report.phases.preflight = 'passed'
    New-Item -ItemType Directory -Path $sandbox | Out-Null

    # This is one raw argument string: /D must remain unquoted and last.
    $arguments = "/S /NS /D=$installDir"
    $report.installArguments = $arguments
    $installationStarted = $true
    $report.installExitCode = Invoke-Nsis $installer $arguments
    $unexpected = @(Stop-IsolatedAppProcesses)
    $report.unexpectedAppProcesses = $unexpected
    Assert-Check ($unexpected.Count -eq 0) 'Silent installation unexpectedly launched the isolated GUI; it was stopped.'
    Assert-Check (Test-Path -LiteralPath $appExe -PathType Leaf) 'Installed starshot.exe is missing.'
    Assert-Check (Test-Path -LiteralPath $uninstaller -PathType Leaf) 'Installed uninstaller is missing.'
    $installedFiles = @(Get-ChildItem -LiteralPath $installDir -Recurse -Force)
    Assert-Check (@($installedFiles | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count -eq 0) 'Installed files contain a reparse point.'
    $report.installedFiles = @($installedFiles | ForEach-Object { $_.FullName.Substring($installDir.Length + 1) })
    $info = [Diagnostics.FileVersionInfo]::GetVersionInfo($appExe)
    $architecture = Get-PeArchitecture $appExe
    $report.executable = @{
        fileVersion = $info.FileVersion; productVersion = $info.ProductVersion
        companyName = $info.CompanyName; productName = $info.ProductName; architecture = $architecture
    }
    Assert-Check ($info.FileVersion -eq $version -and $info.ProductVersion -eq $version) 'Installed executable version differs from tauri.conf.json.'
    Assert-Check ($info.CompanyName -ceq $publisher) 'Installed executable CompanyName differs from the expected publisher.'
    Assert-Check ($info.ProductName -ceq 'Starshot') 'Installed executable ProductName is wrong.'
    Assert-Check ($architecture -eq 'x64') 'This windows-latest check requires the x64 application payload.'
    $entries = @(Find-RegistryKeys $uninstallKey)
    $report.uninstallEntries = $entries
    Assert-Check ($entries.Count -gt 0) 'Installed application has no uninstall registry entry.'
    foreach ($entry in $entries) {
        Assert-Check ($entry.hive -eq 'CurrentUser') 'Installation unexpectedly wrote a machine-wide uninstall entry.'
        Assert-Check ($entry.values['DisplayName'] -ceq 'Starshot') 'Uninstall DisplayName is wrong.'
        Assert-Check ($entry.values['DisplayVersion'] -eq $version) 'Uninstall DisplayVersion is wrong.'
        Assert-Check ($entry.values['Publisher'] -ceq $publisher) 'Uninstall Publisher is wrong.'
        Assert-Check ($entry.values['InstallLocation'] -eq "`"$installDir`"") 'Uninstall InstallLocation is outside the isolated directory.'
        Assert-Check ($entry.values['UninstallString'] -eq "`"$uninstaller`"") 'Uninstall command is not the isolated uninstaller.'
        Assert-Check ($entry.values['MainBinaryName'] -ceq 'starshot.exe') 'Uninstall MainBinaryName is wrong.'
    }
    $report.phases.installMetadata = 'passed'
    & python (Join-Path $PSScriptRoot 'ocr-worker-smoke.py') $appExe *> (Join-Path $evidence 'ocr-worker.log')
    Assert-Check ($LASTEXITCODE -eq 0) "Installed OCR worker failed ($LASTEXITCODE); see ocr-worker.log."
    $report.phases.installedOcrWorker = 'passed'
    $report.manifestEvidence = @{
        Publisher = $info.CompanyName; InstallerType = 'nsis'; Scope = 'user'; Architecture = $architecture
        DisplayName = $entries[0].values['DisplayName']; DisplayVersion = $entries[0].values['DisplayVersion']
        ProductCode = 'Starshot'; InstallerSha256 = $report.installer.sha256
    }
} catch {
    $errors.Add($_.Exception.Message)
} finally {
    if ($installationStarted) {
        try {
            $report.cleanupStoppedAppProcesses = @(Stop-IsolatedAppProcesses)
            if ($report.cleanupStoppedAppProcesses.Count -ne 0) {
                $errors.Add('An unexpected isolated app process remained before uninstall; it was stopped.')
            }
            Assert-Check (Test-Path -LiteralPath $uninstaller -PathType Leaf) 'Cannot clean up: isolated uninstaller is missing.'
            Assert-Check (((Get-Item -LiteralPath $uninstaller).Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) 'Refusing to execute a reparse-point uninstaller.'
            $report.uninstallArguments = "/S _?=$installDir"
            $report.uninstallExitCode = Invoke-Nsis $uninstaller $report.uninstallArguments
            Assert-Check (@(Find-RegistryKeys $uninstallKey).Count -eq 0) 'Uninstaller left an Add/Remove Programs registry entry.'
            Assert-Check (-not (Test-Path -LiteralPath $appExe)) 'Uninstaller left the application executable.'
            # With _?=, NSIS cannot delete the running uninstaller itself. Only
            # that documented leftover is removed by the harness, never a tree.
            $remaining = @(Get-ChildItem -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue)
            $report.filesAfterUninstall = @($remaining | ForEach-Object { $_.FullName })
            Assert-Check (@($remaining | Where-Object { $_.FullName -ne $uninstaller }).Count -eq 0) 'Uninstaller left files other than its own running executable.'
            if (Test-Path -LiteralPath $uninstaller) { Remove-Item -LiteralPath $uninstaller }
            if (Test-Path -LiteralPath $installDir) { [IO.Directory]::Delete($installDir) }
            Assert-Check (-not (Test-Path -LiteralPath $installDir)) 'Isolated installation directory still exists.'

            # Tauri intentionally retains installation preferences on silent
            # uninstall. Preserve this fact in evidence before removing ours.
            $preferences = @(Find-RegistryKeys $preferenceKey)
            $report.retainedInstallerPreferences = $preferences
            foreach ($entry in $preferences) {
                Assert-Check ($entry.hive -eq 'CurrentUser' -and $entry.values[''] -eq $installDir) 'Refusing to clean installer preferences not owned by this run.'
                Assert-Check ($entry.subKeys.Count -eq 0) 'Unexpected nested installer preferences; preserving evidence.'
                Assert-Check (@($entry.values.Keys | Where-Object { $_ -notin @('', 'Installer Language') }).Count -eq 0) 'Unexpected installer preference values; preserving evidence.'
                $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, [Microsoft.Win32.RegistryView]$entry.view)
                try { $base.DeleteSubKey($preferenceKey, $false) } finally { $base.Dispose() }
            }
            Assert-Check (@(Find-RegistryKeys $preferenceKey).Count -eq 0) 'Run-owned installer preferences still exist.'
            foreach ($entry in @(Find-RegistryKeys $runKey)) {
                Assert-Check (-not $entry.values.Contains('Starshot')) 'Starshot autostart entry exists after uninstall.'
            }
            [IO.Directory]::Delete($sandbox)
            $uninstallSucceeded = $true
            $report.phases.uninstall = 'passed'
        } catch {
            $errors.Add("Cleanup: $($_.Exception.Message)")
        }
    }
    $report.success = $errors.Count -eq 0 -and $uninstallSucceeded
    $report.errors = @($errors.ToArray())
    $report.finishedUtc = [DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $evidence 'result.json') -Encoding utf8
}

if (-not $report.success) {
    throw "Windows installer smoke failed. Evidence: $evidence. $($errors -join ' | ')"
}
Write-Host "Windows isolated NSIS install, metadata, fixed-pixel OCR, and uninstall passed. Evidence: $evidence"
