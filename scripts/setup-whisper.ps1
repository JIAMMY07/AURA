param(
  [string]$Destination = (Join-Path $env:APPDATA 'local.aura.assistant\voice')
)

$ErrorActionPreference = 'Stop'
$release = 'v1.9.2'
$binaryUrl = "https://github.com/ggml-org/whisper.cpp/releases/download/$release/whisper-bin-x64.zip"
$modelUrl = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin'
$vadUrl = 'https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin'
$binarySha256 = '49DCC16DE826F20BD53D44F947A1AE49DFA81F86CAD67A64D80820CB192D674A'
$modelSha256 = '60ED5BC3DD14EEA856493D334349B405782DDCAF0028D4B5DF4088345FBA2EFE'
$vadSha256 = '2AA269B785EEB53A82983A20501DDF7C1D9C48E33AB63A41391AC6C9F7FB6987'
$temporary = Join-Path $env:TEMP ("aura-whisper-" + [guid]::NewGuid().ToString('N'))
$archive = Join-Path $temporary 'whisper-bin-x64.zip'
$model = Join-Path $temporary 'ggml-base.bin'
$vadModel = Join-Path $temporary 'ggml-silero-v6.2.0.bin'
$expanded = Join-Path $temporary 'expanded'
$binDestination = Join-Path $Destination 'bin'
$modelDestination = Join-Path $Destination 'models'

function Assert-Hash([string]$Path, [string]$Expected) {
  $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
  if ($actual -ne $Expected) {
    throw "Hash SHA-256 non valido per $Path. Atteso $Expected, ottenuto $actual."
  }
}

try {
  New-Item -ItemType Directory -Force -Path $temporary,$expanded,$binDestination,$modelDestination | Out-Null
  Invoke-WebRequest -Uri $binaryUrl -OutFile $archive
  Invoke-WebRequest -Uri $modelUrl -OutFile $model
  Invoke-WebRequest -Uri $vadUrl -OutFile $vadModel
  Assert-Hash $archive $binarySha256
  Assert-Hash $model $modelSha256
  Assert-Hash $vadModel $vadSha256
  Expand-Archive -LiteralPath $archive -DestinationPath $expanded -Force
  $releaseDirectory = Join-Path $expanded 'Release'
  if (!(Test-Path (Join-Path $releaseDirectory 'whisper-cli.exe'))) {
    throw 'whisper-cli.exe non trovato nel pacchetto verificato.'
  }
  Copy-Item -LiteralPath (Join-Path $releaseDirectory 'whisper-cli.exe') -Destination $binDestination -Force
  Copy-Item -LiteralPath (Join-Path $releaseDirectory 'whisper.dll') -Destination $binDestination -Force
  Copy-Item -Path (Join-Path $releaseDirectory 'ggml*.dll') -Destination $binDestination -Force
  $allowedRuntime = @('whisper-cli.exe', 'whisper.dll') + @(
    Get-ChildItem -LiteralPath $releaseDirectory -Filter 'ggml*.dll' -File | ForEach-Object Name
  )
  Get-ChildItem -LiteralPath $binDestination -File | Where-Object {
    $_.Name -notin $allowedRuntime
  } | ForEach-Object {
    Remove-Item -LiteralPath $_.FullName -Force
  }
  Copy-Item -LiteralPath $model -Destination (Join-Path $modelDestination 'ggml-base.bin') -Force
  Copy-Item -LiteralPath $vadModel -Destination (Join-Path $modelDestination 'ggml-silero-v6.2.0.bin') -Force
  Write-Host "Whisper locale installato in $Destination"
  Write-Host "Runtime: whisper.cpp $release"
  Write-Host "Modello: ggml-base.bin (multilingue)"
  Write-Host "VAD: Silero v6.2.0"
} finally {
  if (Test-Path $temporary) {
    Remove-Item -LiteralPath $temporary -Recurse -Force
  }
}
