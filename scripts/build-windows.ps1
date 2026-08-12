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
$signingConfigPath = $null
if (-not $Debug -and -not $SkipSigning) {
  $certificate = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object Subject -eq 'CN=AURA Local Development Code Signing' |
    Where-Object { $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date).AddDays(30) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1
  if (-not $certificate) {
    throw 'Certificato locale AURA assente: esegui prima sign-windows-artifacts.ps1 -CreateLocalCertificate.'
  }
  $signingConfigPath = Join-Path $env:TEMP ("aura-tauri-signing-$([guid]::NewGuid().ToString('N')).json")
  $signingConfig = @{
    bundle = @{
      windows = @{
        certificateThumbprint = $certificate.Thumbprint
        digestAlgorithm = 'sha256'
        timestampUrl = $null
      }
    }
  } | ConvertTo-Json -Depth 5
  [IO.File]::WriteAllText($signingConfigPath, $signingConfig, [Text.UTF8Encoding]::new($false))
  $arguments += @('--config', $signingConfigPath)
}

try {
  & $pnpm @arguments
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  if (-not $Debug -and -not $SkipSigning) {
    # Verifica e firma nuovamente gli artefatti esterni. La configurazione Tauri
    # sopra è ciò che garantisce che anche l'EXE incluso nei bundle sia firmato.
    & (Join-Path $PSScriptRoot 'sign-windows-artifacts.ps1')
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  }
}
finally {
  if ($signingConfigPath -and (Test-Path -LiteralPath $signingConfigPath)) {
    Remove-Item -LiteralPath $signingConfigPath -Force
  }
}
