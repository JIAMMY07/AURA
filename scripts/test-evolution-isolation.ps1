[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("aura-evolution-isolation-" + [guid]::NewGuid().ToString('N'))
$repoRoot = Join-Path $testRoot 'repo'
$runId = [guid]::NewGuid().ToString()
$runRoot = Join-Path $env:LOCALAPPDATA "AURA\evolution\runs\$runId"
$runner = Join-Path $PSScriptRoot 'evolution-sandbox-run.ps1'

try {
    New-Item -ItemType Directory -Path $repoRoot -Force | Out-Null
    git -C $repoRoot init -q -b main
    git -C $repoRoot config user.name 'AURA Isolation Test'
    git -C $repoRoot config user.email 'isolation-test@invalid.local'
    [IO.File]::WriteAllText((Join-Path $repoRoot 'proof.txt'), "before`n", [Text.UTF8Encoding]::new($false))
    git -C $repoRoot add proof.txt
    git -C $repoRoot commit -qm baseline
    [IO.File]::WriteAllText((Join-Path $repoRoot 'proof.txt'), "after`n", [Text.UTF8Encoding]::new($false))
    $patchPath = Join-Path $testRoot 'proposal.patch'
    $patchText = (git -C $repoRoot diff -- proof.txt) -join "`n"
    [IO.File]::WriteAllText($patchPath, ($patchText + "`n"), [Text.UTF8Encoding]::new($false))
    git -C $repoRoot restore proof.txt
    $hostHashBefore = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'proof.txt') -Algorithm SHA256).Hash

    & $runner -ProjectPath $repoRoot -PatchPath $patchPath -TestProfile Static -RunId $runId | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Runner isolato terminato con codice $LASTEXITCODE." }

    $attestation = Get-Content -LiteralPath (Join-Path $runRoot 'attestation.json') -Raw | ConvertFrom-Json
    $canary = Get-Content -LiteralPath (Join-Path $runRoot 'canary.json') -Raw | ConvertFrom-Json
    $validated = Join-Path $runRoot 'validated.patch'
    $actualHash = (Get-FileHash -LiteralPath $validated -Algorithm SHA256).Hash.ToLowerInvariant()

    if ($attestation.schemaVersion -ne 2) { throw 'Attestazione Evolution non v2.' }
    if (-not $attestation.canaryPassed -or -not $attestation.canaryFreshNamespace -or -not $attestation.canaryNetworkDisabled) { throw 'Attestazione canary incompleta.' }
    if ($attestation.networkDuringTest -ne $false -or $attestation.hostFilesystemMountedReadOnly -ne $true) { throw 'Garanzie di isolamento non attestate.' }
    if (-not $canary.passed -or -not $canary.freshNamespace -or $canary.networkDuringCanary -ne $false) { throw 'Canary isolato non valido.' }
    if ($attestation.patchSha256 -ne $actualHash) { throw 'Hash della patch validata non coerente.' }
    $hostHashAfter = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'proof.txt') -Algorithm SHA256).Hash
    if ($hostHashBefore -ne $hostHashAfter) { throw 'Il runner ha modificato il repository host.' }

    [PSCustomObject]@{
        StrongIsolation = $true
        HostRepositoryUnchanged = $true
        NetworkNamespaceDisabled = $true
        CanaryFreshNamespace = $true
        AttestationVersion = $attestation.schemaVersion
        PatchHashVerified = $true
    } | Format-List
}
finally {
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
    if (Test-Path -LiteralPath $runRoot) { Remove-Item -LiteralPath $runRoot -Recurse -Force }
}
