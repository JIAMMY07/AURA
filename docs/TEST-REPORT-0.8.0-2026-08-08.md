# Rapporto test AURA 0.8.0 — 8 agosto 2026

## Risultato

| Area | Verifica | Esito |
|---|---|---|
| Frontend | TypeScript e build Vite | superata |
| Frontend | Vitest | 20/20 |
| Backend | test Rust, tutti i target | 48/48 |
| Backend | Clippy con warning negati | superato |
| Packaging Windows | build Tauri debug | superata |
| Installer | MSI x64 | generato |
| Installer | NSIS x64 | generato |

## Copertura aggiunta in 0.8.0

- verifica HMAC-SHA256 e rifiuto delle manomissioni Alexa;
- policy remota deny-by-default e conferma per azioni mutanti;
- timestamp remoto con confronto senza overflow;
- nonce persistente anti-replay e revoca della chiave;
- connessioni HTTP automatiche vincolate agli IP pubblici validati;
- estrazione DOCX testata, limiti anti-espansione e importazione PDF locale;
- deduplicazione RAG tramite SHA-256 del testo estratto;
- migrazione SQLite per pairing remoto e hash documenti.

## Limiti dichiarati

- Il plugin Alexa e il relay HTTPS non sono ancora implementati: pairing e protocollo sono pronti, ma il comando vocale Alexa non è ancora end-to-end.
- La sandbox Evolution resta fail-closed finché il provisioning WSL/bubblewrap non diventa disponibile dopo il riavvio Windows già richiesto.
- La firma locale degli installer non equivale a una firma pubblicamente attendibile; serve un certificato di code signing pubblico e la relativa identità cloud/hardware.
- La prova fisica del microfono Bluetooth OPPO richiede interazione dell'utente con il dispositivo acceso e selezionato.
