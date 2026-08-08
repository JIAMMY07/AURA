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

L'esecuzione resterà disabilitata anche dopo la semplice installazione di un runtime. Occorre verificare immagine base, isolamento rete, mount, quote, distruzione dell'ambiente, scansione output e flusso di approvazione. Solo una suite di abuso superata potrà cambiare `executionAllowed`.
