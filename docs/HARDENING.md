# Hardening Fase 2.5

## Confine fidato

La WebView non possiede accesso SQL. Può invocare soltanto comandi Rust registrati con input serializzati e validati:

- `storage_load`
- `storage_save`
- `storage_clear`
- `policy_evaluate`
- `audit_verify`

Ogni futura azione PC deve seguire lo stesso modello: comando specifico, capability registrata, rischio assegnato dal backend e deny-by-default.

## Protezione dei messaggi

Il backend genera una chiave principale casuale e la conserva nel Windows Credential Manager. HKDF-SHA-256 deriva chiavi distinte per contenuti e audit. Ogni messaggio è cifrato con AES-256-GCM e nonce casuale da 96 bit. Il database mantiene vuota la vecchia colonna in chiaro; all'avvio i messaggi legacy vengono migrati automaticamente.

Questa protezione difende i dati a riposo e rileva modifiche al ciphertext. Non protegge da malware eseguito nello stesso account mentre AURA è aperta.

## Audit verificabile

Ogni decisione backend produce un evento con hash del precedente, hash SHA-256 del payload canonico e HMAC-SHA-256. `audit_verify` ricostruisce la catena e segnala alterazioni o rimozioni intermedie.

La cancellazione dell'intero database resta rilevabile soltanto tramite backup o ancoraggio esterno, previsti prima delle funzioni PC.

## CSP

La Content Security Policy nega oggetti, frame, form e origini non dichiarate. L'unica connessione HTTP ammessa dal frontend è Ollama su `127.0.0.1:11434`.

## Limiti residui

- Ollama è ancora contattato dalla WebView, sebbene limitato dalla CSP.
- L'installer non è firmato.
- Non esiste ancora un approval token monouso.
- Non esistono ancora tool PC, sandbox reale o browser runner.
- Per recupero da cancellazione totale dell'audit serve un backup cifrato con ancoraggio dell'ultimo hash.

## Supply chain

`pnpm audit --prod` non segnala vulnerabilità note. `cargo audit` non segnala vulnerabilità applicabili al target Windows; riporta avvisi su dipendenze GTK/Linux presenti nel lockfile multipiattaforma, ma assenti dall'albero `x86_64-pc-windows-msvc`.
