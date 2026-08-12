[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$promotionScript = Join-Path $PSScriptRoot 'evolution-promote-patch.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) "aura-evolution-promotion-$([guid]::NewGuid())"
$previousLocalAppData = $env:LOCALAPPDATA
$utf8 = [Text.UTF8Encoding]::new($false)

function Invoke-Git([string]$Repository, [string[]]$Arguments) {
    & git -C $Repository @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Git fallito: $($Arguments -join ' ')" }
}

function New-ValidatedRun([string]$Repository, [string]$NewContent) {
    [IO.File]::WriteAllText((Join-Path $Repository 'a.txt'), $NewContent, $utf8)
    $diff = ((& git -C $Repository diff --binary) -join "`n") + "`n"
    if ($LASTEXITCODE -ne 0 -or -not $diff.StartsWith('diff --git ')) { throw 'Patch test non prodotta.' }
    Invoke-Git $Repository @('checkout','--','a.txt')
    $runId = [guid]::NewGuid().ToString()
    $runRoot = Join-Path $env:LOCALAPPDATA "AURA\evolution\runs\$runId"
    [IO.Directory]::CreateDirectory($runRoot) | Out-Null
    $patch = Join-Path $runRoot 'validated.patch'
    [IO.File]::WriteAllText($patch, $diff, $utf8)
    $hash = (Get-FileHash -LiteralPath $patch -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$patch.sha256", "$hash  validated.patch`n", $utf8)
    [IO.File]::WriteAllText((Join-Path $runRoot 'test-profile.txt'), "Static`n", $utf8)
    $attestation = [ordered]@{
        schemaVersion=2; runId=$runId; testProfile='Static'; patchSha256=$hash
        dependencyPolicy='immutable-offline'; networkDuringTest=$false
        hostFilesystemMountedReadOnly=$true; canaryPassed=$true
        canaryFreshNamespace=$true; canaryNetworkDisabled=$true
        changedFiles=@('a.txt'); dependencyFiles=@(); licenseFiles=@(); securityFindings=@()
        createdAtUtc=[DateTime]::UtcNow.ToString('o')
    }
    $canary = [ordered]@{
        schemaVersion=1; runId=$runId; passed=$true; freshNamespace=$true
        networkDuringCanary=$false; hostFilesystemMountedReadOnly=$true; exitCode=0
        createdAtUtc=[DateTime]::UtcNow.ToString('o')
    }
    [IO.File]::WriteAllText((Join-Path $runRoot 'attestation.json'), ($attestation | ConvertTo-Json -Depth 5), $utf8)
    [IO.File]::WriteAllText((Join-Path $runRoot 'canary.json'), ($canary | ConvertTo-Json -Depth 5), $utf8)
    return [pscustomobject]@{ RunId=$runId; Root=$runRoot; Patch=$patch }
}

try {
    [IO.Directory]::CreateDirectory($testRoot) | Out-Null
    $env:LOCALAPPDATA = Join-Path $testRoot 'localappdata'
    $repository = Join-Path $testRoot 'repo'
    [IO.Directory]::CreateDirectory($repository) | Out-Null
    Invoke-Git $repository @('init','-q','-b','main')
    Invoke-Git $repository @('config','user.name','AURA Test')
    Invoke-Git $repository @('config','user.email','aura-test@invalid.local')
    [IO.File]::WriteAllText((Join-Path $repository 'a.txt'), "a`n", $utf8)
    Invoke-Git $repository @('add','a.txt')
    Invoke-Git $repository @('commit','-qm','baseline')

    $valid = New-ValidatedRun $repository "b`n"
    & $promotionScript -ProjectPath $repository -ValidatedPatch $valid.Patch -Approve | Out-Null
    if ((& git -C $repository branch --show-current).Trim() -ne "aura/evolution-$($valid.RunId)") { throw 'Branch canary non creato.' }
    if ([IO.File]::ReadAllText((Join-Path $repository 'a.txt')).Replace("`r`n","`n") -ne "b`n") { throw 'Patch valida non applicata.' }
    if (-not (Test-Path -LiteralPath (Join-Path $valid.Root 'promotion-manifest.json'))) { throw 'Manifest recupero assente.' }
    Invoke-Git $repository @('apply','--reverse',$valid.Patch)
    Invoke-Git $repository @('switch','main')
    Invoke-Git $repository @('branch','-D',"aura/evolution-$($valid.RunId)")

    $invalid = New-ValidatedRun $repository "b   `n"
    $failed = $false
    try { & $promotionScript -ProjectPath $repository -ValidatedPatch $invalid.Patch -Approve | Out-Null }
    catch { $failed = $true }
    if (-not $failed) { throw 'Health check negativo non ha bloccato la promozione.' }
    if ((& git -C $repository branch --show-current).Trim() -ne 'main') { throw 'Rollback non è tornato al branch originale.' }
    if (& git -C $repository status --porcelain) { throw 'Rollback ha lasciato modifiche nel worktree.' }
    if (-not (Test-Path -LiteralPath (Join-Path $invalid.Root 'rollback.json'))) {
        $failedManifest = Join-Path $invalid.Root 'rollback-failed.json'
        $detail = if (Test-Path -LiteralPath $failedManifest) { Get-Content -LiteralPath $failedManifest -Raw } else { 'nessun manifest' }
        throw "Attestazione rollback automatico assente: $detail"
    }
    [pscustomobject]@{ ValidPromotion='passed'; FailedHealthCheck='rolled-back'; WorktreeClean=$true }
} finally {
    $env:LOCALAPPDATA = $previousLocalAppData
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
