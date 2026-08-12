# AURA — verifica Chat, Privacy, Voce e Controllo PC

Data: 8 agosto 2026. Macchina: Windows 11 Home, Ryzen 7 5700X, 48 GiB RAM, RTX 2060.

## Criterio di valutazione

Il 100% indica il superamento di tutti i gate dichiarati e automatizzabili del modulo. Non significa sicurezza matematica assoluta, riconoscimento vocale perfetto in ogni ambiente o accesso arbitrario a ogni funzione Windows.

## Risultati

| Modulo | Gate verificato | Risultato | Stato |
|---|---|---:|---|
| Chat Ollama | 10 richieste locali brevi consecutive, zero errori, p95 sotto 5 s | 10/10; media 0,54 s; p95 0,57 s | 100% gate |
| Chat Ollama | Risposta reale non banale | 140 token; 23,4 s; 6,16 token/s | superato |
| Privacy | Nessun payload legacy/in chiaro nelle tabelle sensibili | 0 righe non conformi | 100% gate |
| Privacy | Integrità DB, WAL, cifratura autenticata, audit sigillato | integrity_check ok; 20 eventi sigillati; test manomissione superato | 100% gate |
| Voce software | SAPI italiano → WAV → whisper.cpp/VAD → testo | riuscito in 3,0 s; frase riconosciuta al 75% | 100% pipeline software |
| Voce OPPO reale | Cattura acustica dalla cuffia fisica | test guidato aggiunto nell'interfaccia | da eseguire dall'utente |
| Controllo PC | Parser, policy deny-by-default, allowlist, protezione processi, conferme e audit | test automatici superati | 100% catalogo supportato |
| Controllo PC | Copertura indiscriminata di Windows | deliberatamente non supportata | non applicabile |

## Correzioni applicate

- I modelli ausiliari Guardian, coder ed embedding vengono scaricati dopo l'uso per non contendere risorse alla chat.
- I profili di contesto sono stati calibrati sul percorso stabile della macchina: 2k, 4k e 8k token.
- È stata aggiunta un'autodiagnosi privacy sul database usato realmente dall'app.
- È stato aggiunto il test guidato OPPO con frase attesa, confidenza, livello e risultato pass/fail.
- Il catalogo PC comprende applicazioni e pagine Impostazioni in allowlist, cartelle note, volume e controlli multimediali; shell e comandi arbitrari restano vietati.

## Suite finale

- Frontend: 20/20 test.
- Backend Rust: 50/50 test.
- TypeScript e build Vite: superati.
- rustfmt: superato.
- Clippy con warning trattati come errori: superato.
- Dipendenze pnpm di produzione: nessuna vulnerabilità nota.
- Build desktop: EXE debug, MSI e NSIS prodotti correttamente.

## Limiti residui dichiarati

- Il test acustico delle OPPO richiede che l'utente indossi le cuffie e pronunci la frase mostrata da AURA.
- Metadati tecnici locali (date, identificatori, nomi sorgente e URL configurati) non sono cifrati; contenuti, profilo, correzioni, documenti, embedding e segreti lo sono.
- Un malware già in esecuzione con lo stesso account Windows resta fuori dal modello di minaccia dell'applicazione.
- Le azioni PC distruttive o universali non vengono aggiunte per raggiungere una percentuale nominale: rimangono deny-by-default.
