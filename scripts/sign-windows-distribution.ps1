[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^https://[a-z0-9.-]+\.codesigning\.azure\.net/?$')]
    [string]$Endpoint,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9-]{3,64}$')]
    [string]$AccountName,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9-]{3,64}$')]
    [string]$CertificateProfileName,
    [string]$SignToolPath,
    [string]$DlibPath
)

$ErrorActionPreference = 'Stop'

if (-not $SignToolPath) {
    $SignToolPath = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object FullName -Match '\\x64\\signtool\.exe$' |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $DlibPath) {
    $DlibPath = Get-ChildItem 'C:\Program Files', 'C:\Program Files (x86)' -Recurse -Filter Azure.CodeSigning.Dlib.dll -ErrorAction SilentlyContinue |
        Where-Object FullName -Match '\\x64\\Azure\.CodeSigning\.Dlib\.dll$' |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $SignToolPath -or -not (Test-Path -LiteralPath $SignToolPath -PathType Leaf)) { throw 'SignTool x64 non trovato.' }
if (-not $DlibPath -or -not (Test-Path -LiteralPath $DlibPath -PathType Leaf)) { throw 'Artifact Signing Client Dlib x64 non trovato. Installa Microsoft.Azure.ArtifactSigningClientTools.' }

$version = & $SignToolPath 2>&1 | Select-String 'Version' | Select-Object -First 1
if (-not $version) { throw 'Versione SignTool non verificabile.' }

$config = Get-Content -LiteralPath 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json
$releaseVersion = [string]$config.version
$artifacts = @(
    'src-tauri\target\release\local-ai-assistant.exe',
    "src-tauri\target\release\bundle\msi\Local AI Assistant_${releaseVersion}_x64_en-US.msi",
    "src-tauri\target\release\bundle\nsis\Local AI Assistant_${releaseVersion}_x64-setup.exe"
) | ForEach-Object {
    if (-not (Test-Path -LiteralPath $_ -PathType Leaf)) { throw "Artefatto assente: $_" }
    (Resolve-Path -LiteralPath $_).Path
}

$metadataRoot = Join-Path $env:LOCALAPPDATA 'AURA\signing'
New-Item -ItemType Directory -Path $metadataRoot -Force | Out-Null
$metadataPath = Join-Path $metadataRoot 'artifact-signing-metadata.json'
@{
    Endpoint = $Endpoint.TrimEnd('/')
    CodeSigningAccountName = $AccountName
    CertificateProfileName = $CertificateProfileName
    CorrelationId = "aura-$releaseVersion-$([guid]::NewGuid())"
} | ConvertTo-Json | Set-Content -LiteralPath $metadataPath -Encoding UTF8

try {
    foreach ($artifact in $artifacts) {
        & $SignToolPath sign /v /fd SHA256 /tr 'http://timestamp.acs.microsoft.com' /td SHA256 /dlib $DlibPath /dmdf $metadataPath $artifact
        if ($LASTEXITCODE -ne 0) { throw "Artifact Signing fallita per $artifact." }
        & $SignToolPath verify /pa /all /v $artifact | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Verifica Authenticode pubblica fallita per $artifact." }
        $signature = Get-AuthenticodeSignature -LiteralPath $artifact
        if ($signature.Status -ne 'Valid' -or -not $signature.TimeStamperCertificate) {
            throw "Firma o timestamp non validi per $artifact."
        }
        [PSCustomObject]@{
            Path = $artifact
            Status = $signature.Status
            Publisher = $signature.SignerCertificate.Subject
            Thumbprint = $signature.SignerCertificate.Thumbprint
            Timestamped = [bool]$signature.TimeStamperCertificate
            SHA256 = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash
        }
    }
}
finally {
    if (Test-Path -LiteralPath $metadataPath) { Remove-Item -LiteralPath $metadataPath -Force }
}
