# Remediation sicurezza AURA — 8 agosto 2026

## Stato dopo l'intervento

| Voce | Prima | Dopo |
| --- | --- | --- |
| Granite Guardian | installato, consultivo, non integrato | pre-filtro integrato nella chat per input sospetti; timeout 12 s e fallback deterministico fail-safe; policy Rust ancora autoritative |
| Sandbox | nessun runtime | bootstrap WSL+bubblewrap, self-test namespace/mount/tmpfs e rilevamento backend implementati; attivazione host in attesa di sessione amministrativa e riavvio |
| Evolution Lab | sole proposte cifrate | pipeline patch locale, runner isolato, profili test allowlistati, hash e promozione su branch implementati; esecuzione bloccata finché il runtime non è verificato |
| Auto-miglioramento | non operativo | generazione unified diff con Ollama locale, protezione file critici, test in sandbox e promozione `-Approve` disponibili come script tipizzati |
| WebView2 | due connessioni Microsoft osservate | profilo dati AURA separato e flag anti-background; smoke test di 15 s sulla nuova build: zero connessioni TCP stabilite osservate |
| Firma | assente | EXE, MSI e NSIS contengono firma SHA-256 con certificato locale non esportabile; attendibilità radice locale in attesa di conferma UI; per distribuzione serve certificato pubblico |
| Cifratura disco | non verificata | audit rigorosamente read-only; attivazione disponibile soltanto tramite script manuale separato, conferma testuale, amministratore e recovery key esterna |

## Guardrail Guardian

Il modello viene consultato soltanto quando un filtro deterministico rileva indicatori come prompt injection, credenziali, furto cookie, malware o ransomware. I messaggi normali non pagano il costo di caricamento del modello da 2,7 GB. Se Guardian non risponde entro 12 secondi, il segnale resta prudenzialmente positivo. Guardian non può autorizzare strumenti, abbassare rischio o sostituire `policy.rs`.

## Catena Evolution

- generazione: `scripts/evolution-generate-patch.ps1`;
- sandbox: `scripts/setup-evolution-sandbox.ps1` e `scripts/evolution-sandbox-run.ps1`;
- promozione: `scripts/evolution-promote-patch.ps1`;
- componenti protetti: policy, crittografia, sandbox, firma e promozione;
- rete del runner: namespace separato senza interfacce host;
- filesystem: progetto e patch in sola lettura, lavoro in tmpfs, esportazione limitata a patch/hash/log;
- promozione: worktree pulito, hash valido, `-Approve`, branch separato, nessun commit automatico.

## Firma

Thumbprint locale: `5556B95B2ED7FF2C47FCBF7A193B26D70DE9D619`.

La firma è presente ma `Get-AuthenticodeSignature` restituisce `UnknownError` finché l'utente non approva il certificato self-signed come radice locale. È sufficiente per identificare manomissioni rispetto alla chiave locale, ma non dà reputazione SmartScreen e non è equivalente a un certificato CA pubblico/EV.

## Test

- 35/35 test Rust;
- 20/20 test frontend;
- TypeScript e build Vite superati;
- build Tauri release, MSI e NSIS superata;
- cinque script Evolution/firma analizzati dal parser PowerShell senza errori;
- promozione senza consenso rifiutata;
- nuova build WebView2: zero connessioni TCP stabilite osservate nella finestra di 15 secondi.

## Passaggi amministrativi ancora obbligatori

1. chiudere l'eventuale vecchia finestra UAC rimasta aperta;
2. eseguire `setup-evolution-sandbox.ps1` come amministratore e riavviare se richiesto;
3. approvare il certificato AURA nello store radice dell'utente oppure sostituirlo con un certificato pubblico;
4. eseguire `device-encryption-audit.ps1` come amministratore: non può attivare la cifratura;
5. solo se desiderato, avviare manualmente `enable-device-encryption-manually.ps1` con recovery su un’unità diversa da `C:` e conferma `ATTIVA-CIFRATURA`.
