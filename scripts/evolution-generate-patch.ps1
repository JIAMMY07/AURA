[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectPath,
    [Parameter(Mandatory = $true)]
    [ValidateLength(10, 4000)]
    [string]$Objective,
    [string[]]$ContextFiles = @(),
    [ValidatePattern('^[a-f0-9-]{36}$')]
    [string]$ProposalId = ([guid]::NewGuid().ToString()),
    [string]$Model = 'qwen2.5-coder:3b'
)

$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath $ProjectPath).Path
if (-not (Test-Path (Join-Path $project '.git'))) { throw 'Il progetto deve essere un repository Git.' }
$tracked = @(git -C $project ls-files)
if ($LASTEXITCODE -ne 0) { throw 'Impossibile leggere i file Git tracciati.' }
$blocked = '(^|/)(\.env|secrets?|credentials?|id_rsa)(\.|/|$)'
$protected = @(
    'src-tauri/src/commands/policy.rs',
    'src-tauri/src/crypto.rs',
    'scripts/setup-evolution-sandbox.ps1',
    'scripts/evolution-sandbox-run.ps1',
    'scripts/evolution-promote-patch.ps1',
    'scripts/sign-windows-artifacts.ps1'
)

if ($ContextFiles.Count -eq 0) {
    $ContextFiles = $tracked |
        Where-Object { $_ -match '\.(rs|ts|tsx|css|json|yaml|yml|toml|md)$' } |
        Where-Object { $_ -notmatch $blocked } |
        Where-Object { $_ -notmatch '(^|/)(pnpm-lock\.yaml|Cargo\.lock)$' } |
        Where-Object { $_ -notmatch '(^|/)(target|node_modules|dist)/' } |
        Select-Object -First 10
}
$context = New-Object Text.StringBuilder
foreach ($relative in $ContextFiles) {
    $normalized = $relative.Replace('\','/')
    if ($normalized -notin $tracked -or $normalized -match $blocked) { throw "File contesto non consentito: $relative" }
    $full = (Resolve-Path -LiteralPath (Join-Path $project $relative)).Path
    if (-not $full.StartsWith($project, [StringComparison]::OrdinalIgnoreCase)) { throw 'Path traversal nel contesto.' }
    $content = Get-Content -LiteralPath $full -Raw
    [void]$context.AppendLine("FILE: $normalized")
    [void]$context.AppendLine($content)
    [void]$context.AppendLine('END_FILE')
    if ($context.Length -gt 80000) { throw 'Contesto oltre il limite di 80 KB per il profilo hardware corrente.' }
}

$system = @'
Sei il generatore confinato di AURA. Proponi solo operazioni search/replace JSON sui file di contesto forniti. Ogni search deve essere NON VUOTO e deve copiare una sottostringa letterale presente una sola volta nel file originale; replace contiene il testo sostitutivo per quel frammento. Per inserire testo, scegli come search un anchor vicino e univoco e ripetilo dentro replace insieme al nuovo testo. Usa il numero minimo di operazioni. Non inventare test eseguiti. Non modificare policy, crittografia, sandbox, firma, workflow CI, segreti o dipendenze.
'@
$user = "OBIETTIVO:`n$Objective`n`nCONTESTO:`n$context"
$schema = @{
    type='object'
    properties=@{
        operations=@{
            type='array'; maxItems=20
            items=@{
                type='object'
                properties=@{
                    path=@{type='string'}
                    search=@{type='string'}
                    replace=@{type='string'}
                }
                required=@('path','search','replace')
            }
        }
    }
    required=@('operations')
}
$operations = @()
$retryReason = ''
for ($attempt = 1; $attempt -le 3; $attempt++) {
    $requestUser = $user
    if ($retryReason) {
        $requestUser += "`n`nTENTATIVO PRECEDENTE RIFIUTATO: $retryReason Usa un anchor letterale più lungo, non vuoto e presente una sola volta."
    }
    $body = @{
        model=$Model; stream=$false; keep_alive='5m'
        format=$schema
        messages=@(@{role='system';content=$system},@{role='user';content=$requestUser})
        options=@{temperature=0.15;num_ctx=4096;num_predict=1536;num_gpu=0;seed=$attempt}
    } | ConvertTo-Json -Depth 7
    $response = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/chat' -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 300
    $payload = [string]$response.message.content | ConvertFrom-Json
    $operations = @($payload.operations)
    $retryReason = ''
    if ($operations.Count -eq 0 -or $operations.Count -gt 20) {
        $retryReason = 'numero operazioni fuori dai limiti.'
    } else {
        foreach ($operation in $operations) {
            $relative = ([string]$operation.path).Replace('\','/')
            $search = [string]$operation.search
            $replace = [string]$operation.replace
            if ($relative -notin $ContextFiles -or $relative -notin $tracked -or $relative -match $blocked -or $relative -in $protected) {
                $retryReason = "file non consentito: $relative."
                break
            }
            if ([string]::IsNullOrEmpty($search) -or $search.Length -gt 20000 -or $replace.Length -gt 30000) {
                $retryReason = 'operazione search/replace fuori dai limiti.'
                break
            }
            $original = Get-Content -LiteralPath (Join-Path $project $relative) -Raw
            $originalNormalized = $original.Replace("`r`n", "`n")
            $searchNormalized = $search.Replace("`r`n", "`n")
            $anchorCount = ([regex]::Matches($originalNormalized, [regex]::Escape($searchNormalized))).Count
            if ($anchorCount -ne 1) {
                $retryReason = "anchor non univoco per $relative (occorrenze=$anchorCount, lunghezza=$($searchNormalized.Length))."
                break
            }
        }
    }
    if (-not $retryReason) { break }
}
if ($retryReason) { throw "Generazione non valida dopo tre tentativi: $retryReason" }
$draftRoot = Join-Path $env:LOCALAPPDATA "AURA\evolution\drafts\$ProposalId"
if (Test-Path -LiteralPath $draftRoot) { throw 'Draft Evolution gia esistente.' }
New-Item -ItemType Directory -Path $draftRoot -Force | Out-Null
try {
    foreach ($relative in $ContextFiles) {
        $source = Join-Path $project $relative
        $destination = Join-Path $draftRoot $relative
        New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $destination
    }
    git -C $draftRoot init -q
    git -C $draftRoot config user.name 'AURA Draft'
    git -C $draftRoot config user.email 'draft@invalid.local'
    git -C $draftRoot add -A
    git -C $draftRoot commit -qm baseline
    foreach ($operation in $operations) {
        $relative = ([string]$operation.path).Replace('\','/')
        if ($relative -notin $ContextFiles -or $relative -notin $tracked -or $relative -match $blocked -or $relative -in $protected) {
            throw "Operazione su file non consentito: $relative"
        }
        $search = [string]$operation.search
        $replace = [string]$operation.replace
        if ([string]::IsNullOrEmpty($search) -or $search.Length -gt 20000 -or $replace.Length -gt 30000) { throw 'Operazione search/replace fuori dai limiti.' }
        $target = Join-Path $draftRoot $relative
        $content = Get-Content -LiteralPath $target -Raw
        $contentNormalized = $content.Replace("`r`n", "`n")
        $searchNormalized = $search.Replace("`r`n", "`n")
        $replaceNormalized = $replace.Replace("`r`n", "`n")
        if (([regex]::Matches($contentNormalized, [regex]::Escape($searchNormalized))).Count -ne 1) { throw "Anchor non univoco per $relative" }
        [IO.File]::WriteAllText($target, $contentNormalized.Replace($searchNormalized, $replaceNormalized), [Text.UTF8Encoding]::new($false))
    }
    $diff = ((& git -C $draftRoot diff --binary --no-ext-diff) -join "`n") + "`n"
    if ($LASTEXITCODE -ne 0 -or $diff.Length -gt 5MB -or -not $diff.TrimStart().StartsWith('diff --git ')) { throw 'AURA non ha potuto costruire una patch Git valida.' }
} finally {
    if (Test-Path -LiteralPath $draftRoot) { Remove-Item -LiteralPath $draftRoot -Recurse -Force }
}
$proposalRoot = Join-Path $env:LOCALAPPDATA "AURA\evolution\proposals\$ProposalId"
if (Test-Path $proposalRoot) { throw 'ProposalId già esistente.' }
New-Item -ItemType Directory -Path $proposalRoot -Force | Out-Null
$patchPath = Join-Path $proposalRoot 'proposal.patch'
[IO.File]::WriteAllText($patchPath, $diff, [Text.UTF8Encoding]::new($false))
$null = git -C $project apply --check $patchPath
if ($LASTEXITCODE -ne 0) {
    Remove-Item -LiteralPath $proposalRoot -Recurse -Force
    throw 'Patch generata non applicabile al repository corrente.'
}
[PSCustomObject]@{ ProposalId=$ProposalId; Status='generated-not-executed'; Patch=$patchPath; Model=$Model }
