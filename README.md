# AURA — Local AI Assistant

Assistente desktop Windows local-first per chat privata, voce, strumenti PC controllati, ricerca web isolata e crescita tramite moduli verificati in sandbox.

## Stato

- Fase 0: architettura, backlog e repository completati.
- Fase 1: shell UI, policy engine e simulatore azioni completati.
- Fase 2: Ollama streaming, profili hardware, SQLite e installer Windows completati.
- Fase 2.5: backend Rust fidato, cifratura Windows, audit HMAC e CSP completati.
- Fase 2.6: Ollama mediato dal backend, blocco modelli cloud e cifratura contestuale completati.
- Fase 3: in corso — push-to-talk, Whisper locale, VAD e confidenza completati; Piper resta da fare.

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

Documentazione: `docs/ARCHITECTURE.md`, `docs/BACKLOG.md`, `docs/HARDENING.md`, `docs/MODELS.md`, `docs/VOICE.md`, `docs/REPOSITORY.md` e `project-spec.yaml`.
