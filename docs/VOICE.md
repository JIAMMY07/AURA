# Voce locale

## Stato

La pipeline è operativa su Windows. Il pulsante microfono attiva una registrazione push-to-talk esplicita, mostra un indicatore rosso e termina su richiesta o dopo 30 secondi. L'audio viene convertito nel frontend in WAV PCM mono, 16 kHz, 16 bit e inviato soltanto al backend Rust locale.

Il backend convalida struttura, formato, dimensione e durata, crea un file temporaneo, avvia `whisper.cpp` senza finestra, legge il risultato e cancella audio e trascrizione temporanei. Il testo viene inserito nel composer come bozza. L'utente può attivare l'invio automatico soltanto per trascrizioni con confidenza almeno 70%; le azioni PC che cambiano stato mantengono comunque la conferma nativa Windows.

La sintesi vocale locale usa Windows SAPI con una voce italiana installata, accetta al massimo 4.000 caratteri tramite `stdin` (mai come codice PowerShell) ed è interrompibile dalla UI. Sul PC target è stata verificata `Microsoft Elsa Desktop` (`it-IT`) producendo un WAV RIFF valido.

Due modalità hands-free sono opzionali e disattivate per impostazione predefinita:

- **Wake word AURA locale** registra segmenti di cinque secondi, li trascrive con Whisper locale e invia soltanto una frase che contiene “AURA” seguita da un comando, con confidenza almeno 70%. Sotto soglia lascia il testo in bozza;
- **Follow-up dopo TTS** riapre il push-to-talk al termine stimato della lettura. È mutuamente esclusivo con il wake word.

Il wake word basato su Whisper è accurato ma più pesante di un detector dedicato: l'indicatore del microfono resta sempre visibile e l'opzione va disattivata quando non serve.

## Microfoni supportati

AURA non dipende da un modello o da una marca specifica. Usa l'elenco degli ingressi audio autorizzato da Windows/WebView2 ed è compatibile con microfoni integrati, USB, Bluetooth, cuffie con microfono, webcam, dock e schede audio. Le OPPO Enco Buds2 Pro restano quindi utilizzabili, ma sono trattate come qualsiasi altro ingresso.

La selezione segue un ordine deterministico:

1. dispositivo scelto e ricordato dall'utente, se ancora disponibile;
2. stesso dispositivo riconosciuto dall'etichetta, se WebView2 ne ha cambiato l'identificatore;
3. microfono predefinito configurato in Windows;
4. primo ingresso disponibile come fallback.

Quando un dispositivo viene collegato o rimosso, AURA aggiorna automaticamente l'elenco. Se il microfono attivo si disconnette durante una registrazione, la registrazione viene fermata in sicurezza; al tentativo successivo viene usato il fallback disponibile. La preferenza originale non viene cancellata, così il dispositivo scelto può tornare attivo quando viene ricollegato.

Procedura d'uso:

1. collegare o abilitare il microfono in Windows;
2. in AURA premere **Aggiorna microfoni** e concedere l'accesso la prima volta;
3. scegliere l'ingresso desiderato nel selettore **Microfono**, oppure lasciare quello predefinito di Windows;
4. premere il pulsante microfono, parlare e premerlo nuovamente per terminare;
5. controllare la trascrizione prima di inviarla.

I microfoni Bluetooth usano normalmente il profilo vivavoce, distinto dal profilo stereo A2DP. Durante la cattura Windows può ridurre la qualità di riproduzione: non è una conversione di AURA. La pipeline accetta anche catture Bluetooth a 8 kHz e ricampiona localmente qualsiasi ingresso supportato a 16 kHz per Whisper.

## Installazione riproducibile

```powershell
./scripts/setup-whisper.ps1
```

Lo script installa nei dati applicativi:

- runtime ufficiale `whisper.cpp` v1.9.2 x64;
- modello multilingue `ggml-base.bin`;
- modello Silero VAD v6.2.0;
- SHA-256 verificato prima dell'installazione;
- nessun runtime o modello aggiunto al repository.

## Limiti di sicurezza

- il microfono richiede consenso Windows/WebView2;
- ascolto continuo soltanto su opt-in persistente, con banner visibile e trascrizione esclusivamente locale;
- massimo 30 secondi e circa 2 MiB per richiesta;
- timeout backend di 90 secondi;
- file temporanei con nomi casuali nello spazio dati AURA;
- nessuna rete usata durante la trascrizione;
- VAD locale e confidenza media calcolata dalle probabilità dei token Whisper;
- scelta esplicita del microfono, fallback sul predefinito Windows e aggiornamento su connessione/disconnessione;
- rilascio delle tracce audio anche in caso di errore e blocco delle trascrizioni concorrenti;
- diagnostica locale per segnale troppo basso e saturazione;
- invio automatico disattivato per default e bloccato sotto il 70% di confidenza;
- TTS locale e arresto immediato del processo di sintesi;
- trascrizioni, wake command e invii vocali registrati nel ledger come contenuti AES-256-GCM; l'audio non viene salvato e il testo è decifrato solo su richiesta nel pannello Log.

## Lavoro residuo

- calibrazione della soglia di confidenza su registrazioni reali dell'utente;
- test fisico con diversi ingressi (integrato, USB e Bluetooth), voce reale e rumore ambientale;
- eventuale Piper come voce neurale alternativa a SAPI (non necessario per il flusso TTS locale);
- migrazione da `ScriptProcessorNode` ad `AudioWorklet` e possibile detector openWakeWord per ridurre consumi e latenza;
- accessibilità e scorciatoia push-to-talk configurabile.
