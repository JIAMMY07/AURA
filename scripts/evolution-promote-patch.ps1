[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectPath,
    [Parameter(Mandatory = $true)]
    [string]$ValidatedPatch,
    [Parameter(Mandatory = $true)]
    [switch]$Approve
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
if (-not (Test-Path $hashFile) -or -not (Test-Path $profileFile)) { throw 'Attestazione sandbox incompleta.' }
$expected = ((Get-Content -LiteralPath $hashFile -Raw) -split '\s+')[0]
$actual = (Get-FileHash -LiteralPath $patch -Algorithm SHA256).Hash.ToLowerInvariant()
if ($expected.ToLowerInvariant() -ne $actual) { throw 'Hash patch non valido: promozione bloccata.' }
if (-not (Test-Path (Join-Path $project '.git'))) { throw 'Repository Git non valido.' }
if (git -C $project status --porcelain) { throw 'Worktree non pulito: promozione bloccata.' }
git -C $project apply --check $patch
if ($LASTEXITCODE -ne 0) { throw 'La patch non è applicabile al repository corrente.' }
$branch = "aura/evolution-$(Split-Path $runRoot -Leaf)"
git -C $project switch -c $branch
if ($LASTEXITCODE -ne 0) { throw 'Creazione branch di promozione fallita.' }
git -C $project apply $patch
if ($LASTEXITCODE -ne 0) {
    git -C $project switch -
    throw 'Applicazione patch fallita; branch non promosso.'
}
[PSCustomObject]@{ Status='promoted-to-review-branch'; Branch=$branch; CommitCreated=$false; Rollback="git -C `"$project`" switch -" }
