# Voce locale

## Stato

La prima parte della Fase 3 è operativa su Windows. Il pulsante microfono attiva una registrazione push-to-talk esplicita, mostra un indicatore rosso e termina su richiesta o dopo 30 secondi. L'audio viene convertito nel frontend in WAV PCM mono, 16 kHz, 16 bit e inviato soltanto al backend Rust locale.

Il backend convalida struttura, formato, dimensione e durata, crea un file temporaneo, avvia `whisper.cpp` senza finestra, legge il risultato e cancella audio e trascrizione temporanei. Il testo viene inserito nel composer come bozza: non parte automaticamente verso il modello e non può eseguire azioni PC.

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
- la trascrizione deve essere controllata dall'utente prima dell'invio.

## Lavoro residuo

- calibrazione della soglia di confidenza su registrazioni reali dell'utente;
- test con microfono fisico e rumore ambientale;
- Piper per sintesi vocale interrompibile;
- gestione esplicita dei dispositivi di input;
- accessibilità e scorciatoia push-to-talk configurabile.
