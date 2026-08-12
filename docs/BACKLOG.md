# Backlog MVP e piano di rilascio

Ogni fase deve terminare con test automatici, prova manuale del caso principale, aggiornamento del threat model e criterio di rollback. Una fase non abilita permessi destinati a fasi successive.

## Fase 0 — Fondazioni (completata)

- [x] Architettura, confini di fiducia e non-obiettivi.
- [x] Specifica machine-readable hardware/security/capabilities.
- [x] Struttura repository, convenzioni e configurazione Tauri minima.

**Uscita:** documenti coerenti, dipendenze installabili e build frontend riproducibile.

## Fase 1 — Shell UI e policy (completata)

- [x] Dashboard desktop responsive con navigazione e stato risorse.
- [x] Profili Luna/Terra/Sole con potenza e velocità progressive.
- [x] Progetti e chat multiple cifrate con migrazione della cronologia esistente.
- [x] Esportazione PDF locale con impaginazione, metadati, numerazione e audit senza contenuti.
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

**Uscita:** chat e inferenza locali, streaming e arrestabili. Il contenuto AURA non usa servizi cloud; la diagnostica del runtime WebView2 resta governata da Windows e impedisce di dichiarare zero egress assoluto.

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

## Fase 2.6 — Hardening locale avanzato (completata)

- [x] Ollama spostato dietro comandi Rust e streaming Tauri Channel.
- [x] CSP senza accesso HTTP diretto dalla WebView.
- [x] Rifiuto backend di modelli cloud, remoti, vuoti o non GGUF.
- [x] Policy indipendente dalla reversibilità dichiarata dal frontend.
- [x] Cifratura messaggi v2 con AAD per ID, sessione, ruolo, data e versione.
- [x] Migrazione cifrata v0/v1, `secure_delete`, checkpoint WAL e compattazione.
- [x] Sigillatura una tantum degli audit legacy e riduzione dei dati sensibili nell'audit.
- [x] Buffer streaming UI per ridurre i rendering per token.

**Uscita:** la WebView non può raggiungere direttamente Ollama; un ciphertext spostato fra righe non è autenticabile; il frontend non può dichiarare un'azione reversibile. Approval token e runner restano gate obbligatori della Fase 4.

## Fase 3 — Voce locale (funzioni completate, collaudo hardware pendente)

- [x] Acquisizione microfono push-to-talk, limite 30 secondi e indicatore evidente.
- [x] Whisper locale multilingue con lingua italiana forzata, Silero VAD e confidenza media token.
- [x] Trascrizione in bozza e invio automatico opzionale solo con confidenza almeno 70%.
- [x] Selezione universale degli ingressi, preferenza utente, microfono predefinito Windows, hot-plug e ricampionamento 8/16/48 kHz.
- [x] Diagnostica livello audio, cleanup su errore e singolo worker Whisper.
- [x] TTS italiano locale Windows SAPI con interruzione immediata; Piper resta alternativa opzionale.
- [x] Conferma nativa obbligatoria per ogni azione PC mutante, indipendente dalla trascrizione.
- [ ] AudioWorklet e scorciatoia push-to-talk configurabile.

**Uscita:** comando vocale end-to-end senza cloud; microfono inattivo fuori dalla finestra dichiarata.

## Fase 4 — Strumenti PC sicuri

- [x] Registro tipizzato: metriche, processi, dieci destinazioni Windows, terminazione PID protetta e blocco sessione.
- [x] Centro controllo PC nella UI e grammatica vocale deterministica.
- [x] Comandi locali deterministici e nessuna shell generata dal modello.
- [x] Lettura, editor UTF-8 e ricerca file in radici esplicite.
- [x] Radici cifrate, revocabili, canonicalization, blocco traversal/symlink/junction e radici sensibili.
- [x] Scrittura atomica con rollback, copia/spostamento senza overwrite implicito e Cestino Windows.
- [x] Catalogo applicazioni del Menu Start con ID HMAC e nessun eseguibile/argomento arbitrario.
- [x] Conferma nativa per grant, write, move, open, delete e lancio applicazioni.
- [ ] Runner comandi generici isolato con timeout, annullamento, output limitato e processi non elevati.
- [ ] Planner multi-step con dry-run complessivo, rollback transazionale e approvazione del piano.

**Uscita:** controllo file e applicazioni installate operativo entro scope autorizzati; shell e codice restano confinati al runner sandbox.

## Fase 4.5 — Visione locale (rilevamento operativo)

- [x] Selezione videocamera e consenso esplicito Windows/WebView2.
- [x] Landmark facciali, mani e gesti tramite MediaPipe locale.
- [x] Modelli e runtime WASM locali con verifica SHA-256 in build.
- [x] Limite di frequenza, rilascio risorse e stato videocamera sempre visibile.
- [x] Nessuna conservazione dei frame e nessuna azione PC automatica dai gesti.
- [ ] Enrollment biometrico cifrato, liveness e cancellazione verificabile prima dell'identificazione personale.

**Uscita:** rilevamento locale di mani e volto utilizzabile senza cloud; l'identificazione della persona resta esplicitamente fuori dallo scope corrente.

## Fase 5 — Documenti e memoria RAG

- [ ] Ingestione incrementale PDF/DOCX/TXT con deduplica e quarantena.
- [ ] Ricerca ibrida e risposte con citazioni locali.
- [ ] Scope per raccolta, eliminazione completa e retention.
- [ ] Difese contro prompt injection nei documenti.

**Uscita:** risposta documentale tracciabile alla pagina/chunk e cancellazione verificabile.

## Fase 6 — Ricerca web e browser isolato

- [x] Browser con profilo WebView2 distinto per sessione, blocco nativo dei download, quarantena inerte con SHA-256 e rete HTTPS pubblica osservabile.
- [x] Agente browser goal-driven a passi limitati, DOM revalidato, schema JSON rigido, massimo quattro host e takeover immediato.
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

- [x] Diagnostica runtime e modalità fail-closed quando manca isolamento forte.
- [x] Registrazione proposta/report senza esecuzione né modifica del progetto host.
- [x] Report AES-256-GCM autenticati, legati all'ID e cronologia decifrata con limiti anti-abuso.
- [x] Rilevamento fail-closed: WSL richiede una distribuzione operativa; Docker/Podman devono rispondere entro il timeout.
- [x] Generazione patch Ollama, runner WSL+bubblewrap e promozione manuale collegati al backend/UI.
- [x] Quote CPU/RAM/processi/file descriptor/dimensione file e profili test allowlistati.
- [x] Ispezione diff prima dell'esecuzione: capability, file, dipendenze, indicatori sensibili e hash approvato.
- [x] Retry limitato a tre con invalidazione dell'approvazione e rollback del branch su apply fallita.
- [x] Attestazione sandbox, inventario licenze e scansione statica; dipendenze immutabili e rete disattivata.
- [ ] SDK plugin, template firmato e API di capacità ristrette.
- [ ] Sandbox usa-e-getta/Windows Sandbox o VM con snapshot.
- [ ] SBOM CycloneDX completo e scanner CVE offline aggiornabile con provenienza firmata.
- [x] Promozione manuale su branch di revisione, hash SHA-256, worktree pulito e nessun commit automatico.
- [ ] Canary, monitoraggio e rollback automatico dopo la revisione.

**Uscita:** un plugin dimostrativo viene generato e provato senza modificare l'host, poi installato solo dopo approvazione.

## Fase 9 — Sicurezza difensiva

- [x] Security Center: Defender, scansioni, definizioni, Firewall, UAC, Secure Boot, BitLocker, Windows Update, firma e hash eseguibile.
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
