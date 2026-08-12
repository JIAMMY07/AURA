[CmdletBinding()]
param(
    [switch]$DebugBuild,
    [switch]$RequirePublicSignature
)

$ErrorActionPreference = 'Stop'
$config = Get-Content -LiteralPath 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json
$version = [string]$config.version
$configuration = if ($DebugBuild) { 'debug' } else { 'release' }
$root = "src-tauri\target\$configuration"
$artifacts = @(
    "$root\local-ai-assistant.exe",
    "$root\bundle\msi\Local AI Assistant_${version}_x64_en-US.msi",
    "$root\bundle\nsis\Local AI Assistant_${version}_x64-setup.exe"
)

$results = foreach ($artifact in $artifacts) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "Artefatto assente: $artifact" }
    $file = Get-Item -LiteralPath $artifact
    $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName
    $signed = $null -ne $signature.SignerCertificate -and $signature.Status -ne 'NotSigned'
    $localDevelopment = $signed -and $signature.SignerCertificate.Subject -like '*AURA Local Development*'
    $public = $signature.Status -eq 'Valid' -and -not $localDevelopment -and $null -ne $signature.TimeStamperCertificate
    if (-not $signed) { throw "Artefatto non firmato: $artifact" }
    if ($RequirePublicSignature -and -not $public) { throw "Firma pubblica attendibile e timestamp richiesta: $artifact" }
    [pscustomobject]@{
        Path = $file.FullName
        Bytes = $file.Length
        SHA256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        Authenticode = [string]$signature.Status
        Publisher = [string]$signature.SignerCertificate.Subject
        Timestamped = $null -ne $signature.TimeStamperCertificate
        PubliclyTrusted = $public
    }
}

$manifestRoot = Join-Path $env:LOCALAPPDATA 'AURA\signing'
New-Item -ItemType Directory -Path $manifestRoot -Force | Out-Null
$manifest = Join-Path $manifestRoot "distribution-$version-$configuration.json"
$results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $manifest -Encoding UTF8
$results | Format-Table -AutoSize
Write-Host "Manifest di verifica: $manifest"
