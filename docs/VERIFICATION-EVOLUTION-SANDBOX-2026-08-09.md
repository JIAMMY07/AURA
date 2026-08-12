# Verifica Evolution Sandbox — 9 agosto 2026

## Esito

La sandbox Evolution è operativa sul PC corrente. Non è più soltanto fail-safe:
una patch sintetica è stata copiata in tmpfs, applicata e verificata in due
namespace bubblewrap separati senza modificare il repository host.

| Controllo | Esito |
|---|---|
| WSL | 2.7.11.0, distribuzione Ubuntu versione 2 |
| Kernel | 6.18.33.2-2 |
| bubblewrap | 0.11.1 |
| Marker runtime | `AURA_WSL_BWRAP_V1` |
| Mount Windows nel namespace | assente (`/mnt/c` non visibile) |
| Ambiente ereditato | ripulito con `--clearenv` |
| Repository host | hash invariato prima/dopo |
| Rete | namespace separato tramite `--unshare-all` |
| Canary | superato in un secondo namespace nuovo |
| Attestazione | schema v2, SHA-256 verificato |
| Promozione valida | superata su branch dedicato |
| Health check negativo | rollback automatico, worktree pulito |

## Difetti scoperti e corretti dal test reale

- `/bin/sh` di Ubuntu non supportava `ulimit -u`: il runner usa ora Bash.
- Le espressioni shell in una here-string PowerShell venivano interpolate prima
  dell'esecuzione: i payload sono ora file UTF-8 montati in sola lettura.
- CRLF e permessi DrvFS rendevano incoerente l'applicazione delle patch Windows:
  il repository temporaneo normalizza `core.autocrlf` e ignora `core.filemode`.
- Il `PATH` Windows conteneva una voce Flutter non traducibile e attraversava WSL:
  `/etc/wsl.conf` disabilita `appendWindowsPath`; bubblewrap usa inoltre
  `--clearenv` e un `PATH` Linux esplicito.

## Comandi di regressione

- `pnpm test:evolution:isolation`: prova host reale WSL+bubblewrap.
- `pnpm test:evolution`: promozione e rollback transazionali.
- `pnpm test:all`: suite portabile, build ed E2E.
- `pnpm test:all:host`: suite portabile più prova sandbox del PC.

La promozione resta intenzionalmente subordinata a revisione e conferma umana.
L'operatività della sandbox non concede privilegi amministrativi autonomi e non
consente al modello di cambiare policy, crittografia, runner o firma.
