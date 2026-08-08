# Controllo PC sicuro

## Capacità operative nella versione 0.6.0

AURA dispone di un primo registro di strumenti Windows realmente eseguiti dal backend Rust:

- stato di CPU, memoria, dischi, uptime e numero di processi;
- elenco dei 50 processi con maggiore uso di memoria, senza riga di comando né variabili d'ambiente;
- apertura, dopo conferma nativa, di Calcolatrice, Blocco note, Esplora file, Gestione attività, Paint, Strumento di cattura e pannelli Impostazioni/Audio/Bluetooth/Sicurezza;
- terminazione di un singolo PID dopo conferma, con denylist dei processi critici e dell'app AURA;
- blocco della sessione Windows dopo conferma.

I comandi locali sono riconosciuti con grammatica deterministica. Il modello non produce nomi di eseguibili e non riceve una shell. Stringhe come `powershell`, comandi concatenati, percorsi o programmi fuori allowlist vengono rifiutati.

## Comandi supportati

- `/pc stato` oppure «stato del PC»;
- `/pc processi`;
- «apri calcolatrice»;
- «apri blocco note»;
- «apri esplora file»;
- «apri impostazioni», «apri impostazioni bluetooth», «apri impostazioni audio»;
- «apri sicurezza Windows», «apri gestione attività», «apri Paint»;
- `termina processo 1234`;
- «blocca il PC»;
- `/sandbox stato`.

La dettatura può restare in bozza oppure essere inviata automaticamente sopra la soglia configurata. Un audio ambiguo non è mai un consenso: le azioni mutanti mostrano comunque un dialogo nativo separato.

## Limiti e prossimi gate

Questa versione non modifica file arbitrari, non installa software dall'agente, non cambia protezioni di sicurezza e non eleva privilegi. Il catalogo è completo rispetto allo scope sicuro dichiarato, non rispetto a «qualsiasi funzione Windows»: nuove azioni devono essere aggiunte come tool tipizzati con policy, conferma e test.
