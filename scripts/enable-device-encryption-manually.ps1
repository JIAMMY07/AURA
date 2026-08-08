[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RecoveryKeyDirectory,

    [Parameter(Mandatory = $true)]
    [ValidateSet('ATTIVA-CIFRATURA')]
    [string]$Confirmation
)

$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Attivazione manuale bloccata: esegui PowerShell come amministratore.'
}

$recovery = (Resolve-Path -LiteralPath $RecoveryKeyDirectory).Path
if ([IO.Path]::GetPathRoot($recovery).TrimEnd('\') -eq 'C:') {
    throw 'La recovery key deve essere salvata su un’unità diversa da C:.'
}
$tpm = Get-Tpm
if (-not $tpm.TpmPresent -or -not $tpm.TpmReady) {
    throw 'TPM assente o non pronto: attivazione bloccata.'
}

$current = & manage-bde.exe -status C:
if ($current -match 'Protezione attivata|Protection On') {
    Write-Host 'La protezione dell’unità C: risulta già attiva.'
    exit 0
}

Write-Warning 'Stai per attivare manualmente la cifratura dell’unità C:. Non spegnere il PC durante la configurazione iniziale.'
& manage-bde.exe -protectors -add C: -RecoveryPassword
if ($LASTEXITCODE -ne 0) { throw 'Creazione del recovery protector fallita.' }

$recoveryFile = Join-Path $recovery "AURA-BitLocker-Recovery-$env:COMPUTERNAME.txt"
& manage-bde.exe -protectors -get C: -Type RecoveryPassword | Out-File -LiteralPath $recoveryFile -Encoding utf8
$acl = Get-Acl -LiteralPath $recoveryFile
$acl.SetAccessRuleProtection($true, $false)
$rule = New-Object Security.AccessControl.FileSystemAccessRule($env:USERNAME, 'FullControl', 'Allow')
$acl.SetAccessRule($rule)
Set-Acl -LiteralPath $recoveryFile -AclObject $acl

& manage-bde.exe -on C: -UsedSpaceOnly -SkipHardwareTest
if ($LASTEXITCODE -ne 0) {
    throw "Avvio cifratura fallito. Conserva comunque la recovery key creata in $recoveryFile."
}
Write-Host "Cifratura avviata manualmente. Recovery key: $recoveryFile"
