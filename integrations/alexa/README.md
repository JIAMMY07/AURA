# Integrazione Alexa → AURA → PC

La cartella contiene il modello vocale italiano e un handler Node.js 20 per AWS Lambda. L'handler trasforma un intento Alexa in una busta AURA HMAC, la inserisce nel relay HTTPS e risponde subito ad Alexa. AURA interroga il relay in uscita ogni 15 secondi: nessuna porta del PC viene aperta.

## Variabili Lambda

- `ALEXA_SKILL_ID`: application ID della Custom Skill.
- `AURA_RELAY_URL`: URL HTTPS pubblico del relay, con slash finale.
- `AURA_PAIRING_SECRET`: segreto mostrato una sola volta da AURA durante il pairing.

`relay-worker.mjs` implementa il relay su Cloudflare Workers con un namespace KV associato come `AURA_COMMANDS`. Configurare `AURA_PAIRING_SECRET` come secret del Worker e non inserirlo in `wrangler.toml` o nel repository. Il relay offre:

- `POST /commands`: riceve la busta dall'handler Alexa.
- `GET /commands/next`: restituisce `204` oppure una singola busta JSON; richiede `Authorization: Bearer <pairing-secret>`.

Il Worker conserva al massimo un comando per 120 secondi e rifiuta sovrascritture con HTTP 409. Questa scelta evita una coda ambigua: Alexa invita semplicemente a riprovare quando AURA non ha ancora ritirato il comando precedente.

Le azioni sensibili non aggirano AURA: firma, scadenza, nonce anti-replay, allowlist e conferma locale restano obbligatori.

Riferimenti ufficiali Amazon: [request/response Custom Skill](https://developer.amazon.com/en-US/docs/alexa/custom-skills/request-and-response-json-reference.html) e [account linking](https://developer.amazon.com/en-US/docs/alexa/account-linking/add-account-linking.html).
