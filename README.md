# AURA — Local AI Assistant

Assistente desktop Windows local-first per chat privata, voce, strumenti PC controllati, ricerca web isolata e crescita tramite moduli verificati in sandbox.

## Stato

- Fase 0: architettura, backlog e repository completati.
- Fase 1: shell UI, policy engine e simulatore azioni completati.
- Fase 2: Ollama streaming, profili hardware, SQLite e installer Windows completati.
- Fase 3: prossima — push-to-talk, Whisper locale e Piper.

## Installazione già configurata sul PC

- Rust stable MSVC e Visual Studio 2022 Build Tools;
- Ollama locale;
- `qwen3:4b-instruct` come modello consigliato;
- `qwen3:4b` come variante reasoning opzionale.

## Sviluppo

```powershell
pnpm install
pnpm dev
pnpm test
pnpm build
pnpm build:windows:debug
```

L'app usa `http://127.0.0.1:11434` e non invia le conversazioni a servizi cloud. In Tauri, la cronologia è salvata in `aura.db`; l'anteprima web usa esclusivamente un fallback locale.

Documentazione: `docs/ARCHITECTURE.md`, `docs/BACKLOG.md`, `docs/REPOSITORY.md` e `project-spec.yaml`.
