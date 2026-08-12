import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity, Bot, Boxes, CircleStop, Cpu, Database, FileDown, FolderPlus, MessageSquarePlus, Mic, PanelLeftClose, PanelLeftOpen, RefreshCw, Search, Send,
  ShieldCheck, SquareTerminal, ThumbsUp, Trash2, UserRound, Volume2, VolumeX, Wifi, WifiOff
} from 'lucide-react';
import { type ChatMessage, type OllamaModel } from './adapters/ollama';
import { createDefaultAiProvider } from './adapters/aiProvider';
import { chooseModelForPrompt, choosePreferredModel, RESOURCE_PROFILES, type ResourceProfileName } from './core/modelProfiles';
import { evaluate } from './core/policyEngine';
import type { ActionPlan, PolicyResult, Risk } from './core/types';
import {
  createConversationStore, type ConversationSummary, type ProjectSummary, type StorageMode
} from './services/conversationStore';
import { saveConversationPdf } from './services/pdfExport';
import { evaluateAction, recordInteraction, verifyAudit, type AuditStatus } from './adapters/security';
import {
  choosePreferredVoiceInput, describeMicrophoneError, getVoiceStatus, listVoiceInputDevices,
  speakText, stopSpeaking, transcribeVoice, VoiceRecorder, type VoiceDevicePreference, type VoiceInputDevice, type VoiceStatus
} from './adapters/voice';
import {
  analyzeScreen, approveEvolutionPatch, clearProfile, closeControlledBrowser, consolidateMemory, controlActiveWindow, controlMedia, detectEvolutionProjectRoot,
  generate3d, generateEvolutionPatch, getCreation3dStatus, getGuardrailStatus, getMemoryStatus, getPrivacyStatus, getSecurityCenterStatus,
  getSandboxStatus, getSystemSnapshot, listInstalledApplications, listProcesses, loadProfile, lockWorkstation, open3dInBlender,
  openApplication, openControlledBrowser, pauseBrowserAgent, promoteEvolutionPatch, recordEvolutionProposal, recordFeedback,
  openInstalledApplication, rollbackMemory, runEvolutionSandbox, saveProfile, searchInternet, setAutomaticMemory, startBrowserAgent, terminateProcess, listWorkspaceGrants,
  listWorkspaceEntries, readWorkspaceText, writeWorkspaceText, createWorkspaceDirectory, openWorkspaceItem, stopAllTasks, trashWorkspaceItem, type Creation3dResult,
  type Creation3dStatus, type GuardrailStatus, type MemoryStatus, type PcAccessMode, type PersonalProfile, type PrivacyStatus
} from './adapters/agent';
import { isVoiceMutationBlocked, parseLocalCommand, type LocalCommand } from './core/localCommands';
import { RemoteAccessPanel } from './features/remote/RemoteAccessPanel';
import { SecurityCenterPanel } from './features/security/SecurityCenterPanel';
import { PcAccessBanner } from './features/security/PcAccessBanner';
import { KnowledgeToolsPanel } from './features/tools/KnowledgeToolsPanel';
import { EvolutionLabPanel } from './features/evolution/EvolutionLabPanel';
import { AuditLogPanel } from './features/activity/AuditLogPanel';
import { VisionPanel } from './features/vision/VisionPanel';
import { WorkspaceControlPanel } from './features/pc/WorkspaceControlPanel';
import { InstalledAppsPanel } from './features/pc/InstalledAppsPanel';
import { AiProvidersPanel } from './features/tools/AiProvidersPanel';

const client = createDefaultAiProvider();
const conversationStore = createConversationStore();
const VOICE_DEVICE_KEY = 'aura.voice-device.v1';
const WAKE_WORD_KEY = 'aura.wake-word.v1';
const FOLLOW_UP_KEY = 'aura.voice-follow-up.v1';
const VOICE_TEST_PHRASE = 'AURA prova microfono completata';
const EMPTY_PROFILE: PersonalProfile = {
  displayName: '', communicationStyle: '', goals: '', preferences: '', constraints: '', projectContext: ''
};
const demos: { label: string; risk: Risk; capability: string; scope: string }[] = [
  { label: 'Controlla utilizzo del PC', risk: 'safe', capability: 'system.inspect', scope: 'metriche locali' },
  { label: 'Organizza una cartella', risk: 'moderate', capability: 'files.organize', scope: 'cartella selezionata' },
  { label: 'Installa un nuovo modulo', risk: 'critical', capability: 'module.install', scope: 'sandbox temporanea' }
];

type Connection = 'checking' | 'online' | 'offline';
type AppSection = 'assistant' | 'activity' | 'security';
type PromptSource = 'text' | 'voice';
type QueuedPrompt = { id: string; text: string; conversationId: string; createdAt: string; source: PromptSource };

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

function loadBooleanPreference(key: string): boolean {
  try { return window.localStorage.getItem(key) === 'true'; } catch { return false; }
}

function saveBooleanPreference(key: string, enabled: boolean) {
  try { window.localStorage.setItem(key, String(enabled)); } catch { /* preferenza opzionale */ }
}

function phraseMatch(actual: string, expected: string): number {
  const words = (value: string) => value.toLocaleLowerCase('it').normalize('NFKD')
    .replace(/[^a-z0-9\s]/g, ' ').split(/\s+/).filter(Boolean);
  const wanted = words(expected);
  const received = new Set(words(actual));
  return wanted.length ? wanted.filter(word => received.has(word)).length / wanted.length : 0;
}

export function App() {
  const [activeSection, setActiveSection] = useState<AppSection>('assistant');
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [pcAccessMode, setPcAccessModeState] = useState<PcAccessMode>('controlled');
  const [profileName, setProfileName] = useState<ResourceProfileName>('Terra');
  const [selectedAction, setSelectedAction] = useState(0);
  const [connection, setConnection] = useState<Connection>('checking');
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [hydrated, setHydrated] = useState(false);
  const [storageMode, setStorageMode] = useState<StorageMode>(conversationStore.mode);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [chatSearch, setChatSearch] = useState('');
  const [activeProjectId, setActiveProjectId] = useState('default');
  const [activeConversationId, setActiveConversationId] = useState('default');
  const [workspaceNotice, setWorkspaceNotice] = useState('');
  const [auditStatus, setAuditStatus] = useState<AuditStatus>();
  const [draft, setDraft] = useState('');
  const [queuedPrompts, setQueuedPrompts] = useState<QueuedPrompt[]>([]);
  const [queueNotice, setQueueNotice] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState('');
  const [tokensPerSecond, setTokensPerSecond] = useState<number>();
  const [lastUsedModel, setLastUsedModel] = useState('');
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
  const [wakeWordEnabled, setWakeWordEnabled] = useState(() => loadBooleanPreference(WAKE_WORD_KEY));
  const [wakeListening, setWakeListening] = useState(false);
  const [followUpEnabled, setFollowUpEnabled] = useState(() => loadBooleanPreference(FOLLOW_UP_KEY));
  const [speaking, setSpeaking] = useState(false);
  const [toolRunning, setToolRunning] = useState(false);
  const [terminatePid, setTerminatePid] = useState('');
  const [personalProfile, setPersonalProfile] = useState<PersonalProfile>(EMPTY_PROFILE);
  const [profileNotice, setProfileNotice] = useState('');
  const [memoryStatus, setMemoryStatus] = useState<MemoryStatus>();
  const [guardrail, setGuardrail] = useState<GuardrailStatus>();
  const [creation3dStatus, setCreation3dStatus] = useState<Creation3dStatus>();
  const [creation3dInput, setCreation3dInput] = useState('');
  const [creation3dJob, setCreation3dJob] = useState('aura-mesh');
  const [creation3dResult, setCreation3dResult] = useState<Creation3dResult>();
  const [labBusy, setLabBusy] = useState(false);
  const [labNotice, setLabNotice] = useState('');
  const [privacyStatus, setPrivacyStatus] = useState<PrivacyStatus>();
  const [emergencyStopping, setEmergencyStopping] = useState(false);
  const [emergencyNotice, setEmergencyNotice] = useState('');
  const abortRef = useRef<AbortController | undefined>(undefined);
  const pendingQueuedRef = useRef<QueuedPrompt | undefined>(undefined);
  const queueDispatchingRef = useRef(false);
  const streamBufferRef = useRef('');
  const fullResponseRef = useRef('');
  const streamFlushRef = useRef<number | undefined>(undefined);
  const recorderRef = useRef<VoiceRecorder | undefined>(undefined);
  const wakeRecorderRef = useRef<VoiceRecorder | undefined>(undefined);
  const voiceTimeoutRef = useRef<number | undefined>(undefined);
  const ttsTimeoutRef = useRef<number | undefined>(undefined);
  const activeVoiceLabelRef = useRef('');
  const voiceDiagnosticRef = useRef(false);
  const draftSourceRef = useRef<PromptSource>('text');
  const chatRef = useRef<HTMLElement | null>(null);
  const composerRef = useRef<HTMLFormElement | null>(null);
  const chatPinnedRef = useRef(true);
  const profile = RESOURCE_PROFILES[profileName];
  const activeProject = projects.find(project => project.id === activeProjectId);
  const activeConversation = conversations.find(conversation => conversation.id === activeConversationId);
  const activeQueuedPrompts = queuedPrompts.filter(item => item.conversationId === activeConversationId);
  const filteredConversations = useMemo(() => {
    const normalize = (value: string) => value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase('it-IT');
    const query = normalize(chatSearch.trim());
    return query ? conversations.filter(conversation => normalize(conversation.title).includes(query)) : conversations;
  }, [chatSearch, conversations]);

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
        : choosePreferredModel(availableModels.map(model => model.name), profileName));
      setConnection('online');
    } catch {
      setConnection('offline');
    } finally {
      window.clearTimeout(timeout);
    }
  }

  useEffect(() => { void refreshOllama(); }, []);
  useEffect(() => {
    const keyboardStop = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.altKey && event.key.toLocaleLowerCase() === 's') {
        event.preventDefault();
        void emergencyStop();
      }
    };
    window.addEventListener('keydown', keyboardStop, true);
    return () => window.removeEventListener('keydown', keyboardStop, true);
  });
  useEffect(() => {
    if (!models.length) return;
    setSelectedModel(choosePreferredModel(models.map(model => model.name), profileName));
  }, [profileName, models]);
  useEffect(() => { void getVoiceStatus().then(setVoiceStatus).catch(() => undefined); }, []);
  useEffect(() => {
    void loadProfile().then(profile => { if (profile) setPersonalProfile(profile); }).catch(() => undefined);
    void getGuardrailStatus().then(setGuardrail).catch(() => undefined);
    void getCreation3dStatus().then(setCreation3dStatus).catch(() => undefined);
    void getPrivacyStatus().then(setPrivacyStatus).catch(() => undefined);
    void getMemoryStatus().then(status => {
      setMemoryStatus(status);
      if (status.automatic) void consolidateMemory().then(setMemoryStatus).catch(() => undefined);
    }).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!memoryStatus?.automatic) return;
    const timer = window.setInterval(() => {
      void consolidateMemory().then(setMemoryStatus).catch(() => undefined);
    }, 30 * 60 * 1000);
    return () => window.clearInterval(timer);
  }, [memoryStatus?.automatic]);
  useEffect(() => {
    if (!voiceStatus?.available || !navigator.mediaDevices) return;
    void refreshVoiceDevices(false);
    const handleDeviceChange = () => { void refreshVoiceDevices(false); };
    navigator.mediaDevices.addEventListener?.('devicechange', handleDeviceChange);
    return () => navigator.mediaDevices.removeEventListener?.('devicechange', handleDeviceChange);
  }, [voiceStatus?.available]);
  useEffect(() => {
    saveBooleanPreference(WAKE_WORD_KEY, wakeWordEnabled);
    if (!wakeWordEnabled || !voiceStatus?.available || voiceRecording || voiceProcessing || streaming || toolRunning || speaking) {
      setWakeListening(false);
      void wakeRecorderRef.current?.cancel();
      wakeRecorderRef.current = undefined;
      return;
    }
    let cancelled = false;
    let retryTimer: number | undefined;
    const listen = async () => {
      if (cancelled) return;
      const recorder = new VoiceRecorder();
      wakeRecorderRef.current = recorder;
      try {
        await recorder.start(selectedVoiceDeviceId || undefined, () => { cancelled = true; });
        setWakeListening(true);
        await new Promise<void>(resolve => { retryTimer = window.setTimeout(resolve, 5_000); });
        if (cancelled) return;
        const recording = await recorder.stop();
        wakeRecorderRef.current = undefined;
        setWakeListening(false);
        let transcript;
        try { transcript = await transcribeVoice(recording.wav); }
        finally { recording.wav.fill(0); }
        const match = /(?:^|\s)aura(?:\s|[,:;.!?]|$)/i.exec(transcript.text);
        if (match) {
          const command = transcript.text.slice((match.index ?? 0) + match[0].length).trim();
          const confidence = Math.round(transcript.confidence * 100);
          await recordInteraction({
            source: 'voice', action: 'wake-command', content: command || transcript.text,
            outcome: confidence >= 70 && command ? 'allow' : 'caution',
            metadata: { confidence, wakeWord: true, automaticallySubmitted: Boolean(command && confidence >= 70) }
          }).catch(() => false);
          draftSourceRef.current = 'voice';
          setDraft(command);
          setVoiceNotice(command
            ? `Wake word AURA rilevata (${confidence}%). ${confidence >= 70 ? 'Comando inviato localmente.' : 'Confidenza bassa: comando lasciato in bozza.'}`
            : 'Wake word AURA rilevata: pronuncia anche il comando nella stessa frase.');
          if (command && confidence >= 70) window.setTimeout(() => composerRef.current?.requestSubmit(), 0);
        }
      } catch (cause) {
        if (!cancelled) setVoiceNotice(`Ascolto AURA sospeso: ${describeMicrophoneError(cause)}`);
      } finally {
        if (wakeRecorderRef.current === recorder) wakeRecorderRef.current = undefined;
        setWakeListening(false);
        if (!cancelled) retryTimer = window.setTimeout(() => { void listen(); }, 350);
      }
    };
    void listen();
    return () => {
      cancelled = true;
      if (retryTimer) window.clearTimeout(retryTimer);
      void wakeRecorderRef.current?.cancel();
      wakeRecorderRef.current = undefined;
      setWakeListening(false);
    };
  }, [wakeWordEnabled, voiceStatus?.available, selectedVoiceDeviceId, voiceRecording, voiceProcessing, streaming, toolRunning, speaking]);
  useEffect(() => { saveBooleanPreference(FOLLOW_UP_KEY, followUpEnabled); }, [followUpEnabled]);
  useEffect(() => () => {
    abortRef.current?.abort();
    if (voiceTimeoutRef.current) window.clearTimeout(voiceTimeoutRef.current);
    if (ttsTimeoutRef.current) window.clearTimeout(ttsTimeoutRef.current);
    void recorderRef.current?.cancel();
    void wakeRecorderRef.current?.cancel();
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
    void (async () => {
      let availableProjects = await conversationStore.listProjects();
      if (!availableProjects.length) availableProjects = [await conversationStore.createProject('Progetto principale')];
      const projectId = availableProjects[0].id;
      let availableConversations = await conversationStore.listConversations(projectId);
      if (!availableConversations.length) {
        availableConversations = [await conversationStore.createConversation(projectId, 'Conversazione principale')];
      }
      const conversationId = availableConversations[0].id;
      const stored = await conversationStore.load(conversationId);
      if (!active) return;
      setProjects(availableProjects);
      setActiveProjectId(projectId);
      setConversations(availableConversations);
      setActiveConversationId(conversationId);
      setMessages(stored);
    })()
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
    const sessionId = activeConversationId;
    const timer = window.setTimeout(() => {
      void conversationStore.save(sessionId, messages).catch(() => {
        setStorageMode('memory');
        setError('Non riesco a salvare la conversazione. Nessun dato è stato inviato fuori dal PC.');
      });
    }, 350);
    return () => window.clearTimeout(timer);
  }, [hydrated, messages, activeConversationId]);

  useEffect(() => {
    if (streaming || toolRunning || queueDispatchingRef.current) return;
    const next = queuedPrompts.find(item => item.conversationId === activeConversationId);
    if (!next) return;
    if (!parseLocalCommand(next.text) && (connection !== 'online' || !selectedModel)) return;
    queueDispatchingRef.current = true;
    pendingQueuedRef.current = next;
    setQueuedPrompts(current => current.filter(item => item.id !== next.id));
    window.setTimeout(() => composerRef.current?.requestSubmit(), 0);
  }, [queuedPrompts, activeConversationId, streaming, toolRunning, connection, selectedModel]);

  function enqueuePrompt(text: string, prioritize = false, source: PromptSource = 'text'): boolean {
    const normalized = text.trim();
    if (!normalized || normalized.length > 32_000) {
      setError('La domanda da accodare deve contenere da 1 a 32.000 caratteri.');
      return false;
    }
    const conversationCount = queuedPrompts.filter(item => item.conversationId === activeConversationId).length;
    if (conversationCount >= 20) {
      setError('La coda di questa chat ha raggiunto il limite di 20 domande.');
      return false;
    }
    const item: QueuedPrompt = {
      id: crypto.randomUUID(), text: normalized, conversationId: activeConversationId, createdAt: new Date().toISOString(), source
    };
    setQueuedPrompts(current => prioritize ? [item, ...current] : [...current, item]);
    setQueueNotice(prioritize ? 'Domanda inserita in testa alla coda.' : 'Domanda aggiunta alla coda.');
    return true;
  }

  function updateQueuedPrompt(id: string, text: string) {
    setQueuedPrompts(current => current.map(item => item.id === id ? { ...item, text: text.slice(0, 32_000) } : item));
  }

  function deleteQueuedPrompt(id: string) {
    setQueuedPrompts(current => current.filter(item => item.id !== id));
    setQueueNotice('Domanda rimossa dalla coda.');
  }

  function sendQueuedPromptNow(id: string) {
    setQueuedPrompts(current => {
      const selected = current.find(item => item.id === id);
      return selected ? [selected, ...current.filter(item => item.id !== id)] : current;
    });
    setQueueNotice(streaming
      ? 'Interruzione richiesta: la domanda selezionata sarà inviata appena AURA arresta la risposta corrente.'
      : toolRunning ? 'Domanda prioritaria: sarà inviata appena termina lo strumento corrente.' : 'Domanda prioritaria pronta per l’invio.');
    if (streaming) abortRef.current?.abort();
  }

  async function sendMessage(event: FormEvent) {
    event.preventDefault();
    const pendingQueued = pendingQueuedRef.current;
    pendingQueuedRef.current = undefined;
    queueDispatchingRef.current = false;
    const prompt = (pendingQueued?.text ?? draft).trim();
    const source = pendingQueued?.source ?? draftSourceRef.current;
    if (!prompt) return;
    if (source === 'voice') {
      await recordInteraction({
        source: 'voice', action: 'chat-submit', content: prompt, outcome: 'allow',
        correlationId: activeConversationId,
        metadata: { conversationId: activeConversationId, queued: Boolean(pendingQueued) }
      }).catch(() => false);
    }
    if (pendingQueued) setQueueNotice('Domanda prelevata dalla coda e inviata ad AURA.');
    if (streaming || toolRunning) {
      if (pendingQueued) setQueuedPrompts(current => [pendingQueued, ...current]);
      else if (enqueuePrompt(prompt, false, source)) { setDraft(''); draftSourceRef.current = 'text'; }
      return;
    }
    if (!messages.length && activeConversation?.title === 'Nuova chat') {
      const title = prompt.replace(/\s+/g, ' ').slice(0, 60);
      void conversationStore.renameConversation(activeConversationId, title)
        .then(() => setConversations(current => current.map(chat =>
          chat.id === activeConversationId ? { ...chat, title, updatedAt: new Date().toISOString() } : chat
        )))
        .catch(() => undefined);
    }
    const localCommand = parseLocalCommand(prompt);
    if (localCommand) {
      if (source === 'voice' && isVoiceMutationBlocked(localCommand)) {
        if (!pendingQueued) setDraft('');
        draftSourceRef.current = 'text';
        const notice = 'Comando vocale bloccato: per ora la voce può leggere lo schermo e controllare applicazioni o finestre, ma non creare, cancellare, installare o modificare file e codice.';
        setVoiceNotice(notice);
        setMessages(current => [...current, newMessage('user', prompt), newMessage('assistant', notice)]);
        return;
      }
      if (!pendingQueued) setDraft('');
      draftSourceRef.current = 'text';
      await executeLocalCommand(prompt, localCommand);
      return;
    }
    if (connection !== 'online' || !selectedModel) {
      if (pendingQueued) setQueuedPrompts(current => [pendingQueued, ...current]);
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
    if (!pendingQueued) setDraft('');
    draftSourceRef.current = 'text';
    setError('');
    setStreaming(true);
    setTokensPerSecond(undefined);
    const controller = new AbortController();
    abortRef.current = controller;
    streamBufferRef.current = '';
    fullResponseRef.current = '';
    // Sole non ha una deadline complessiva: resta sempre interrompibile con Stop.
    const generationTimeout = profileName === 'Sole'
      ? undefined
      : window.setTimeout(() => controller.abort(), 180_000);

    try {
      const requestModel = chooseModelForPrompt(prompt, selectedModel, models.map(model => model.name));
      setLastUsedModel(requestModel);
      const stream = await client.chat(
        requestModel,
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
      if (generationTimeout) window.clearTimeout(generationTimeout);
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
      if (command.kind === 'help') {
        complete(`Posso aiutarti con:
• chat e ragionamento usando modelli Ollama locali;
• analisi critica permanente: verifico le premesse e non ti do automaticamente ragione;
• comandi vocali locali, trascrizione Whisper e lettura delle risposte;
• lettura dello schermo con il modello visivo locale, senza salvare screenshot;
• stato del PC, processi, applicazioni installate, workspace autorizzati, file, audio e controlli multimediali;
• Security Center, Defender, firewall, privacy e audit cifrato;
• ricerca Internet in tempo reale con fonti e browser WebView2 controllabile;
• documenti locali tramite RAG e connettori API HTTPS;
• generazione 3D locale con TripoSR e apertura in Blender;
• proposte di miglioramento nell’Evolution Lab, eseguibili solo quando la sandbox forte è disponibile;
• collegamento Alexa tramite il relay AURA configurabile.

Puoi iniziare con “stato del PC”, “vai su https://github.com/…”, “crea un modello 3D da C:\\immagini\\foto.png”, “Evolution Lab: aggiungi un test”, una domanda normale oppure il pulsante del microfono. Le azioni sensibili richiedono conferma e non eseguo comandi shell arbitrari.`);
      } else if (command.kind === 'identity') {
        complete('Sono AURA, il tuo assistente Windows local-first. Uso modelli Ollama sul PC, mantengo chat e profilo cifrati localmente e passo attraverso policy e conferme prima delle azioni sensibili.');
      } else if (command.kind === 'coding-capabilities') {
        complete(`Sì. Posso generare, spiegare, correggere e revisionare codice in linguaggi come Python, TypeScript/JavaScript, Rust, C/C++, C#, Java, PowerShell, SQL, HTML/CSS e altri linguaggi supportati dal modello locale.

Posso anche progettare file, patch, test e comandi di build. L’esecuzione reale non è illimitata: deve avvenire tramite strumenti tipizzati o nell’Evolution Lab con sandbox forte disponibile. Se la sandbox non è installata posso comunque generare la soluzione, ma devo dichiarare che non è stata eseguita. Malware, furto di credenziali, persistenza occulta e operazioni distruttive restano bloccati.`);
      } else if (command.kind === 'performance-diagnosis') {
        const [snapshot, processes] = await Promise.all([getSystemSnapshot(), listProcesses()]);
        const memoryPercent = snapshot.memoryTotalBytes > 0 ? snapshot.memoryUsedBytes / snapshot.memoryTotalBytes * 100 : 0;
        const top = processes.slice(0, 5).map(process =>
          `• ${process.name} · PID ${process.pid} · RAM ${formatBytes(process.memoryBytes)} · CPU ${process.cpuUsagePercent.toFixed(1)}%`
        ).join('\n');
        const observations = [
          snapshot.cpuUsagePercent >= 85 ? 'CPU sotto carico elevato.' : `CPU non satura al momento (${snapshot.cpuUsagePercent.toFixed(1)}%).`,
          memoryPercent >= 85 ? `RAM quasi piena (${memoryPercent.toFixed(1)}%).` : `RAM con margine disponibile (${memoryPercent.toFixed(1)}% usata).`,
          snapshot.disks.some(disk => disk.totalBytes > 0 && disk.availableBytes / disk.totalBytes < 0.1) ? 'Almeno un disco ha meno del 10% libero.' : 'Nessun disco rilevato sotto il 10% libero.'
        ];
        complete(`Diagnosi prestazioni eseguita in sola lettura.\n${observations.join('\n')}\n\nProcessi principali:\n${top || 'nessun dato'}\n\nNon ho chiuso processi né modificato impostazioni. Posso mostrarti i processi o aprire Gestione attività per un intervento manuale.`);
      } else if (command.kind === 'security-diagnosis') {
        const status = await getSecurityCenterStatus();
        const warnings = status.checks.filter(check => check.status !== 'healthy');
        complete(`Diagnosi sicurezza eseguita in sola lettura.\nPostura: ${status.postureScore}/100 · copertura: ${status.coverageScore}%.\n${warnings.length ? `Controlli da verificare:\n${warnings.map(check => `• ${check.label}: ${check.detail}`).join('\n')}` : 'Tutti i controlli disponibili risultano sani.'}\n${status.recommendations.length ? `\nRaccomandazioni:\n${status.recommendations.map(item => `• ${item}`).join('\n')}` : ''}`);
      } else if (command.kind === 'system-status') {
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
      } else if (command.kind === 'open-installed-app') {
        const applications = await listInstalledApplications();
        const normalizedQuery = command.query.toLocaleLowerCase('it');
        const candidates = applications.filter(application => application.label.toLocaleLowerCase('it').includes(normalizedQuery));
        const selected = candidates.find(application => application.label.toLocaleLowerCase('it') === normalizedQuery) ?? candidates[0];
        if (!selected) {
          complete(`${command.label} non risulta nel catalogo del Menu Start. Posso comunque usare il browser isolato AURA per navigare senza dipendere da Chrome.`);
        } else {
          const opened = await openInstalledApplication(selected.id);
          complete(opened ? `${selected.label} avviato dal collegamento Windows verificato.` : `Apertura di ${selected.label} annullata.`);
        }
      } else if (command.kind === 'screen-read') {
        const result = await analyzeScreen(command.prompt);
        complete(`${result.description}\n\nAnalisi eseguita localmente con ${result.model} su ${result.width}×${result.height}. Lo screenshot è rimasto in memoria e non è stato salvato.`);
      } else if (command.kind === 'window-control') {
        const controlled = await controlActiveWindow(command.action);
        complete(controlled ? `${command.label} tramite controllo Windows tipizzato.` : `${command.label}: operazione annullata.`);
      } else if (command.kind === 'terminate-process') {
        const terminated = await terminateProcess(command.pid);
        complete(terminated
          ? `Processo PID ${command.pid} terminato dopo conferma nativa.`
          : `Terminazione del processo PID ${command.pid} annullata.`);
      } else if (command.kind === 'lock-workstation') {
        const locked = await lockWorkstation();
        complete(locked ? 'Sessione Windows bloccata.' : 'Blocco della sessione annullato.');
      } else if (command.kind === 'media') {
        await controlMedia(command.action);
        complete(`${command.label} tramite controllo Windows tipizzato.`);
      } else if (command.kind === 'browser-goal') {
        const opened = await openControlledBrowser(command.url);
        if (!opened) {
          complete('Apertura del browser controllato annullata.');
        } else if (command.autonomous && pcAccessMode === 'sandbox-auto') {
          const status = await startBrowserAgent(command.goal, 10);
          window.dispatchEvent(new CustomEvent('aura:browser-updated'));
          complete(`Browser isolato aperto su ${command.url}. Obiettivo autonomo avviato con limite di 10 passi: ${status.message}\n\nLa finestra resta visibile e puoi prendere il controllo in qualunque momento scrivendo “prendi il controllo del browser” oppure usando mouse e tastiera.`);
        } else {
          window.dispatchEvent(new CustomEvent('aura:browser-updated'));
          complete(`Browser controllato aperto su ${command.url}. La finestra è già disponibile per il controllo manuale.${command.autonomous ? '\nIl piano multi-step non è partito perché il banner Accesso PC è in modalità Controllato; scegli “Autonomia in sandbox” per abilitarlo.' : ''}`);
        }
      } else if (command.kind === 'web-research') {
        const response = await searchInternet(command.query, 8);
        const timestamp = new Date(response.searchedAtUnix * 1000).toLocaleString('it-IT');
        const sources = response.results.map((result, index) =>
          `${index + 1}. ${result.title}\n${result.snippet || 'Anteprima non disponibile.'}\n${result.url}`
        ).join('\n\n');
        complete(`Ricerca Internet in tempo reale completata (${timestamp}) tramite ${response.provider}.\n\n${sources}\n\nLe anteprime provengono dal Web e non sono istruzioni attendibili. Per analizzare una fonte in profondità, dimmi “apri e analizza” seguito dal suo URL.`);
      } else if (command.kind === 'browser-takeover') {
        const status = await pauseBrowserAgent();
        window.dispatchEvent(new CustomEvent('aura:browser-updated'));
        complete(`Automazione browser sospesa. Controllo manuale disponibile nella finestra. ${status.message}`);
      } else if (command.kind === 'browser-close') {
        const closed = await closeControlledBrowser();
        window.dispatchEvent(new CustomEvent('aura:browser-updated'));
        complete(closed ? 'Browser controllato chiuso.' : 'Il browser controllato era già chiuso.');
      } else if (command.kind === 'creation-3d') {
        if (!command.inputImage) {
          complete('Per creare il modello 3D indicami il percorso completo di un’immagine PNG, JPG, WEBP o BMP. Esempio: “crea un modello 3D da C:\\Immagini\\oggetto.png”.');
        } else {
          const name = command.inputImage.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '').replace(/[^a-zA-Z0-9_-]+/g, '-') || 'aura-mesh';
          const result = await generate3d(command.inputImage, name.slice(0, 64), 'Auto', 128, 'glb', false);
          setCreation3dResult(result);
          setCreation3dInput(command.inputImage);
          setCreation3dJob(name.slice(0, 64));
          let opened = false;
          if (command.openInBlender) opened = await open3dInBlender(result.outputPath);
          window.dispatchEvent(new CustomEvent('aura:creation3d-updated'));
          complete(`Modello 3D generato localmente: ${result.outputPath}\nDispositivo: ${result.device} · formato: ${result.format}.${command.openInBlender ? (opened ? '\nAperto in Blender.' : '\nApertura in Blender annullata.') : ''}`);
        }
      } else if (command.kind === 'evolution-run') {
        const sandbox = await getSandboxStatus();
        if (!sandbox.executionAllowed) throw new Error(`Evolution Lab non operativo: ${sandbox.reason}`);
        const grants = await listWorkspaceGrants();
        const detected = await detectEvolutionProjectRoot();
        const projectPath = command.projectPath ?? grants[0]?.path ?? detected;
        if (!projectPath) throw new Error('Repository non individuato. Autorizza prima la cartella in “Workspace, file e workflow” oppure specifica: repo "C:\\percorso\\progetto".');
        const projectLabel = activeProject?.name || projectPath.split(/[\\/]/).pop() || 'Progetto AURA';
        const proposal = await recordEvolutionProposal(projectLabel, command.objective);
        const generated = await generateEvolutionPatch(proposal.id, projectPath);
        if (pcAccessMode !== 'sandbox-auto') {
          window.dispatchEvent(new CustomEvent('aura:evolution-updated'));
          complete(`Evolution ha generato e ispezionato la patch: ${generated.artifactPath}.\nIl ciclo automatico si è fermato perché Accesso PC è in modalità Controllato. Esamina il diff nel pannello Evolution oppure abilita “Autonomia in sandbox” dal banner.`);
          return;
        }
        const approved = await approveEvolutionPatch(proposal.id);
        if (approved.status !== 'diff-approved') throw new Error('Diff non approvato; ciclo Evolution arrestato.');
        const tested = await runEvolutionSandbox(proposal.id, projectPath, 'Static');
        if (tested.status !== 'canary-validated') throw new Error(`Test Evolution non completato: ${tested.status}`);
        const promoted = await promoteEvolutionPatch(proposal.id, projectPath, tested.artifactPath);
        window.dispatchEvent(new CustomEvent('aura:evolution-updated'));
        complete(`Evolution Lab completato attraverso i gate previsti.\nRepository: ${projectPath}\nStato: ${promoted.status}\nPatch: ${promoted.artifactPath}\n\nLa modifica è in un branch Git di revisione senza commit automatico.`);
      } else if (command.kind === 'workspace-list' || command.kind === 'workspace-read'
        || command.kind === 'workspace-write' || command.kind === 'workspace-mkdir'
        || command.kind === 'workspace-open' || command.kind === 'workspace-trash') {
        const grants = await listWorkspaceGrants();
        const grant = grants[0];
        if (!grant) throw new Error('Nessun workspace autorizzato. Apri “Workspace, file e workflow” e autorizza una cartella di progetto.');
        if (command.kind === 'workspace-list') {
          const entries = await listWorkspaceEntries(grant.id, command.relativePath);
          complete(`Workspace ${grant.name} · /${command.relativePath}\n${entries.slice(0, 50).map(entry => `${entry.isDirectory ? '📁' : '📄'} ${entry.relativePath}${entry.isDirectory ? '' : ` · ${formatBytes(entry.sizeBytes)}`}`).join('\n') || 'Cartella vuota.'}`);
        } else if (command.kind === 'workspace-read') {
          const file = await readWorkspaceText(grant.id, command.relativePath);
          complete(`File ${file.relativePath} · ${formatBytes(file.sizeBytes)}\n\n${file.content}`);
        } else if (command.kind === 'workspace-write') {
          const written = await writeWorkspaceText(grant.id, command.relativePath, command.content);
          complete(written ? `File ${command.relativePath} creato dopo conferma nativa.` : 'Creazione file annullata.');
        } else if (command.kind === 'workspace-mkdir') {
          const created = await createWorkspaceDirectory(grant.id, command.relativePath);
          complete(created ? `Cartella ${command.relativePath} creata dopo conferma nativa.` : 'Creazione cartella annullata.');
        } else if (command.kind === 'workspace-open') {
          const opened = await openWorkspaceItem(grant.id, command.relativePath);
          complete(opened ? `Elemento ${command.relativePath} aperto con Windows.` : 'Apertura annullata.');
        } else {
          const removed = await trashWorkspaceItem(grant.id, command.relativePath);
          complete(removed ? `Elemento ${command.relativePath} spostato nel Cestino di Windows.` : 'Operazione annullata.');
        }
      } else {
        const status = await getSandboxStatus();
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
      setMemoryStatus(await getMemoryStatus());
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
      if (correction && memoryStatus?.automatic) setMemoryStatus(await consolidateMemory());
      setProfileNotice(correction
        ? 'Correzione cifrata registrata e disponibile per le prossime risposte.'
        : 'Feedback positivo registrato localmente.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
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
      ttsTimeoutRef.current = window.setTimeout(() => {
        setSpeaking(false);
        if (followUpEnabled && !wakeWordEnabled) void startVoiceRecording(false);
      }, estimatedMilliseconds + 750);
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

  async function openConversation(sessionId: string, projectId = activeProjectId) {
    if (streaming || toolRunning || sessionId === activeConversationId) return;
    setHydrated(false);
    setWorkspaceNotice('');
    try {
      await conversationStore.save(activeConversationId, messages);
      const stored = await conversationStore.load(sessionId);
      setActiveProjectId(projectId);
      setActiveConversationId(sessionId);
      setMessages(stored);
      setError('');
      chatPinnedRef.current = true;
    } catch {
      setError('Impossibile aprire la conversazione selezionata.');
    } finally {
      setHydrated(true);
    }
  }

  async function emergencyStop() {
    if (emergencyStopping) return;
    setEmergencyStopping(true);
    setEmergencyNotice('Arresto globale in corso…');
    abortRef.current?.abort();
    abortRef.current = undefined;
    if (voiceTimeoutRef.current) window.clearTimeout(voiceTimeoutRef.current);
    if (ttsTimeoutRef.current) window.clearTimeout(ttsTimeoutRef.current);
    voiceTimeoutRef.current = undefined;
    ttsTimeoutRef.current = undefined;
    await recorderRef.current?.cancel().catch(() => undefined);
    await wakeRecorderRef.current?.cancel().catch(() => undefined);
    recorderRef.current = undefined;
    wakeRecorderRef.current = undefined;
    window.dispatchEvent(new CustomEvent('aura:stop-all'));
    setStreaming(false);
    setToolRunning(false);
    setLabBusy(false);
    setVoiceRecording(false);
    setVoiceProcessing(false);
    setWakeListening(false);
    setSpeaking(false);
    try {
      const result = await stopAllTasks();
      setEmergencyNotice(`${result.message} Generazioni: ${result.generationsCancelled}; processi: ${result.childProcessesSignalled}.`);
    } catch (cause) {
      setEmergencyNotice(`Interfaccia sbloccata; arresto backend da verificare: ${cause instanceof Error ? cause.message : String(cause)}`);
    } finally {
      setEmergencyStopping(false);
      window.setTimeout(() => setEmergencyNotice(''), 8_000);
    }
  }

  async function openProject(projectId: string) {
    if (streaming || toolRunning || projectId === activeProjectId) return;
    setHydrated(false);
    setWorkspaceNotice('');
    try {
      await conversationStore.save(activeConversationId, messages);
      let available = await conversationStore.listConversations(projectId);
      if (!available.length) available = [await conversationStore.createConversation(projectId, 'Nuova chat')];
      const sessionId = available[0].id;
      const stored = await conversationStore.load(sessionId);
      setActiveProjectId(projectId);
      setConversations(available);
      setChatSearch('');
      setActiveConversationId(sessionId);
      setMessages(stored);
      setError('');
    } catch {
      setError('Impossibile aprire il progetto selezionato.');
    } finally {
      setHydrated(true);
    }
  }

  async function createProject() {
    const requested = window.prompt('Nome del nuovo progetto:')?.trim();
    if (!requested) return;
    try {
      await conversationStore.save(activeConversationId, messages);
      const project = await conversationStore.createProject(requested.slice(0, 80));
      const conversation = await conversationStore.createConversation(project.id, 'Nuova chat');
      setProjects(current => [project, ...current]);
      setConversations([conversation]);
      setChatSearch('');
      setActiveProjectId(project.id);
      setActiveConversationId(conversation.id);
      setMessages([]);
      setWorkspaceNotice(`Progetto “${project.name}” creato.`);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function createChat() {
    if (!activeProjectId) return;
    try {
      await conversationStore.save(activeConversationId, messages);
      const conversation = await conversationStore.createConversation(activeProjectId, 'Nuova chat');
      setConversations(current => [conversation, ...current]);
      setChatSearch('');
      setActiveConversationId(conversation.id);
      setMessages([]);
      setWorkspaceNotice('Nuova chat creata. Il titolo verrà ricavato dalla prima richiesta.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function deleteConversation(sessionId: string) {
    if (streaming || toolRunning) return;
    const deletingActive = sessionId === activeConversationId;
    if (deletingActive) setHydrated(false);
    setWorkspaceNotice('');
    try {
      const deleted = await conversationStore.deleteConversation(sessionId);
      if (!deleted) {
        setWorkspaceNotice('Eliminazione della chat annullata.');
        return;
      }
      const remaining = conversations.filter(conversation => conversation.id !== sessionId);
      setQueuedPrompts(current => current.filter(item => item.conversationId !== sessionId));
      if (pendingQueuedRef.current?.conversationId === sessionId) pendingQueuedRef.current = undefined;
      if (!deletingActive) {
        setConversations(remaining);
        setWorkspaceNotice('Chat eliminata definitivamente.');
        return;
      }

      let next = remaining[0];
      if (!next) next = await conversationStore.createConversation(activeProjectId, 'Nuova chat');
      const nextMessages = await conversationStore.load(next.id);
      setConversations(remaining.length ? remaining : [next]);
      setActiveConversationId(next.id);
      setMessages(nextMessages);
      setChatSearch('');
      setQueueNotice('');
      queueDispatchingRef.current = false;
      chatPinnedRef.current = true;
      setWorkspaceNotice('Chat eliminata definitivamente.');
      setError('');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Impossibile eliminare la chat.');
    } finally {
      if (deletingActive) setHydrated(true);
    }
  }

  async function exportCurrentConversation() {
    if (!activeProject || !activeConversation) return;
    try {
      const path = await saveConversationPdf({
        projectName: activeProject.name,
        conversationTitle: activeConversation.title,
        messages,
        modelLabel: `${profileName} (${lastUsedModel || selectedModel || 'locale'})`
      });
      setWorkspaceNotice(`PDF creato: ${path}`);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Esportazione PDF non riuscita.');
    }
  }

  async function resetConversation() {
    abortRef.current?.abort();
    try {
      const cleared = await conversationStore.clear(activeConversationId);
      if (!cleared) return;
    } catch {
      setError('Impossibile cancellare l’archivio locale.');
      return;
    }
    setMessages([]);
    setQueuedPrompts(current => current.filter(item => item.conversationId !== activeConversationId));
    if (pendingQueuedRef.current?.conversationId === activeConversationId) pendingQueuedRef.current = undefined;
    queueDispatchingRef.current = false;
    setQueueNotice('Cronologia e coda della chat cancellate.');
    setError('');
  }

  async function refreshVoiceDevices(requestPermission: boolean): Promise<VoiceInputDevice | undefined> {
    setVoiceDeviceBusy(true);
    try {
      const devices = await listVoiceInputDevices(requestPermission);
      setVoiceDevices(devices);
      const preference = loadVoiceDevicePreference();
      const selected = choosePreferredVoiceInput(devices, preference);
      setSelectedVoiceDeviceId(selected?.deviceId ?? '');
      // Aggiorna un ID WebView2 cambiato soltanto quando l'etichetta conferma
      // che si tratta dello stesso dispositivo scelto dall'utente.
      if (selected && preference.label && selected.label === preference.label) {
        saveVoiceDevicePreference(selected);
      }
      if (requestPermission) {
        setVoiceNotice(selected
          ? `Microfono pronto: ${selected.label || 'dispositivo predefinito'}.`
          : 'Nessun microfono di ingresso rilevato. Collega o abilita un microfono in Windows.');
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

  async function startVoiceRecording(diagnostic = false) {
    if (!voiceStatus?.available || voiceRecording || voiceProcessing || wakeListening) return;
    setError('');
    setVoiceNotice('');
    const recorder = new VoiceRecorder();
    recorderRef.current = recorder;
    voiceDiagnosticRef.current = diagnostic;
    try {
      let device = voiceDevices.find(candidate => candidate.deviceId === selectedVoiceDeviceId);
      if (!device || !device.label) device = await refreshVoiceDevices(true);
      if (!device) throw new DOMException('No microphone', 'NotFoundError');
      const capture = await recorder.start(device.deviceId || undefined, () => {
        setError('Il microfono si è disconnesso durante la registrazione. AURA userà un altro ingresso disponibile al prossimo tentativo.');
        void finishVoiceRecording(true);
      });
      activeVoiceLabelRef.current = capture.label;
      setActiveVoiceLabel(capture.label);
      setVoiceNotice(diagnostic
        ? `Test microfono: pronuncia chiaramente “${VOICE_TEST_PHRASE}”, poi premi di nuovo il microfono.`
        : `Registrazione da ${capture.label} · ${Math.round(capture.sampleRate / 1000)} kHz.`);
      setVoiceRecording(true);
      voiceTimeoutRef.current = window.setTimeout(() => { void finishVoiceRecording(); }, voiceStatus.maxSeconds * 1000);
    } catch (cause) {
      voiceDiagnosticRef.current = false;
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
      const diagnostic = voiceDiagnosticRef.current;
      voiceDiagnosticRef.current = false;
      let transcript;
      try {
        transcript = await transcribeVoice(recording.wav);
      } finally {
        recording.wav.fill(0);
      }
      if (!diagnostic) {
        draftSourceRef.current = 'voice';
        setDraft(transcript.text);
      }
      const confidence = Math.round(transcript.confidence * 100);
      await recordInteraction({
        source: 'voice', action: diagnostic ? 'microphone-test' : 'transcription', content: transcript.text,
        outcome: disconnected || confidence < 70 ? 'caution' : 'allow',
        metadata: {
          confidence,
          durationSeconds: Number(transcript.durationSeconds.toFixed(2)),
          diagnostic,
          disconnected,
          audioStored: false
        }
      }).catch(() => false);
      const quality = recording.rms < 0.006
        ? ' Segnale molto basso: avvicina il microfono o controlla il profilo vivavoce.'
        : recording.peak >= 0.995
          ? ' Audio vicino alla saturazione: abbassa il livello del microfono.'
          : '';
      const interrupted = disconnected ? ' Registrazione interrotta dalla disconnessione.' : '';
      const match = diagnostic ? Math.round(phraseMatch(transcript.text, VOICE_TEST_PHRASE) * 100) : 0;
      const diagnosticResult = diagnostic
        ? `Test acustico ${match >= 75 && confidence >= 60 && !disconnected ? 'SUPERATO' : 'NON SUPERATO'} · frase “${transcript.text}” · corrispondenza ${match}% · `
        : '';
      setVoiceNotice(`${diagnosticResult}Trascrizione locale da ${activeVoiceLabelRef.current || 'microfono'} (${transcript.durationSeconds.toFixed(1)} s, confidenza ${confidence}%). ${confidence < 70 ? 'Confidenza bassa: rileggila con particolare attenzione.' : diagnostic ? 'Catena microfono → WAV → Whisper verificata.' : 'Controllala prima di inviarla.'}${quality}${interrupted}`);
      if (!diagnostic && autoSendVoice && confidence >= 70 && !disconnected) {
        window.setTimeout(() => composerRef.current?.requestSubmit(), 0);
      }
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(disconnected ? `Microfono disconnesso. ${message}` : message);
    } finally {
      voiceDiagnosticRef.current = false;
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

  function navigateTo(section: AppSection) {
    setActiveSection(section);
    window.requestAnimationFrame(() => {
      const target = document.getElementById(`${section}-section`);
      if (target instanceof HTMLDetailsElement) target.open = true;
      target?.scrollIntoView({ behavior: 'smooth', block: 'start' });
      target?.focus({ preventScroll: true });
    });
  }

  return <div className={`app ${sidebarOpen ? '' : 'sidebar-collapsed'}`}>
    <button type="button" className="global-task-stop" onClick={() => void emergencyStop()} disabled={emergencyStopping} title="Arresta chat, browser, voce e processi AURA (Ctrl+Alt+S)">
      <CircleStop /> {emergencyStopping ? 'Arresto…' : 'Arresta tutto'}
    </button>
    {emergencyNotice && <div className="global-stop-notice" role="status">{emergencyNotice}</div>}
    <aside aria-label="Barra laterale AURA">
      <div className="sidebar-top"><div className="brand"><Bot /> AURA</div><button type="button" className="sidebar-hide" onClick={() => setSidebarOpen(false)} title="Nascondi barra laterale" aria-label="Nascondi barra laterale"><PanelLeftClose /></button></div>
      <section className="workspace-switcher" aria-label="Progetti e conversazioni">
        <div className="workspace-heading"><span>Progetti</span><button type="button" onClick={() => void createProject()} title="Nuovo progetto" aria-label="Nuovo progetto"><FolderPlus /></button></div>
        <div className="workspace-list projects-list">
          {projects.map(project => <button type="button" key={project.id} className={project.id === activeProjectId ? 'chosen' : ''} onClick={() => void openProject(project.id)} disabled={streaming || toolRunning} title={project.name}>{project.name}</button>)}
        </div>
        <div className="workspace-heading"><span>Chat</span><button type="button" onClick={() => void createChat()} title="Nuova chat" aria-label="Nuova chat" disabled={!activeProjectId}><MessageSquarePlus /></button></div>
        <label className="chat-search"><Search aria-hidden="true" /><input type="search" value={chatSearch} onChange={event => setChatSearch(event.target.value)} placeholder="Cerca chat…" aria-label="Cerca chat per nome" /></label>
        <div className="workspace-list conversations-list">
          {filteredConversations.map(conversation => <div className="conversation-row" key={conversation.id}>
            <button type="button" className={`conversation-open ${conversation.id === activeConversationId ? 'chosen' : ''}`} onClick={() => void openConversation(conversation.id)} disabled={streaming || toolRunning} title={conversation.title}>{conversation.title}</button>
            <button type="button" className="chat-delete" onClick={() => void deleteConversation(conversation.id)} disabled={streaming || toolRunning} title={`Elimina ${conversation.title}`} aria-label={`Elimina chat ${conversation.title}`}><Trash2 /></button>
          </div>)}
          {!filteredConversations.length && <small className="chat-search-empty">Nessuna chat trovata</small>}
        </div>
      </section>
      <nav aria-label="Sezioni principali">
        <button type="button" className={activeSection === 'assistant' ? 'active' : ''} aria-current={activeSection === 'assistant' ? 'page' : undefined} onClick={() => navigateTo('assistant')}><SquareTerminal /> Assistente</button>
        <button type="button" className={activeSection === 'activity' ? 'active' : ''} aria-current={activeSection === 'activity' ? 'page' : undefined} onClick={() => navigateTo('activity')}><Activity /> Attività</button>
        <button type="button" className={activeSection === 'security' ? 'active' : ''} aria-current={activeSection === 'security' ? 'page' : undefined} onClick={() => navigateTo('security')}><ShieldCheck /> Sicurezza</button>
      </nav>
      <div className={`privacy ${connection}`}>
        <span /> {connection === 'online' ? client.displayName : connection === 'checking' ? 'Verifica locale…' : 'Provider IA offline'}
      </div>
    </aside>
    {!sidebarOpen && <button type="button" className="sidebar-restore" onClick={() => setSidebarOpen(true)} title="Mostra barra laterale" aria-label="Mostra barra laterale"><PanelLeftOpen /><span>Menu AURA</span></button>}

    <main>
      <header id="assistant-section" className="section-anchor" tabIndex={-1}>
        <div><p className="eyebrow">LOCAL AI CONTROL CENTER</p><h1>Buonasera. Cosa vuoi fare?</h1></div>
        <div className="header-controls">
          <select aria-label="Modello AURA" value={profileName} onChange={event => setProfileName(event.target.value as ResourceProfileName)}>
            {(Object.keys(RESOURCE_PROFILES) as ResourceProfileName[]).map(name => (
              <option key={name} value={name}>{name} · potenza {RESOURCE_PROFILES[name].power.toLocaleLowerCase()}</option>
            ))}
          </select>
          <button className="icon-button" onClick={() => void exportCurrentConversation()} title="Esporta la chat in PDF" aria-label="Esporta chat in PDF" disabled={!messages.length}><FileDown /></button>
          <button className="icon-button" onClick={() => void refreshOllama()} title="Aggiorna provider IA" disabled={connection === 'checking'}><RefreshCw /></button>
        </div>
      </header>

      <PcAccessBanner onModeChange={setPcAccessModeState} />

      <section className="model-bar">
        <div className={`connection-pill ${connection}`}>
          {connection === 'online' ? <Wifi /> : <WifiOff />}
          <span>{connection === 'online' ? `${models.length} modelli locali · ${client.displayName}` : connection === 'checking' ? 'Connessione…' : 'Provider IA non rilevato'}</span>
        </div>
        <select aria-label="Modello IA avanzato" title={`Modello del provider ${client.displayName}, scelto automaticamente dal profilo Luna, Terra o Sole`} value={selectedModel} onChange={event => setSelectedModel(event.target.value)} disabled={!models.length}>
          {!models.length && <option>Nessun modello</option>}
          {models.map(model => <option key={model.name} value={model.name}>{model.name}</option>)}
        </select>
        {selectedModel && <span className="model-detail">{profileName}: velocità {profile.speed.toLocaleLowerCase()}, potenza {profile.power.toLocaleLowerCase()}{profileName === 'Sole' ? ' · ragionamento senza scadenza' : ''} · motore {selectedModel} · {models.find(model => model.name === selectedModel)?.details?.parameter_size ?? 'dimensione ignota'} · {formatBytes(models.find(model => model.name === selectedModel)?.size ?? 0)}</span>}
      </section>

      {voiceStatus?.available && <section className="voice-device-bar">
        <Mic />
        <label htmlFor="voice-device">Microfono</label>
        <select
          id="voice-device"
          aria-label="Microfono per i comandi vocali"
          value={selectedVoiceDeviceId}
          onChange={event => selectVoiceDevice(event.target.value)}
          disabled={voiceRecording || voiceProcessing || voiceDeviceBusy || wakeListening}
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
          disabled={voiceRecording || voiceProcessing || voiceDeviceBusy || wakeListening}
          title="Aggiorna e autorizza i microfoni"
          aria-label="Aggiorna microfoni"
        ><RefreshCw /></button>
        <button
          type="button"
          className="secondary voice-test-button"
          onClick={() => void startVoiceRecording(true)}
          disabled={voiceRecording || voiceProcessing || voiceDeviceBusy || wakeListening}
          title={`Pronuncia: ${VOICE_TEST_PHRASE}`}
        >Test microfono</button>
        <small>{voiceDevices.length
          ? `${voiceDevices.length} ingresso${voiceDevices.length === 1 ? '' : 'i'} disponibile${voiceDevices.length === 1 ? '' : 'i'} · selezione automatica o manuale`
          : 'Compatibile con microfoni integrati, USB, Bluetooth, webcam e schede audio'}</small>
        <label className="voice-toggle"><input type="checkbox" checked={autoSendVoice} onChange={event => setAutoSendVoice(event.target.checked)} /> Invia automaticamente (confidenza ≥70%)</label>
        <label className="voice-toggle"><input type="checkbox" checked={autoSpeak} onChange={event => setAutoSpeak(event.target.checked)} disabled={!voiceStatus.ttsAvailable} /> Leggi risposte</label>
        <label className="voice-toggle" title="Segmenti di 5 secondi trascritti esclusivamente da Whisper locale; aumenta l'uso di CPU/GPU."><input type="checkbox" checked={wakeWordEnabled} onChange={event => {
          const enabled = event.target.checked;
          if (enabled) void refreshVoiceDevices(true).then(device => { if (device) setWakeWordEnabled(true); });
          else setWakeWordEnabled(false);
        }} /> Wake word “AURA” locale</label>
        <label className="voice-toggle"><input type="checkbox" checked={followUpEnabled} onChange={event => setFollowUpEnabled(event.target.checked)} disabled={!voiceStatus.ttsAvailable || wakeWordEnabled} /> Follow-up dopo TTS</label>
        {speaking && <button type="button" className="icon-button" onClick={() => void stopReadAloud()} title="Ferma voce"><VolumeX /></button>}
      </section>}

      <VisionPanel onHandCommand={controlMedia} />

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
      {wakeListening && <div className="voice-banner wake-listening"><span /> Ascolto locale attivo — pronuncia “AURA” seguito dal comando</div>}
      {voiceProcessing && <div className="voice-banner processing"><RefreshCw /> Trascrizione locale in corso…</div>}
      {voiceNotice && <div className="voice-notice">{voiceNotice}</div>}
      {activeQueuedPrompts.length > 0 && <section className="prompt-queue" aria-label="Domande in coda">
        <header><strong>Coda · {activeQueuedPrompts.length}/20</strong><small>Le domande partono automaticamente in ordine. Puoi modificarle finché non vengono prelevate.</small></header>
        <ol>{activeQueuedPrompts.map((item, index) => <li key={item.id}>
          <span aria-hidden="true">{index + 1}</span>
          <textarea aria-label={`Domanda in coda ${index + 1}`} rows={2} value={item.text} onChange={event => updateQueuedPrompt(item.id, event.target.value)} />
          <div>
            <button type="button" disabled={!item.text.trim()} onClick={() => sendQueuedPromptNow(item.id)}><Send /> Invia ora</button>
            <button type="button" className="danger" onClick={() => deleteQueuedPrompt(item.id)}><Trash2 /> Elimina</button>
          </div>
        </li>)}</ol>
      </section>}
      {queueNotice && <div className="queue-notice" role="status">{queueNotice}</div>}
      <form ref={composerRef} className={`composer ${streaming ? 'has-stop' : ''}`} onSubmit={sendMessage}>
        <textarea value={draft} onChange={event => { draftSourceRef.current = 'text'; setDraft(event.target.value); }} placeholder="Scrivi un comando o una domanda…" rows={2} onKeyDown={event => {
          if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }} />
        <button
          type="button"
          className={`voice-button ${voiceRecording ? 'recording' : ''}`}
          onClick={() => voiceRecording ? void finishVoiceRecording() : void startVoiceRecording()}
          disabled={!voiceStatus?.available || voiceProcessing || streaming || toolRunning || wakeListening}
          title={voiceStatus?.available ? (voiceRecording ? 'Termina registrazione' : 'Avvia push-to-talk') : 'Motore vocale locale non installato'}
          aria-label={voiceRecording ? 'Termina registrazione vocale' : 'Avvia registrazione vocale'}
        ><Mic /></button>
        {streaming && <button type="button" className="stop" onClick={() => abortRef.current?.abort()}><CircleStop /> Stop</button>}
        <button type="submit" disabled={!draft.trim()}><Send /> {streaming || toolRunning ? 'Accoda' : 'Invia'}</button>
        <button type="button" className="secondary" onClick={() => void resetConversation()} title="Cancella cronologia"><Trash2 /></button>
      </form>

      <details id="activity-section" className="section-block activity-block section-anchor" tabIndex={-1} onToggle={event => { if (event.currentTarget.open) setActiveSection('activity'); }}>
        <summary><Activity /><span><strong>Attività</strong><small>Azioni PC, automazioni, log, workspace, applicazioni e adattamento</small></span></summary>
        <div className="section-block-content">
      <details className="simulation">
        <summary>Laboratorio policy — simula un’azione</summary>
        <div className="actions">{demos.map((demo, index) => <button key={demo.label} onClick={() => setSelectedAction(index)} className={index === selectedAction ? 'chosen' : ''}>{demo.label}</button>)}</div>
        <div className="plan">
          <div><span className={`badge ${result.risk}`}>{result.risk}</span><h2>{plan.summary}</h2><p>Capability: {plan.capability} · Scope: {plan.scope}</p></div>
          <div className={`decision ${result.decision}`}><strong>{result.decision.toUpperCase()}</strong><span>{result.reason}</span></div>
        </div>
      </details>

      <AuditLogPanel />

      <details className="personalization pc-control">
        <summary><Cpu /> Centro controllo PC</summary>
        <p className="section-help">Azioni tipizzate: nessun comando arbitrario. Aperture, terminazioni e blocco sessione richiedono una conferma nativa Windows.</p>
        <div className="pc-action-grid">
          <button type="button" onClick={() => void executeLocalCommand('Stato del PC', { kind: 'system-status' })}>Stato PC</button>
          <button type="button" onClick={() => void executeLocalCommand('Leggi lo schermo', { kind: 'screen-read', prompt: 'Descrivi cosa è visibile sullo schermo.' })}>Leggi schermo</button>
          <button type="button" onClick={() => void executeLocalCommand('Mostra i processi', { kind: 'process-list' })}>Processi</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni', { kind: 'open-app', application: 'settings', label: 'Impostazioni Windows' })}>Impostazioni</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni Bluetooth', { kind: 'open-app', application: 'bluetooth-settings', label: 'Impostazioni Bluetooth' })}>Bluetooth</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri impostazioni audio', { kind: 'open-app', application: 'sound-settings', label: 'Impostazioni audio' })}>Audio</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri sicurezza Windows', { kind: 'open-app', application: 'security-settings', label: 'Sicurezza di Windows' })}>Sicurezza</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri Gestione attivita', { kind: 'open-app', application: 'task-manager', label: 'Gestione attivita' })}>Gestione attivita</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri Download', { kind: 'open-app', application: 'downloads', label: 'Download' })}>Download</button>
          <button type="button" onClick={() => void executeLocalCommand('Apri Documenti', { kind: 'open-app', application: 'documents', label: 'Documenti' })}>Documenti</button>
          <button type="button" onClick={() => void executeLocalCommand('Attiva muto', { kind: 'media', action: 'mute', label: 'Muto commutato' })}>Muto</button>
          <button type="button" onClick={() => void executeLocalCommand('Play pausa', { kind: 'media', action: 'play-pause', label: 'Riproduzione commutata' })}>Play/Pausa</button>
          <button type="button" onClick={() => void executeLocalCommand('Blocca il PC', { kind: 'lock-workstation' })}>Blocca sessione</button>
        </div>
        <div className="terminate-control">
          <label>PID da terminare<input inputMode="numeric" value={terminatePid} onChange={event => setTerminatePid(event.target.value.replace(/\D/g, '').slice(0, 10))} placeholder="es. 1234" /></label>
          <button type="button" className="danger" disabled={!Number(terminatePid) || toolRunning} onClick={() => void executeLocalCommand(`Termina processo ${terminatePid}`, { kind: 'terminate-process', pid: Number(terminatePid) })}>Termina con conferma</button>
        </div>
      </details>

      <WorkspaceControlPanel />

      <InstalledAppsPanel />

      <RemoteAccessPanel onChatRequest={query => setDraft(query)} />

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
        <div className="memory-control">
          <span>Memoria versionata: {memoryStatus?.lessons ?? 0} lezioni · {memoryStatus?.snapshots ?? 0} versioni</span>
          <label><input type="checkbox" checked={memoryStatus?.automatic ?? false} onChange={event => void setAutomaticMemory(event.target.checked).then(setMemoryStatus).catch(cause => setError(String(cause)))} /> Heartbeat ogni 30 minuti</label>
          <button type="button" onClick={() => void consolidateMemory().then(setMemoryStatus).catch(cause => setError(String(cause)))}>Consolida ora</button>
          <button type="button" disabled={(memoryStatus?.snapshots ?? 0) < 2} onClick={() => void rollbackMemory().then(setMemoryStatus).catch(cause => setError(String(cause)))}>Ripristina versione</button>
        </div>
        {profileNotice && <div className="voice-notice">{profileNotice}</div>}
      </details>

      <EvolutionLabPanel projectName={personalProfile.projectContext} />

      <AiProvidersPanel />

        </div>
      </details>

      <details id="security-section" className="section-block security-block section-anchor" tabIndex={-1} onToggle={event => { if (event.currentTarget.open) setActiveSection('security'); }}>
        <summary><ShieldCheck /><span><strong>Sicurezza</strong><small>Postura Windows, incident response, privacy, cifratura e integrità</small></span></summary>
        <div className="section-block-content">

      <SecurityCenterPanel />

      <details className="personalization security-center">
        <summary><Database /> Privacy e cifratura verificabili</summary>
        <p className="section-help">Autodiagnosi sul database reale: integrita, PRAGMA di sicurezza, payload cifrati e catena audit autenticata.</p>
        {privacyStatus && <div className="evolution-history">
          <article><span>Database SQLite</span><strong className={privacyStatus.databaseIntegrity ? 'security-healthy' : 'security-warning'}>{privacyStatus.databaseIntegrity ? 'integro' : 'errore'}</strong><small>WAL {privacyStatus.walEnabled ? 'attivo' : 'non attivo'} · secure delete {privacyStatus.secureDelete ? 'attivo' : 'non attivo'} · foreign keys {privacyStatus.foreignKeys ? 'attive' : 'non attive'}</small></article>
          <article><span>Payload sensibili</span><strong className={privacyStatus.encryptedPayloads ? 'security-healthy' : 'security-warning'}>{privacyStatus.encryptedPayloads ? 'cifrati' : 'anomalia'}</strong><small>{privacyStatus.plaintextPayloadRows} righe non conformi · chiave in {privacyStatus.keyStore}</small></article>
          <article><span>Audit autenticato</span><strong className={privacyStatus.auditChainValid ? 'security-healthy' : 'security-warning'}>{privacyStatus.auditChainValid ? 'integro' : 'alterato'}</strong><small>{privacyStatus.auditedEvents} eventi verificati</small></article>
          <article><span>Limite dichiarato</span><small>{privacyStatus.metadataNotice}</small></article>
        </div>}
        <div className="settings-actions"><button type="button" disabled={labBusy} onClick={() => void runLab(async () => { setPrivacyStatus(await getPrivacyStatus()); setLabNotice('Autodiagnosi privacy completata sul database reale.'); })}>Esegui autodiagnosi</button></div>
      </details>

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

      <KnowledgeToolsPanel onUseRag={(citation, excerpt) => setDraft(current => `${current}\n\n${citation}\n${excerpt}`.trim())} />

      {labNotice && <div className="voice-notice">{labNotice}</div>}

      <section className="stats">
        <article><span>Modello</span><strong>{lastUsedModel || selectedModel || 'Non connesso'}</strong></article>
        <article><span>Modello AURA</span><strong>{profileName} · potenza {profile.power.toLocaleLowerCase()} · {profile.contextTokens / 1024}k</strong></article>
        <article><span>Prestazioni</span><strong>{tokensPerSecond ? `${tokensPerSecond.toFixed(1)} tok/s` : 'In attesa'}</strong></article>
        <article><span>Memoria chat</span><strong><Database /> {storageMode === 'encrypted-windows' ? 'Cifrata' : storageMode === 'browser-fallback' ? 'Locale' : 'RAM'} · {messages.length}/100</strong></article>
        <article><span>Audit</span><strong><ShieldCheck /> {auditStatus ? `${auditStatus.valid ? 'Integro' : 'Alterato'} · ${auditStatus.count}` : 'Anteprima web'}</strong></article>
        <article><span>Controllo PC</span><strong><Cpu /> Tool tipizzati · conferme native</strong></article>
        <article><span>Guardrail</span><strong><ShieldCheck /> Policy Rust · Guardian {guardrail?.installed ? 'integrato' : 'assente'}</strong></article>
        <article><span>Analisi critica</span><strong><Bot /> Sempre attiva · no compiacenza</strong></article>
        <article><span>Coding</span><strong><SquareTerminal /> Generazione attiva · esecuzione isolata</strong></article>
      </section>

      <section className="workspace-context" aria-label="Conversazione attiva">
        <span>{activeProject?.name ?? 'Progetto'}</span><strong>{activeConversation?.title ?? 'Conversazione'}</strong>
        <button type="button" onClick={() => void createChat()}><MessageSquarePlus /> Nuova chat</button>
        <button type="button" onClick={() => void exportCurrentConversation()} disabled={!messages.length}><FileDown /> PDF</button>
      </section>

      {workspaceNotice && <div className="voice-notice">{workspaceNotice}</div>}
    </main>
  </div>;
}
