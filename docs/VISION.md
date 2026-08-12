# Visione locale: mani e volto

## Origine e scelta tecnica

L'analisi dei progetti di riferimento ha rilevato:

- **OmniBot** implementa l'identificazione biometrica del volto con InsightFace, OpenCV e ONNX Runtime, profili di enrollment e confronto tramite similarità del coseno;
- **Ada-SI** non contiene un riconoscitore di mani nei sorgenti correnti. Il lockfile storico menziona `@mediapipe/tasks-vision`, ma il pacchetto non è dichiarato né utilizzato dal codice applicativo.

AURA adotta quindi una pipeline originale basata su MediaPipe Tasks Vision, più leggera e adatta alla WebView desktop. La prima versione rileva landmark facciali, mani e gesti comuni; non identifica l'identità della persona. L'identificazione biometrica di OmniBot non viene copiata direttamente perché richiede enrollment, retention, cifratura dedicata, gestione di falsi positivi e un modello InsightFace molto più pesante.

## Funzionamento

- Supporto per videocamera integrata, USB o virtuale esposta da Windows/WebView2.
- Selezione manuale e fallback sull'ingresso predefinito.
- Modelli MediaPipe e runtime WASM inclusi localmente nell'applicazione.
- Analisi limitata a circa 6 fotogrammi al secondo per contenere CPU e latenza UI.
- Fino a due volti e due mani contemporaneamente.
- Gesti riconosciuti: pugno chiuso, palmo aperto, indice alzato, pollice su/giù, vittoria e “ti voglio bene”.
- Overlay locale dei landmark di volto e mani.

## Privacy e sicurezza

La videocamera è spenta per impostazione predefinita e viene aperta soltanto dal pulsante **Avvia** dopo il consenso Windows. Un indicatore `ATTIVA` resta visibile per tutta la sessione. Arrestando la funzione o chiudendo la vista, tutte le tracce video vengono terminate e l'engine MediaPipe viene rilasciato.

I fotogrammi:

- non vengono salvati;
- non vengono inviati a Ollama;
- non vengono inviati a Internet;
- fotogrammi, landmark e identità non entrano nei log o nella memoria personale;
- soltanto i comandi discreti realmente inviati al PC (click, scroll, finestra o media) entrano nel ledger con contenuto cifrato ed esito;
- non attivano automaticamente comandi del PC.

I modelli sono verificati tramite SHA-256 durante la build. L'abilitazione WASM nella CSP è limitata agli script `self`; connessioni di rete esterne e frame remoti restano bloccati.

## Limite biometrico

“Volto rilevato” significa presenza e geometria del volto, non riconoscimento dell'identità. Prima di aggiungere l'enrollment biometrico serviranno almeno: consenso separato, profili cifrati, cancellazione verificabile, soglia calibrata, anti-spoofing/liveness, conferma alternativa e divieto di usare il volto come unico fattore per azioni sensibili.
