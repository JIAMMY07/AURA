# Verifica AURA — Guardian, Sandbox, Evolution e 3D

Data: 8 agosto 2026.

## Risultato reale

| Modulo | Risultato verificato | Stato |
|---|---|---:|
| Guardrail combinato | routing 10/10 sul corpus, pattern critici bloccati da Rust indipendentemente dal modello | 100% gate applicativo |
| Modello Granite Guardian isolato | 8/10 sul corpus; due falsi positivi difensivi | 80% modello grezzo |
| Sandbox fail-safe | esecuzione negata quando WSL+bubblewrap non è attestato | 100% fail-safe |
| Sandbox operativa | WSL non installato; UAC/riavvio richiesti | non ancora operativa |
| Evolution — generazione | patch valida prodotta su repository sintetico, schema percorsi chiuso | 100% gate generazione |
| Evolution — test/promozione | runner e correlazione proposta/run/hash pronti; esecuzione bloccata senza sandbox | in attesa runtime |
| Creazione 3D | nuova mesh GLB generata e importata in Blender | 100% gate 3D |

## Evidenze

- Guardrail: i termini difensivi generici non attivano più il modello lento; richieste di ransomware, furto credenziali, esfiltrazione, bypass antivirus e rivelazione del prompt vengono instradate e i casi critici sono bloccati deterministicamente.
- Evolution: il primo test ha correttamente rifiutato un percorso inventato dal modello. Lo schema ora usa `enum` con i soli percorsi autorizzati; il secondo test ha generato una patch Git applicabile.
- Runner Node: esegue typecheck e test del progetto in modalità offline dentro bubblewrap.
- Promozione: la patch deve coincidere sia con l'artefatto registrato sia con il `run_id` della proposta; non basta provenire da una cartella run qualsiasi.
- 3D: `verification-20260808-2012/0/mesh.glb`, 83.328 byte, import Blender riuscito, 2 oggetti mesh, 2.072 vertici e 4.114 facce.
- Il backend ora rifiuta GLB troncati, lunghezze dichiarate incoerenti, output fuori cartella e OBJ privi di vertici/facce.

## Blocco amministrativo residuo

Windows 11 Home non dispone di Windows Sandbox e WSL risulta non installato. AURA include ora il comando e il pulsante **Installa runtime (UAC)**. L'utente deve accettare UAC; Windows può richiedere un riavvio. Fino all'attestazione `AURA_WSL_BWRAP_V1`, esecuzione e promozione rimangono bloccate.

## Suite finale

- Frontend: 20/20.
- Backend Rust: 51/51.
- TypeScript/Vite: superati.
- rustfmt: superato.
- Clippy con warning come errori: superato.
- EXE, MSI e installer NSIS: prodotti.
