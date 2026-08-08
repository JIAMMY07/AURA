# Evolution Lab: auto-miglioramento confinato

## Principio di sicurezza

«Accesso completo» significa piena libertà soltanto dentro un ambiente effimero e separato. Non significa accesso autonomo illimitato a Windows, credenziali, profilo utente o repository originali. L'agente può proporre, generare e collaudare una modifica isolata; l'installazione sull'host richiede sempre revisione e approvazione umana.

## Stato rilevato su questo PC

La verifica del 8 agosto 2026 non ha trovato Windows Sandbox, una distribuzione WSL funzionante, Docker o Podman. La virtualizzazione firmware e l'hypervisor risultano disponibili, quindi il PC può essere predisposto in seguito, ma oggi non esiste un confine abbastanza forte per eseguire codice auto-generato.

Per questo AURA opera in modalità fail-closed:

1. registra obiettivo e metadati in un report JSON cifrato AES-256-GCM;
2. non esegue il codice proposto;
3. marca ogni proposta `proposal-only` e `manual-review-required`;
4. conserva una voce nell'audit autenticato.

La versione 0.6.0 rilegge i report soltanto dopo aver verificato nome UUID, dimensione massima, versione dell'envelope, AES-GCM e identità autenticata come AAD. La UI mostra al massimo le 20 proposte più recenti e collega generazione, test e promozione. Un file corrotto, sostituito o associato a un altro ID viene rifiutato invece di essere interpretato.

Una cartella o un worktree Git non costituiscono una sandbox di sicurezza.

## Architettura target

Il runner futuro deve usare una VM, Windows Sandbox o un container dentro WSL con:

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

## Pipeline operativa 0.6

La pipeline è ora implementata in quattro passaggi separati:

1. `evolution-generate-patch.ps1` invia solo file Git tracciati e filtrati al coder locale, accetta operazioni JSON `search/replace` con anchor univoco e costruisce deterministicamente una patch Git verificata;
2. `evolution-sandbox-run.ps1` monta progetto e patch in sola lettura dentro WSL+bubblewrap, senza rete né profilo host, applica la patch in tmpfs ed esegue un profilo allowlistato: `Node` (`tsc --noEmit`), `Rust` (`cargo test --offline`) o `Static` (solo struttura diff);
3. il runner esporta soltanto `validated.patch`, hash SHA-256, profilo test e log;
4. `evolution-promote-patch.ps1` richiede `-Approve`, worktree pulito, hash valido e crea un branch `aura/evolution-*` senza commit automatico.

Il runner applica limiti di 600 secondi CPU, 8 GiB di memoria virtuale, 256 processi, 256 file descriptor e 256 MiB per file. Il profilo generazione è ridotto a 4k token, massimo 10 file/80 KB di contesto e 768 token di output. Il coder usa la CPU: sul runtime Ollama 0.32.6 la RTX 2060/Turing ha mostrato stalli ripetibili sui prompt di patch, mentre il percorso CPU evita quel backend e lascia la GPU alla chat.

I file di policy, crittografia, sandbox, firma e promozione sono esclusi dalla generazione automatica. Il bootstrap `setup-evolution-sandbox.ps1` abilita WSL, installa Ubuntu e bubblewrap ed esegue un test in cui `/mnt/c` deve risultare invisibile. L'8 agosto 2026 il primo passaggio elevato è terminato con `3010`: i componenti Windows sono predisposti ma serve riavviare e rieseguire lo script. Fino al superamento del marker `AURA_WSL_BWRAP_V1`, il backend resta fail-closed.
