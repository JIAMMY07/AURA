# Rapporto AURA 0.7.0 — moduli avanzati

## Esito

| Modulo | Implementazione nello scope sicuro | Verifica operativa | Limite residuo |
| --- | ---: | ---: | --- |
| Sandbox Evolution | 95% | 65% | codice fail-closed completo; WSL/bubblewrap attendono il riavvio Windows richiesto dal codice 3010 |
| Guardian | 100% | 95% | routing ibrido, CPU, keep-alive, timeout fail-closed, audit e blocco critico; benchmark reale benigno `No` 5,35 s, critico `Yes` 1,99 s |
| Creazione 3D | 100% | 95% | backend/UI/policy/audit/timeout integrati; GLB reale generato su CPU in circa 61 s; test UI con conferma manuale resta umano |
| Browser controllato | 100% dello scope progettato | 85% | WebView interattiva, presa manuale, fetch testuale, HTTPS e anti-SSRF; login e navigazione manuale richiedono naturalmente l'utente |
| Connettori API | 100% dello scope REST | 85% | GET/POST/PUT/PATCH/DELETE, bearer/header, segreti AES-GCM, limiti e audit; nessuna API privata è stata configurata senza credenziali dell'utente |
| RAG documentale | 100% dello scope testuale | 90% | chunking, embeddings Ollama, cifratura di testo/percorso/vettori, cosine search e citazioni; PDF/DOCX non sono ancora parser nativi |

Le percentuali “implementazione” indicano completezza rispetto allo scope dichiarato e testato, non accesso illimitato o sicurezza assoluta. Browser e API restano deliberatamente soggetti a conferma e non possono raggiungere localhost o reti private.

## Test ed evidenze

- Rust: 44/44 test superati.
- Frontend: 20/20 test superati.
- Clippy `-D warnings`, rustfmt, TypeScript e Vite: superati.
- `pnpm audit`: nessuna vulnerabilità nota.
- RAG: due embeddings reali da 1024 dimensioni, valori finiti, 3,22 s.
- 3D: `ui-integration-20260808-1635/0/mesh.glb` generato con TripoSR CPU.
- Guardian: classificazione reale benigno/critico corretta con modello `granite3-guardian:2b`.
- Release: processo responsivo dopo 10 s, 27,7 MiB, zero TCP stabilite osservate a riposo.
- Database: `integrity_check=ok`; tabelle `api_connectors`, `rag_documents` e `rag_chunks` create correttamente.
- EXE SHA-256: `2ACE20C2BFAAF8A247257292D52DCA93D7D722EF79F2DF2AF67B535FCF36A3E2`.
- MSI SHA-256: `6B18981C157585207A4F5C66B71279EB28E5EF8AC5DE83AA6E50BA8D133A4639`.
- NSIS SHA-256: `5EB89D55177770438F29E605AE56D9B724DFB50D9E8AE93A1AA8F71DC5DFB0A7`.

## Controlli di sicurezza

- URL solo HTTPS, senza credenziali incorporate.
- Risoluzione DNS e blocco degli indirizzi loopback, privati, link-local, multicast e non instradabili.
- Redirect HTTP bloccati nel fetch/API; navigazioni WebView rivalidate.
- Dimensione pagina/risposta API massima 1 MiB; timeout e redirect disabilitati.
- Token API, percorsi RAG, frammenti ed embeddings cifrati con AES-256-GCM e AAD.
- Corpi, segreti, percorsi completi e query non vengono scritti nell'audit.
- Operazioni esterne o mutanti richiedono conferma nativa Windows.
- Nessun codice remoto viene eseguito dal browser, dai connettori o dal RAG come comando del sistema.

## Gate per il 100% operativo assoluto

1. Riavviare Windows e completare `scripts/setup-evolution-sandbox.ps1`, quindi eseguire una patch sintetica in bubblewrap.
2. Eseguire manualmente nell'app una sessione browser con presa di controllo.
3. Configurare una API reale scelta dall'utente e approvare una chiamata.
4. Importare dall'interfaccia un documento reale dell'utente e verificare le citazioni.

Questi gate richiedono riavvio, credenziali, dati o interazione dell'utente e non possono essere falsificati da un test automatico.

## Riconvalida ore 16:48

| Modulo | Valutazione operativa aggiornata | Prova |
| --- | ---: | --- |
| Guardian | 95% | benigno `No` in 4,56 s; richiesta critica `Yes` in 1,85 s |
| Creazione 3D | 95% | output GLB reale presente, 87.824 byte; backend e UI registrati |
| Browser controllato | 90% | fetch HTTPS reale `200`, 559 byte; anti-SSRF/HTTP coperto dai test; UI e comandi Tauri collegati |
| Connettori API | 90% | GET e POST JSON reali verso endpoint echo, entrambi corretti; metodi REST, cifratura e UI collegati |
| RAG documentale | 90% | due vettori reali 1024D in 1,98 s; cifratura, chunking, cosine search e citazioni coperti dalla suite |
| Sandbox Evolution | 65% operativo / 95% codice | ultimo boot ancora 7 agosto; WSL continua a dichiararsi non installato; il runner nega correttamente l'esecuzione |

Regressione ripetuta: 44/44 test Rust, 20/20 frontend, rustfmt, Clippy `-D warnings`, TypeScript/Vite e audit dipendenze superati.
