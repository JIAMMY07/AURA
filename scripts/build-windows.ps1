param(
  [switch]$Debug,
  [switch]$SkipSigning
)

$ErrorActionPreference = 'Stop'
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
  throw 'Visual Studio Build Tools non trovato.'
}

$installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installPath) {
  throw 'Workload C++ MSVC non trovato.'
}

$devShell = Join-Path $installPath 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll'
Import-Module $devShell
Enter-VsDevShell -VsInstallPath $installPath -SkipAutomaticLocation -DevCmdArguments '-arch=x64' | Out-Null
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$pnpm = (Get-Command pnpm.cmd -ErrorAction SilentlyContinue).Source
if (-not $pnpm) {
  $pnpm = "$env:USERPROFILE\.cache\codex-runtimes\codex-primary-runtime\dependencies\bin\fallback\pnpm.cmd"
}
if (-not (Test-Path -LiteralPath $pnpm)) {
  throw 'pnpm non trovato.'
}

$arguments = @('tauri', 'build')
if ($Debug) { $arguments += '--debug' }
& $pnpm @arguments
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not $Debug -and -not $SkipSigning) {
  & (Join-Path $PSScriptRoot 'sign-windows-artifacts.ps1')
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
