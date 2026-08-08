# Voce locale

## Stato

La prima parte della Fase 3 è operativa su Windows. Il pulsante microfono attiva una registrazione push-to-talk esplicita, mostra un indicatore rosso e termina su richiesta o dopo 30 secondi. L'audio viene convertito nel frontend in WAV PCM mono, 16 kHz, 16 bit e inviato soltanto al backend Rust locale.

Il backend convalida struttura, formato, dimensione e durata, crea un file temporaneo, avvia `whisper.cpp` senza finestra, legge il risultato e cancella audio e trascrizione temporanei. Il testo viene inserito nel composer come bozza. L'utente può attivare l'invio automatico soltanto per trascrizioni con confidenza almeno 70%; le azioni PC che cambiano stato mantengono comunque la conferma nativa Windows.

La sintesi vocale locale usa Windows SAPI con una voce italiana installata, accetta al massimo 4.000 caratteri tramite `stdin` (mai come codice PowerShell) ed è interrompibile dalla UI. Sul PC target è stata verificata `Microsoft Elsa Desktop` (`it-IT`) producendo un WAV RIFF valido.

## Cuffie OPPO Enco Buds2 Pro

Windows rileva le cuffie associate e il relativo servizio `OPPO Enco Buds2 Pro Hands-Free`. AURA non salva un identificatore hardware Windows e non forza un endpoint invisibile: usa l'elenco microfoni autorizzato da WebView2, ricorda localmente la scelta e ritrova il dispositivo anche se l'identificatore web cambia dopo una nuova associazione.

Procedura d'uso:

1. indossare e connettere le cuffie a Windows;
2. in AURA premere **Aggiorna microfoni** e concedere l'accesso la prima volta;
3. scegliere `OPPO Enco Buds2 Pro Hands-Free` nel selettore Microfono;
4. premere il pulsante microfono, parlare e premerlo nuovamente per terminare;
5. controllare la trascrizione prima di inviarla.

Il microfono Bluetooth usa il profilo vivavoce, distinto dal profilo stereo A2DP. Durante la cattura Windows può ridurre la qualità di riproduzione: non è una conversione di AURA. La pipeline accetta anche catture Bluetooth a 8 kHz e le ricampiona localmente a 16 kHz per Whisper.

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
- nessun ascolto continuo o wake word;
- massimo 30 secondi e circa 2 MiB per richiesta;
- timeout backend di 90 secondi;
- file temporanei con nomi casuali nello spazio dati AURA;
- nessuna rete usata durante la trascrizione;
- VAD locale e confidenza media calcolata dalle probabilità dei token Whisper;
- scelta esplicita del microfono, preferenza OPPO e aggiornamento su connessione/disconnessione;
- rilascio delle tracce audio anche in caso di errore e blocco delle trascrizioni concorrenti;
- diagnostica locale per segnale troppo basso e saturazione;
- invio automatico disattivato per default e bloccato sotto il 70% di confidenza;
- TTS locale, contenuto escluso dall'audit e arresto immediato del processo di sintesi.

## Lavoro residuo

- calibrazione della soglia di confidenza su registrazioni reali dell'utente;
- test fisico con OPPO connesse, voce reale e rumore ambientale;
- eventuale Piper come voce neurale alternativa a SAPI (non necessario per il flusso TTS locale);
- migrazione da `ScriptProcessorNode` ad `AudioWorklet` prima dell'ascolto continuo;
- accessibilità e scorciatoia push-to-talk configurabile.
