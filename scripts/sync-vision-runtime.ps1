$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $projectRoot 'node_modules\@mediapipe\tasks-vision\wasm'
$destination = Join-Path $projectRoot 'public\mediapipe\wasm'

if (-not (Test-Path -LiteralPath $source)) {
    throw 'Runtime MediaPipe non installato. Eseguire pnpm install.'
}

New-Item -ItemType Directory -Force -Path $destination | Out-Null
$assets = @(
    'vision_wasm_internal.js',
    'vision_wasm_internal.wasm',
    'vision_wasm_nosimd_internal.js',
    'vision_wasm_nosimd_internal.wasm'
)
foreach ($asset in $assets) {
    Copy-Item -LiteralPath (Join-Path $source $asset) -Destination (Join-Path $destination $asset) -Force
}

$modelRoot = Join-Path $projectRoot 'public\models\vision'
$manifestPath = Join-Path $modelRoot 'SHA256SUMS.txt'
if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw 'Manifest SHA-256 dei modelli visivi mancante.'
}

$expected = @{}
Get-Content -LiteralPath $manifestPath | ForEach-Object {
    if ($_ -match '^([0-9a-fA-F]{64})\s+(.+)$') { $expected[$Matches[2]] = $Matches[1].ToLowerInvariant() }
}
foreach ($model in @('face_landmarker.task', 'gesture_recognizer.task')) {
    $path = Join-Path $modelRoot $model
    if (-not (Test-Path -LiteralPath $path)) { throw "Modello visivo mancante: $model" }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$model]) { throw "Hash modello visivo non valido: $model" }
}

Write-Output 'Runtime e modelli MediaPipe verificati.'

