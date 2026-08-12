# Controllo PC e workspace

## Modello operativo

AURA usa un capability broker Rust deny-by-default. Il controllo esteso non equivale a una shell amministrativa: segue lo stesso principio di un agente coding locale, cioè piena operatività dentro cartelle esplicitamente autorizzate e conferme visibili per gli effetti che modificano il PC.

Restano disponibili anche metriche CPU/RAM/dischi, elenco limitato dei processi, terminazione PID con protezione dei processi critici, controllo multimediale, blocco sessione e pannelli Windows tipizzati.

## Arresto globale delle task

Il pulsante rosso `Arresta tutto`, sempre visibile, e la scorciatoia `Ctrl+Alt+S` annullano gli stream IA, chiudono il browser controllato, fermano voce/TTS, disarmano la visione e terminano esclusivamente i processi figli registrati da AURA per 3D, Security Center ed Evolution Lab. Non termina applicazioni dell'utente aperte tramite il controllo PC. L'evento è registrato nel ledger di audit.

## Workspace autorizzati

- massimo 8 radici specifiche;
- elenco delle radici cifrato AES-256-GCM con chiave nel Credential Manager;
- profilo utente completo, dischi, Windows, Program Files, ProgramData, AppData e cartelle di credenziali non autorizzabili;
- frontend usa un ID grant, non sceglie liberamente la radice a ogni chiamata;
- percorsi operativi sempre relativi;
- `..`, prefissi assoluti, symlink, junction e reparse point fuori radice bloccati;
- lettura UTF-8 fino a 2 MiB, ricerca fino a 200 risultati e profondità 12;
- scrittura atomica con rollback locale se la sostituzione fallisce;
- nessuna sovrascrittura implicita durante copia o spostamento;
- eliminazione esclusivamente tramite Cestino Windows.

Le operazioni disponibili sono elenco, ricerca, lettura, creazione/modifica di testo, creazione cartella, copia file, spostamento, apertura di documenti e Cestino. Script ed eseguibili presenti nel workspace non vengono aperti: il codice deve passare dal runner sandbox.

## Applicazioni installate

AURA indicizza fino a 300 collegamenti `.lnk` presenti nei Menu Start utente e di sistema. Il frontend riceve nome e ID HMAC, non il percorso del collegamento. All'avvio il backend ricalcola l'ID, verifica che il collegamento esista ancora e mostra una conferma nativa Windows. Percorsi eseguibili e argomenti arbitrari non sono accettati.

## Comandi chat e voce

Dopo aver autorizzato almeno un workspace, il primo grant può essere controllato anche tramite grammatica locale deterministica:

- `mostra i file del workspace`;
- `elenca i file in src`;
- `leggi file README.md`;
- `crea file note.txt con contenuto Ciao`;
- `crea cartella docs/nuova`;
- `apri file rapporto.pdf`;
- `cestina vecchio.txt`.

Questi intenti vengono interpretati soltanto dal messaggio dell'utente o dalla sua trascrizione vocale, non da testo trovato in pagine web o documenti RAG.

## Limiti intenzionali residui

- niente shell generata liberamente dal modello;
- niente esecuzione di `.exe`, `.ps1`, `.bat`, `.cmd`, `.js`, `.vbs`, `.msi` o collegamenti dal workspace;
- niente azioni elevate invisibili;
- niente piano multi-step autonomo finché sandbox forte e rollback transazionale non sono operativi.

Questi limiti non riducono il controllo quotidiano di file e applicazioni, ma impediscono che una prompt injection trasformi AURA in un esecutore arbitrario.
