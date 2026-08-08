# Modelli locali Ollama

## Profilo installato

La suite è dimensionata per AMD Ryzen 7 5700X, 48 GiB di RAM e NVIDIA RTX 2060 con 6 GiB di VRAM. I modelli da 3B-4B quantizzati sono il punto operativo consigliato: mantengono una buona accelerazione GPU senza affidarsi stabilmente all'offload su RAM.

| Ruolo | Modello | Dimensione indicativa | Uso previsto |
| --- | --- | ---: | --- |
| Chat | `qwen3:4b-instruct` | 2,5 GB | assistente quotidiano e istruzioni |
| Ragionamento | `qwen3:4b` | 2,5 GB | pianificazione e problemi complessi |
| Codice | `qwen2.5-coder:3b` | 1,9 GB | codice, test, correzioni e moduli sandbox |
| Visione | `qwen2.5vl:3b` | 3,2 GB | schermate, immagini e documenti |
| Embedding | `qwen3-embedding:0.6b` | 639 MB | RAG, memoria e ricerca semantica multilingue |
| Guardrail | `granite3-guardian:2b` | 2,7 GB | rischio, jailbreak, rilevanza e groundedness RAG |

Whisper e Piper non sono modelli Ollama: restano servizi locali separati rispettivamente per trascrizione e sintesi vocale.

## Regole di esecuzione

- Caricare un solo modello GPU pesante alla volta.
- Usare il profilo `Bilanciato` e 8192 token di contesto come default.
- Attivare `think` solo per modelli che dichiarano la capacità `thinking`; il backend non inoltra la catena interna all'interfaccia.
- Scaricare il modello specializzato dopo l'uso con `keep_alive: 0` quando viene invocato da un job isolato.
- Non mostrare i modelli con sola capacità `embedding` nel selettore della chat.
- Non eseguire mai codice generato direttamente: il modello di coding produce una proposta da validare, testare in sandbox e sottoporre alla policy.
- Il guardrail aggiunge un segnale di rischio, ma non sostituisce policy deterministiche, approvazioni, sandbox o controlli Windows.
- `granite3-guardian:2b` ha superato il controllo differenziale in inglese (`No` innocuo, `Yes` rischioso), ma ha prodotto falsi positivi nei test italiani. Finché non viene calibrato, è solo consultivo, riceve input normalizzato in inglese e non può autorizzare o bloccare azioni da solo.

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
