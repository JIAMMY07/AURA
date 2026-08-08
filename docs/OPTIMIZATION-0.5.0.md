# Ottimizzazione e hardening AURA 0.5.0

## Risultato

La release 0.5.0 riduce la distanza fra policy dichiarata e comportamento effettivo. Ogni comando che tratta chat, cronologia, profilo, voce, strumenti PC o proposte Evolution verifica una capability backend statica. Le capability sconosciute e le classificazioni incoerenti vengono negate per impostazione predefinita.

## Miglioramenti principali

- La cancellazione della cronologia cifrata richiede conferma Windows nativa, distingue l'annullamento dal guasto e registra un evento privo di contenuto.
- Il contesto personale limita ogni campo, preserva UTF-8, deduplica le correzioni e include soltanto le otto più recenti; questo riduce token, latenza e rischio di contesto avvelenato.
- I report Evolution sono decifrati e mostrati solo dopo la verifica completa di envelope, versione, dimensione, UUID, autenticazione AES-GCM e identità AAD.
- Il rilevamento sandbox non confonde più la presenza di `wsl.exe`, `docker.exe` o `podman.exe` con un ambiente operativo: prova distribuzione/daemon e applica timeout.
- `executionAllowed` resta deliberatamente `false` per ogni runtime rilevato: la presenza del software non dimostra isolamento, rete negata, mount sicuri, distruzione e scansione degli output.
- Granite Guardian viene presentato come consultivo e non come barriera bloccante. Il controllo autorizzativo resta deterministico in Rust, evitando che i falsi positivi del modello blocchino azioni innocue.

## Limiti espliciti

Questa release non concede shell libera, privilegi amministrativi autonomi, auto-installazione sull'host o auto-promozione del codice. L'Evolution Lab conserva e verifica proposte, ma non le esegue. Browser isolato, connettori API, RAG e sicurezza difensiva avanzata restano fasi successive del backlog.

## Criteri di verifica

- test frontend e backend tutti verdi;
- TypeScript e build Vite senza errori;
- `cargo clippy` con warning trattati come errori;
- benchmark AI locale, audit dipendenze e build installer ripetuti prima della consegna;
- nessuna affermazione di sicurezza assoluta: i risultati valgono per casi, macchina e versioni provate.

## Esito verificato — 8 agosto 2026

| Controllo | Risultato |
|---|---:|
| Vitest frontend | 20/20 |
| Test Rust | 33/33 |
| Benchmark AI/coding/cyber locale | 19/19 |
| TypeScript + Vite production | superato |
| Clippy `-D warnings` | superato |
| `pnpm audit --prod` | 0 vulnerabilità note |
| `cargo audit` | 0 vulnerabilità bloccanti; 17 warning consentiti multipiattaforma/unmaintained |
| Avvio smoke dell'eseguibile release | superato |
| Defender su MSI e NSIS | nessuna minaccia, exit code 0 |

Gli artefatti 0.5.0 sono riproducibili localmente ma non firmati Authenticode. SHA-256: eseguibile `6FA8EA4627D47CC25A0DA34C86D53939714E4A313D2A0762F3A779019C94DA78`; MSI `01D9DEF89EC15B84197E0C94FBFE5AE6FB89CAA0A9C9490238A293336E599A8E`; NSIS `BB90123B7F9BCAB633585F133C51FB87570F1FD2F48D1033D9396375F51E5D24`.
