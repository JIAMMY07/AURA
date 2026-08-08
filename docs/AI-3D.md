# Generazione 3D locale per AURA

## Esito della valutazione

La configurazione scelta per questo PC è **TripoSR + Blender 5.2 LTS**:

- TripoSR converte una singola immagine in una mesh OBJ o GLB, localmente;
- Blender importa, controlla, ripara e rifinisce il risultato;
- AURA usa un launcher con selezione automatica CPU/GPU e output confinato in `%LOCALAPPDATA%\AURA\data\3d\jobs`;
- nessun add-on MCP capace di eseguire Python arbitrario in Blender è stato installato.

Il PC verificato dispone di Ryzen 7 5700X, 48 GB RAM e RTX 2060 da 6 GB. RAM e CPU sono adeguate. La VRAM è il limite principale: TripoSR dichiara circa 6 GB per i parametri standard, quindi il profilo AURA usa risoluzione 128, chunk 1024 e fallback CPU.

## Componenti installati

| Componente | Percorso/versione | Stato |
| --- | --- | --- |
| Blender | `%LOCALAPPDATA%\AURA\tools\3d\Blender-5.2.0` | 5.2.0 LTS, eseguibile firmato da Blender Foundation |
| TripoSR | `%LOCALAPPDATA%\AURA\tools\3d\TripoSR` | commit `107cefdc244c39106fa830359024f6a2f1c78871` |
| Python | ambiente `.venv` di TripoSR | 3.10.20 isolato |
| PyTorch | ambiente `.venv` di TripoSR | 2.2.0 + CUDA 12.1; RTX 2060 rilevata |
| Pesi | `%LOCALAPPDATA%\AURA\models\huggingface` | `stabilityai/TripoSR`, scaricati localmente |
| torchmcubes | wheel locale | commit fissato `3381600...`, build CPU per evitare il CUDA Toolkit |

## Benchmark realmente eseguito

Il test ha usato `examples/chair.png`, CPU, mesh resolution 64, chunk 2048 ed esportazione GLB.

| Controllo | Risultato |
| --- | --- |
| Pipeline immagine → mesh | superata |
| Tempo primo avvio | 178,4 s, incluso il download iniziale dei pesi |
| Inferenza CPU | 22,8 s |
| Estrazione mesh | 1,0 s |
| Output | GLB da 99.300 byte |
| Validazione Blender headless | superata |
| Geometria importata | 2 mesh, 2.479 vertici, 4.902 poligoni |
| GPU | configurata e rilevata; test non eseguito perché al momento erano occupati 4.806/6.144 MiB |

## Uso

Da PowerShell nella radice del repository:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\generate-3d.ps1 -InputImage 'C:\percorso\oggetto.png' -JobName 'oggetto-01'
```

Il profilo `Auto` usa la GPU soltanto se trova almeno 5.200 MiB liberi; altrimenti sceglie la CPU. Per richiedere esplicitamente un dispositivo:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\generate-3d.ps1 -InputImage 'C:\percorso\oggetto.png' -JobName 'oggetto-gpu' -Device Gpu
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\generate-3d.ps1 -InputImage 'C:\percorso\oggetto.png' -JobName 'oggetto-cpu' -Device Cpu
```

Il bypass vale soltanto per quel processo PowerShell; la policy globale di Windows non viene modificata.

Prima del profilo GPU chiudere giochi, launcher, browser con accelerazione grafica e altri modelli locali. Se la GPU produce un errore di memoria, non ripetere automaticamente: passare a CPU oppure ridurre `MeshResolution` a 64.

## Confronto critico

| Sistema | Compatibilità con questo PC | Decisione |
| --- | --- | --- |
| TripoSR | circa 6 GB VRAM, CPU supportata, Windows utilizzabile | installato |
| Stable Fast 3D | circa 6 GB, Windows sperimentale; pesi con accesso Hugging Face e licenza community | non installato senza consenso/token dell'utente |
| Hunyuan3D-2 | 6 GB per sola shape, 16 GB per shape + texture | non installato: duplicazione pesante e margine VRAM insufficiente |
| TRELLIS | richiede GPU NVIDIA con almeno 16 GB; Windows non pienamente testato | escluso |

Fonti primarie: [TripoSR ufficiale](https://github.com/VAST-AI-Research/TripoSR), [Stable Fast 3D ufficiale](https://github.com/Stability-AI/stable-fast-3d), [model card Stable Fast 3D](https://huggingface.co/stabilityai/stable-fast-3d), [Hunyuan3D-2 ufficiale](https://github.com/Tencent-Hunyuan/Hunyuan3D-2), [TRELLIS ufficiale](https://github.com/microsoft/TRELLIS), [Blender](https://www.blender.org/).

## Integrazione sicura futura

La successiva integrazione nell'agente deve esporre azioni tipizzate come `generate_mesh`, `inspect_mesh`, `convert_format` e `open_in_blender`, con input validati, timeout, quote disco e audit. Non deve accettare codice Python generato dal modello da inviare direttamente a Blender. L'esecuzione di script Blender personalizzati deve passare dall'Evolution Lab, test in sandbox e approvazione esplicita prima della promozione.
