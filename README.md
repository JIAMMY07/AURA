# AURA 0.8.0 — Local AI Assistant

Assistente desktop Windows local-first per chat privata, voce, strumenti PC controllati, ricerca web isolata e crescita tramite moduli verificati in sandbox. La release 0.8.0 aggiunge il contratto sicuro per il futuro plugin Alexa, pairing e revoca locali, protezione HMAC/anti-replay, DNS pinning per browser e API, oltre a RAG cifrato per PDF e DOCX con deduplicazione.

## Stato

- Fase 0: architettura, backlog e repository completati.
- Fase 1: shell UI, policy engine e simulatore azioni completati.
- Fase 2: Ollama streaming, profili hardware, SQLite e installer Windows completati.
- Fase 2.5: backend Rust fidato, cifratura Windows, audit HMAC e CSP completati.
- Fase 2.6: Ollama mediato dal backend, blocco modelli cloud e cifratura contestuale completati.
- Fase 3: pipeline locale completa nello scope corrente — push-to-talk, Whisper, VAD, invio vocale opzionale e TTS italiano SAPI interrompibile; resta la prova fisica finale con le cuffie OPPO.

- Fase 4: catalogo sicuro operativo — metriche, processi, dieci destinazioni Windows, terminazione PID protetta e blocco sessione.
- Fase 8: pipeline Evolution collegata alla UI; generazione, runner e promozione implementati, esecuzione fail-closed fino al riavvio/provisioning WSL+bubblewrap.
- Policy 0.6: capability backend applicate direttamente a chat, voce, profilo, archivio, strumenti PC ed Evolution Lab; deny-by-default.
- Personalizzazione: profilo e correzioni esplicite cifrati, deduplicati e limitati, usati soltanto dal modello Ollama locale.
- Guardrail: Granite Guardian è esposto come segnale consultivo; le policy Rust restano l'unica autorità perché il benchmark locale ha rilevato falsi positivi.
- Alexa: protocollo locale e interfaccia di pairing pronti; plugin Alexa, relay HTTPS e trasporto polling saranno realizzati e collaudati in una fase successiva.
- RAG: PDF, DOCX, testo, dati e codice sorgente; estrazione limitata, embeddings locali, contenuti cifrati e citazioni per frammento.
- Rete: browser automatico e connettori API vincolano le connessioni agli indirizzi IP pubblici già verificati per ridurre il rischio DNS rebinding/SSRF.

## Installazione già configurata sul PC

- Rust stable MSVC e Visual Studio 2022 Build Tools;
- Ollama locale;
- `qwen3:4b-instruct` come modello consigliato;
- `qwen3:4b` come variante reasoning;
- `qwen2.5-coder:3b` per coding e manutenzione dei moduli;
- `qwen2.5vl:3b` per immagini, schermate e documenti;
- `qwen3-embedding:0.6b` per memoria semantica e RAG;
- `granite3-guardian:2b` per classificazione locale dei rischi;
- `whisper.cpp` 1.9.2 con modello multilingue `base` per la dettatura locale.
- selezione e riconnessione del microfono Bluetooth `OPPO Enco Buds2 Pro Hands-Free`.

## Sviluppo

```powershell
pnpm install
pnpm dev
pnpm test
pnpm build
pnpm build:windows:debug
./scripts/setup-whisper.ps1
./scripts/setup-ollama-models.ps1
```

Il backend Rust usa esclusivamente Ollama su `127.0.0.1:11434` e rifiuta modelli cloud, remoti o non GGUF. In Tauri, la cronologia è cifrata in `aura.db`; la chiave principale è custodita dal Windows Credential Manager. La voce viene convertita in WAV PCM locale e trascritta da `whisper.cpp`; la trascrizione resta una bozza finché l'utente non la invia. L'anteprima web usa esclusivamente un fallback locale.

Documentazione: `docs/ARCHITECTURE.md`, `docs/ALEXA-INTEGRATION.md`, `docs/BACKLOG.md`, `docs/HARDENING.md`, `docs/MODELS.md`, `docs/VOICE.md`, `docs/PC-CONTROL.md`, `docs/AI-3D.md`, `docs/EVOLUTION-LAB.md`, `docs/PERSONALIZATION.md`, `docs/REPOSITORY.md` e `project-spec.yaml`.

Rapporti: `docs/TEST-REPORT-0.6.0-2026-08-08.md`, `docs/TEST-REPORT-2026-08-08.md`, `docs/OPTIMIZATION-0.5.0.md` e `docs/HARDENING-REMEDIATION-2026-08-08.md`.
