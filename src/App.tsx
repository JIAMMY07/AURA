import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity, BookOpen, Bot, Boxes, CircleStop, Cpu, Database, FlaskConical, Globe2, Headphones, Mic, Plug, RefreshCw, Send,
  ShieldCheck, SquareTerminal, ThumbsUp, Trash2, UserRound, Volume2, VolumeX, Wifi, WifiOff
} from 'lucide-react';
import { OllamaClient, type ChatMessage, type OllamaModel } from './adapters/ollama';
import { choosePreferredModel, RESOURCE_PROFILES, type ResourceProfileName } from './core/modelProfiles';
import { evaluate } from './core/policyEngine';
import type { ActionPlan, PolicyResult, Risk } from './core/types';
import { createConversationStore, type StorageMode } from './services/conversationStore';
import { evaluateAction, verifyAudit, type AuditStatus } from './adapters/security';
import {
  choosePreferredVoiceInput, describeMicrophoneError, getVoiceStatus, listVoiceInputDevices,
  speakText, stopSpeaking, transcribeVoice, VoiceRecorder, type VoiceDevicePreference, type VoiceInputDevice, type VoiceStatus
} from './adapters/voice';
import {
  callApi, clearProfile, closeControlledBrowser, deleteApiConnector, deleteRagDocument, fetchBrowserPage, generate3d, generateEvolutionPatch, getBrowserStatus, getCreation3dStatus, getGuardrailStatus, getSandboxStatus, getSystemSnapshot, importRagDocument, listApiConnectors, listEvolutionProposals, listProcesses, listRagDocuments, loadProfile, lockWorkstation, navigateControlledBrowser, open3dInBlender, openApplication, openControlledBrowser,
  promoteEvolutionPatch, recordEvolutionProposal, recordFeedback, runDefenderQuickScan, runEvolutionSandbox, saveApiConnector, saveProfile, searchRag, terminateProcess, updateDefenderDefinitions, getSecurityCenterStatus, type ApiConnector, type BrowserPage, type BrowserStatus, type Creation3dResult, type Creation3dStatus, type EvolutionReportView, type GuardrailStatus, type PersonalProfile, type RagDocument, type RagHit, type SandboxStatus, type SecurityCenterStatus
} from './adapters/agent';
import { parseLocalCommand, type LocalCommand } from './core/localCommands';
import { RemoteAccessPanel } from './features/remote/RemoteAccessPanel';

const client = new OllamaClient();
const conversationStore = createConversationStore();
const VOICE_DEVICE_KEY = 'aura.voice-device.v1';
const EMPTY_PROFILE: PersonalProfile = {
  displayName: '', communicationStyle: '', goals: '', preferences: '', constraints: '', projectContext: ''
};
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
  const [autoSendVoice, setAutoSendVoice] = useState(false);
  const [autoSpeak, setAutoSpeak] = useState(false);
  const [speaking, setSpeaking] = useState(false);
  const [toolRunning, setToolRunning] = useState(false);
  const [terminatePid, setTerminatePid] = useState('');
  const [personalProfile, setPersonalProfile] = useState<PersonalProfile>(EMPTY_PROFILE);
  const [profileNotice, setProfileNotice] = useState('');
  const [sandbox, setSandbox] = useState<SandboxStatus>();
  const [evolutionObjective, setEvolutionObjective] = useState('');
  const [evolutionNotice, setEvolutionNotice] = useState('');
  const [evolutionReports, setEvolutionReports] = useState<EvolutionReportView[]>([]);
  const [evolutionProjectPath, setEvolutionProjectPath] = useState('');
  const [evolutionTestProfile, setEvolutionTestProfile] = useState<'Static' | 'Node' | 'Rust'>('Node');
  const [evolutionBusy, setEvolutionBusy] = useState(false);
  const [guardrail, setGuardrail] = useState<GuardrailStatus>();
  const [creation3dStatus, setCreation3dStatus] = useState<Creation3dStatus>();
  const [creation3dInput, setCreation3dInput] = useState('');
  const [creation3dJob, setCreation3dJob] = useState('aura-mesh');
  const [creation3dResult, setCreation3dResult] = useState<Creation3dResult>();
  const [browserStatus, setBrowserStatus] = useState<BrowserStatus>();
  const [browserUrl, setBrowserUrl] = useState('https://example.com');
  const [browserPage, setBrowserPage] = useState<BrowserPage>();
  const [apiConnectors, setApiConnectors] = useState<ApiConnector[]>([]);
  const [apiName, setApiName] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiSecret, setApiSecret] = useState('');
  const [apiSelected, setApiSelected] = useState('');
  const [apiMethod, setApiMethod] = useState<'GET'|'POST'|'PUT'|'PATCH'|'DELETE'>('GET');
  const [apiPath, setApiPath] = useState('');
  const [apiBody, setApiBody] = useState('');
  const [apiOutput, setApiOutput] = useState('');
  const [ragDocuments, setRagDocuments] = useState<RagDocument[]>([]);
  const [ragPath, setRagPath] = useState('');
  const [ragQuery, setRagQuery] = useState('');
  const [ragHits, setRagHits] = useState<RagHit[]>([]);
  const [labBusy, setLabBusy] = useState(false);
  const [labNotice, setLabNotice] = useState('');
  const [securityCenter, setSecurityCenter] = useState<SecurityCenterStatus>();
  const abortRef = useRef<AbortController | undefined>(undefined);
  const streamBufferRef = useRef('');
  const fullResponseRef = useRef('');
  const streamFlushRef = useRef<number | undefined>(undefined);
  const recorderRef = useRef<VoiceRecorder | undefined>(undefined);
  const voiceTimeoutRef = useRef<number | undefined>(undefined);
  const ttsTimeoutRef = useRef<number | undefined>(undefined);
  const activeVoiceLabelRef = useRef('');
  const chatRef = useRef<HTMLElement | null>(null);
  const composerRef = useRef<HTMLFormElement | null>(null);
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
    void loadProfile().then(profile => { if (profile) setPersonalProfile(profile); }).catch(() => undefined);
    void getSandboxStatus().then(setSandbox).catch(() => undefined);
    void listEvolutionProposals().then(setEvolutionReports).catch(() => undefined);
    void getGuardrailStatus().then(setGuardrail).catch(() => undefined);
    void getCreation3dStatus().then(setCreation3dStatus).catch(() => undefined);
    void getBrowserStatus().then(setBrowserStatus).catch(() => undefined);
    void listApiConnectors().then(connectors => { setApiConnectors(connectors); setApiSelected(connectors[0]?.id ?? ''); }).catch(() => undefined);
    void listRagDocuments().then(setRagDocuments).catch(() => undefined);
    void getSecurityCenterStatus().then(setSecurityCenter).catch(() => undefined);
  }, []);
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
    if (ttsTimeoutRef.current) window.clearTimeout(ttsTimeoutRef.current);
    void recorderRef.current?.cancel();
    void stopSpeaking();
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
    if (!prompt || streaming || toolRunning) return;
    const localCommand = parseLocalCommand(prompt);
    if (localCommand) {
      setDraft('');
      await executeLocalCommand(prompt, localCommand);
      return;
    }
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
    fullResponseRef.current = '';
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
      if (autoSpeak && fullResponseRef.current.trim() && voiceStatus?.ttsAvailable) {
        await readAloud(fullResponseRef.current);
      }
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

  async function executeLocalCommand(prompt: string, command: LocalCommand) {
    const userMessage = newMessage('user', prompt);
    const assistantMessage = newMessage('assistant', 'Esecuzione strumento locale in corso…');
    chatPinnedRef.current = true;
    setMessages(current => [...current, userMessage, assistantMessage]);
    setError('');
    setToolRunning(true);
    const complete = (content: string) => {
      setMessages(current => current.map(message =>
        message.id === assistantMessage.id ? { ...message, content } : message
      ));
      if (autoSpeak && voiceStatus?.ttsAvailable) void readAloud(content);
    };
    try {
      if (command.kind === 'system-status') {
        const snapshot = await getSystemSnapshot();
        const disks = snapshot.disks.map(disk =>
          `${disk.mountPoint}: ${formatBytes(disk.availableBytes)} liberi su ${formatBytes(disk.totalBytes)}`
        ).join('\n');
        complete(`Stato PC verificato localmente:\nCPU: ${snapshot.cpuUsagePercent.toFixed(1)}% su ${snapshot.logicalCpus} thread logici\nRAM: ${formatBytes(snapshot.memoryUsedBytes)} / ${formatBytes(snapshot.memoryTotalBytes)}\nProcessi: ${snapshot.processCount}\nUptime: ${(snapshot.uptimeSeconds / 3600).toFixed(1)} ore\nDischi:\n${disks || 'nessun dato'}`);
      } else if (command.kind === 'process-list') {
        const processes = await listProcesses();
        complete(`Processi con maggiore uso di memoria (lettura locale, massimo 15 mostrati):\n${processes.slice(0, 15).map(process =>
          `• ${process.name} · PID ${process.pid} · ${formatBytes(process.memoryBytes)}`
        ).join('\n')}`);
      } else if (command.kind === 'open-app') {
        const opened = await openApplication(command.application);
        if (opened) {
          complete(`${command.label} avviata tramite tool tipizzato e allowlist. Nessuna shell è stata usata.`);
        } else complete(`Apertura di ${command.label} annullata nel dialogo nativo Windows.`);
      } else if (command.kind === 'terminate-process') {
        const terminated = await terminateProcess(command.pid);
        complete(terminated
          ? `Processo PID ${command.pid} terminato dopo conferma nativa.`
          : `Terminazione del processo PID ${command.pid} annullata.`);
      } else if (command.kind === 'lock-workstation') {
        const locked = await lockWorkstation();
        complete(locked ? 'Sessione Windows bloccata.' : 'Blocco della sessione annullato.');
      } else {
        const status = await getSandboxStatus();
        setSandbox(status);
        complete(`Evolution Lab:\nIsolamento forte: ${status.strongIsolationAvailable ? 'rilevato' : 'non disponibile'}\nEsecuzione codice: ${status.executionAllowed ? 'abilitata' : 'bloccata'}\n${status.reason}`);
      }
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      complete(`Strumento non eseguito: ${message}`);
      setError(message);
    } finally {
      setToolRunning(false);
    }
  }

  async function persistProfile() {
    setProfileNotice('');
    try {
      const saved = await saveProfile(personalProfile);
      if (!saved) return;
      setProfileNotice('Profilo cifrato salvato. Sarà applicato dalla prossima risposta locale.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function removeProfile() {
    try {
      const cleared = await clearProfile();
      if (!cleared) return;
      setPersonalProfile(EMPTY_PROFILE);
      setProfileNotice('Profilo e feedback cancellati definitivamente.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function submitFeedback(messageId: string, correction = false) {
    const text = correction
      ? window.prompt('Spiega ad AURA cosa era sbagliato e come dovrebbe comportarsi in futuro. Non inserire password o segreti.')
      : '';
    if (correction && !text?.trim()) return;
    try {
      await recordFeedback(messageId, correction ? 'correction' : 'helpful', text?.trim() ?? '');
      setProfileNotice(correction
        ? 'Correzione cifrata registrata e disponibile per le prossime risposte.'
        : 'Feedback positivo registrato localmente.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function createEvolutionProposal() {
    if (!evolutionObjective.trim()) return;
    try {
      const proposal = await recordEvolutionProposal(
        personalProfile.projectContext.trim() || 'Progetto non specificato',
        evolutionObjective.trim()
      );
      setEvolutionNotice(`Proposta ${proposal.id} registrata senza eseguire codice. Report: ${proposal.reportPath}`);
      setEvolutionReports(await listEvolutionProposals());
      setEvolutionObjective('');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function runEvolutionStage(report: EvolutionReportView, stage: 'generate' | 'test' | 'promote') {
    if (!evolutionProjectPath.trim() || evolutionBusy) return;
    setEvolutionBusy(true);
    setEvolutionNotice('');
    setError('');
    try {
      const operation = stage === 'generate'
        ? await generateEvolutionPatch(report.id, evolutionProjectPath.trim())
        : stage === 'test'
          ? await runEvolutionSandbox(report.id, evolutionProjectPath.trim(), evolutionTestProfile)
          : await promoteEvolutionPatch(report.id, evolutionProjectPath.trim(), report.artifactPath);
      setEvolutionNotice(`${operation.status}. Artefatto: ${operation.artifactPath}`);
      setEvolutionReports(await listEvolutionProposals());
      setSandbox(await getSandboxStatus());
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message);
      setEvolutionNotice(`Fase non completata: ${message}`);
    } finally {
      setEvolutionBusy(false);
    }
  }

  function queueStreamToken(messageId: string, token: string) {
    streamBufferRef.current += token;
    fullResponseRef.current += token;
    if (streamFlushRef.current) return;
    streamFlushRef.current = window.setTimeout(() => flushStreamTokens(messageId), 40);
  }

  async function readAloud(text: string) {
    if (!text.trim() || !voiceStatus?.ttsAvailable) return;
    try {
      setSpeaking(true);
      await speakText(text.slice(0, 4_000));
      if (ttsTimeoutRef.current) window.clearTimeout(ttsTimeoutRef.current);
      const estimatedMilliseconds = Math.min(180_000, Math.max(2_000, text.length * 70));
      ttsTimeoutRef.current = window.setTimeout(() => setSpeaking(false), estimatedMilliseconds);
    } catch (cause) {
      setSpeaking(false);
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function stopReadAloud() {
    await stopSpeaking().catch(() => false);
    if (ttsTimeoutRef.current) window.clearTimeout(ttsTimeoutRef.current);
    ttsTimeoutRef.current = undefined;
    setSpeaking(false);
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
      const cleared = await conversationStore.clear();
      if (!cleared) return;
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
      if (autoSendVoice && confidence >= 70 && !disconnected) {
        window.setTimeout(() => composerRef.current?.requestSubmit(), 0);
      }
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(disconnected ? `Cuffie disconnesse. ${message}` : message);
    } finally {
      setVoiceProcessing(false);
    }
  }

  async function runLab(action: () => Promise<void>) {
    setLabBusy(true); setLabNotice('');
    try { await action(); } catch (reason) { setLabNotice(reason instanceof Error ? reason.message : String(reason)); }
    finally { setLabBusy(false); }
  }

  async function createMesh() {
    await runLab(async () => {
      const result = await generate3d(creation3dInput.trim(), creation3dJob.trim(), 'Auto', 128, 'glb', false);
      setCreation3dResult(result); setLabNotice(`Mesh generata: ${result.outputPath}`);
    });
  }

  async function inspectPage() {
    await runLab(async () => { const page = await fetchBrowserPage(browserUrl.trim()); setBrowserPage(page); setLabNotice(`Pagina acquisita: ${page.title}`); });
  }

  async function persistApiConnector() {
    await runLab(async () => {
      const id = await saveApiConnector({ name: apiName.trim(), baseUrl: apiBaseUrl.trim(), authKind: apiSecret ? 'bearer' : 'none', authHeader: 'Authorization', secret: apiSecret });
      const connectors = await listApiConnectors(); setApiConnectors(connectors); setApiSelected(id); setApiSecret(''); setLabNotice('Connettore salvato con credenziale cifrata.');
    });
  }

  async function executeApiCall() {
    await runLab(async () => { const result = await callApi(apiSelected, apiMethod, apiPath, apiBody); setApiOutput(`HTTP ${result.status}\n${result.body}`); setLabNotice('Chiamata API completata.'); });
  }

  async function indexRagDocument() {
    await runLab(async () => { const result = await importRagDocument(ragPath.trim()); setRagDocuments(await listRagDocuments()); setLabNotice(`${result.sourceName}: ${result.chunkCount} frammenti cifrati indicizzati.`); });
  }

  async function queryRag() {
    await runLab(async () => { const hits = await searchRag(ragQuery.trim(), 5); setRagHits(hits); setLabNotice(`${hits.length} fonti locali recuperate.`); });
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
        <label className="voice-toggle"><input type="checkbox" checked={autoSendVoice} onChange={event => setAutoSendVoice(event.target.checked)} /> Invia automaticamente (confidenza ≥70%)</label>
        <label className="voice-toggle"><input type="checkbox" checked={autoSpeak} onChange={event => setAutoSpeak(event.target.checked)} disabled={!voiceStatus.ttsAvailable} /> Leggi risposte</label>
        {speaking && <button type="button" className="icon-button" onClick={() => void stopReadAloud()} title="Ferma voce"><VolumeX /></button>}
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
          {message.role === 'assistant' && message.content && <div className="feedback-actions">
            {voiceStatus?.ttsAvailable && <button type="button" onClick={() => void readAloud(message.content)} title="Leggi risposta"><Volume2 /> Ascolta</button>}
            <button type="button" onClick={() => void submitFeedback(message.id)} title="Risposta utile"><ThumbsUp /> Utile</button>
            <button type="button" onClick={() => void submitFeedback(message.id, true)}>Correggi</button>
          </div>}
        </article>)}
      </section>

      {error && <div className="error-banner">{error}</div>}
      {voiceRecording && <div className="voice-banner"><span /> {activeVoiceLabel || 'Microfono'} attivo — premi di nuovo per terminare</div>}
      {voiceProcessing && <div className="voice-banner processing"><RefreshCw /> Trascrizione locale in corso…</div>}
      {voiceNotice && <div className="voice-notice">{voiceNotice}</div>}
      <form ref={composerRef} className="composer" onSubmit={sendMessage}>
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
          disabled={!voiceStatus?.available || voiceProcessing || streaming || toolRunning}
          title={voiceStatus?.available ? (voiceRecording ? 'Termina registrazione' : 'Avvia push-to-talk') : 'Motore vocale locale non installato'}
          aria-label={voiceRecording ? 'Termina registrazione vocale' : 'Avvia registrazione vocale'}
        ><Mic /></button>
        {streaming
          ? <button type="button" className="stop" onClick={() => abortRef.current?.abort()}><CircleStop /> Stop</button>
          : <button type="submit" disabled={!draft.trim() || toolRunning}><Send /> {toolRunning ? 'Strumento…' : 'Invia'}</button>}
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

      <details className="personalization pc-control">
        <summary><Cpu /> Centro controllo PC</summary>
        <p className="section-help">Azioni tipizzate: nessun comando arbitrario. Aperture, terminazioni e blocco sessione richiedono una conferma nativa Windows.</p>
        <div className="pc-action-grid">
          <button type="button" onClick={() => void executeLocalCommand('Stato del PC', { kind: 'system-status' })}>Stato PC</button>
          <button type="button" onClick={() => void executeLocalCommand('Mostra i processi', { kind: 'process-list' })}>Processi</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni', { kind: 'open-app', application: 'settings', label: 'Impostazioni Windows' })}>Impostazioni</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni Bluetooth', { kind: 'open-app', application: 'bluetooth-settings', label: 'Impostazioni Bluetooth' })}>Bluetooth</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni audio', { kind: 'open-app', application: 'sound-settings', label: 'Impostazioni audio' })}>Audio</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri sicurezza Windows', { kind: 'open-app', application: 'security-settings', label: 'Sicurezza di Windows' })}>Sicurezza</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri Gestione attivita', { kind: 'open-app', application: 'task-manager', label: 'Gestione attivita' })}>Gestione attivita</button>
          <button type="button" onClick={() => void executeLocalCommand('Blocca il PC', { kind: 'lock-workstation' })}>Blocca sessione</button>
        </div>
        <div className="terminate-control">
          <label>PID da terminare<input inputMode="numeric" value={terminatePid} onChange={event => setTerminatePid(event.target.value.replace(/\D/g, '').slice(0, 10))} placeholder="es. 1234" /></label>
          <button type="button" className="danger" disabled={!Number(terminatePid) || toolRunning} onClick={() => void executeLocalCommand(`Termina processo ${terminatePid}`, { kind: 'terminate-process', pid: Number(terminatePid) })}>Termina con conferma</button>
        </div>
      </details>

      <RemoteAccessPanel />

      <details className="personalization">
        <summary><UserRound /> Profilo personale e adattamento</summary>
        <p className="section-help">Dati cifrati sul PC e inviati soltanto al modello Ollama locale. Non inserire password, token, documenti d’identità o dati bancari.</p>
        <div className="profile-grid">
          <label>Come vuoi essere chiamato<input value={personalProfile.displayName} onChange={event => setPersonalProfile(current => ({ ...current, displayName: event.target.value }))} /></label>
          <label>Stile di comunicazione<input value={personalProfile.communicationStyle} onChange={event => setPersonalProfile(current => ({ ...current, communicationStyle: event.target.value }))} placeholder="es. diretto, tecnico, in italiano" /></label>
          <label>Obiettivi<textarea value={personalProfile.goals} onChange={event => setPersonalProfile(current => ({ ...current, goals: event.target.value }))} /></label>
          <label>Preferenze<textarea value={personalProfile.preferences} onChange={event => setPersonalProfile(current => ({ ...current, preferences: event.target.value }))} /></label>
          <label>Vincoli e cose da evitare<textarea value={personalProfile.constraints} onChange={event => setPersonalProfile(current => ({ ...current, constraints: event.target.value }))} /></label>
          <label>Contesto del progetto corrente<textarea value={personalProfile.projectContext} onChange={event => setPersonalProfile(current => ({ ...current, projectContext: event.target.value }))} placeholder="nome, tecnologie, convenzioni, obiettivo" /></label>
        </div>
        <div className="settings-actions">
          <button type="button" onClick={() => void persistProfile()}>Salva cifrato</button>
          <button type="button" className="danger" onClick={() => void removeProfile()}>Cancella profilo e lezioni</button>
        </div>
        {profileNotice && <div className="voice-notice">{profileNotice}</div>}
      </details>

      <details className="personalization">
        <summary><FlaskConical /> Evolution Lab</summary>
        <p className="section-help">{sandbox?.reason ?? 'Verifica sandbox…'} L’accesso completo è concesso solo dentro un runtime isolato verificato; la promozione sul progetto reale resta manuale.</p>
        <div className="evolution-config">
          <label>Repository Git<input value={evolutionProjectPath} onChange={event => setEvolutionProjectPath(event.target.value)} placeholder="C:\\percorso\\progetto" /></label>
          <label>Profilo test<select value={evolutionTestProfile} onChange={event => setEvolutionTestProfile(event.target.value as 'Static' | 'Node' | 'Rust')}><option>Node</option><option>Rust</option><option>Static</option></select></label>
        </div>
        <textarea value={evolutionObjective} onChange={event => setEvolutionObjective(event.target.value)} placeholder="Descrivi la funzione o il miglioramento da simulare…" rows={3} />
        <div className="settings-actions">
          <button type="button" onClick={() => void createEvolutionProposal()} disabled={!evolutionObjective.trim() || evolutionBusy}>Registra proposta isolata</button>
          <button type="button" onClick={() => void Promise.all([getSandboxStatus().then(setSandbox), listEvolutionProposals().then(setEvolutionReports)])}>Aggiorna stato sandbox</button>
        </div>
        {evolutionNotice && <div className="voice-notice">{evolutionNotice}</div>}
        {evolutionReports.length > 0 && <div className="evolution-history">
          <strong>Proposte cifrate verificabili</strong>
          {evolutionReports.map(report => <article key={report.id}>
            <span>{report.projectName} · {report.createdAt ? new Date(Number(report.createdAt)).toLocaleString('it-IT') : 'data legacy'}</span>
            <p>{report.objective}</p>
            <small>{report.state} · esecuzione {report.executionAllowed ? 'abilitata' : 'bloccata'} · promozione manuale</small>
            <div className="settings-actions evolution-actions">
              <button type="button" disabled={evolutionBusy || report.state !== 'proposal-only'} onClick={() => void runEvolutionStage(report, 'generate')}>1. Genera patch</button>
              <button type="button" disabled={evolutionBusy || !sandbox?.executionAllowed || report.state !== 'generated-not-executed'} onClick={() => void runEvolutionStage(report, 'test')}>2. Testa in sandbox</button>
              <button type="button" disabled={evolutionBusy || !sandbox?.executionAllowed || report.state !== 'sandbox-validated' || !report.artifactPath} onClick={() => void runEvolutionStage(report, 'promote')}>3. Promuovi su branch</button>
            </div>
          </article>)}
        </div>}
      </details>

      <details className="personalization security-center">
        <summary><ShieldCheck /> Security Center AURA</summary>
        <p className="section-help">Controlli difensivi in sola lettura; scansione e aggiornamento Defender richiedono conferma nativa. Punteggio postura: <strong>{securityCenter?.postureScore ?? '…'}/100</strong>.</p>
        {securityCenter && <div className="evolution-history">
          {securityCenter.checks.map(check => <article key={check.id}><span>{check.label}</span><strong className={`security-${check.status}`}>{check.status}</strong><small>{check.detail}</small></article>)}
          <article><span>Firma applicazione</span><strong>{securityCenter.publicSignature ? 'pubblica attendibile' : securityCenter.localSignature ? 'locale verificabile' : 'assente'}</strong><small>SHA-256 {securityCenter.executableSha256}</small></article>
        </div>}
        <div className="settings-actions">
          <button type="button" disabled={labBusy} onClick={() => void runLab(async () => { setSecurityCenter(await getSecurityCenterStatus()); setLabNotice('Postura Windows aggiornata.'); })}>Aggiorna controlli</button>
          <button type="button" disabled={labBusy} onClick={() => void runLab(async () => { await runDefenderQuickScan(); setSecurityCenter(await getSecurityCenterStatus()); setLabNotice('Scansione rapida Defender completata.'); })}>Scansione rapida</button>
          <button type="button" disabled={labBusy} onClick={() => void runLab(async () => { await updateDefenderDefinitions(); setSecurityCenter(await getSecurityCenterStatus()); setLabNotice('Definizioni Defender aggiornate.'); })}>Aggiorna definizioni</button>
        </div>
      </details>

      <details className="personalization">
        <summary><Boxes /> Creazione 3D locale</summary>
        <p className="section-help">{creation3dStatus?.reason ?? 'Verifica TripoSR e Blender…'} Input immagine, output GLB confinato e apertura Blender con conferma.</p>
        <div className="profile-grid">
          <label>Immagine sorgente<input value={creation3dInput} onChange={event => setCreation3dInput(event.target.value)} placeholder="C:\percorso\oggetto.png" /></label>
          <label>Nome job<input value={creation3dJob} onChange={event => setCreation3dJob(event.target.value)} /></label>
        </div>
        <div className="settings-actions">
          <button type="button" disabled={labBusy || !creation3dStatus?.installed || !creation3dInput.trim()} onClick={() => void createMesh()}>Genera GLB</button>
          <button type="button" disabled={labBusy || !creation3dResult} onClick={() => creation3dResult && void open3dInBlender(creation3dResult.outputPath)}>Apri in Blender</button>
        </div>
        {creation3dResult && <small>{creation3dResult.device} · {creation3dResult.format} · {creation3dResult.outputPath}</small>}
      </details>

      <details className="personalization">
        <summary><Globe2 /> Browser autonomo controllato</summary>
        <p className="section-help">HTTPS pubblico, redirect e SSRF bloccati. La finestra è sempre interattiva: puoi prenderne il controllo con mouse e tastiera in qualsiasi momento.</p>
        <label>URL<input value={browserUrl} onChange={event => setBrowserUrl(event.target.value)} /></label>
        <div className="settings-actions">
          <button type="button" disabled={labBusy || !browserUrl.trim()} onClick={() => void inspectPage()}>Leggi pagina</button>
          <button type="button" disabled={labBusy || !browserUrl.trim()} onClick={() => void runLab(async () => { await openControlledBrowser(browserUrl.trim()); setBrowserStatus(await getBrowserStatus()); })}>Apri e prendi controllo</button>
          <button type="button" disabled={labBusy || !browserStatus?.interactive} onClick={() => void runLab(async () => { await navigateControlledBrowser(browserUrl.trim()); })}>Naviga</button>
          <button type="button" disabled={labBusy || !browserStatus?.interactive} onClick={() => void runLab(async () => { await closeControlledBrowser(); setBrowserStatus(await getBrowserStatus()); })}>Chiudi</button>
        </div>
        {browserPage && <article className="message assistant"><span>{browserPage.title}</span><p>{browserPage.text.slice(0, 4000)}</p><small>{browserPage.url}{browserPage.truncated ? ' · contenuto limitato' : ''}</small></article>}
      </details>

      <details className="personalization">
        <summary><Plug /> Connettori API REST</summary>
        <p className="section-help">Connettori generici HTTPS con token bearer cifrato. Host, metodo e chiamata richiedono conferma; credenziali e corpo non entrano nell’audit.</p>
        <div className="profile-grid">
          <label>Nome<input value={apiName} onChange={event => setApiName(event.target.value)} placeholder="La mia API" /></label>
          <label>Base URL<input value={apiBaseUrl} onChange={event => setApiBaseUrl(event.target.value)} placeholder="https://api.example.com/v1/" /></label>
          <label>Token facoltativo<input type="password" value={apiSecret} onChange={event => setApiSecret(event.target.value)} autoComplete="off" /></label>
        </div>
        <button type="button" disabled={labBusy || !apiName.trim() || !apiBaseUrl.trim()} onClick={() => void persistApiConnector()}>Salva cifrato</button>
        {apiConnectors.length > 0 && <div className="evolution-config">
          <label>Connettore<select value={apiSelected} onChange={event => setApiSelected(event.target.value)}>{apiConnectors.map(connector => <option key={connector.id} value={connector.id}>{connector.name} · {connector.baseUrl}</option>)}</select></label>
          <label>Metodo<select value={apiMethod} onChange={event => setApiMethod(event.target.value as typeof apiMethod)}><option>GET</option><option>POST</option><option>PUT</option><option>PATCH</option><option>DELETE</option></select></label>
          <label>Percorso relativo<input value={apiPath} onChange={event => setApiPath(event.target.value)} placeholder="status" /></label>
          <label>Body JSON<textarea value={apiBody} onChange={event => setApiBody(event.target.value)} placeholder='{"campo":"valore"}' /></label>
          <button type="button" disabled={labBusy || !apiSelected} onClick={() => void executeApiCall()}>{apiMethod} con conferma</button>
          <button type="button" className="danger" disabled={labBusy || !apiSelected} onClick={() => void runLab(async () => { await deleteApiConnector(apiSelected); const next = await listApiConnectors(); setApiConnectors(next); setApiSelected(next[0]?.id ?? ''); })}>Elimina</button>
        </div>}
        {apiOutput && <pre className="tool-output">{apiOutput.slice(0, 8000)}</pre>}
      </details>

      <details className="personalization">
        <summary><BookOpen /> RAG documentale locale</summary>
        <p className="section-help">Indicizzazione semantica locale di PDF, DOCX, testo, dati e codice con qwen3-embedding. Percorso, testo ed embeddings sono cifrati; duplicati e file fuori limite vengono rifiutati.</p>
        <div className="evolution-config">
          <label>Percorso documento<input value={ragPath} onChange={event => setRagPath(event.target.value)} placeholder="C:\documenti\manuale.pdf" /></label>
          <button type="button" disabled={labBusy || !ragPath.trim()} onClick={() => void indexRagDocument()}>Importa e indicizza</button>
        </div>
        {ragDocuments.length > 0 && <div className="evolution-history">{ragDocuments.map(document => <article key={document.id}><span>{document.sourceName} · {document.chunkCount} frammenti</span><button type="button" className="danger" disabled={labBusy} onClick={() => void runLab(async () => { await deleteRagDocument(document.id); setRagDocuments(await listRagDocuments()); })}>Elimina</button></article>)}</div>}
        <div className="evolution-config"><label>Domanda<input value={ragQuery} onChange={event => setRagQuery(event.target.value)} /></label><button type="button" disabled={labBusy || !ragQuery.trim()} onClick={() => void queryRag()}>Cerca fonti</button></div>
        {ragHits.map(hit => <article key={`${hit.documentId}-${hit.ordinal}`} className="message assistant"><span>{hit.citation} · {(hit.score * 100).toFixed(1)}%</span><p>{hit.excerpt}</p><button type="button" onClick={() => setDraft(current => `${current}\n\n${hit.citation}\n${hit.excerpt}`.trim())}>Usa nella chat</button></article>)}
      </details>

      {labNotice && <div className="voice-notice">{labNotice}</div>}

      <section className="stats">
        <article><span>Modello</span><strong>{selectedModel || 'Non connesso'}</strong></article>
        <article><span>Profilo</span><strong>{profileName} · {profile.contextTokens / 1024}k</strong></article>
        <article><span>Prestazioni</span><strong>{tokensPerSecond ? `${tokensPerSecond.toFixed(1)} tok/s` : 'In attesa'}</strong></article>
        <article><span>Memoria chat</span><strong><Database /> {storageMode === 'encrypted-windows' ? 'Cifrata' : storageMode === 'browser-fallback' ? 'Locale' : 'RAM'} · {messages.length}/100</strong></article>
        <article><span>Audit</span><strong><ShieldCheck /> {auditStatus ? `${auditStatus.valid ? 'Integro' : 'Alterato'} · ${auditStatus.count}` : 'Anteprima web'}</strong></article>
        <article><span>Controllo PC</span><strong><Cpu /> Tool tipizzati · conferme native</strong></article>
        <article><span>Guardrail</span><strong><ShieldCheck /> Policy Rust · Guardian {guardrail?.installed ? 'integrato' : 'assente'}</strong></article>
      </section>
    </main>
  </div>;
}
