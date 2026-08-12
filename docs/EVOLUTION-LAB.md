# Evolution Lab: auto-miglioramento confinato

## Principio di sicurezza

«Accesso completo» significa piena libertà soltanto dentro un ambiente effimero e separato. Non significa accesso autonomo illimitato a Windows, credenziali, profilo utente o repository originali. L'agente può proporre, generare e collaudare una modifica isolata; l'installazione sull'host richiede sempre revisione e approvazione umana.

## Stato rilevato su questo PC

Il 9 agosto 2026 il provisioning è stato completato: WSL 2.7.11, Ubuntu 2,
kernel 6.18.33.2 e bubblewrap 0.11.1 sono operativi. La distribuzione contiene il
marker `AURA_WSL_BWRAP_V1`; l'importazione del `PATH` Windows è disabilitata e il
processo bubblewrap riceve un ambiente ripulito con `PATH`, `HOME` e `LANG`
espliciti.

Il test host reale `pnpm test:evolution:isolation` ha verificato:

1. assenza di `/mnt/c` nel namespace isolato e rete disabilitata da `--unshare-all`;
2. repository host invariato prima/dopo il runner;
3. primo test e canary in namespace bubblewrap distinti;
4. attestazione v2 e SHA-256 della patch validata coerenti.

AURA continua a operare fail-closed: se marker o probe forte falliscono, registra
la proposta ma non esegue né promuove codice.

La versione 0.6.0 rilegge i report soltanto dopo aver verificato nome UUID, dimensione massima, versione dell'envelope, AES-GCM e identità autenticata come AAD. La UI mostra al massimo le 20 proposte più recenti e collega generazione, test e promozione. Un file corrotto, sostituito o associato a un altro ID viene rifiutato invece di essere interpretato.

Una cartella o un worktree Git non costituiscono una sandbox di sicurezza.

## Architettura del runner

Il runner usa WSL 2 + bubblewrap con:

- copia usa-e-getta del solo progetto necessario;
- input dell'host in sola lettura e nessun montaggio del profilo utente;
- assenza di Credential Manager, token, chiavi SSH, cookie e microfono;
- rete disattiva per impostazione predefinita; allowlist temporanea per dipendenze firmate;
- utente non amministratore, limiti CPU/RAM/disco/tempo e numero di processi;
- blocco di device, socket host e pipe privilegiate;
- diff, test, SBOM, licenze, malware scan e report vulnerabilità;
- esportazione del solo patch/artifact verificato;
- promozione manuale con commit dedicato, canary e rollback.

## Ciclo di apprendimento

Ogni tentativo produce dati strutturati: obiettivo, ipotesi, test fallito, causa, correzione, risultato e regressioni. Gli errori possono cambiare priorità e suggerimenti futuri, ma non possono allargare permessi, disattivare policy o auto-approvare una promozione. Il sistema apprende tramite memoria esplicita e retrieval locale; non modifica autonomamente i pesi del modello.

## Condizioni per abilitare l'esecuzione

La semplice presenza di un runtime non abilita l'esecuzione. `executionAllowed` diventa vero soltanto quando la distribuzione Ubuntu contiene il marker di provisioning AURA e il test bubblewrap dimostra tmpfs scrivibile, namespace separati e assenza del mount host `/mnt/c`. Docker/Podman e la sola presenza di WSL non sono sufficienti.

## Pipeline operativa AURA Forge

La pipeline è ora implementata in quattro passaggi separati:

1. `evolution-generate-patch.ps1` invia solo file Git tracciati e filtrati al coder locale, accetta operazioni JSON `search/replace` con anchor univoco e costruisce deterministicamente una patch Git verificata;
2. il backend calcola SHA-256, file cambiati, capability richieste, variazioni delle dipendenze e indicatori sensibili, quindi mostra il diff prima di qualsiasi esecuzione;
3. una conferma nativa approva esattamente quell'hash; ogni rigenerazione invalida l'approvazione. Sono ammessi al massimo tre tentativi;
4. `evolution-sandbox-run.ps1` monta progetto e patch in sola lettura dentro WSL+bubblewrap, senza rete né profilo host, applica la patch in tmpfs ed esegue un profilo allowlistato: `Node` (`tsc --noEmit`), `Rust` (`cargo test --offline`) o `Static`;
5. la sandbox blocca ogni modifica ai manifest/lockfile delle dipendenze e produce `attestation.json`, elenco file, scansione statica, inventario licenze, log e patch validata;
6. `evolution-promote-patch.ps1` richiede conferma, worktree pulito, hash valido e crea un branch `aura/evolution-*` senza commit automatico. Se `git apply` fallisce torna al branch originale ed elimina il branch incompleto.

Il runner applica limiti di 600 secondi CPU, 8 GiB di memoria virtuale, 256 processi, 256 file descriptor e 256 MiB per file. Il profilo generazione è ridotto a 4k token, massimo 10 file/80 KB di contesto e 768 token di output. Il coder usa la CPU: sul runtime Ollama 0.32.6 la RTX 2060/Turing ha mostrato stalli ripetibili sui prompt di patch, mentre il percorso CPU evita quel backend e lascia la GPU alla chat.

I file di policy, crittografia, sandbox, firma e promozione sono esclusi dalla generazione automatica. Il bootstrap `setup-evolution-sandbox.ps1` abilita WSL, installa Ubuntu e bubblewrap, disabilita l'importazione del `PATH` Windows ed esegue un test in cui `/mnt/c` deve risultare invisibile. Il provisioning è stato completato il 9 agosto 2026; il backend torna automaticamente fail-closed se il marker `AURA_WSL_BWRAP_V1` o il probe forte non sono verificabili.

## Ciclo autonomo controllato

Il pulsante **Genera e valida autonomamente** concatena registrazione della proposta,
generazione con `aura-coder:3b` e ispezione del diff, poi si ferma per l'approvazione
esplicita. Dopo l'approvazione l'utente può avviare i test isolati. Il repository AURA viene rilevato
risalendo dall'eseguibile soltanto quando sono presenti `.git`, `package.json` e
`src-tauri/Cargo.toml`. I file di contesto sono classificati in base ai termini
dell'obiettivo, limitati a 70 KB e 16 file.

Il ciclo si interrompe automaticamente quando:

- la sandbox forte non è verificata;
- il modello non dispone del contesto necessario;
- la patch tocca file protetti o segreti;
- un anchor non è univoco;
- `git apply --check`, i controlli statici o i test falliscono;
- la patch supera i limiti dimensionali.

La rigenerazione non installa pacchetti e non riusa automaticamente codice trovato su
Internet. Una futura capability può proporre una dipendenza, ma licenza, provenienza,
hash e vulnerabilità dovranno essere approvati in un flusso separato.

Una patch validata non viene applicata alla versione in esecuzione. La promozione
richiede conferma nativa, crea un branch di revisione e non crea commit automatici.

## Pipeline v2: test, canary e rollback

La pipeline aggiornata non considera più sufficiente la sola presenza di
`validated.patch`:

1. l'hash della patch eseguita deve coincidere con quello approvato nel report cifrato;
2. il primo namespace bubblewrap esegue i test offline sul progetto copiato in tmpfs;
3. un secondo namespace nuovo esegue il canary senza rete e senza mount scrivibili dell'host;
4. un canary fallito elimina automaticamente patch e hash validati;
5. il backend verifica l'attestazione v2: run ID, profilo, SHA-256, policy dipendenze,
   rete disabilitata, host read-only e canary separato;
6. la promozione crea un branch di revisione, esegue `git diff --check` e verifica che
   la patch possa essere invertita prima di dichiarare il successo;
7. qualsiasi errore dopo la creazione del branch applica la patch inversa, torna al
   branch originale ed elimina quello incompleto. L'esito è scritto in
   `rollback.json`; un eventuale fallimento del rollback usa invece
   `rollback-failed.json` e richiede recupero manuale.

Il test transazionale `scripts/test-evolution-promotion.ps1` verifica sia una
promozione valida sia un health check negativo con ritorno automatico al branch
originale e worktree pulito. Il test host
`scripts/test-evolution-isolation.ps1` esegue realmente una patch sintetica nel
primo namespace e nel canary, controlla che il repository host non cambi e valida
attestazione e hash. Entrambi risultano superati sul PC corrente.
