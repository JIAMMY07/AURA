# Incident response di AURA

AURA usa un modello difensivo a più livelli: rilevamento, analisi, contenimento,
eradicazione, ripristino e miglioramento successivo. Il modello linguistico non può
autorizzare azioni: le policy Rust e le conferme native restano autoritative.

## Funzioni implementate

- triage in sola lettura di Defender, Network Inspection, Firewall, rilevamenti degli
  ultimi sette giorni, connessioni TCP, endpoint in ascolto e firma dell'eseguibile;
- severità deterministica: `healthy`, `warning`, `high`, `critical`;
- remediation delle sole minacce attive riconosciute da Microsoft Defender, con
  conferma nativa, scansione, verifica successiva e audit privo di nomi/IP sensibili;
- stato `contained` solo quando la scansione termina e risultano zero minacce attive;
- nessun hack-back, scansione di terzi o blocco automatico di indirizzi interni.

Le connessioni e le porte sono telemetria, non prova autonoma di compromissione.
L'isolamento completo dell'host e le regole firewall automatiche non sono abilitati:
un falso positivo potrebbe interrompere rete, router, NAS, domotica o servizi legittimi.

## Riferimenti

- NIST SP 800-61 Rev. 3: https://csrc.nist.gov/pubs/sp/800/61/r3/final
- CISA Incident and Vulnerability Response Playbooks: https://www.cisa.gov/news-events/news/federal-government-cybersecurity-incident-and-vulnerability-response-playbooks
- Microsoft Defender PowerShell: https://learn.microsoft.com/defender-endpoint/use-powershell-cmdlets-microsoft-defender-antivirus
- Windows Firewall: https://learn.microsoft.com/windows/security/operating-system-security/network-security/windows-firewall/
