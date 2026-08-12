[CmdletBinding()]
param(
  [switch]$InstallPrerequisites,
  [switch]$InstallVoice,
  [switch]$InstallModels,
  [switch]$SkipTests,
  [switch]$NoInstall
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:OS -ne 'Windows_NT') {
  throw 'AURA desktop supporta attualmente Windows.'
}
if (-not [Environment]::Is64BitOperatingSystem) {
  throw 'È richiesto Windows x64.'
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repositoryRoot

function Test-Command([string]$Name) {
  return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Install-WingetPackage([string]$Id, [string[]]$Extra = @()) {
  if (-not (Test-Command 'winget.exe')) {
    throw "winget non disponibile: installa manualmente $Id e riesegui lo script."
  }
  & winget.exe install --id $Id --exact --accept-package-agreements --accept-source-agreements @Extra
  if ($LASTEXITCODE -ne 0) { throw "Installazione di $Id fallita." }
}

$missing = [System.Collections.Generic.List[string]]::new()
if (-not (Test-Command 'node.exe')) { $missing.Add('Node.js LTS') }
if (-not (Test-Command 'pnpm.cmd')) { $missing.Add('pnpm 11') }
if (-not (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) { $missing.Add('Rust stable MSVC') }
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$hasBuildTools = Test-Path -LiteralPath $vswhere
if ($hasBuildTools) {
  $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  $hasBuildTools = [bool]$vsPath
}
if (-not $hasBuildTools) { $missing.Add('Visual Studio 2022 Build Tools C++') }

if ($missing.Count -and -not $InstallPrerequisites) {
  throw "Prerequisiti mancanti: $($missing -join ', '). Riesegui con -InstallPrerequisites dopo aver approvato le installazioni."
}

if (-not (Test-Command 'node.exe')) { Install-WingetPackage 'OpenJS.NodeJS.LTS' }
if (-not (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) { Install-WingetPackage 'Rustlang.Rustup' }
if (-not $hasBuildTools) {
  Install-WingetPackage 'Microsoft.VisualStudio.2022.BuildTools' @(
    '--override', '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
  )
}

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\nodejs;$env:APPDATA\npm;$env:Path"
if (-not (Test-Command 'pnpm.cmd')) {
  if (-not (Test-Command 'npm.cmd')) { throw 'npm non disponibile dopo l’installazione di Node.js.' }
  & npm.cmd install --global pnpm@11.16.0
  if ($LASTEXITCODE -ne 0) { throw 'Installazione pnpm fallita.' }
}

& pnpm.cmd install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { throw 'Installazione dipendenze fallita.' }

if (-not $SkipTests) {
  & pnpm.cmd test
  if ($LASTEXITCODE -ne 0) { throw 'Test frontend/Alexa falliti.' }
  & "$env:USERPROFILE\.cargo\bin\cargo.exe" test --manifest-path src-tauri\Cargo.toml
  if ($LASTEXITCODE -ne 0) { throw 'Test Rust falliti.' }
}

& (Join-Path $PSScriptRoot 'build-windows.ps1') -SkipSigning
if ($LASTEXITCODE -ne 0) { throw 'Build Windows fallita.' }

if ($InstallVoice) {
  & (Join-Path $PSScriptRoot 'setup-whisper.ps1')
}

if ($InstallModels) {
  $ollama = Get-Command 'ollama.exe' -ErrorAction SilentlyContinue
  if (-not $ollama -and $InstallPrerequisites) {
    Install-WingetPackage 'Ollama.Ollama'
    $env:Path = "$env:LOCALAPPDATA\Programs\Ollama;$env:Path"
    $ollama = Get-Command 'ollama.exe' -ErrorAction SilentlyContinue
  }
  if (-not $ollama) { throw 'Ollama non trovato. Installa Ollama o usa -InstallPrerequisites.' }
  & (Join-Path $PSScriptRoot 'setup-ollama-models.ps1') -OllamaPath $ollama.Source -ApplyRtx2060Workaround:$false
}

$installer = Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'src-tauri\target\release\bundle\nsis') -Filter '*-setup.exe' |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw 'Installer NSIS non trovato dopo la build.' }
$hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash
Write-Host "Installer creato: $($installer.FullName)"
Write-Host "SHA-256: $hash"

if (-not $NoInstall) {
  $result = Start-Process -FilePath $installer.FullName -ArgumentList '/S' -Wait -PassThru
  if ($result.ExitCode -ne 0) { throw "Installazione AURA fallita con codice $($result.ExitCode)." }
  Write-Host 'AURA installata. Avviala dal menu Start: Local AI Assistant.'
}
