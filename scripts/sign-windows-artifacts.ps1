[CmdletBinding()]
param(
    [switch]$CreateLocalCertificate,
    [switch]$TrustLocalCertificate,
    [switch]$IncludeHistoricalArtifacts
)

$ErrorActionPreference = 'Stop'
$subject = 'CN=AURA Local Development Code Signing'
$certificate = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object Subject -eq $subject |
    Where-Object { $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date).AddDays(30) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

if (-not $certificate -and $CreateLocalCertificate) {
    $certificate = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $subject `
        -CertStoreLocation Cert:\CurrentUser\My `
        -KeyAlgorithm RSA `
        -KeyLength 3072 `
        -HashAlgorithm SHA256 `
        -KeyExportPolicy NonExportable `
        -NotAfter (Get-Date).AddYears(2)
}
if (-not $certificate) {
    throw 'Certificato di firma assente. Usa -CreateLocalCertificate oppure configura un certificato pubblico.'
}

# Verifica sempre i trust store: una vecchia esecuzione può aver creato il
# certificato senza completare uno dei due import.
$publicPath = Join-Path $env:LOCALAPPDATA 'AURA\signing\aura-local-code-signing.cer'
New-Item -ItemType Directory -Force -Path (Split-Path $publicPath) | Out-Null
Export-Certificate -Cert $certificate -FilePath $publicPath -Force | Out-Null
foreach ($store in @('Cert:\CurrentUser\TrustedPublisher') + $(if ($TrustLocalCertificate) { 'Cert:\CurrentUser\Root' })) {
    $trusted = Get-ChildItem $store | Where-Object Thumbprint -eq $certificate.Thumbprint
    if (-not $trusted) { Import-Certificate -FilePath $publicPath -CertStoreLocation $store | Out-Null }
}

$roots = if ($IncludeHistoricalArtifacts) {
    @('src-tauri\target\release')
} else {
    @('src-tauri\target\release\local-ai-assistant.exe')
    foreach ($bundle in @('src-tauri\target\release\bundle\msi', 'src-tauri\target\release\bundle\nsis')) {
        Get-ChildItem -LiteralPath $bundle -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1 -ExpandProperty FullName
    }
}
$artifacts = foreach ($root in $roots) {
    if (Test-Path -LiteralPath $root -PathType Leaf) {
        Get-Item -LiteralPath $root
    } elseif (Test-Path -LiteralPath $root -PathType Container) {
        Get-ChildItem -LiteralPath $root -Recurse -File -Include *.exe,*.msi
    }
}
if (-not $artifacts) { throw 'Nessun artefatto Windows da firmare.' }

foreach ($artifact in $artifacts) {
    $signature = Set-AuthenticodeSignature -LiteralPath $artifact.FullName -Certificate $certificate -HashAlgorithm SHA256
    if ($signature.Status -notin @('Valid', 'UnknownError')) {
        throw "Firma fallita per $($artifact.FullName): $($signature.Status)"
    }
    $verified = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
    [PSCustomObject]@{
        Path = $artifact.FullName
        Status = $verified.Status
        Thumbprint = $certificate.Thumbprint
        TrustScope = if ($verified.Status -eq 'Valid') { 'CurrentUser trusted' } else { 'Signed, local root approval pending; non-public' }
    }
}
