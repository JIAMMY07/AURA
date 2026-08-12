param(
    [string]$Name = 'aura-coder:3b'
)

$ErrorActionPreference = 'Stop'
$ollama = Join-Path $env:LOCALAPPDATA 'Programs\Ollama\ollama.exe'
$modelFile = Join-Path (Split-Path $PSScriptRoot -Parent) 'ollama\AURA-Coder.Modelfile'

if (-not (Test-Path -LiteralPath $ollama)) {
    throw "Ollama non trovato in $ollama"
}

if (-not (Test-Path -LiteralPath $modelFile)) {
    throw "Modelfile non trovato in $modelFile"
}

Write-Host "Creazione del modello locale $Name..."
& $ollama create $Name -f $modelFile
if ($LASTEXITCODE -ne 0) {
    throw "Creazione di $Name non riuscita (exit code $LASTEXITCODE)."
}

Write-Host "Modello creato. Dettagli:"
& $ollama show $Name
