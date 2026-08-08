[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectPath,
    [Parameter(Mandatory = $true)]
    [string]$PatchPath,
    [ValidateSet('Static', 'Node', 'Rust')]
    [string]$TestProfile = 'Node',
    [ValidatePattern('^[a-f0-9-]{36}$')]
    [string]$RunId = ([guid]::NewGuid().ToString())
)

$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath $ProjectPath).Path
$patch = (Resolve-Path -LiteralPath $PatchPath).Path
if (-not (Test-Path -LiteralPath (Join-Path $project '.git'))) { throw 'Il progetto deve essere un repository Git.' }
if ((Get-Item -LiteralPath $patch).Length -gt 5MB) { throw 'Patch oltre il limite di 5 MiB.' }
if ([IO.Path]::GetExtension($patch).ToLowerInvariant() -notin @('.patch', '.diff')) { throw 'Formato patch non valido.' }

$marker = wsl.exe --distribution Ubuntu --user root --exec sh -lc 'cat /etc/aura-sandbox-version 2>/dev/null' 2>$null
if (($marker -replace "`0", '').Trim() -ne 'AURA_WSL_BWRAP_V1') {
    throw 'Runtime WSL AURA non verificato. Esegui setup-evolution-sandbox.ps1 come amministratore.'
}

function Convert-ToWslPath([string]$Path) {
    $value = wsl.exe --distribution Ubuntu --exec wslpath -a $Path
    if ($LASTEXITCODE -ne 0) { throw "Conversione percorso WSL fallita: $Path" }
    $value = ($value -replace "`0", '').Trim()
    if (-not $value.StartsWith('/mnt/')) { throw 'Il runner accetta soltanto percorsi host espliciti convertiti da WSL.' }
    return $value
}

$runRoot = Join-Path $env:LOCALAPPDATA "AURA\evolution\runs\$RunId"
if (Test-Path -LiteralPath $runRoot) { throw 'RunId Evolution già esistente.' }
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
$projectWsl = Convert-ToWslPath $project
$patchWsl = Convert-ToWslPath $patch
$outputWsl = Convert-ToWslPath $runRoot

$cargoRegistry = Join-Path $env:USERPROFILE '.cargo\registry'
$cargoArgs = @()
if ($TestProfile -eq 'Rust') {
    if (-not (Test-Path -LiteralPath $cargoRegistry)) { throw 'Cache Cargo assente: test Rust offline impossibile.' }
    $cargoWsl = Convert-ToWslPath $cargoRegistry
    $cargoArgs = @('--ro-bind', $cargoWsl, '/cargo-registry')
}

$nodeArgs = @()
if ($TestProfile -eq 'Node') {
    $nodeModules = Join-Path $project 'node_modules'
    if (-not (Test-Path -LiteralPath (Join-Path $nodeModules 'typescript\bin\tsc'))) { throw 'Dipendenze Node locali assenti: esegui pnpm install prima del test isolato.' }
    $nodeModulesWsl = Convert-ToWslPath $nodeModules
    $nodeArgs = @('--ro-bind', $nodeModulesWsl, '/node-modules')
}

$testCommand = if ($TestProfile -eq 'Rust') {
    'mkdir -p /work/cargo; cp -a /cargo-registry /work/cargo/registry; export CARGO_HOME=/work/cargo; cargo test --offline --manifest-path src-tauri/Cargo.toml'
} elseif ($TestProfile -eq 'Node') {
    'ln -s /node-modules node_modules; node node_modules/typescript/bin/tsc --noEmit'
} else {
    'git diff --check; test -n "$(git diff --name-only)"'
}
$script = @"
set -eu
ulimit -t 600
ulimit -v 8388608
ulimit -u 256
ulimit -f 262144
ulimit -n 256
mkdir -p /work/project
(cd /input && tar --exclude=.git --exclude=node_modules --exclude=target -cf - .) | (cd /work/project && tar -xf -)
cd /work/project
git init -q
git config user.name 'AURA Sandbox'
git config user.email 'sandbox@invalid.local'
git add -A
git commit -qm baseline
git apply --check /proposal.patch
git apply /proposal.patch
git diff --check
if git diff --name-only | grep -E '(^|/)(\.env|id_rsa|credentials|secrets?)(\.|/|$)' >/dev/null; then exit 41; fi
$testCommand
git diff --binary > /output/validated.patch
sha256sum /output/validated.patch > /output/validated.patch.sha256
printf '%s\n' '$TestProfile' > /output/test-profile.txt
"@

$arguments = @(
    '--distribution','Ubuntu','--user','root','--exec','bwrap',
    '--unshare-all','--die-with-parent','--new-session',
    '--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib',
    '--ro-bind-try','/lib64','/lib64','--ro-bind','/etc','/etc','--proc','/proc','--dev','/dev',
    '--tmpfs','/tmp','--tmpfs','/work',
    '--ro-bind',$projectWsl,'/input','--ro-bind',$patchWsl,'/proposal.patch','--bind',$outputWsl,'/output'
) + $cargoArgs + $nodeArgs + @('--chdir','/work','/bin/sh','-lc',$script)

$log = Join-Path $runRoot 'runner.log'
& wsl.exe @arguments 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Test sandbox fallito con codice $LASTEXITCODE. Nessuna promozione consentita." }
$validated = Join-Path $runRoot 'validated.patch'
if (-not (Test-Path -LiteralPath $validated)) { throw 'Patch validata non prodotta.' }
[PSCustomObject]@{ RunId=$RunId; Status='validated'; TestProfile=$TestProfile; Patch=$validated; Log=$log }
