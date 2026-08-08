import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity, Bot, CircleStop, Database, Headphones, Mic, RefreshCw, Send,
  ShieldCheck, SquareTerminal, Trash2, Wifi, WifiOff
} from 'lucide-react';
import { OllamaClient, type ChatMessage, type OllamaModel } from './adapters/ollama';
import { choosePreferredModel, RESOURCE_PROFILES, type ResourceProfileName } from './core/modelProfiles';
import { evaluate } from './core/policyEngine';
import type { ActionPlan, PolicyResult, Risk } from './core/types';
import { createConversationStore, type StorageMode } from './services/conversationStore';
import { evaluateAction, verifyAudit, type AuditStatus } from './adapters/security';
import {
  choosePreferredVoiceInput, describeMicrophoneError, getVoiceStatus, listVoiceInputDevices,
  transcribeVoice, VoiceRecorder, type VoiceDevicePreference, type VoiceInputDevice, type VoiceStatus
} from './adapters/voice';

const client = new OllamaClient();
const conversationStore = createConversationStore();
const VOICE_DEVICE_KEY = 'aura.voice-device.v1';
const demos: { label: string; risk: Risk; capability: string; scope: string }[] = [
  { label: 'Controlla utilizzo del PC', risk: 'safe', capability: 'system.inspect', scope: 'metriche locali' },
  { label: 'Organizza una cartella', risk: 'moderate', capability: 'files.organize', scope: 'cartella selezionata' },
  { label: 'Installa un nuovo modulo', risk: 'critical', capability: 'module.install', scope: 'sandbox temporanea' }
];

type Connection = 'checking' | 'online' | 'offline';

function newMessage(role: ChatMessage['role'], content: string): ChatMessage {
  return { id: crypto.randomUUID(), role, content, createdAt: new Date().toISOString() };
}

function formatBytes(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GiB`;
}

function loadVoiceDevicePreference(): VoiceDevicePreference {
  try {
    const raw = window.localStorage.getItem(VOICE_DEVICE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as VoiceDevicePreference;
    return {
      deviceId: typeof parsed.deviceId === 'string' ? parsed.deviceId : undefined,
      label: typeof parsed.label === 'string' ? parsed.label : undefined
    };
  } catch {
    return {};
  }
}

function saveVoiceDevicePreference(device?: VoiceInputDevice) {
  try {
    if (!device) window.localStorage.removeItem(VOICE_DEVICE_KEY);
    else window.localStorage.setItem(VOICE_DEVICE_KEY, JSON.stringify({ deviceId: device.deviceId, label: device.label }));
  } catch {
    // La preferenza è opzionale: la registrazione continua anche senza localStorage.
  }
}

export function App() {
  const [profileName, setProfileName] = useState<ResourceProfileName>('Bilanciato');
  const [selectedAction, setSelectedAction] = useState(0);
  const [connection, setConnection] = useState<Connection>('checking');
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [hydrated, setHydrated] = useState(false);
  const [storageMode, setStorageMode] = useState<StorageMode>(conversationStore.mode);
  const [auditStatus, setAuditStatus] = useState<AuditStatus>();
  const [draft, setDraft] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState('');
  const [tokensPerSecond, setTokensPerSecond] = useState<number>();
  const [voiceStatus, setVoiceStatus] = useState<VoiceStatus>();
  const [voiceRecording, setVoiceRecording] = useState(false);
  const [voiceProcessing, setVoiceProcessing] = useState(false);
  const [voiceNotice, setVoiceNotice] = useState('');
  const [voiceDevices, setVoiceDevices] = useState<VoiceInputDevice[]>([]);
  const [selectedVoiceDeviceId, setSelectedVoiceDeviceId] = useState(() => loadVoiceDevicePreference().deviceId ?? '');
  const [voiceDeviceBusy, setVoiceDeviceBusy] = useState(false);
  const [activeVoiceLabel, setActiveVoiceLabel] = useState('');
  const abortRef = useRef<AbortController | undefined>(undefined);
  const streamBufferRef = useRef('');
  const streamFlushRef = useRef<number | undefined>(undefined);
  const recorderRef = useRef<VoiceRecorder | undefined>(undefined);
  const voiceTimeoutRef = useRef<number | undefined>(undefined);
  const activeVoiceLabelRef = useRef('');
  const chatRef = useRef<HTMLElement | null>(null);
  const chatPinnedRef = useRef(true);
  const profile = RESOURCE_PROFILES[profileName];

  const plan: ActionPlan = useMemo(() => ({
    id: crypto.randomUUID(),
    summary: demos[selectedAction].label,
    risk: demos[selectedAction].risk,
    capability: demos[selectedAction].capability,
    scope: demos[selectedAction].scope,
    reversible: selectedAction !== 2
  }), [selectedAction]);
  const [result, setResult] = useState<PolicyResult>(() => evaluate(plan));

  async function refreshOllama() {
    setConnection('checking');
    setError('');
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), 2500);
    try {
      const available = await client.health(controller.signal);
      if (!available) {
        setConnection('offline');
        setModels([]);
        return;
      }
      const availableModels = await client.listModels(controller.signal);
      setModels(availableModels);
      setSelectedModel(current => availableModels.some(model => model.name === current)
        ? current
        : choosePreferredModel(availableModels.map(model => model.name)));
      setConnection('online');
    } catch {
      setConnection('offline');
    } finally {
      window.clearTimeout(timeout);
    }
  }

  useEffect(() => { void refreshOllama(); }, []);
  useEffect(() => { void getVoiceStatus().then(setVoiceStatus).catch(() => undefined); }, []);
  useEffect(() => {
    if (!voiceStatus?.available || !navigator.mediaDevices) return;
    void refreshVoiceDevices(false);
    const handleDeviceChange = () => { void refreshVoiceDevices(false); };
    navigator.mediaDevices.addEventListener?.('devicechange', handleDeviceChange);
    return () => navigator.mediaDevices.removeEventListener?.('devicechange', handleDeviceChange);
  }, [voiceStatus?.available]);
  useEffect(() => () => {
    abortRef.current?.abort();
    if (voiceTimeoutRef.current) window.clearTimeout(voiceTimeoutRef.current);
    void recorderRef.current?.cancel();
  }, []);
  useEffect(() => {
    if (!chatPinnedRef.current) return;
    chatRef.current?.scrollTo({ top: chatRef.current.scrollHeight, behavior: streaming ? 'auto' : 'smooth' });
  }, [messages, streaming]);
  useEffect(() => {
    let active = true;
    void evaluateAction(plan)
      .then(decision => {
        if (!active) return;
        setResult(decision);
        return verifyAudit().then(status => { if (active) setAuditStatus(status); });
      })
      .catch(() => { if (active) setResult({ decision: 'deny', risk: 'critical', reason: 'Policy backend non disponibile.' }); });
    return () => { active = false; };
  }, [plan]);
  useEffect(() => {
    let active = true;
    void conversationStore.load()
      .then(stored => { if (active) setMessages(stored); })
      .catch(() => {
        if (active) {
          setStorageMode('memory');
          setError('Archivio locale non disponibile: questa sessione resterà solo in memoria.');
        }
      })
      .finally(() => { if (active) setHydrated(true); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (!hydrated) return;
    const timer = window.setTimeout(() => {
      void conversationStore.save(messages).catch(() => {
        setStorageMode('memory');
        setError('Non riesco a salvare la conversazione. Nessun dato è stato inviato fuori dal PC.');
      });
    }, 350);
    return () => window.clearTimeout(timer);
  }, [hydrated, messages]);

  async function sendMessage(event: FormEvent) {
    event.preventDefault();
    const prompt = draft.trim();
    if (!prompt || streaming) return;
    if (connection !== 'online' || !selectedModel) {
      setError(connection !== 'online'
        ? 'Avvia Ollama per usare la chat locale.'
        : 'Nessun modello installato: aggiungine uno in Ollama.');
      return;
    }

    const userMessage = newMessage('user', prompt);
    const assistantMessage = newMessage('assistant', '');
    const history = [...messages, userMessage];
    chatPinnedRef.current = true;
    setMessages([...history, assistantMessage]);
    setDraft('');
    setError('');
    setStreaming(true);
    setTokensPerSecond(undefined);
    const controller = new AbortController();
    abortRef.current = controller;
    streamBufferRef.current = '';
    const generationTimeout = window.setTimeout(() => controller.abort(), 180_000);

    try {
      const stream = await client.chat(
        selectedModel,
        history,
        profile,
        assistantMessage.id,
        token => queueStreamToken(assistantMessage.id, token),
        controller.signal
      );
      flushStreamTokens(assistantMessage.id);
      setTokensPerSecond(stream.tokensPerSecond);
    } catch (cause) {
      flushStreamTokens(assistantMessage.id);
      if (cause instanceof DOMException && cause.name === 'AbortError') {
        setError('Generazione interrotta. La risposta parziale è stata conservata.');
        setMessages(current => current.filter(message =>
          message.id !== assistantMessage.id || message.content.length > 0
        ));
      } else {
        setError(cause instanceof Error ? cause.message : 'Errore durante la generazione.');
        setMessages(current => current.filter(message =>
          message.id !== assistantMessage.id || message.content.length > 0
        ));
      }
    } finally {
      flushStreamTokens(assistantMessage.id);
      window.clearTimeout(generationTimeout);
      abortRef.current = undefined;
      setStreaming(false);
    }
  }

  function queueStreamToken(messageId: string, token: string) {
    streamBufferRef.current += token;
    if (streamFlushRef.current) return;
    streamFlushRef.current = window.setTimeout(() => flushStreamTokens(messageId), 40);
  }

  function flushStreamTokens(messageId: string) {
    if (streamFlushRef.current) window.clearTimeout(streamFlushRef.current);
    streamFlushRef.current = undefined;
    const pending = streamBufferRef.current;
    streamBufferRef.current = '';
    if (!pending) return;
    setMessages(current => current.map(message =>
      message.id === messageId ? { ...message, content: message.content + pending } : message
    ));
  }

  async function resetConversation() {
    abortRef.current?.abort();
    try {
      await conversationStore.clear();
    } catch {
      setError('Impossibile cancellare l’archivio locale.');
      return;
    }
    setMessages([]);
    setError('');
  }

  async function refreshVoiceDevices(requestPermission: boolean): Promise<VoiceInputDevice | undefined> {
    setVoiceDeviceBusy(true);
    try {
      const devices = await listVoiceInputDevices(requestPermission);
      setVoiceDevices(devices);
      const preference = loadVoiceDevicePreference();
      const preferredOppoMissing = /oppo/i.test(preference.label ?? '')
        && !devices.some(device => device.deviceId === preference.deviceId || device.label === preference.label);
      const selected = preferredOppoMissing ? undefined : choosePreferredVoiceInput(devices, preference);
      setSelectedVoiceDeviceId(selected?.deviceId ?? '');
      if (selected && (/oppo/i.test(selected.label) || preference.deviceId || preference.label)) {
        saveVoiceDevicePreference(selected);
      }
      if (requestPermission) {
        setVoiceNotice(selected
          ? `Microfono pronto: ${selected.label || 'dispositivo predefinito'}.`
          : 'Nessun microfono di ingresso rilevato. Connetti le cuffie OPPO.');
      }
      return selected;
    } catch (cause) {
      if (requestPermission) setError(describeMicrophoneError(cause));
      return undefined;
    } finally {
      setVoiceDeviceBusy(false);
    }
  }

  function selectVoiceDevice(deviceId: string) {
    const device = voiceDevices.find(candidate => candidate.deviceId === deviceId);
    setSelectedVoiceDeviceId(deviceId);
    saveVoiceDevicePreference(device);
    setVoiceNotice(device ? `Microfono selezionato: ${device.label || 'dispositivo predefinito'}.` : '');
  }

  async function startVoiceRecording() {
    if (!voiceStatus?.available || voiceRecording || voiceProcessing) return;
    setError('');
    setVoiceNotice('');
    const recorder = new VoiceRecorder();
    recorderRef.current = recorder;
    try {
      let device = voiceDevices.find(candidate => candidate.deviceId === selectedVoiceDeviceId);
      if (!device || !device.label) device = await refreshVoiceDevices(true);
      if (!device) throw new DOMException('No microphone', 'NotFoundError');
      const capture = await recorder.start(device.deviceId || undefined, () => {
        setError('Il microfono si è disconnesso durante la registrazione. Riconnetti le cuffie OPPO.');
        void finishVoiceRecording(true);
      });
      activeVoiceLabelRef.current = capture.label;
      setActiveVoiceLabel(capture.label);
      setVoiceNotice(`Registrazione da ${capture.label} · ${Math.round(capture.sampleRate / 1000)} kHz.`);
      setVoiceRecording(true);
      voiceTimeoutRef.current = window.setTimeout(() => { void finishVoiceRecording(); }, voiceStatus.maxSeconds * 1000);
    } catch (cause) {
      if (recorderRef.current === recorder) recorderRef.current = undefined;
      await recorder.cancel();
      setError(describeMicrophoneError(cause));
    }
  }

  async function finishVoiceRecording(disconnected = false) {
    const recorder = recorderRef.current;
    if (!recorder) return;
    recorderRef.current = undefined;
    if (voiceTimeoutRef.current) window.clearTimeout(voiceTimeoutRef.current);
    voiceTimeoutRef.current = undefined;
    setVoiceRecording(false);
    setVoiceProcessing(true);
    if (!disconnected) setError('');
    try {
      const recording = await recorder.stop();
      let transcript;
      try {
        transcript = await transcribeVoice(recording.wav);
      } finally {
        recording.wav.fill(0);
      }
      setDraft(transcript.text);
      const confidence = Math.round(transcript.confidence * 100);
      const quality = recording.rms < 0.006
        ? ' Segnale molto basso: avvicina il microfono o controlla il profilo vivavoce.'
        : recording.peak >= 0.995
          ? ' Audio vicino alla saturazione: abbassa il livello del microfono.'
          : '';
      const interrupted = disconnected ? ' Registrazione interrotta dalla disconnessione.' : '';
      setVoiceNotice(`Trascrizione locale da ${activeVoiceLabelRef.current || 'microfono'} (${transcript.durationSeconds.toFixed(1)} s, confidenza ${confidence}%). ${confidence < 70 ? 'Confidenza bassa: rileggila con particolare attenzione.' : 'Controllala prima di inviarla.'}${quality}${interrupted}`);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(disconnected ? `Cuffie disconnesse. ${message}` : message);
    } finally {
      setVoiceProcessing(false);
    }
  }

  return <div className="app">
    <aside>
      <div className="brand"><Bot /> AURA</div>
      <nav>
        <button className="active"><SquareTerminal /> Assistente</button>
        <button><Activity /> Attività</button>
        <button><ShieldCheck /> Sicurezza</button>
      </nav>
      <div className={`privacy ${connection}`}>
        <span /> {connection === 'online' ? 'Ollama locale' : connection === 'checking' ? 'Verifica locale…' : 'Rete AI off'}
      </div>
    </aside>

    <main>
      <header>
        <div><p className="eyebrow">LOCAL AI CONTROL CENTER</p><h1>Buonasera. Cosa vuoi fare?</h1></div>
        <div className="header-controls">
          <select aria-label="Profilo risorse" value={profileName} onChange={event => setProfileName(event.target.value as ResourceProfileName)}>
            {(Object.keys(RESOURCE_PROFILES) as ResourceProfileName[]).map(name => <option key={name}>{name}</option>)}
          </select>
          <button className="icon-button" onClick={() => void refreshOllama()} title="Aggiorna Ollama" disabled={connection === 'checking'}><RefreshCw /></button>
        </div>
      </header>

      <section className="model-bar">
        <div className={`connection-pill ${connection}`}>
          {connection === 'online' ? <Wifi /> : <WifiOff />}
          <span>{connection === 'online' ? `${models.length} modelli locali` : connection === 'checking' ? 'Connessione…' : 'Ollama non rilevato'}</span>
        </div>
        <select aria-label="Modello locale" value={selectedModel} onChange={event => setSelectedModel(event.target.value)} disabled={!models.length}>
          {!models.length && <option>Nessun modello</option>}
          {models.map(model => <option key={model.name} value={model.name}>{model.name}</option>)}
        </select>
        {selectedModel && <span className="model-detail">{models.find(model => model.name === selectedModel)?.details?.parameter_size ?? 'dimensione ignota'} · {formatBytes(models.find(model => model.name === selectedModel)?.size ?? 0)}</span>}
      </section>

      {voiceStatus?.available && <section className="voice-device-bar">
        <Headphones />
        <label htmlFor="voice-device">Microfono</label>
        <select
          id="voice-device"
          aria-label="Microfono per i comandi vocali"
          value={selectedVoiceDeviceId}
          onChange={event => selectVoiceDevice(event.target.value)}
          disabled={voiceRecording || voiceProcessing || voiceDeviceBusy}
        >
          {!voiceDevices.length && <option value="">Autorizza e rileva microfoni</option>}
          {voiceDevices.map((device, index) => <option key={device.deviceId || `${device.groupId}-${index}`} value={device.deviceId}>
            {device.label || `Microfono ${index + 1}`}
          </option>)}
        </select>
        <button
          type="button"
          className="icon-button"
          onClick={() => void refreshVoiceDevices(true)}
          disabled={voiceRecording || voiceProcessing || voiceDeviceBusy}
          title="Aggiorna e autorizza i microfoni"
          aria-label="Aggiorna microfoni"
        ><RefreshCw /></button>
        <small>{voiceDevices.some(device => /oppo/i.test(device.label)) ? 'Cuffie OPPO rilevate' : 'Connetti le OPPO per usare il loro microfono'}</small>
      </section>}

      <section
        className="chat"
        aria-live="polite"
        ref={chatRef}
        onScroll={event => {
          const target = event.currentTarget;
          chatPinnedRef.current = target.scrollHeight - target.scrollTop - target.clientHeight < 80;
        }}
      >
        {!messages.length && <div className="empty-chat">
          <div className="orb"><Mic /></div>
          <p>Chat locale pronta</p>
          <small>{voiceStatus?.available ? 'Push-to-talk locale pronto. La trascrizione non viene inviata automaticamente.' : 'Motore vocale locale non disponibile.'}</small>
        </div>}
        {messages.map(message => <article key={message.id} className={`message ${message.role}`}>
          <span>{message.role === 'user' ? 'Tu' : 'AURA'}</span>
          <p>{message.content || (streaming ? 'Sto elaborando…' : '')}</p>
        </article>)}
      </section>

      {error && <div className="error-banner">{error}</div>}
      {voiceRecording && <div className="voice-banner"><span /> {activeVoiceLabel || 'Microfono'} attivo — premi di nuovo per terminare</div>}
      {voiceProcessing && <div className="voice-banner processing"><RefreshCw /> Trascrizione locale in corso…</div>}
      {voiceNotice && <div className="voice-notice">{voiceNotice}</div>}
      <form className="composer" onSubmit={sendMessage}>
        <textarea value={draft} onChange={event => setDraft(event.target.value)} placeholder="Scrivi un comando o una domanda…" rows={2} onKeyDown={event => {
          if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }} />
        <button
          type="button"
          className={`voice-button ${voiceRecording ? 'recording' : ''}`}
          onClick={() => voiceRecording ? void finishVoiceRecording() : void startVoiceRecording()}
          disabled={!voiceStatus?.available || voiceProcessing || streaming}
          title={voiceStatus?.available ? (voiceRecording ? 'Termina registrazione' : 'Avvia push-to-talk') : 'Motore vocale locale non installato'}
          aria-label={voiceRecording ? 'Termina registrazione vocale' : 'Avvia registrazione vocale'}
        ><Mic /></button>
        {streaming
          ? <button type="button" className="stop" onClick={() => abortRef.current?.abort()}><CircleStop /> Stop</button>
          : <button type="submit" disabled={!draft.trim()}><Send /> Invia</button>}
        <button type="button" className="secondary" onClick={() => void resetConversation()} title="Cancella cronologia"><Trash2 /></button>
      </form>

      <details className="simulation">
        <summary>Laboratorio policy — simula un’azione</summary>
        <div className="actions">{demos.map((demo, index) => <button key={demo.label} onClick={() => setSelectedAction(index)} className={index === selectedAction ? 'chosen' : ''}>{demo.label}</button>)}</div>
        <div className="plan">
          <div><span className={`badge ${result.risk}`}>{result.risk}</span><h2>{plan.summary}</h2><p>Capability: {plan.capability} · Scope: {plan.scope}</p></div>
          <div className={`decision ${result.decision}`}><strong>{result.decision.toUpperCase()}</strong><span>{result.reason}</span></div>
        </div>
      </details>

      <section className="stats">
        <article><span>Modello</span><strong>{selectedModel || 'Non connesso'}</strong></article>
        <article><span>Profilo</span><strong>{profileName} · {profile.contextTokens / 1024}k</strong></article>
        <article><span>Prestazioni</span><strong>{tokensPerSecond ? `${tokensPerSecond.toFixed(1)} tok/s` : 'In attesa'}</strong></article>
        <article><span>Memoria chat</span><strong><Database /> {storageMode === 'encrypted-windows' ? 'Cifrata' : storageMode === 'browser-fallback' ? 'Locale' : 'RAM'} · {messages.length}/100</strong></article>
        <article><span>Audit</span><strong><ShieldCheck /> {auditStatus ? `${auditStatus.valid ? 'Integro' : 'Alterato'} · ${auditStatus.count}` : 'Anteprima web'}</strong></article>
      </section>
    </main>
  </div>;
}
