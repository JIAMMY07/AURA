[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Esegui questo script da PowerShell come amministratore. WSL e VirtualMachinePlatform modificano componenti Windows.'
}

$features = @('Microsoft-Windows-Subsystem-Linux', 'VirtualMachinePlatform')
$restartRequired = $false
foreach ($feature in $features) {
    $current = Get-WindowsOptionalFeature -Online -FeatureName $feature
    if ($current.State -ne 'Enabled') {
        $result = Enable-WindowsOptionalFeature -Online -FeatureName $feature -All -NoRestart
        $restartRequired = $restartRequired -or $result.RestartNeeded
    }
}
if ($restartRequired) {
    Write-Warning 'Riavvio necessario. Dopo il riavvio riesegui questo stesso script come amministratore.'
    exit 3010
}

$ubuntu = wsl.exe --list --quiet | ForEach-Object { $_ -replace "`0", '' } | Where-Object { $_.Trim() -eq 'Ubuntu' }
if (-not $ubuntu) {
    wsl.exe --install --distribution Ubuntu --no-launch
    if ($LASTEXITCODE -ne 0) { throw 'Installazione Ubuntu WSL fallita.' }
}

wsl.exe --distribution Ubuntu --user root --exec bash -lc 'export DEBIAN_FRONTEND=noninteractive; apt-get update && apt-get install -y --no-install-recommends bubblewrap ca-certificates git nodejs npm cargo rustc && npm install -g pnpm@9.15.4 && printf AURA_WSL_BWRAP_V1 >/etc/aura-sandbox-version'
if ($LASTEXITCODE -ne 0) { throw 'Provisioning del runtime AURA fallito.' }

wsl.exe --distribution Ubuntu --user root --exec bwrap --unshare-all --die-with-parent --new-session --ro-bind /usr /usr --ro-bind /bin /bin --ro-bind /lib /lib --ro-bind-try /lib64 /lib64 --ro-bind /etc /etc --proc /proc --dev /dev --tmpfs /tmp --chdir /tmp /bin/sh -lc 'test ! -e /mnt/c && touch /tmp/write-test && test -w /tmp'
if ($LASTEXITCODE -ne 0) { throw 'Test di isolamento bubblewrap fallito: Evolution Lab resta disabilitato.' }
Write-Host 'AURA sandbox WSL+bubblewrap installata e test base superato.'
