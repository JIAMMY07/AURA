# Modelli locali Ollama

## Profilo installato

La suite è dimensionata per AMD Ryzen 7 5700X, 48 GiB di RAM e NVIDIA RTX 2060 con 6 GiB di VRAM. I modelli da 3B-4B quantizzati sono il punto operativo consigliato. Su questo PC Ollama 0.32.6 ha mostrato un blocco riproducibile durante l'offload CUDA di Qwen; il backend AURA forza quindi l'inferenza CPU (`num_gpu: 0`) per chat e Guardian. È meno veloce del percorso GPU ideale, ma nei canary reali evita timeout di 180 secondi e mantiene tutto locale.

| Ruolo | Modello | Dimensione indicativa | Uso previsto |
| --- | --- | ---: | --- |
| Chat | `qwen3:4b-instruct` | 2,5 GB | assistente quotidiano e istruzioni |
| Ragionamento | `qwen3:4b` | 2,5 GB | pianificazione e problemi complessi |
| Codice | `qwen2.5-coder:3b` | 1,9 GB | codice, test, correzioni e moduli sandbox |
| Visione | `qwen2.5vl:3b` | 3,2 GB | schermate, immagini e documenti |
| Embedding | `qwen3-embedding:0.6b` | 639 MB | RAG, memoria e ricerca semantica multilingue |
| Guardrail | `granite3-guardian:2b` | 2,7 GB | rischio, jailbreak, rilevanza e groundedness RAG |

Whisper non è un modello Ollama; la trascrizione usa `whisper.cpp` e la sintesi usa Windows SAPI con voce italiana.

## Modelli AURA visibili all'utente

| Nome | Velocità | Potenza | Contesto | Motore generale preferito |
| --- | --- | --- | ---: | --- |
| **Luna** | Massima | Essenziale | 4k | `qwen3:4b-instruct`, con fallback leggero |
| **Terra** | Bilanciata | Elevata | 8k | `aura:4b`, profilo predefinito |
| **Sole** | Ridotta | Massima | 12k | `qwen3:4b` reasoning, senza scadenza complessiva e con fallback AURA |

I tre nomi sono profili operativi, non falsi nomi dei pesi sottostanti. L'interfaccia mostra anche il motore Ollama effettivo per trasparenza. Coding, Guardian, embedding e visione restano specialisti interni e vengono selezionati soltanto quando servono.

## Regole di esecuzione

- Caricare un solo modello GPU pesante alla volta.
- Usare `Terra` come profilo predefinito; `Luna` privilegia la velocità e `Sole` usa fino a 12288 token di contesto.
- `Sole` non applica un timeout complessivo alla richiesta o allo stream. Il timeout di connessione iniziale resta attivo, il limite di output resta 2048 token e il pulsante **Stop** controlla l'annullamento almeno ogni 200 ms anche quando Ollama non produce token.
- Attivare `think` solo per modelli che dichiarano la capacità `thinking`; il backend non inoltra la catena interna all'interfaccia.
- Conservare i modelli interattivi per pochi minuti; il coder Evolution usa CPU e `keep_alive: 5m` per evitare reload concorrenti del runner.
- Non mostrare i modelli con sola capacità `embedding` nel selettore della chat.
- Non eseguire mai codice generato direttamente: il modello di coding produce una proposta da validare, testare in sandbox e sottoporre alla policy.
- Per output con formato rigoroso il coder Evolution usa uno schema JSON `search/replace`; AURA genera la diff e la verifica con Git, senza fidarsi di hunk o risultati di test dichiarati dal modello.
- Il guardrail aggiunge un segnale di rischio, ma non sostituisce policy deterministiche, approvazioni, sandbox o controlli Windows.
- `granite3-guardian:2b` ha superato un controllo differenziale in inglese, ma ha prodotto falsi positivi nei test italiani. Finché non viene calibrato, un filtro deterministico gli inoltra solo messaggi con indicatori di rischio; resta consultivo e non può autorizzare o bloccare azioni da solo.
- Sul PC target Ollama 0.32.6 con RTX 2060/Turing ha mostrato stalli con Flash Attention. Il setup imposta `OLLAMA_FLASH_ATTENTION=false` e `OLLAMA_LLM_LIBRARY=cuda_v13`; serve riavviare Ollama dopo la modifica.

## Verifica manuale

```powershell
ollama list
ollama ps
ollama run qwen2.5-coder:3b "Rispondi soltanto con OK"
```

Per gli embedding usare `POST /api/embed`; per chat, coding, visione e guardrail usare `POST /api/chat`. Tutto il traffico dell'applicazione resta mediato dal backend Rust verso `127.0.0.1:11434`.

## Fonti ufficiali

- [Qwen2.5-Coder su Ollama](https://ollama.com/library/qwen2.5-coder)
- [Qwen2.5-VL su Ollama](https://ollama.com/library/qwen2.5vl)
- [Qwen3 Embedding su Ollama](https://ollama.com/library/qwen3-embedding)
- [Granite 3 Guardian su Ollama](https://ollama.com/library/granite3-guardian)
