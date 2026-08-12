# AURA 0.8.0 — Local AI Assistant

Assistente desktop Windows local-first per chat privata, voce, strumenti PC controllati, ricerca web isolata e crescita tramite moduli verificati in sandbox. La release 0.8.0 aggiunge il contratto sicuro per il futuro plugin Alexa, pairing e revoca locali, protezione HMAC/anti-replay, DNS pinning per browser e API, oltre a RAG cifrato per PDF e DOCX con deduplicazione.

## Installazione facile

Per affidare l'installazione direttamente a Codex usa [CODEX-INSTALL.md](CODEX-INSTALL.md): contiene un prompt pronto da copiare, controlli di sicurezza e installazione verificata su Windows.

Da PowerShell nella cartella del repository:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\install-for-users.ps1
```

Usa `-InstallPrerequisites` soltanto dopo aver approvato l'installazione dei componenti mancanti; `-InstallVoice` e `-InstallModels` aggiungono opzionalmente Whisper e la suite Ollama locale.

## Stato

- Fase 0: architettura, backlog e repository completati.
- Fase 1: shell UI, policy engine e simulatore azioni completati.
- Fase 2: Ollama streaming, profili hardware, SQLite e installer Windows completati.
- Fase 2.5: backend Rust fidato, cifratura Windows, audit HMAC e CSP completati.
- Fase 2.6: Ollama mediato dal backend, blocco modelli cloud e cifratura contestuale completati.
- Fase 3: pipeline locale completa nello scope corrente — selezione universale del microfono, push-to-talk, Whisper, VAD, invio vocale opzionale e TTS italiano SAPI interrompibile; restano le prove fisiche sui diversi tipi di ingresso.
- Fase 4.5: visione locale opt-in — rilevamento di volto, mani e gesti con MediaPipe, modelli inclusi e nessun salvataggio dei fotogrammi.

- Fase 4: controllo PC esteso — metriche e processi, catalogo completo delle app del Menu Start, workspace cifrati e revocabili, lettura/ricerca/editor, copia/spostamento, Cestino e comandi chat/voce tipizzati.
- Fase 8: Evolution Lab operativo su questo PC con WSL 2 + Ubuntu + bubblewrap; patch approvata per hash, test offline, canary in namespace distinto, attestazione v2 e promozione transazionale con rollback automatico. Resta fail-closed se il probe forte non supera marker, rete, mount e tmpfs.
- Policy 0.6: capability backend applicate direttamente a chat, voce, profilo, archivio, strumenti PC ed Evolution Lab; deny-by-default.
- Personalizzazione: profilo e correzioni esplicite cifrati, deduplicati e limitati, usati soltanto dal modello Ollama locale.
- Workspace: più progetti e conversazioni cifrate, titolo automatico, esportazione PDF e coda per-chat modificabile con eliminazione, priorità e invio automatico sequenziale.
- Guardrail: Granite Guardian è esposto come segnale consultivo; le policy Rust restano l'unica autorità perché il benchmark locale ha rilevato falsi positivi.
- Alexa: protocollo locale e interfaccia di pairing pronti; plugin Alexa, relay HTTPS e trasporto polling saranno realizzati e collaudati in una fase successiva.
- RAG: PDF, DOCX, testo, dati e codice sorgente; estrazione limitata, embeddings locali, contenuti cifrati e citazioni per frammento.
- Provider IA: la chat usa un contratto v1 indipendente dal runtime; Ollama è attivo, mentre llama.cpp server, LocalAI ed endpoint locali OpenAI-compatible sono slot fail-closed non ancora attivati.
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
- selezione manuale o automatica di qualsiasi microfono Windows, preferenza ricordata, fallback sul dispositivo predefinito e aggiornamento hot-plug.

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

Il backend Rust usa esclusivamente Ollama su `127.0.0.1:11434` e rifiuta modelli cloud, remoti o non GGUF. In Tauri, la cronologia è cifrata in `aura.db`; la chiave principale è custodita dal Windows Credential Manager. La voce viene convertita in WAV PCM locale e trascritta da `whisper.cpp`; il push-to-talk resta in bozza, mentre il wake word opzionale invia solo “AURA + comando” con confidenza almeno 70%. L'anteprima web usa esclusivamente un fallback locale.

Documentazione: `docs/ARCHITECTURE.md`, `docs/AI-PROVIDERS.md`, `docs/ALEXA-INTEGRATION.md`, `docs/BACKLOG.md`, `docs/HARDENING.md`, `docs/MODELS.md`, `docs/VOICE.md`, `docs/PC-CONTROL.md`, `docs/AI-3D.md`, `docs/EVOLUTION-LAB.md`, `docs/PERSONALIZATION.md`, `docs/REPOSITORY.md` e `project-spec.yaml`.

Rapporti: `docs/TEST-REPORT-0.6.0-2026-08-08.md`, `docs/TEST-REPORT-2026-08-08.md`, `docs/OPTIMIZATION-0.5.0.md` e `docs/HARDENING-REMEDIATION-2026-08-08.md`.
