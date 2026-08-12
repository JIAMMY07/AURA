# Provider IA estendibili

## Obiettivo

AURA non deve dipendere per sempre da un unico runtime. La chat usa quindi il contratto `AiProvider` v1:

- verifica di disponibilità;
- scoperta e normalizzazione dei modelli;
- chat in streaming e annullamento tramite `AbortSignal`;
- applicazione dei profili Luna, Terra e Sole;
- metadati dichiarativi sulle capacità.

`OllamaClient` implementa oggi il contratto ed è l'unico provider attivo. La UI della chat dipende dal contratto,
non direttamente dalla classe Ollama. Il backend espone inoltre un catalogo fail-closed che distingue chiaramente
provider attivi, raggiungibili e soltanto predisposti.

## Slot predisposti

| Provider | Protocollo previsto | Endpoint predefinito | Stato |
|---|---|---|---|
| Ollama | API Ollama | `127.0.0.1:11434` | attivo |
| llama.cpp server | OpenAI-compatible | `127.0.0.1:8080/v1` | predisposto |
| LocalAI | OpenAI-compatible | `127.0.0.1:8081/v1` | predisposto |
| Server locale generico | OpenAI-compatible | da configurare | predisposto |

Uno slot è solo configurazione dichiarativa: non scarica, installa o esegue codice. Un nuovo runtime dovrà essere
installato esplicitamente e superare controlli di versione, firma/hash, loopback, timeout, limiti di risorse e test.

## Cosa serve per attivare un provider

1. Implementare l'adapter frontend/backend e il parser dello stream.
2. Normalizzare nomi, dimensioni e capacità dei modelli.
3. Applicare timeout, annullamento, limiti di contesto e profili hardware.
4. Limitare l'endpoint al loopback per i provider locali.
5. Aggiungere test di compatibilità, privacy, error handling e regressione.
6. Collegare separatamente le capacità specialistiche necessarie.
7. Rendere la scelta disponibile nell'interfaccia soltanto dopo il superamento dei test.

## Limiti attuali dichiarati

L'astrazione copre la chat e la scoperta modelli. RAG/embeddings, Guardian, analisi schermo, pianificazione browser e
generazione delle patch di Evolution Lab chiamano ancora modelli Ollama specifici. Non sono automaticamente portabili
a un nuovo provider: serviranno interfacce dedicate per embeddings, vision, safety e tool calling.

I provider cloud sono intenzionalmente disabilitati. Prima di introdurli servono consenso esplicito, informativa su
quali dati lasciano il PC, archivio credenziali cifrato, scope minimi, revoca, budget e audit. Le chiavi non devono mai
essere conservate nel profilo personale o in file di configurazione in chiaro.

## Evoluzione consigliata

- v2: adapter OpenAI-compatible locale completo e selezione manuale del provider;
- v3: interfacce separate `ChatProvider`, `EmbeddingProvider`, `VisionProvider` e `SafetyProvider`;
- v4: routing automatico opzionale basato su capacità e benchmark, sempre disattivabile;
- v5: provider remoti opt-in con privacy preview e gestione sicura delle credenziali.
