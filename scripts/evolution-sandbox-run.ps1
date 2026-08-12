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

$strictPreference = $ErrorActionPreference
$ErrorActionPreference = 'SilentlyContinue'
$marker = wsl.exe --distribution Ubuntu --user root --exec sh -lc 'cat /etc/aura-sandbox-version 2>/dev/null' 2>$null
$markerExit = $LASTEXITCODE
$ErrorActionPreference = $strictPreference
if ($markerExit -ne 0 -or ($marker -replace "`0", '').Trim() -ne 'AURA_WSL_BWRAP_V1') {
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
    'ln -s /node-modules node_modules; export CI=1 npm_config_offline=true; node node_modules/typescript/bin/tsc --noEmit; npm test --if-present'
} else {
    'git diff --check; test -n "$(git diff --name-only)"'
}
$script = @'
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
git config core.autocrlf true
git config core.filemode false
git add -A
git commit -qm baseline
git apply --check /proposal.patch
git apply /proposal.patch
git diff --check
if git diff --name-only | grep -E '(^|/)(\.env|id_rsa|credentials|secrets?)(\.|/|$)' >/dev/null; then exit 41; fi
git diff --name-only > /output/changed-files.txt
if git diff --name-only | grep -E '(^|/)(package\.json|pnpm-lock\.yaml|Cargo\.toml|Cargo\.lock|requirements\.txt|pyproject\.toml)$' > /output/dependency-files.txt; then exit 43; fi
touch /output/dependency-files.txt
git diff --unified=0 | grep '^+' | grep -v '^+++' | grep -Ei 'subprocess|Command::new|powershell|cmd\.exe|keyring|credential|api[_-]?key|access[_-]?token|reqwest|httpx|requests\.|fetch\(' > /output/security-findings.txt || true
find . -maxdepth 4 -type f \( -iname 'LICENSE' -o -iname 'LICENSE.*' -o -iname 'COPYING' -o -iname 'NOTICE' \) -print | sort > /output/license-files.txt
__TEST_COMMAND__
git diff --binary > /output/validated.patch
sha256sum /output/validated.patch > /output/validated.patch.sha256
printf '%s\n' '__TEST_PROFILE__' > /output/test-profile.txt
'@.Replace('__TEST_COMMAND__', $testCommand).Replace('__TEST_PROFILE__', $TestProfile)
$stagingRoot = Join-Path $env:LOCALAPPDATA 'AURA\evolution\staging'
New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null
$runnerScriptPath = Join-Path $stagingRoot "$RunId-runner.sh"
[IO.File]::WriteAllText($runnerScriptPath, $script, [Text.UTF8Encoding]::new($false))
$runnerScriptWsl = Convert-ToWslPath $runnerScriptPath

$arguments = @(
    '--distribution','Ubuntu','--user','root','--exec','bwrap',
    '--unshare-all','--die-with-parent','--new-session',
    '--clearenv','--setenv','PATH','/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
    '--setenv','HOME','/tmp','--setenv','LANG','C.UTF-8',
    '--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib',
    '--ro-bind-try','/lib64','/lib64','--ro-bind','/etc','/etc','--proc','/proc','--dev','/dev',
    '--tmpfs','/tmp','--tmpfs','/work',
    '--ro-bind',$projectWsl,'/input','--ro-bind',$patchWsl,'/proposal.patch',
    '--ro-bind',$runnerScriptWsl,'/runner.sh','--bind',$outputWsl,'/output'
) + $cargoArgs + $nodeArgs + @('--chdir','/work','/bin/bash','/runner.sh')

$log = Join-Path $runRoot 'runner.log'
$runnerPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
& wsl.exe @arguments 2>&1 | Tee-Object -FilePath $log
$runnerExit = $LASTEXITCODE
$ErrorActionPreference = $runnerPreference
Remove-Item -LiteralPath $runnerScriptPath -Force -ErrorAction SilentlyContinue
if ($runnerExit -ne 0) { throw "Test sandbox fallito con codice $runnerExit. Nessuna promozione consentita." }
$validated = Join-Path $runRoot 'validated.patch'
if (-not (Test-Path -LiteralPath $validated)) { throw 'Patch validata non prodotta.' }

# Canary indipendente: nuova istanza bubblewrap, nuova /work, stessa patch e
# stessi mount read-only. Un successo del primo runner non viene riutilizzato.
$canaryCommand = if ($TestProfile -eq 'Rust') {
    'mkdir -p /work/cargo; cp -a /cargo-registry /work/cargo/registry; export CARGO_HOME=/work/cargo; cargo test --offline --no-run --manifest-path src-tauri/Cargo.toml'
} elseif ($TestProfile -eq 'Node') {
    'ln -s /node-modules node_modules; export CI=1 npm_config_offline=true; node node_modules/typescript/bin/tsc --noEmit'
} else {
    'git diff --check; test -n "$(git diff --name-only)"'
}
$canaryScript = @'
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
git config user.name 'AURA Canary'
git config user.email 'canary@invalid.local'
git config core.autocrlf true
git config core.filemode false
git add -A
git commit -qm baseline
git apply --check /proposal.patch
git apply /proposal.patch
git diff --check
__CANARY_COMMAND__
printf 'passed\n' > /output/canary-pass.txt
'@.Replace('__CANARY_COMMAND__', $canaryCommand)
$canaryScriptPath = Join-Path $stagingRoot "$RunId-canary.sh"
[IO.File]::WriteAllText($canaryScriptPath, $canaryScript, [Text.UTF8Encoding]::new($false))
$canaryScriptWsl = Convert-ToWslPath $canaryScriptPath
$canaryArguments = @(
    '--distribution','Ubuntu','--user','root','--exec','bwrap',
    '--unshare-all','--die-with-parent','--new-session',
    '--clearenv','--setenv','PATH','/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
    '--setenv','HOME','/tmp','--setenv','LANG','C.UTF-8',
    '--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib',
    '--ro-bind-try','/lib64','/lib64','--ro-bind','/etc','/etc','--proc','/proc','--dev','/dev',
    '--tmpfs','/tmp','--tmpfs','/work',
    '--ro-bind',$projectWsl,'/input','--ro-bind',$patchWsl,'/proposal.patch',
    '--ro-bind',$canaryScriptWsl,'/canary.sh','--bind',$outputWsl,'/output'
) + $cargoArgs + $nodeArgs + @('--chdir','/work','/bin/bash','/canary.sh')
$canaryLog = Join-Path $runRoot 'canary.log'
$runnerPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
& wsl.exe @canaryArguments 2>&1 | Tee-Object -FilePath $canaryLog
$canaryExit = $LASTEXITCODE
$ErrorActionPreference = $runnerPreference
Remove-Item -LiteralPath $canaryScriptPath -Force -ErrorAction SilentlyContinue
$canaryPassed = $canaryExit -eq 0 -and (Test-Path -LiteralPath (Join-Path $runRoot 'canary-pass.txt'))
$canary = [ordered]@{
    schemaVersion = 1
    runId = $RunId
    passed = $canaryPassed
    freshNamespace = $true
    networkDuringCanary = $false
    hostFilesystemMountedReadOnly = $true
    exitCode = $canaryExit
    createdAtUtc = [DateTime]::UtcNow.ToString('o')
}
[IO.File]::WriteAllText((Join-Path $runRoot 'canary.json'), ($canary | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
if (-not $canaryPassed) {
    Remove-Item -LiteralPath $validated,"$validated.sha256" -Force -ErrorAction SilentlyContinue
    throw "Canary isolato fallito con codice $canaryExit. Artefatto validato revocato automaticamente."
}
$attestation = [ordered]@{
    schemaVersion = 2
    runId = $RunId
    testProfile = $TestProfile
    patchSha256 = (Get-FileHash -LiteralPath $validated -Algorithm SHA256).Hash.ToLowerInvariant()
    dependencyPolicy = 'immutable-offline'
    networkDuringTest = $false
    hostFilesystemMountedReadOnly = $true
    canaryPassed = $true
    canaryFreshNamespace = $true
    canaryNetworkDisabled = $true
    changedFiles = @(Get-Content -LiteralPath (Join-Path $runRoot 'changed-files.txt') -ErrorAction SilentlyContinue)
    dependencyFiles = @(Get-Content -LiteralPath (Join-Path $runRoot 'dependency-files.txt') -ErrorAction SilentlyContinue)
    licenseFiles = @(Get-Content -LiteralPath (Join-Path $runRoot 'license-files.txt') -ErrorAction SilentlyContinue)
    securityFindings = @(Get-Content -LiteralPath (Join-Path $runRoot 'security-findings.txt') -ErrorAction SilentlyContinue)
    createdAtUtc = [DateTime]::UtcNow.ToString('o')
}
[IO.File]::WriteAllText((Join-Path $runRoot 'attestation.json'), ($attestation | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
[PSCustomObject]@{ RunId=$RunId; Status='validated'; TestProfile=$TestProfile; Patch=$validated; Log=$log }
