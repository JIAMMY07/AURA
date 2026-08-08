import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity, Bot, CircleStop, Database, Mic, RefreshCw, Send,
  ShieldCheck, SquareTerminal, Trash2, Wifi, WifiOff
} from 'lucide-react';
import { OllamaClient, type ChatMessage, type OllamaModel } from './adapters/ollama';
import { choosePreferredModel, RESOURCE_PROFILES, type ResourceProfileName } from './core/modelProfiles';
import { evaluate } from './core/policyEngine';
import type { ActionPlan, PolicyResult, Risk } from './core/types';
import { createConversationStore, type StorageMode } from './services/conversationStore';
import { evaluateAction, verifyAudit, type AuditStatus } from './adapters/security';

const client = new OllamaClient();
const conversationStore = createConversationStore();
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
  const abortRef = useRef<AbortController | undefined>(undefined);
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
  useEffect(() => {
    let active = true;
    void evaluateAction(plan)
      .then(decision => {
        if (!active) return;
        setResult(decision);
        return verifyAudit().then(status => { if (active) setAuditStatus(status); });
      })
      .catch(() => { if (active) setResult({ decision: 'deny', reason: 'Policy backend non disponibile.' }); });
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
    setMessages([...history, assistantMessage]);
    setDraft('');
    setError('');
    setStreaming(true);
    setTokensPerSecond(undefined);
    const controller = new AbortController();
    abortRef.current = controller;
    const generationTimeout = window.setTimeout(() => controller.abort(), 180_000);

    try {
      const stream = await client.chat(
        selectedModel,
        history,
        profile,
        token => setMessages(current => current.map(message =>
          message.id === assistantMessage.id
            ? { ...message, content: message.content + token }
            : message
        )),
        controller.signal
      );
      setTokensPerSecond(stream.tokensPerSecond);
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') {
        setError('Generazione interrotta. La risposta parziale è stata conservata.');
      } else {
        setError(cause instanceof Error ? cause.message : 'Errore durante la generazione.');
        setMessages(current => current.filter(message =>
          message.id !== assistantMessage.id || message.content.length > 0
        ));
      }
    } finally {
      window.clearTimeout(generationTimeout);
      abortRef.current = undefined;
      setStreaming(false);
    }
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

      <section className="chat" aria-live="polite">
        {!messages.length && <div className="empty-chat">
          <div className="orb"><Mic /></div>
          <p>Chat locale pronta</p>
          <small>Push-to-talk arriverà nella Fase 3. Il microfono non è in ascolto.</small>
        </div>}
        {messages.map(message => <article key={message.id} className={`message ${message.role}`}>
          <span>{message.role === 'user' ? 'Tu' : 'AURA'}</span>
          <p>{message.content || (streaming ? 'Sto elaborando…' : '')}</p>
        </article>)}
      </section>

      {error && <div className="error-banner">{error}</div>}
      <form className="composer" onSubmit={sendMessage}>
        <textarea value={draft} onChange={event => setDraft(event.target.value)} placeholder="Scrivi un comando o una domanda…" rows={2} onKeyDown={event => {
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }} />
        {streaming
          ? <button type="button" className="stop" onClick={() => abortRef.current?.abort()}><CircleStop /> Stop</button>
          : <button type="submit" disabled={!draft.trim()}><Send /> Invia</button>}
        <button type="button" className="secondary" onClick={() => void resetConversation()} title="Cancella cronologia"><Trash2 /></button>
      </form>

      <details className="simulation">
        <summary>Laboratorio policy — simula un’azione</summary>
        <div className="actions">{demos.map((demo, index) => <button key={demo.label} onClick={() => setSelectedAction(index)} className={index === selectedAction ? 'chosen' : ''}>{demo.label}</button>)}</div>
        <div className="plan">
          <div><span className={`badge ${plan.risk}`}>{plan.risk}</span><h2>{plan.summary}</h2><p>Capability: {plan.capability} · Scope: {plan.scope}</p></div>
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
