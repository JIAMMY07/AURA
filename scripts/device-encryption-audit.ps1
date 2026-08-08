[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Esegui questo script da PowerShell come amministratore.'
}

& manage-bde.exe -status C:
$tpm = Get-Tpm
[PSCustomObject]@{
    AuditOnly = $true
    AutomaticEnablement = $false
    TpmPresent = $tpm.TpmPresent
    TpmReady = $tpm.TpmReady
    ManualActivationScript = (Join-Path $PSScriptRoot 'enable-device-encryption-manually.ps1')
}
