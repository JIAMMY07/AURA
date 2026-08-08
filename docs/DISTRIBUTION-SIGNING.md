# Firma distributiva AURA

La firma locale self-signed protegge il ciclo di sviluppo sul singolo PC, ma non crea attendibilità pubblica. La release pubblica usa Microsoft Azure Artifact Signing con profilo **Public Trust**, identità verificata e timestamp Microsoft.

## Prerequisiti esterni

1. Sottoscrizione Azure e account Artifact Signing.
2. Identity validation completata dal proprietario legale.
3. Certificate Profile di tipo Public Trust.
4. Ruolo `Artifact Signing Certificate Profile Signer` assegnato all'identità che firma.
5. Accesso effettuato tramite una credenziale supportata da `DefaultAzureCredential`, per esempio Azure CLI.

Questi passaggi non possono essere automatizzati da AURA perché richiedono identità, fatturazione e consenso del titolare.

## Pipeline predisposta

Il PC dispone già di Windows SDK SignTool 10.0.26100. Il pacchetto ufficiale `Microsoft.Azure.ArtifactSigningClientTools` fornisce la Dlib. Dopo l'onboarding Azure:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\sign-windows-distribution.ps1 `
  -Endpoint 'https://weu.codesigning.azure.net' `
  -AccountName 'NOME-ACCOUNT' `
  -CertificateProfileName 'NOME-PROFILO'
```

Lo script firma EXE, MSI e NSIS con SHA-256, applica il timestamp `http://timestamp.acs.microsoft.com/`, verifica ogni firma con SignTool `/pa /all /v`, richiede `Get-AuthenticodeSignature=Valid` e rifiuta un artefatto senza timestamp.

I metadati temporanei non contengono la chiave privata e vengono rimossi al termine. La chiave di firma resta custodita dal servizio Microsoft.

## Fonti ufficiali

- [Microsoft: opzioni di firma per applicazioni Windows](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)
- [Microsoft: Artifact Signing Public Trust e Private Trust](https://learn.microsoft.com/azure/artifact-signing/concept-trust-models)
- [Microsoft: integrazione SignTool con Artifact Signing](https://learn.microsoft.com/azure/artifact-signing/how-to-signing-integrations)
- [Microsoft: configurazione account, identity validation e certificate profile](https://learn.microsoft.com/azure/artifact-signing/quickstart)
