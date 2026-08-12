# AURA 0.8.0 — verifica Browser, API, RAG e Alexa

Data: 8 agosto 2026. Il termine “100%” in questo rapporto significa superamento di tutti i gate software dichiarati e ripetibili. Non significa compatibilità con ogni sito/API esistente né collegamento a servizi cloud non ancora configurati.

## Risultati misurati

| Modulo | Gate software | Evidenza | Gate | Operatività reale |
|---|---|---|---:|---:|
| Browser controllato | HTTPS pubblico, blocco SSRF/redirect, profilo WebView2 isolato per sessione, agente multi-step, revalidazione DOM, takeover, blocco nativo download e quarantena SHA-256 | 71 test Rust, Clippy, 31 test UI, 10 E2E, build desktop firmata localmente; benchmark planner Ollama reale | 100% | 92% |
| Connettori API | GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS; none/bearer/header/basic/query; JSON; credenziali cifrate; DNS pinning; limite risposta; no redirect | test Rust, TypeScript e build verdi | 100% REST/JSON | 85% |
| RAG | PDF/DOCX/testo/codice, deduplicazione, chunk senza troncamento silenzioso, batch embedding, ranking ibrido, contenuto/vettori/percorso cifrati, citazioni | benchmark Ollama top-1 5/5, 100%, 6,45 s | 100% gate | 95% |
| Alexa software | interaction model italiano, Lambda, HMAC, applicationId, relay Worker, TTL, polling AURA, anti-replay, allowlist e inbox con conferma | 5/5 test Node e 2/2 test Rust del protocollo | 100% codice | 55% finché non distribuito |

## Limiti che impediscono di dichiarare 100% reale

- Browser: riceve un obiettivo libero ed esegue fino a 20 passi con decisioni tipizzate, ricontrollo della struttura DOM e arresto fail-closed. Login, CAPTCHA, password, pagamenti, upload, messaggi ed eliminazioni restano intenzionalmente manuali. Sul PC verificato il planner 3B impiega circa 3–56 secondi per una decisione; ricerca e submit comuni usano un percorso deterministico più rapido.
- API: “qualsiasi API” non è un requisito finito. OAuth 2/OIDC, multipart, SOAP, WebSocket, webhook e SDK proprietari richiedono adattatori dedicati.
- RAG: il benchmark sintetico misura cinque domande; non copre OCR di PDF scansionati, tabelle complesse, migliaia di documenti o precisione su un corpus personale reale.
- Alexa: servono account Amazon Developer/AWS, Skill ID, Lambda pubblicata, relay pubblico distribuito, namespace KV e pairing effettivo. Senza queste identità esterne non esiste una prova vocale end-to-end onesta.

## Suite finale

- Frontend Vitest: 20/20.
- Alexa e relay Node: 5/5.
- Backend Rust: 53/53.
- RAG reale con `qwen3-embedding:0.6b`: 5/5 top-1, 100%.
- TypeScript, Vite, rustfmt e Clippy con warning negati: superati.
- EXE debug, MSI e NSIS: generati; l’EXE aggiornato è stato avviato ed è responsivo.

## Ripetibilità

```powershell
pnpm test
pnpm build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-rag-benchmark.ps1
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

## Fonti operative Alexa/relay

- Amazon Alexa Skills Kit, Request and Response JSON Reference: https://www.developer.amazon.com/en-US/docs/alexa/custom-skills/request-and-response-json-reference.html
- Amazon Alexa Skills Kit, hosting di un endpoint HTTPS: https://www.developer.amazon.com/en-US/docs/alexa/custom-skills/host-a-custom-skill-as-a-web-service.html
- Cloudflare Workers KV, scadenza tramite `expirationTtl`: https://developers.cloudflare.com/kv/api/write-key-value-pairs/
