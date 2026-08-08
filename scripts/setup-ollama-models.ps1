[CmdletBinding()]
param(
    [string]$OllamaPath = "$env:LOCALAPPDATA\Programs\Ollama\ollama.exe"
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $OllamaPath -PathType Leaf)) {
    throw "Ollama non trovato in: $OllamaPath"
}

$models = @(
    'qwen3:4b-instruct',
    'qwen3:4b',
    'qwen2.5-coder:3b',
    'qwen2.5vl:3b',
    'qwen3-embedding:0.6b',
    'granite3-guardian:2b'
)

foreach ($model in $models) {
    Write-Host "Verifica/installazione $model"
    & $OllamaPath pull $model
    if ($LASTEXITCODE -ne 0) {
        throw "Installazione Ollama fallita per $model"
    }
}

Write-Host 'Suite Ollama installata:'
& $OllamaPath list
