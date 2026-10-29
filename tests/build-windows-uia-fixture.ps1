param([string]$Output = (Join-Path $PSScriptRoot '../artifacts/windows-uia/fixture.exe'))
$ErrorActionPreference = 'Stop'
$framework = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319'
$Output = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path $Output) | Out-Null
$manifest = Join-Path $PSScriptRoot 'windows-uia-fixture.manifest'
& "$framework/csc.exe" /nologo /target:winexe /platform:x64 /out:$Output /win32manifest:$manifest /r:System.Windows.Forms.dll /r:System.Drawing.dll /r:System.Web.Extensions.dll /r:System.Xaml.dll /r:"$framework/WPF/WindowsBase.dll" /r:"$framework/WPF/PresentationFramework.dll" /r:"$framework/WPF/PresentationCore.dll" /r:"$framework/WPF/WindowsFormsIntegration.dll" /r:"$framework/WPF/UIAutomationTypes.dll" /r:"$framework/WPF/UIAutomationProvider.dll" (Join-Path $PSScriptRoot 'windows-uia-fixture.cs')
if ($LASTEXITCODE -ne 0) { throw 'Fixture compilation failed' }
Write-Output $Output
