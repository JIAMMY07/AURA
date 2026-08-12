# AURA 0.8.0 — Security Center, firma, UI e manutenzione

Data: 8 agosto 2026.

## Risultati

| Area | Gate software | Verifica reale | Esito |
|---|---|---|---:|
| Security Center | 13 controlli pesati fino a 100, copertura separata dalla postura, hash EXE, firma, raccomandazioni, scan/update con conferma | Defender, realtime, stack esteso, antimanomissione, cloud, firewall, UAC, definizioni, scan, Secure Boot, Update, cifratura e TPM | 100% copertura |
| Firma locale | EXE, MSI e NSIS 0.8.0 firmati, SHA-256 e manifest ripetibile | 3/3 artefatti con publisher `AURA Local Development Code Signing` | 100% gate locale |
| Firma pubblica | Pipeline Microsoft Artifact Signing parametrica, verifica obbligatoria di trust e timestamp | certificato/identità pubblica non forniti | non attivabile localmente |
| Interfaccia | feature separate per Remote, Security, Evolution e strumenti Browser/API/RAG | build TypeScript e 3/3 E2E Chromium | 100% gate UI dichiarati |
| Test/manutenzione | unit, integrazione protocollo, E2E, build, rustfmt, Clippy, CI e aggiornamenti dipendenze | 20 frontend + 5 Alexa/relay + 53 Rust + 3 E2E, zero errori | 100% gate |
| HTTPS backend | rustls compilato e prova reale verso endpoint HTTPS | `200 OK` | superato |

Il punteggio di postura mostrato dal Security Center descrive la configurazione del PC e può essere inferiore a 100 anche quando il modulo è completo. Un controllo `unavailable` non viene trasformato artificialmente in `healthy`.

## Artefatti 0.8.0

| Artefatto | SHA-256 |
|---|---|
| `local-ai-assistant.exe` | `33045506402A6969624E09AA5C4049C53CFADB6CB1BD6C600B95474F7441317A` |
| `Local AI Assistant_0.8.0_x64_en-US.msi` | `D32B776F5EA6A13F798A4140C78F57EE9388980EF48E76D4E46954CABA34184F` |
| `Local AI Assistant_0.8.0_x64-setup.exe` | `56EA8364F222034C02EEAE29275CCB5EFA458FB1A4C8D7B176EF1AE2E05E4B09` |

La firma locale è crittograficamente presente ma non è una reputazione pubblica SmartScreen. Non è stata aggiunta automaticamente la CA autofirmata al Root Store: Windows ha richiesto un’approvazione esplicita e il processo è stato annullato. La pipeline pubblica richiede endpoint, account e profilo certificato Microsoft Artifact Signing.

## Automazione

- `.github/workflows/ci.yml`: frontend, unit test, build, Playwright, rustfmt, Rust test e Clippy su Windows.
- `.github/dependabot.yml`: aggiornamenti settimanali npm, Cargo e GitHub Actions.
- `pnpm test:all`: unit test, build e E2E.
- `pnpm verify:windows`: firma, hash e manifest dei tre artefatti correnti.
- `cargo run --example https_probe`: prova TLS reale del backend Rust.

## Stato finale

La release aggiornata è stata avviata da `target/release` ed è risultata responsiva. Il manifest della distribuzione è salvato in `%LOCALAPPDATA%\AURA\signing\distribution-0.8.0-release.json`.
