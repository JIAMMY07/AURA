# Controllo PC sicuro

## Capacità operative nella versione 0.4.0

AURA dispone di un primo registro di strumenti Windows realmente eseguiti dal backend Rust:

- stato di CPU, memoria, dischi, uptime e numero di processi;
- elenco dei 50 processi con maggiore uso di memoria, senza riga di comando né variabili d'ambiente;
- apertura di Calcolatrice, Blocco note ed Esplora file dopo conferma visibile.

I comandi locali sono riconosciuti con grammatica deterministica. Il modello non produce nomi di eseguibili e non riceve una shell. Stringhe come `powershell`, comandi concatenati, percorsi o programmi fuori allowlist vengono rifiutati.

## Comandi supportati

- `/pc stato` oppure «stato del PC»;
- `/pc processi`;
- «apri calcolatrice»;
- «apri blocco note»;
- «apri esplora file»;
- `/sandbox stato`.

La dettatura vocale deposita sempre il testo nella casella di composizione: l'utente lo verifica e lo invia. Un audio ambiguo non è un consenso.

## Limiti e prossimi gate

Questa versione non modifica file, non termina processi, non installa software, non cambia configurazioni di sicurezza e non eleva privilegi. Prima di aggiungere capacità scriventi servono token di approvazione monouso emessi dal backend, canonicalizzazione dei percorsi, timeout, rollback, kill switch e test contro injection e race condition.

