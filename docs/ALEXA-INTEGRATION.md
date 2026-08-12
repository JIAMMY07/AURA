# Integrazione Alexa di AURA

## Obiettivo e confine di fiducia

AURA è predisposta per ricevere in futuro comandi vocali da un plugin Alexa senza esporre una porta di rete sul PC. Il componente Alexa e il relay remoto non ricevono accesso generale a Windows: possono inviare esclusivamente intenti appartenenti a una allowlist. Ogni intento viene nuovamente valutato dai guardrail locali di AURA.

Il flusso obbligatorio è **Alexa → AURA → PC**. È vietata qualsiasi architettura **Alexa → PC → AURA** o Alexa → PC: Alexa non deve conoscere endpoint Windows, credenziali del PC, comandi di sistema, percorsi locali o API native. Il relay consegna soltanto una richiesta firmata ad AURA; AURA è l'unico componente autorizzato a decidere se trasformarla in un'azione sul PC.

Il supporto attuale implementa associazione, conservazione cifrata del segreto, autenticazione delle richieste, protezione anti-replay, scadenza e classificazione delle policy. Il futuro plugin Alexa e il relay HTTPS non fanno parte di questa versione.

## Architettura prevista

```text
Voce utente
   |
Dispositivo Alexa
   |
Alexa Custom Skill / funzione cloud
   |
Relay HTTPS autenticato
   ^
   | polling HTTPS in uscita; nessuna porta aperta sul PC
   |
AURA desktop -> verifica HMAC -> anti-replay -> policy locale -> eventuale conferma -> adapter PC
```

Responsabilità obbligatorie:

1. Alexa raccoglie la voce e produce un intento remoto limitato.
2. Il relay trasporta l'intento firmato senza interpretarlo come comando Windows.
3. AURA autentica, valida, applica policy e chiede eventuali conferme.
4. Soltanto un adapter locale di AURA può richiamare una funzione tipizzata del PC.
5. L'esito ritorna lungo il percorso inverso: PC → AURA → relay → Alexa.

Il client desktop dovrà contattare il relay soltanto tramite HTTPS pubblico. Il relay non deve poter iniziare una connessione verso il PC. Timeout, backoff con jitter e un limite di richieste devono essere applicati dal futuro trasporto.

## Associazione

1. L'utente inserisce in AURA il nome del dispositivo e l'URL HTTPS pubblico del relay.
2. AURA verifica schema, host e risoluzione verso indirizzi pubblici e richiede una conferma nativa.
3. AURA genera 32 byte casuali tramite il generatore crittografico del sistema.
4. Il segreto in Base64 URL-safe senza padding viene mostrato una sola volta.
5. A riposo, AURA conserva soltanto la copia cifrata AES-256-GCM, associata al contesto `aura-remote-pairing-v1|default`.
6. Il segreto non deve essere inserito in log, audit, telemetria, URL o messaggi Alexa.
7. La revoca elimina associazione, segreto cifrato e nonce già osservati.

Il futuro plugin deve offrire un canale amministrativo protetto per ricevere il segreto. In produzione è preferibile scambiarlo mediante un codice monouso a vita breve, invece di copiarlo manualmente in un pannello cloud.

## Busta firmata versione 1

```json
{
  "version": 1,
  "id": "UUID-v4",
  "issuedAt": 1786200000,
  "nonce": "valore_casuale_url_safe_minimo_16_caratteri",
  "command": "pc.status",
  "payloadJson": "{}",
  "signature": "base64url_senza_padding"
}
```

`payloadJson` è una stringa contenente JSON valido, non un oggetto JSON annidato. La sua lunghezza massima è 8192 byte/caratteri nel contratto attuale.

La stringa canonica da firmare usa esattamente questi campi, nell'ordine seguente, separati da `LF` (`\n`) e senza un newline finale:

```text
version\nid\nissuedAt\nnonce\ncommand\npayloadJson
```

La firma è:

```text
Base64UrlNoPadding(HMAC-SHA256(pairing_secret_raw, canonical_string_utf8))
```

Il `pairing_secret` visualizzato da AURA deve prima essere decodificato da Base64 URL-safe senza padding per ottenere la chiave HMAC originale di 32 byte.

## Controlli applicati da AURA

- versione del protocollo uguale a 1;
- identificativo comando in formato UUID;
- timestamp entro ±120 secondi dall'orologio del PC;
- nonce URL-safe lungo da 16 a 128 caratteri;
- nonce mai usato nelle precedenti 24 ore;
- payload JSON valido e limitato;
- confronto della firma HMAC resistente agli attacchi temporali;
- comando presente nell'allowlist;
- audit con richiesta/comando cifrati AES-256-GCM, mai con firma, nonce, token o credenziali; il contenuto viene decifrato solo su richiesta nel pannello Log;
- guardrail locale applicato anche dopo l'autenticazione remota.

## Allowlist iniziale

| Comando | Scopo | Policy locale |
|---|---|---|
| `pc.status` | Leggere metriche generali del PC | allow |
| `security.status` | Leggere la postura del Security Center | allow |
| `app.open` | Aprire una delle applicazioni già allowlistate | confirm |
| `system.lock` | Bloccare la sessione Windows | confirm |
| `chat.ask` | Inviare una richiesta alla chat locale | confirm |

Non sono ammessi shell, PowerShell arbitrario, installazione di software, gestione credenziali, disattivazione della sicurezza, accesso libero ai file o esecuzione di codice. Nuovi comandi devono essere introdotti singolarmente con schema del payload, test e policy esplicita.

## Esecuzione e risposte vocali future

Il verificatore attuale autentica e classifica la richiesta, ma non esegue ancora l'azione. Il dispatcher futuro dovrà:

1. deserializzare il payload mediante uno schema specifico per l'intento;
2. rifiutare campi sconosciuti e valori fuori limite;
3. richiamare soltanto funzioni AURA esistenti e tipizzate;
4. applicare conferma locale o challenge aggiuntiva per azioni mutanti;
5. produrre una risposta breve e priva di dati personali per Alexa;
6. registrare esito, latenza e policy senza contenuti sensibili.

Il dispatcher deve risiedere nel processo fidato di AURA. Non deve essere implementato nel plugin Alexa, nel relay o come servizio Windows direttamente raggiungibile dal relay.

La conferma locale non deve essere sostituita da frasi generiche udibili nella stanza. Per operazioni più sensibili si può aggiungere un PIN vocale temporaneo, una conferma nell'app o Windows Hello; il PIN permanente non deve essere pronunciato né registrato.

## Requisiti del futuro relay/plugin

- autenticazione dell'account e separazione rigorosa tra utenti;
- cifratura TLS e gestione dei segreti tramite un secret manager;
- coda per dispositivo con consegna singola e idempotenza;
- rate limit per account, dispositivo, IP e intento;
- scadenza server-side non superiore a quella del client;
- nessuna memorizzazione di trascrizioni non necessarie;
- comando di revoca e rotazione della chiave;
- registro amministrativo, alert sugli abusi e test di replay;
- risposta neutra quando il PC è offline, senza rivelare dettagli della rete;
- possibilità di disabilitare totalmente Alexa dall'interfaccia AURA.

## Criteri di completamento dell'integrazione futura

L'integrazione sarà considerata operativa soltanto quando esisteranno relay e plugin Alexa testati end-to-end su un account reale, inclusi: pairing, PC offline, retry, firma errata, timestamp scaduto, replay, revoca, rate limit, conferma locale, risposta vocale e assenza di porte in ascolto. La sola presenza del pannello AURA non equivale a compatibilità Alexa completa.
