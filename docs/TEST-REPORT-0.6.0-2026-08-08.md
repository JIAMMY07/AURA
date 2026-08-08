# Rapporto di completamento AURA 0.6.0 — 8 agosto 2026

## Esito sintetico

| Area | Codice nello scope dichiarato | Verifica reale | Residuo |
| --- | ---: | ---: | --- |
| Chat locale Ollama | 100% | 95% | latenza a freddo elevata; workaround RTX applicato |
| Cifratura e privacy applicativa | 100% | 95% | firma locale non attendibile fuori dal PC; nessuna sicurezza è assoluta |
| Comandi vocali | 100% | 90% | prova acustica completa con microfono OPPO da eseguire dall'utente |
| Controllo PC sicuro | 100% del catalogo allowlist | 80% | non equivale al controllo arbitrario di tutte le API Windows |
| Evolution Lab | 100% pipeline codice | 60% operativo | riavvio e provisioning WSL+bubblewrap ancora necessari |

## Modifiche principali

- TTS locale Windows SAPI italiano, interruzione, lettura manuale/automatica delle risposte e input inviato via `stdin`.
- Invio vocale automatico opzionale soltanto con confidenza Whisper almeno 70%; conferme native indipendenti per azioni mutanti.
- Centro controllo PC, dieci destinazioni Windows allowlistate, terminazione PID con denylist e blocco sessione.
- Evolution Lab collegato end-to-end alla UI: proposta cifrata, generazione locale, runner isolato e promozione su branch.
- Generatore strutturato `search/replace`: AURA costruisce e verifica la diff Git; tre tentativi limitati e nessuna fiducia nei test dichiarati dal modello.
- Profili sandbox `Node`, `Rust` e `Static`; Node esegue `tsc --noEmit`, Rust usa Cargo offline.
- Quote bubblewrap: 600 s CPU, 8 GiB memoria virtuale, 256 processi, 256 file descriptor e 256 MiB per file.
- Vite/Vitest/plugin React aggiornati; audit npm da 5 vulnerabilità a zero.

## Test superati

- Rust: 37/37 test.
- Frontend: 20/20 test.
- `cargo clippy --all-targets -- -D warnings`: superato.
- TypeScript + Vite 8.2.1: superato.
- Parser di tutti gli script PowerShell: superato.
- `pnpm audit`: zero vulnerabilità note.
- `cargo audit`: nessuna vulnerabilità bloccante; 17 warning transitive non mantenute/unsound, principalmente GTK/Linux non usate dal target Windows.
- TTS: `Microsoft Elsa Desktop` `it-IT`, WAV RIFF locale da 83.360 byte.
- OPPO Enco Buds2 Pro: dispositivi Bluetooth, Hands-Free AG e Hands-Free presenti con stato `OK`.
- Ollama chat: risposta canary corretta; dopo `OLLAMA_FLASH_ATTENTION=false` e CUDA v13 il coder minimo risponde in 18,1 s a 12,7 tok/s.
- Generatore Evolution: inferenza CPU funzionante; patch Git applicabile prodotta. Una patch semanticamente errata è stata individuata, motivando il profilo Node obbligatorio consigliato.
- EXE 0.6.0: smoke test 6 s, processo responsivo, zero connessioni TCP stabilite osservate.

## Gate esterni ancora aperti

1. Il bootstrap WSL elevato è terminato con codice `3010`: riavviare Windows e rieseguire `scripts/setup-evolution-sandbox.ps1` come amministratore.
2. Dopo il marker `AURA_WSL_BWRAP_V1`, eseguire una proposta sintetica con profilo Node e verificare che la patch TypeScript errata venga respinta.
3. Eseguire dall'interfaccia una registrazione reale con `OPPO Enco Buds2 Pro Hands-Free`, controllare confidenza e provare TTS/interruzione.
4. La firma è presente con certificato locale `5556B95B2ED7FF2C47FCBF7A193B26D70DE9D619`, ma Windows restituisce `UnknownError` finché il certificato non è attendibile o sostituito da uno pubblico.

## Artefatti 0.6.0

- EXE SHA-256: `F19D31DEAF6E018201F9DC9955221BBD9FDEC98B615D8557E2D275824F07C656`
- MSI SHA-256: `D4C8BB1B83ADEBB625A00397D751E4BEB7C633CAAD0C1781AC101B46BE962CA7`
- NSIS SHA-256: `7DBBE34B755E4C63BFA1532019A577A7641462CECD2BBF0ED2B557FD2C595BAA`

## Nota sul runtime Ollama

Ollama 0.32.6 ha mostrato stalli ripetibili con Flash Attention sulla RTX 2060/Turing durante prompt di patch. Il setup AURA forza CUDA v13 e disattiva Flash Attention; il coder Evolution usa inoltre CPU e contesto 4k. Il workaround è coerente con segnalazioni upstream su stalli/forti regressioni del backend GPU, ma va rivalutato dopo ogni aggiornamento Ollama.

## Riconvalida del 8 agosto 2026, ore 16:20

| Area | Esito operativo verificato | Valutazione | Evidenza / limite |
| --- | --- | ---: | --- |
| Chat locale Ollama | superato con fallback CPU | 92% | Ollama 0.32.6 e sei modelli rilevati; `qwen3:4b-instruct` ha risposto `AURA_OK` in 6,47 s con `num_gpu: 0`. Il percorso CUDA ha invece raggiunto il timeout di 180 s ed è stato disattivato nel backend. |
| Cifratura e privacy | superato | 95% | test AES-GCM/AAD, antimanomissione, migrazioni cifrate e catena HMAC superati; SQLite `integrity_check=ok`; audit dipendenze frontend senza vulnerabilità note. La cifratura del disco resta correttamente manuale. |
| Comandi vocali | superato lato software | 90% | round-trip reale Microsoft Elsa TTS → WAV da 96.146 byte → Whisper: `Apri la calcolatrice.`; profili OPPO Bluetooth e Hands-Free `OK`. Manca soltanto una frase realmente pronunciata dall'utente nel microfono. |
| Controllo PC sicuro | superato per il catalogo dichiarato | 87% | snapshot reale, processi, 10 destinazioni allowlistate, terminazione PID protetta e blocco sessione con conferma nativa; binding UI/backend presenti. Non è e non deve essere controllo arbitrario senza policy. |
| Evolution Lab | codice e guardrail superati; runtime bloccato | 65% operativo / 95% codice | generazione, runner e promozione sono collegati; la prova senza WSL fallisce prima di applicare patch. Windows non ha ancora finalizzato WSL perché il riavvio richiesto non è avvenuto. |

Risultati regressione: 37/37 test Rust, 20/20 test frontend, Clippy `-D warnings`, build TypeScript/Vite, 12/12 script PowerShell analizzati senza errori e `pnpm audit` pulito. La build release è responsiva, usa 27,4 MiB nello smoke test e non ha mostrato connessioni TCP stabilite nei primi otto secondi.

Artefatti ricostruiti e firmati localmente dopo il fix CPU:

- EXE SHA-256: `8C687F153BCCB3ECD4FC6EBF5B9A454CBEEE6C7131B1D6B9600C7CF9E93A84BA`
- MSI SHA-256: `E28049C5A9CC970E04088D33C206345E76405128080CDFBA6C8AE51BAAEACB89`
- NSIS SHA-256: `72FC27811A453BA0F334B94E9CCEF225965906B95E0965E98E30E3EF7CB16EC9`

La firma usa il certificato locale con thumbprint `5556B95B2ED7FF2C47FCBF7A193B26D70DE9D619`; `UnknownError` indica che non è una firma pubblicamente attendibile, non che i file siano privi di firma.
