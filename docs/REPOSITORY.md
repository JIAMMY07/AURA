# Struttura del repository

```text
local-ai-assistant/
├─ docs/                       architettura, backlog e decisioni
├─ src/                        frontend React e logica TypeScript condivisa
│  ├─ core/                    contratti e policy pure, senza dipendenze UI
│  ├─ features/                moduli UI per chat, voce, audit e impostazioni
│  ├─ adapters/                client Ollama/Tauri e boundary esterni
│  └─ components/              componenti visuali riusabili
├─ src-tauri/                  host desktop Rust
│  ├─ capabilities/            permessi Tauri minimi per finestra
│  └─ src/                     command handler e runner nativi
├─ tests/                      test end-to-end e fixture non sensibili
├─ project-spec.yaml           requisiti machine-readable
├─ package.json                script frontend/desktop
└─ pnpm-lock.yaml              dipendenze riproducibili
```

La Fase 1 mantiene pochi file per evitare astrazioni premature. Quando un modulo entra nel backlog, viene creato nella directory prevista con contratto pubblico, test e ownership esplicita.

## Regole di dipendenza

- `core` non importa React, Tauri o adapter di rete;
- la UI chiama use-case, non API native direttamente;
- gli adapter implementano interfacce definite nel core;
- il processo Rust valida nuovamente ogni input ricevuto dalla UI;
- un tool non può chiamare un altro tool senza tornare alla policy;
- test e fixture non contengono token o dati personali.

## Convenzioni

- TypeScript strict e schemi runtime ai confini;
- Rust con errori tipizzati, niente `unwrap` nei percorsi di produzione;
- ID di correlazione per richiesta, piano, azione e audit;
- migrazioni dati in avanti e procedura di backup/restore;
- commit per fase piccoli e reversibili; release locali firmate.
