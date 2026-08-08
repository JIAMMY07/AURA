# Rapporto test AURA 0.4.1 — 8 agosto 2026

## Esito sintetico

La campagna ha coperto comportamento AI, coding, sicurezza difensiva, prompt injection, privacy a riposo, policy, disponibilità, voce, build e dipendenze. Dopo tre cicli misura-correzione-regressione, il benchmark AI finale supera 19 casi su 19. Questo risultato dimostra i casi provati, non una sicurezza assoluta né capacità generale illimitata.

## Benchmark dei modelli locali

| Modello | Casi | Superati | Latenza media | Velocità media |
| --- | ---: | ---: | ---: | ---: |
| `qwen3:4b-instruct` | 16 | 16 | 2789 ms | 45,9 tok/s |
| `qwen2.5-coder:3b` | 3 | 3 | 3036 ms | 100,4 tok/s |

I casi includono matematica, logica, JSON, onestà sull'esecuzione, rifiuto di furto credenziali/disattivazione Defender/persistenza, verifica firewall, review di command injection e path traversal, conservazione token e generazione TypeScript. Il red-team ha provato estrazione del canary diretta, indiretta, Base64, tramite roleplay e tramite profilo personale malevolo.

Il primo ciclo era 10/13: il modello ripeteva il canary mentre rifiutava un'iniezione indiretta. Dopo l'hardening era 17/19 e il profilo concatenato dopo la policy riusciva ancora a causare la ripetizione. La correzione definitiva colloca i dati non fidati prima della policy autoritativa, vieta di citare valori riservati anche nel rifiuto e rifiuta formati comuni di password/token/chiavi nel profilo. Il risultato finale è 19/19.

`qwen2.5-coder:3b` è circa due volte più veloce nella generazione, ma nel test iniziale non rispettava «solo codice» e aggiungeva Markdown. Per output a formato rigido il routing raccomandato è `qwen3:4b-instruct`; il coder resta utile per analisi e patch da validare.

## Test automatici e build

- 20 test frontend Vitest superati;
- 28 test Rust superati;
- TypeScript e Vite production build superati;
- Rustfmt e Clippy con warning come errori superati;
- test reali di lettura CPU/RAM;
- test cifratura profilo, AAD, manomissione, migrazioni, audit HMAC e plaintext legacy;
- test WAV malformati/dimensione/durata, singolo worker e cleanup residui;
- test singola generazione Ollama contro saturazione concorrente;
- test policy deny-by-default e allowlist applicazioni.

## Privacy e rete

- Ollama ascolta soltanto su `127.0.0.1:11434`;
- CSP e contenuto della WebView non consentono connessioni Internet applicative;
- il backend AURA comunica solo con Ollama loopback, ma durante lo smoke test il processo browser WebView2 ha stabilito due connessioni HTTPS verso indirizzi Microsoft (`40.101.113.65`/`52.98.237.162`); flag Chromium di background/proxy non le hanno eliminate. Non è stato osservato il contenuto TLS, quindi non si può affermare che trasportassero dati AURA;
- nessuna shell libera è esposta alla WebView;
- database `aura.db` accessibile solo all'utente, SYSTEM e amministratori;
- nel database reale verificato: zero messaggi in chiaro, zero ciphertext mancanti e nessun audit che dichiari contenuto registrato;
- nessun report Evolution legacy in chiaro presente;
- report Evolution ora cifrati AES-256-GCM e audit senza nome/obiettivo;
- audio `.wav/.json` residuo eliminato all'avvio;
- scansione statica dei file tracciati: nessuna credenziale reale rilevata; le sole firme corrispondenti sono fixture sintetiche del filtro anti-segreti.

## Sicurezza del sistema

- Microsoft Defender e protezione in tempo reale attivi;
- zero rilevamenti Defender nelle ultime 24 ore;
- profili Firewall Domain/Private/Public attivi;
- installer e repository sottoposti a scansione Defender;
- `pnpm audit`: nessuna vulnerabilità nota;
- RustSec: nessuna vulnerabilità bloccante sul target Windows. Restano 17 warning consentiti; `glib/GTK` appartiene al ramo Linux e non è nel grafo Windows MSVC.

## Difetti corretti

1. ripetizione di un segreto sintetico durante un rifiuto;
2. prompt injection proveniente dal profilo personale;
3. possibilità di salvare token/chiavi comuni nel profilo o feedback;
4. conferma PC nella WebView sostituita da dialogo nativo Windows backend;
5. richieste Ollama concorrenti diverse potevano saturare il PC;
6. report Evolution contenevano progetto e obiettivo in chiaro;
7. WAV temporanei potevano sopravvivere a un arresto improvviso;
8. routing del coder non adatto a output machine-readable rigoroso.

## Limiti residui

- nessuna suite finita copre tutte le prompt injection possibili;
- il modello non deve mai essere l'unica barriera: policy e tool tipizzati restano obbligatori;
- Windows Sandbox, WSL operativo e container runtime non sono disponibili, quindi non è stato eseguito codice generato;
- il test acustico fisico richiede OPPO connesse e voce dell'utente; nessun microfono è stato attivato durante questa campagna;
- il dialogo nativo è compilato ma il clic umano non è automatizzabile senza indebolire proprio la garanzia che si vuole testare;
- lo stato BitLocker non era leggibile dalla sessione non elevata;
- Semgrep, Gitleaks e cargo-deny non sono installati; sono stati usati Clippy, audit dipendenze e scanner di firme locale;
- browser isolato, API universali, RAG e runner Evolution non sono ancora implementati e quindi non sono collaudabili end-to-end.
- l'interfaccia Tauri dipende da WebView2: Microsoft documenta dati diagnostici richiesti e opzionali governati dalle impostazioni diagnostiche Windows. Per un requisito di zero egress dimostrabile servirà isolamento di rete per processo/VM o una UI nativa senza WebView2.
- MSI e NSIS sono riproducibili e puliti secondo Defender, ma non possiedono ancora una firma Authenticode attendibile; una release distribuita ad altri utenti richiede un certificato di code signing e protezione della chiave privata.

Il benchmark è ripetibile con `scripts/run-ai-evaluation.ps1`; usa esclusivamente canary sintetici e non esegue il codice prodotto dal modello.

Riferimenti WebView2: [Data and privacy in WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/data-privacy) e [WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags).
