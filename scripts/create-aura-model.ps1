param([string]$Name = 'aura:4b')

$ErrorActionPreference = 'Stop'
$ollama = Join-Path $env:LOCALAPPDATA 'Programs\Ollama\ollama.exe'
$modelFile = Join-Path (Split-Path $PSScriptRoot -Parent) 'ollama\AURA.Modelfile'
if (-not (Test-Path -LiteralPath $ollama)) { throw 'Ollama non trovato.' }
if (-not (Test-Path -LiteralPath $modelFile)) { throw 'Modelfile AURA non trovato.' }
& $ollama create $Name -f $modelFile
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $ollama show $Name
