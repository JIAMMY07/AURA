# Verifica AURA Forge, memoria e voce — 9 agosto 2026

## Funzioni implementate

- ispezione del diff con SHA-256, file, capability, dipendenze e indicatori sensibili;
- approvazione nativa legata al diff corrente prima della sandbox;
- massimo tre tentativi, fallimenti persistiti e invalidazione dell'approvazione alla rigenerazione;
- dipendenze immutabili, rete disattivata, scansione statica, inventario licenze e attestazione del run;
- promozione su branch senza commit e ritorno al branch originale in caso di apply fallita;
- snapshot cifrati delle correzioni esplicite, heartbeat opzionale e rollback confermato;
- wake word “AURA” locale a segmenti, soglia 70% e follow-up opzionale dopo TTS;
- UI per diff, rischio, tentativi, memoria, heartbeat e controlli vocali.

## Risultati verificati

| Controllo | Risultato |
|---|---:|
| Rust unit/integration | 60/60 pass |
| TypeScript/Vitest | 28/28 pass |
| Alexa Node integration | 5/5 pass |
| Playwright E2E Chromium | 8/8 pass |
| TypeScript + Vite release build | pass |
| Rust Clippy `-D warnings` | pass |
| EXE/MSI/NSIS | generati |
| Firma locale + manifest SHA-256 | generati |

## Limiti osservati sul PC

- WSL risulta non installato nella sessione del test. La sandbox forte rimane quindi
  fail-closed: nessuna patch viene eseguita sull'host come sostituto insicuro.
- I modelli Ollama sono installati sotto
  `%LOCALAPPDATA%\Programs\Ollama\ollama.exe`, ma quella directory non
  è nel `PATH` della shell. Sono presenti `aura:4b`, `aura-coder:3b`,
  `granite3-guardian:2b`, `qwen3-embedding:0.6b`, `qwen2.5vl:3b`,
  `qwen2.5-coder:3b`, `qwen3:4b-instruct` e `qwen3:4b`.
- La firma usa un certificato locale `CN=AURA Local Development Code Signing`: protegge
  l'integrità locale ma non è pubblicamente attendibile, non ha timestamp pubblico e
  Windows la riporta come `UnknownError` finché il certificato non viene considerato
  attendibile nella catena locale.
- Wake word e prova acustica richiedono una verifica manuale reale con almeno un microfono disponibile;
  un test software non può certificare rumore, profilo Bluetooth o qualità della voce.

## Artefatti

- `src-tauri/target/release/local-ai-assistant.exe`
- `src-tauri/target/release/bundle/msi/Local AI Assistant_0.8.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Local AI Assistant_0.8.0_x64-setup.exe`
- `%LOCALAPPDATA%\AURA\signing\distribution-0.8.0-release.json`
