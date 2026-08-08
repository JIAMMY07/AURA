# Profilo personale e adattamento

AURA può conservare informazioni fornite volontariamente dall'utente per adattare tono e lavoro: nome preferito, stile di comunicazione, obiettivi, preferenze, vincoli e contesto dei progetti. Il profilo e le correzioni sono cifrati con AES-256-GCM; la chiave principale è protetta dal Windows Credential Manager. Il contesto viene inviato soltanto a Ollama locale.

## Regole invarianti

- il profilo descrive preferenze, non autorizzazioni;
- una frase salvata non può concedere privilegi, rete, acquisti o accesso a segreti;
- le istruzioni correnti e le policy di sicurezza prevalgono sempre;
- non inserire password, token, dati bancari, documenti d'identità o segreti aziendali;
- i feedback vengono acquisiti solo dopo un gesto esplicito «Utile» o «Correggi»;
- nessuna osservazione nascosta di tastiera, schermo, audio o cronologia;
- la cancellazione del profilo elimina anche le correzioni ed è irreversibile.

## Adattamento utile

Le correzioni recenti vengono recuperate localmente per evitare errori ripetuti. In futuro conviene aggiungere categorie, scadenza e un pannello di revisione/esportazione, così l'utente può vedere esattamente cosa l'assistente crede di sapere e correggerlo.

