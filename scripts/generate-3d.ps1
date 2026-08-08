[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputImage,

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$')]
    [string]$JobName = (Get-Date -Format 'yyyyMMdd-HHmmss'),

    [ValidateSet('Auto', 'Gpu', 'Cpu')]
    [string]$Device = 'Auto',

    [ValidateRange(64, 256)]
    [int]$MeshResolution = 128,

    [ValidateRange(256, 8192)]
    [int]$ChunkSize = 1024,

    [ValidateSet('glb', 'obj')]
    [string]$Format = 'glb',

    [switch]$BakeTexture
)

$ErrorActionPreference = 'Stop'

$resolvedInput = (Resolve-Path -LiteralPath $InputImage).Path
if ([System.IO.Path]::GetExtension($resolvedInput).ToLowerInvariant() -notin @('.png', '.jpg', '.jpeg', '.webp')) {
    throw 'Formato immagine non supportato. Usa PNG, JPG, JPEG o WEBP.'
}

$toolRoot = Join-Path $env:LOCALAPPDATA 'AURA\tools\3d\TripoSR'
$python = Join-Path $toolRoot '.venv\Scripts\python.exe'
$runner = Join-Path $toolRoot 'run.py'
if (-not (Test-Path -LiteralPath $python) -or -not (Test-Path -LiteralPath $runner)) {
    throw 'TripoSR non risulta installato nella cartella locale prevista.'
}

$outputRoot = Join-Path $env:LOCALAPPDATA 'AURA\data\3d\jobs'
$outputDirectory = Join-Path $outputRoot $JobName
if (Test-Path -LiteralPath $outputDirectory) {
    throw "Il job '$JobName' esiste già. Scegli un nome diverso."
}
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

$selectedDevice = $Device.ToLowerInvariant()
if ($Device -eq 'Auto') {
    $selectedDevice = 'cpu'
    $smi = Get-Command 'nvidia-smi.exe' -ErrorAction SilentlyContinue
    if ($smi) {
        $freeText = & $smi.Source --query-gpu=memory.free --format=csv,noheader,nounits 2>$null | Select-Object -First 1
        $freeMiB = 0
        if ([int]::TryParse(($freeText -replace '\D', ''), [ref]$freeMiB) -and $freeMiB -ge 5200) {
            $selectedDevice = 'cuda:0'
        }
    }
} elseif ($Device -eq 'Gpu') {
    $selectedDevice = 'cuda:0'
}

$env:HF_HOME = Join-Path $env:LOCALAPPDATA 'AURA\models\huggingface'
$env:HF_HUB_DISABLE_SYMLINKS_WARNING = '1'

$arguments = @(
    $runner,
    $resolvedInput,
    '--device', $selectedDevice,
    '--chunk-size', $ChunkSize,
    '--mc-resolution', $MeshResolution,
    '--model-save-format', $Format,
    '--output-dir', $outputDirectory
)
if ($BakeTexture) {
    $arguments += @('--bake-texture', '--texture-resolution', '1024')
}

Write-Host "AURA 3D: device=$selectedDevice resolution=$MeshResolution chunk=$ChunkSize"
& $python @arguments
if ($LASTEXITCODE -ne 0) {
    throw "TripoSR è terminato con codice $LASTEXITCODE."
}

$result = Get-ChildItem -LiteralPath $outputDirectory -Recurse -File |
    Where-Object Extension -in @('.glb', '.obj') |
    Select-Object -First 1
if (-not $result) {
    throw 'Generazione completata senza un file mesh riconoscibile.'
}

Write-Output $result.FullName
