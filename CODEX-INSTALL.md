# Installare AURA affidando il repository a Codex

Questa guida è pensata per un PC Windows 10/11 x64. Codex deve lavorare in una cartella dedicata,
con permessi `workspace-write` e approvazione dell'utente per rete, installazioni ed elevazione.
Non è necessario concedere accesso completo permanente.

## Prompt da consegnare a Codex

Copia il testo seguente in una nuova task Codex:

```text
Installa AURA da questo repository GitHub sul mio PC Windows.

1. Leggi completamente README.md, CODEX-INSTALL.md e docs/SECURITY-INCIDENT-RESPONSE.md.
2. Clona il repository in una nuova cartella dedicata, senza sovrascrivere progetti esistenti.
3. Controlla architettura Windows, spazio libero e RAM. Non mostrare né raccogliere credenziali.
4. Verifica firma/hash dei download quando il progetto fornisce un hash.
5. Esegui scripts/install-for-users.ps1. Chiedimi approvazione prima di installare prerequisiti,
   prima di usare privilegi amministrativi e prima di scaricare i modelli Ollama opzionali.
6. Non disattivare Defender, Firewall, UAC, SmartScreen o i guardrail di AURA.
7. Esegui i test automatici e installa il pacchetto NSIS generato.
8. Avvia AURA e verifica che risponda, che Ollama sia raggiungibile e che il pulsante
   “Arresta tutto” sia visibile. Non dichiarare completata una verifica non eseguita.
9. Alla fine indicami percorso installato, versione, test superati, modelli presenti e problemi residui.
```

## Installazione manuale rapida

Apri PowerShell nella cartella clonata:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\install-for-users.ps1
```

Per installare anche voce e modelli locali consigliati:

```powershell
.\scripts\install-for-users.ps1 -InstallVoice -InstallModels
```

I modelli richiedono diversi gigabyte e vengono scaricati da Ollama. Su hardware più debole è
preferibile iniziare senza `-InstallModels` e installare in seguito soltanto i modelli necessari.

## Cosa fa lo script

- controlla Windows x64, Node.js, pnpm, Rust, Visual Studio Build Tools e WebView2;
- può usare `winget` per installare i prerequisiti mancanti, ma soltanto dopo conferma esplicita;
- installa le dipendenze bloccate da `pnpm-lock.yaml`;
- esegue test frontend/Alexa, test Rust e build web;
- crea una build Windows senza usare certificati privati dello sviluppatore;
- opzionalmente installa Whisper verificando gli hash pubblicati nello script;
- opzionalmente scarica i modelli Ollama;
- installa silenziosamente il pacchetto NSIS solo dopo che è stato generato correttamente.

## Avvertenze

La build compilata localmente non possiede una firma pubblica attendibile. Windows può mostrare SmartScreen:
controlla che il sorgente provenga dal repository corretto e non disattivare SmartScreen globalmente.
Le funzioni Alexa richiedono ancora un relay/plugin separato. Evolution Lab rimane fail-closed se WSL2 e
bubblewrap non superano le attestazioni previste.

