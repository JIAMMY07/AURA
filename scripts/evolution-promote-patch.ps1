[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ProjectPath,
    [Parameter(Mandatory = $true)][string]$ValidatedPatch,
    [Parameter(Mandatory = $true)][switch]$Approve
)

$ErrorActionPreference = 'Stop'
if (-not $Approve) { throw 'Promozione bloccata: è richiesta l’approvazione esplicita -Approve.' }
$project = (Resolve-Path -LiteralPath $ProjectPath).Path
$patch = (Resolve-Path -LiteralPath $ValidatedPatch).Path
$runRoot = Split-Path $patch
if ((Split-Path $runRoot) -ne (Join-Path $env:LOCALAPPDATA 'AURA\evolution\runs')) {
    throw 'La patch non proviene dalla cartella dei run Evolution validati.'
}

$hashFile = "$patch.sha256"
$profileFile = Join-Path $runRoot 'test-profile.txt'
$attestationFile = Join-Path $runRoot 'attestation.json'
$canaryFile = Join-Path $runRoot 'canary.json'
foreach ($required in @($hashFile, $profileFile, $attestationFile, $canaryFile)) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Attestazione sandbox/canary incompleta: $required" }
}
$expected = ((Get-Content -LiteralPath $hashFile -Raw) -split '\s+')[0].ToLowerInvariant()
$actual = (Get-FileHash -LiteralPath $patch -Algorithm SHA256).Hash.ToLowerInvariant()
if ($expected -ne $actual) { throw 'Hash patch non valido: promozione bloccata.' }
$attestation = Get-Content -LiteralPath $attestationFile -Raw | ConvertFrom-Json
$canary = Get-Content -LiteralPath $canaryFile -Raw | ConvertFrom-Json
$runId = Split-Path $runRoot -Leaf
if ($attestation.schemaVersion -ne 2 -or $attestation.runId -ne $runId -or
    -not $attestation.canaryPassed -or -not $attestation.canaryFreshNamespace -or
    -not $attestation.canaryNetworkDisabled -or $attestation.networkDuringTest -or
    -not $attestation.hostFilesystemMountedReadOnly -or
    $attestation.patchSha256.ToLowerInvariant() -ne $actual) {
    throw 'Attestazione sandbox v2 non conforme o non correlata alla patch.'
}
if (-not $canary.passed -or -not $canary.freshNamespace -or
    $canary.networkDuringCanary -or -not $canary.hostFilesystemMountedReadOnly -or
    $canary.runId -ne $runId) {
    throw 'Canary non conforme o non correlato al run.'
}
if (-not (Test-Path -LiteralPath (Join-Path $project '.git'))) { throw 'Repository Git non valido.' }
if (git -C $project status --porcelain) { throw 'Worktree non pulito: promozione bloccata.' }
$originalBranch = (git -C $project branch --show-current).Trim()
if (-not $originalBranch) { throw 'HEAD scollegato: promozione atomica non disponibile.' }
$originalCommit = (git -C $project rev-parse HEAD).Trim()
git -C $project apply --check $patch
if ($LASTEXITCODE -ne 0) { throw 'La patch non è applicabile al repository corrente.' }
$branch = "aura/evolution-$runId"
$branchCreated = $false
$patchApplied = $false
try {
    git -C $project switch -c $branch
    if ($LASTEXITCODE -ne 0) { throw 'Creazione branch di promozione fallita.' }
    $branchCreated = $true
    git -C $project apply $patch
    if ($LASTEXITCODE -ne 0) { throw 'Applicazione patch fallita.' }
    $patchApplied = $true
    git -C $project diff --check
    if ($LASTEXITCODE -ne 0) { throw 'Health check post-promozione fallito.' }
    git -C $project apply --reverse --check $patch
    if ($LASTEXITCODE -ne 0) { throw 'Rollback della patch non verificabile.' }
    $manifest = [ordered]@{
        schemaVersion = 1; status = 'canary-promoted'; originalBranch = $originalBranch
        originalCommit = $originalCommit; reviewBranch = $branch; patchSha256 = $actual
        rollbackVerified = $true; createdAtUtc = [DateTime]::UtcNow.ToString('o')
    }
    [IO.File]::WriteAllText((Join-Path $runRoot 'promotion-manifest.json'), ($manifest | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
} catch {
    $failure = $_.Exception.Message
    $rollbackSucceeded = $true
    $strictPreference = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    if ($patchApplied) {
        git -C $project apply --reverse $patch 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { $rollbackSucceeded = $false }
    }
    if ($branchCreated) {
        git -C $project switch $originalBranch 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { $rollbackSucceeded = $false }
        if ($rollbackSucceeded) {
            git -C $project branch -D $branch 2>$null | Out-Null
            if ($LASTEXITCODE -ne 0) { $rollbackSucceeded = $false }
        }
    }
    $ErrorActionPreference = $strictPreference
    $rollback = [ordered]@{
        schemaVersion = 1; status = $(if ($rollbackSucceeded) { 'rolled-back-automatically' } else { 'rollback-failed' }); originalBranch = $originalBranch
        originalCommit = $originalCommit; failedBranch = $branch; reason = $failure
        createdAtUtc = [DateTime]::UtcNow.ToString('o')
    }
    $rollbackName = if ($rollbackSucceeded) { 'rollback.json' } else { 'rollback-failed.json' }
    [IO.File]::WriteAllText((Join-Path $runRoot $rollbackName), ($rollback | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
    if ($rollbackSucceeded) { throw "Promozione fallita e rollback automatico verificato: $failure" }
    throw "Promozione fallita; anche il rollback richiede intervento manuale immediato: $failure"
}
[PSCustomObject]@{
    Status = 'canary-promoted-to-review-branch'; Branch = $branch; CommitCreated = $false
    CanaryPassed = $true; RollbackVerified = $true
    RecoveryManifest = (Join-Path $runRoot 'promotion-manifest.json')
}
