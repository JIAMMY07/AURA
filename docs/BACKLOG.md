# Backlog MVP e piano di rilascio

Ogni fase deve terminare con test automatici, prova manuale del caso principale, aggiornamento del threat model e criterio di rollback. Una fase non abilita permessi destinati a fasi successive.

## Fase 0 — Fondazioni (completata)

- [x] Architettura, confini di fiducia e non-obiettivi.
- [x] Specifica machine-readable hardware/security/capabilities.
- [x] Struttura repository, convenzioni e configurazione Tauri minima.

**Uscita:** documenti coerenti, dipendenze installabili e build frontend riproducibile.

## Fase 1 — Shell UI e policy (completata)

- [x] Dashboard desktop responsive con navigazione e stato risorse.
- [x] Profili Leggero/Bilanciato/Qualità.
- [x] Policy engine `allow/confirm/sandbox/deny`.
- [x] Simulatore di richieste e anteprima decisione.
- [x] Test dei percorsi allow, sandbox e deny.
- [ ] Persistenza SQLite dell'audit.

**Uscita:** nessuna azione sensibile appare autorizzata senza passare dalla policy; build TypeScript e test verdi.

## Fase 2 — Inferenza locale e persistenza (completata)

- [x] Client Ollama su `localhost:11434`, health check e messaggi di recupero.
- [x] Catalogo modelli e streaming cancellabile.
- [x] Profili modello/context coerenti con 6 GiB VRAM.
- [x] SQLite con migrazioni, sessioni, messaggi e audit append-only.
- [x] Limiti token, timeout e selezione modello con fallback.
- [x] Build desktop, MSI e installer NSIS verificati.

**Uscita:** chat locale streaming, arrestabile, funzionante offline; nessun dato lascia il PC.

## Fase 2.5 — Hardening del confine desktop (completata)

- [x] SQL arbitrario rimosso dalla WebView.
- [x] Storage e policy spostati in comandi Rust tipizzati.
- [x] Messaggi cifrati AES-256-GCM con chiave protetta dal Credential Manager Windows.
- [x] Migrazione automatica dei messaggi legacy in chiaro.
- [x] Audit concatenato e autenticato con HMAC, verificabile dall'interfaccia.
- [x] Policy backend deny-by-default e rischio non controllabile dal modello.
- [x] CSP stretta, timeout generazione e finestra conversazione basata sui token.
- [x] Test Rust per cifratura, manomissione, policy e migrazione.

**Uscita:** la WebView non può scrivere SQL; dati locali e audit superano test di alterazione. Restano obbligatori approval token e runner isolato prima di introdurre azioni PC.

## Fase 3 — Voce locale

- [ ] Acquisizione microfono push-to-talk e indicatore evidente.
- [ ] Whisper locale con VAD, lingua italiana e punteggio di confidenza.
- [ ] Piper locale con interruzione immediata della sintesi.
- [ ] Conferma obbligatoria se la trascrizione sensibile è ambigua.

**Uscita:** comando vocale end-to-end senza cloud; microfono inattivo fuori dalla finestra dichiarata.

## Fase 4 — Strumenti PC sicuri

- [ ] Registro tool tipizzati: apri app, note, metriche, ricerca file.
- [ ] Radici leggibili/scrivibili esplicite e path canonicalization.
- [ ] Dry-run, timeout, annullamento, circuit breaker e rollback.
- [ ] Processi figli senza privilegi amministrativi e output limitato.
- [ ] Approval UI con effetto, dati, destinazione e durata permesso.

**Uscita:** suite di abuso su traversal, injection, symlink e race; kill switch verificato.

## Fase 5 — Documenti e memoria RAG

- [ ] Ingestione incrementale PDF/DOCX/TXT con deduplica e quarantena.
- [ ] Ricerca ibrida e risposte con citazioni locali.
- [ ] Scope per raccolta, eliminazione completa e retention.
- [ ] Difese contro prompt injection nei documenti.

**Uscita:** risposta documentale tracciabile alla pagina/chunk e cancellazione verificabile.

## Fase 6 — Ricerca web e browser isolato

- [ ] Browser con profilo separato, download in quarantena e rete osservabile.
- [ ] Ricerca multi-fonte con URL, data, deduplica e limiti configurabili.
- [ ] Takeover manuale immediato; pausa completa dell'agente durante il controllo umano.
- [ ] Accesso esteso esplicito, temporaneo, revocabile e visibile.
- [ ] Blocco di password, pagamenti e azioni irreversibili senza conferma dedicata.

**Uscita:** test di prompt injection web; ripresa agente solo dopo consenso e nuova fotografia dello stato.

## Fase 7 — Connettori API universali

- [ ] Manifest con host, metodi, scope, schema, rate limit e classificazione dati.
- [ ] REST/OpenAPI, GraphQL, OAuth 2.1/OIDC, webhook; adapter gRPC/MCP.
- [ ] Vault credenziali, rotazione, redazione log e revoca.
- [ ] Separazione read/write e conferma per effetti esterni.

**Uscita:** connettore di esempio read-only e uno write con consenso; test SSRF e secret leakage.

## Fase 8 — Evolution Lab in sandbox

- [ ] SDK plugin, template firmato e API di capacità ristrette.
- [ ] Sandbox usa-e-getta/Windows Sandbox o VM con snapshot.
- [ ] SBOM, scansione dipendenze/licenze, test e report dei diff.
- [ ] Promozione manuale, canary, monitoraggio e rollback automatico.

**Uscita:** un plugin dimostrativo viene generato e provato senza modificare l'host, poi installato solo dopo approvazione.

## Fase 9 — Sicurezza difensiva

- [ ] Stato Defender, Firewall, BitLocker, aggiornamenti e backup.
- [ ] Analisi eventi e vulnerabilità con fonti e severità.
- [ ] Correzioni prima simulate e poi approvate singolarmente.
- [ ] Incident mode: isola rete/processo solo tramite playbook autorizzato.

**Uscita:** report ripetibile; nessuna capacità offensiva, persistenza occulta, furto segreti o disattivazione autonoma delle protezioni.

## Definition of Done generale

- test unitari e integrazione verdi;
- threat case e failure case documentati;
- nessun segreto nei log o fixture;
- accessibilità tastiera e screen reader per il flusso modificato;
- consumo risorse misurato sul PC target;
- migrazione e rollback provati;
- documentazione e changelog aggiornati.
