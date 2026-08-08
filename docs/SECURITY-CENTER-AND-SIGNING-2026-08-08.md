# Security Center e firma distributiva — 8 agosto 2026

## Security Center AURA

Il modulo è integrato nel backend Rust, nell'interfaccia AURA, nella policy e nell'audit. Espone:

- stato Microsoft Defender, antivirus e protezione in tempo reale;
- anzianità e versione delle definizioni;
- scansione rapida Defender con conferma nativa;
- aggiornamento definizioni con conferma nativa;
- Firewall Domain, Private e Public;
- UAC, Secure Boot, Windows Update e BitLocker;
- Authenticode, firmatario e SHA-256 dell'eseguibile AURA;
- punteggio di postura separato dalla completezza del modulo.

Prova reale del PC:

| Controllo | Esito |
| --- | --- |
| Defender / antivirus / tempo reale | attivi |
| Definizioni | aggiornate, versione `1.457.65.0`, età 0 giorni |
| Scansione rapida | completata in 265,6 s alle 17:04:36 |
| Rilevamenti ultime 24 ore | 0 |
| Firewall | Domain, Private e Public attivi |
| UAC | attivo |
| Windows Update | non disabilitato |
| Secure Boot | lettura non disponibile senza contesto appropriato |
| BitLocker | lettura amministrativa non disponibile; attivazione resta manuale |

Il modulo è completo; la postura osservabile non viene falsificata quando una lettura richiede privilegi amministrativi.

## Firma distributiva

Gli artefatti sono firmati con certificato locale non esportabile per sviluppo. La pipeline Public Trust è in `scripts/sign-windows-distribution.ps1` e implementa firma SHA-256, timestamp Microsoft, verifica SignTool `/pa /all /v`, verifica Authenticode `Valid` e controllo del timestamp.

La firma pubblica non può essere emessa autonomamente senza un'identità verificata. Microsoft richiede:

- account Azure Artifact Signing;
- identity validation del titolare;
- Certificate Profile Public Trust;
- ruolo Certificate Profile Signer;
- autenticazione Azure valida.

Il PC possiede SignTool 10.0.26100. Il tentativo non elevato di installare `Microsoft.Azure.ArtifactSigningClientTools` è rimasto bloccato nella fase MSI e il pacchetto non è stato registrato; il processo winget avviato da AURA è stato terminato senza interrompere Windows Installer. L'installazione va ripetuta in una PowerShell amministrativa.

La parte di pipeline è completa e fail-closed; l'attendibilità pubblica resta un gate esterno di identità/fatturazione, non un difetto correggibile con codice o un certificato self-signed.

Artefatti ricostruiti:

- EXE SHA-256 `FD8D8F82046768690EF930761794C064FE8307191F2E71B041442BA7102DE162`
- MSI SHA-256 `13E9C15A0AF5F801432E47775083172C357716CE2B53BAEB4D0006EBE9CCED9F`
- NSIS SHA-256 `75A28AC77596E60431FD06DB328262ADA5EE3583544E100572D24D4F75A1B2C9`

Il certificato locale è presente in `CurrentUser\TrustedPublisher`. L'aggiunta a `CurrentUser\Root` apre correttamente una conferma di sicurezza Windows e non è stata forzata automaticamente; fino all'approvazione dell'utente lo stato resta `UnknownError` anziché `Valid` sul PC locale.
