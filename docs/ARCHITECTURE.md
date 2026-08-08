# Architettura tecnica

## Obiettivo e principi

AURA è un assistente Windows local-first. Il modello linguistico interpreta la richiesta e propone un piano strutturato, ma non riceve una shell libera e non decide autonomamente i permessi. L'esecuzione è deterministica: schema, policy, eventuale consenso, tool tipizzato, verifica del risultato e audit.

Principi obbligatori:

1. minimo privilegio e deny-by-default;
2. separazione tra ragionamento ed esecuzione;
3. conferma proporzionata al rischio e sempre revocabile;
4. simulazione prima di installazioni, plugin o auto-miglioramenti;
5. segreti mai inseriti nei prompt o nei log;
6. azioni tracciabili, con dry-run e rollback quando possibile;
7. contenuti web, documenti e risposte API sono dati non attendibili.

## Vista logica

```text
React UI / Voice UI
        |
Intent + contesto
        v
Agent Orchestrator ----> Ollama Adapter
        |
ActionPlan tipizzato
        v
Policy Engine ----> Approval Broker ----> Kill Switch
        |
Tool Registry
   |        |          |           |
PC tools  Browser   Connectors   Sandbox/VM
   |        |          |           |
Result validator + Audit ledger + Rollback metadata
        |
Encrypted local storage
```

## Componenti

- **Desktop UI:** Tauri 2, React e TypeScript. Chat, push-to-talk, piano, approvazioni, cronologia, uso risorse, log e arresto immediato.
- **Agent Orchestrator:** mantiene la macchina a stati del lavoro; impone limiti di passi, tempo, token e tentativi.
- **Ollama Adapter:** health check, catalogo modelli, streaming e profili Leggero/Bilanciato/Qualità.
- **Policy Engine:** valuta capacità, oggetto, destinazione, impatto, reversibilità e provenienza dei dati. Restituisce `allow`, `confirm`, `sandbox` o `deny`.
- **Approval Broker:** mostra una descrizione concreta dell'effetto e usa autorizzazioni a singola azione, sessione o regola stretta.
- **Typed Tool Registry:** espone solo strumenti registrati con input/output JSON validati. Niente comandi generati liberamente dal modello.
- **PC Runner:** azioni Windows consentite, processi figli con timeout, radici scrivibili e identità non amministrativa.
- **Browser Runner:** profilo separato per ricerche automatiche; allowlist/denylist, download in quarantena, takeover manuale e modalità accesso esteso esplicita e temporanea.
- **Connector Host:** REST/OpenAPI, GraphQL, OAuth 2.1/OIDC, webhook, gRPC e MCP tramite manifest e scope minimi.
- **Evolution Lab:** genera una proposta di plugin, esegue analisi e test in sandbox/VM con dati fittizi, produce diff e report; la promozione richiede firma e consenso.
- **Security Center:** modulo Tauri/UI con postura Defender, firme, scansioni, Firewall, UAC, Secure Boot, BitLocker, Windows Update, hash/firma dell'eseguibile e audit; solo difesa, nessun exploit, modifica delle esclusioni o estrazione di credenziali.
- **Storage:** SQLite per stato e audit; indice vettoriale opzionale; Windows Credential Manager + DPAPI per segreti; cifratura applicativa autenticata.

## Flusso di una richiesta

1. testo o voce viene normalizzato e, per la voce, accompagnato da confidenza;
2. l'orchestratore produce un `ActionPlan` con obiettivo, prerequisiti e strumenti;
3. input e piano vengono validati contro schemi locali;
4. il policy engine assegna decisione e motivazioni;
5. se necessario, l'utente vede anteprima, destinazione e dati coinvolti;
6. il runner opera entro timeout, limiti risorse e directory autorizzate;
7. un validatore controlla l'esito, senza fidarsi del solo exit code;
8. audit e metadati di rollback vengono salvati; l'utente riceve un riepilogo.

## Livelli di rischio

| Livello | Esempi | Decisione predefinita |
|---|---|---|
| Safe | leggere metriche, aprire un'app nota | allow |
| Moderate | creare un file nel workspace, chiamare API senza scrittura | confirm configurabile |
| Sensitive | inviare dati, modificare configurazioni, installare dipendenze | confirm sempre |
| Critical | privilegi, sicurezza, modifiche di sistema, plugin generato | sandbox prima della conferma |
| Forbidden | esportare segreti, disattivare protezioni, escalation autonoma | deny |

La modalità di accesso esteso non supera i divieti assoluti e non rende attendibili le istruzioni provenienti dal web.

## Isolamento e auto-evoluzione

Ogni estensione è un pacchetto versionato con manifest, permessi, hash, origine e test. La pipeline è: proposta -> scansione statica/SBOM -> installazione in ambiente usa-e-getta -> test funzionali e di abuso -> diff filesystem/rete/registro -> report -> approvazione -> firma -> installazione controllata -> monitoraggio -> rollback. La rete è negata nella sandbox salvo host dichiarati e temporanei.

## Crittografia e segreti

- chiavi radice protette con DPAPI e segreti in Windows Credential Manager;
- AES-256-GCM o XChaCha20-Poly1305 per dati applicativi;
- Argon2id solo quando una password utente deve derivare una chiave;
- nonce univoci, rotazione, versionamento e autenticazione dei metadati;
- BitLocker raccomandato per protezione del disco a riposo;
- log con redazione automatica di token, cookie, header e dati personali.

## Profilo hardware

Target: Ryzen 7 5700X, 48 GiB RAM, RTX 2060 6 GiB VRAM, Windows 11. Profilo predefinito: modello 3B/4B Q4, contesto chat 8k e un solo carico GPU pesante. Il coder Evolution usa 4k per evitare reload/stalli del runner; Whisper usa il modello base e il TTS usa Windows SAPI. I job non interattivi hanno deadline e il modello effettua fallback su un profilo più leggero.

## Affidabilità e osservabilità

Ogni job ha ID di correlazione, stato, deadline, numero massimo di retry e causa terminale. Metriche locali: latenza, token/s, CPU/RAM/VRAM, errori per tool e frequenza di annullamento. I log sono locali, strutturati, cifrabili e con retention configurabile. Circuit breaker impediscono loop e chiamate ripetute a servizi non disponibili.

## Desktop gate

Tauri 2 richiede Rust/Cargo e Microsoft C++ Build Tools. WebView2 è normalmente già disponibile su Windows 11. Le capability Tauri rimangono minime e dichiarate in `src-tauri/capabilities`; nuovi plugin non ereditano automaticamente i permessi della finestra principale.
