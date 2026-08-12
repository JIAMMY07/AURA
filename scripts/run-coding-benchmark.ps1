param(
    [string]$Model = 'aura-coder:3b',
    [string]$OutputPath = '.audit-work\aura-coder-benchmark.json',
    [string[]]$Languages = @()
)

$ErrorActionPreference = 'Stop'
$cases = @(
    @{ language = 'Python'; prompt = 'Scrivi solo codice, massimo 15 righe: funzione Python fibonacci(n: int) iterativa che solleva ValueError per n negativo.'; required = @('def fibonacci', 'ValueError') },
    @{ language = 'Java'; prompt = 'Scrivi una classe Java 17 Counter thread-safe con metodo increment e value.'; required = @('class Counter', 'AtomicInteger') },
    @{ language = 'C'; prompt = 'Scrivi solo codice, massimo 15 righe: funzione C11 parse_positive_int con strtol, errno e controllo completo.'; required = @('strtol', 'errno') },
    @{ language = 'C++'; prompt = 'Scrivi solo codice, massimo 20 righe: classe C++17 FileHandle RAII basata su std::FILE*, con fclose nel distruttore, move abilitata e copy = delete.'; required = @('~FileHandle', '= delete', 'fclose') },
    @{ language = 'C#'; prompt = 'Scrivi un metodo C# async che scarica testo con HttpClient e CancellationToken.'; required = @('async', 'CancellationToken') },
    @{ language = 'JavaScript'; prompt = 'Scrivi una funzione JavaScript debounce senza librerie.'; required = @('function debounce', 'setTimeout') },
    @{ language = 'HTML'; prompt = 'Scrivi un form HTML accessibile con email, label e messaggio di errore aria-live.'; required = @('<label', 'aria-live') },
    @{ language = 'PHP'; prompt = 'Scrivi una funzione PHP 8 che cerca un utente con PDO e prepared statement.'; required = @('prepare', 'execute') },
    @{ language = 'SQL'; prompt = 'Scrivi una query SQL parametrizzata che seleziona ordini per customer_id e data minima.'; required = @('SELECT', 'WHERE') }
)

if ($Languages.Count -gt 0) {
    $cases = @($cases | Where-Object { $Languages -contains $_.language })
}
if ($cases.Count -eq 0) {
    throw 'Nessun caso di benchmark selezionato.'
}

$results = @(foreach ($case in $cases) {
    $body = @{
        model = $Model
        stream = $false
        think = $false
        messages = @(@{ role = 'user'; content = $case.prompt })
        options = @{ num_predict = 144; temperature = 0.1 }
    } | ConvertTo-Json -Depth 6

    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $response = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/chat' -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 180
        $watch.Stop()
        $text = [string]$response.message.content
        $missing = @($case.required | Where-Object { $text -notmatch [regex]::Escape($_) })
        [pscustomobject]@{
            language = $case.language
            passed = ($missing.Count -eq 0)
            elapsed_ms = $watch.ElapsedMilliseconds
            tokens_per_second = if ($response.eval_duration -gt 0) { [math]::Round($response.eval_count / ($response.eval_duration / 1000000000), 2) } else { 0 }
            missing_markers = $missing
            response = $text
        }
    } catch {
        $watch.Stop()
        [pscustomobject]@{
            language = $case.language
            passed = $false
            elapsed_ms = $watch.ElapsedMilliseconds
            tokens_per_second = 0
            missing_markers = @('request_failed')
            response = $_.Exception.Message
        }
    }
})

$summary = [pscustomobject]@{
    generated_at = (Get-Date).ToString('o')
    model = $Model
    passed = @($results | Where-Object passed).Count
    total = $results.Count
    pass_rate_percent = [math]::Round(100 * @($results | Where-Object passed).Count / $results.Count, 1)
    average_tokens_per_second = [math]::Round(($results | Measure-Object tokens_per_second -Average).Average, 2)
    results = $results
}

$target = Join-Path (Split-Path $PSScriptRoot -Parent) $OutputPath
$targetDirectory = Split-Path $target -Parent
New-Item -ItemType Directory -Force -Path $targetDirectory | Out-Null
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $target -Encoding UTF8
$summary | Select-Object model, passed, total, pass_rate_percent, average_tokens_per_second | Format-Table -AutoSize
$results | Select-Object language, passed, elapsed_ms, tokens_per_second, @{Name='missing';Expression={$_.missing_markers -join ', '}} | Format-Table -AutoSize

if ($summary.passed -ne $summary.total) {
    exit 1
}
