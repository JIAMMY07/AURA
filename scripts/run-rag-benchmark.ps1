[CmdletBinding()]
param([string]$Model = 'qwen3-embedding:0.6b')

$ErrorActionPreference = 'Stop'
$cases = @(
  @{ Id='wifi'; Text='La password della rete ospiti si cambia dal pannello Router nella sezione Wi-Fi Guest.'; Query='Dove cambio la password del Wi-Fi ospiti?' },
  @{ Id='backup'; Text='Il backup AURA viene eseguito ogni venerdi alle ore 21 nella cartella Documenti Backup.'; Query='Quando viene eseguito il backup di AURA?' },
  @{ Id='errore'; Text='Per risolvere il codice errore AURA-742 bisogna riavviare il servizio Ollama.'; Query='Come risolvo AURA-742?' },
  @{ Id='fattura'; Text='La fattura del fornitore Blu scade il 18 settembre e ammonta a 240 euro.'; Query='Quanto vale e quando scade la fattura Blu?' },
  @{ Id='progetto'; Text='Il progetto Orione usa Rust per il backend e React TypeScript per la sua interfaccia.'; Query='Quali tecnologie usa il progetto Orione?' }
)

function Get-Embeddings([string[]]$InputText) {
  $body = @{ model=$Model; input=$InputText; keep_alive='0'; options=@{ num_ctx=2048; num_gpu=0 } } | ConvertTo-Json -Depth 4
  (Invoke-RestMethod -Method Post -Uri 'http://127.0.0.1:11434/api/embed' -ContentType 'application/json' -Body $body -TimeoutSec 180).embeddings
}
function Get-Cosine($Left, $Right) {
  $dot=0.0; $aa=0.0; $bb=0.0
  for ($index=0; $index -lt $Left.Count; $index++) { $dot += $Left[$index]*$Right[$index]; $aa += $Left[$index]*$Left[$index]; $bb += $Right[$index]*$Right[$index] }
  if ($aa -eq 0 -or $bb -eq 0) { return -1.0 }
  $dot / ([Math]::Sqrt($aa) * [Math]::Sqrt($bb))
}
function Get-Terms([string]$Text) {
  @(($Text.ToLowerInvariant() -split '[^\p{L}\p{Nd}]+') | Where-Object { $_.Length -ge 3 } | Select-Object -Unique)
}
function Get-Lexical($QueryTerms, [string]$Text) {
  if ($QueryTerms.Count -eq 0) { return 0.0 }
  $textTerms = Get-Terms $Text
  $matches = @($QueryTerms | Where-Object { $textTerms -contains $_ }).Count
  $matches / $QueryTerms.Count
}

$started = Get-Date
$documentVectors = Get-Embeddings @($cases.Text)
$queryVectors = Get-Embeddings @($cases.Query)
$results = for ($queryIndex=0; $queryIndex -lt $cases.Count; $queryIndex++) {
  $terms = Get-Terms $cases[$queryIndex].Query
  $ranked = for ($documentIndex=0; $documentIndex -lt $cases.Count; $documentIndex++) {
    $semantic = Get-Cosine $queryVectors[$queryIndex] $documentVectors[$documentIndex]
    $lexical = Get-Lexical $terms $cases[$documentIndex].Text
    [pscustomobject]@{ Id=$cases[$documentIndex].Id; Score=(0.9*$semantic + 0.1*$lexical) }
  }
  $top = $ranked | Sort-Object Score -Descending | Select-Object -First 1
  [pscustomobject]@{ Query=$cases[$queryIndex].Query; Expected=$cases[$queryIndex].Id; Top1=$top.Id; Score=[Math]::Round($top.Score,4); Passed=($top.Id -eq $cases[$queryIndex].Id) }
}
$results | Format-Table -AutoSize
$passed = @($results | Where-Object Passed).Count
$summary = [pscustomobject]@{ Passed=$passed; Total=$results.Count; Accuracy=[Math]::Round(100*$passed/$results.Count,1); Seconds=[Math]::Round(((Get-Date)-$started).TotalSeconds,2); Model=$Model }
$summary | Format-List
if ($passed -ne $results.Count) { exit 1 }
