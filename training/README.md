# Curriculum locale AURA

Questo materiale specializza e verifica il comportamento di AURA, ma non modifica i pesi del modello base. Il modello `aura:4b` viene derivato da `qwen3:4b-instruct` tramite Modelfile, regole autoritative ed esempi few-shot.

Un fine-tuning LoRA completo non viene eseguito automaticamente sulla RTX 2060: richiederebbe un ambiente ML separato, dataset revisionato e valutazioni contro regressioni. L'autonomia reale dipende soprattutto dai tool tipizzati collegati all'app, non dalla capacità del modello di descrivere azioni.

Il file `pc-autonomy-curriculum.jsonl` è un corpus di regressione: ogni riga indica richiesta, intento atteso, decisione e requisito. Non contiene password, dati personali o comandi distruttivi eseguibili.

## Specializzazione per il codice

`coding-curriculum.jsonl` copre Python, Java, C, C++, C#, JavaScript, TypeScript,
HTML/CSS, PHP, SQL, Rust, PowerShell, Go e Kotlin. Il modello Ollama derivato è
definito in `ollama/AURA-Coder.Modelfile` e si crea con:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/create-aura-coder-model.ps1
```

Questa è una specializzazione locale tramite istruzioni ed esempi, non un fine-tuning
LoRA dei pesi. AURA instrada automaticamente le richieste di programmazione al
modello coder e mantiene `aura:4b` per le conversazioni generali.

## Curriculum generale

`general-assistant-curriculum.jsonl` definisce regressioni per risposta diretta,
formati, giudizio critico, incertezza, ambiguità, pianificazione, privacy,
prompt injection, controllo PC, memoria e adattamento. Il benchmark ripetibile è:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-general-ai-benchmark.ps1
```

Il curriculum non viene trattato come memoria autobiografica e non autorizza nuove
azioni sul PC. Ogni miglioramento deve superare i test senza indebolire guardrail,
onestà sull'esecuzione o separazione tra dati e istruzioni.

## Sicurezza e incident response

`security-incident-curriculum.jsonl` specializza il comportamento per triage,
contenimento verificato, conservazione delle evidenze, integrità di AURA, recupero,
prompt injection e divieto di hack-back. Il benchmark è eseguibile con:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-security-ai-benchmark.ps1
```

Il modello non decide autonomamente le modifiche al sistema: policy Rust, conferme
native e risultati Defender sono autoritativi. Una risposta testuale non costituisce
mai prova di neutralizzazione.

## Auto-miglioramento del sorgente

`self-improvement-curriculum.jsonl` copre definizione dell'obiettivo, patch minima,
selezione del contesto, test di regressione, errori, rollback, file protetti, segreti
e promozione. Il ciclo autonomo può generare e validare una patch, ma non può
riscrivere il processo in esecuzione né promuoversi senza conferma.
