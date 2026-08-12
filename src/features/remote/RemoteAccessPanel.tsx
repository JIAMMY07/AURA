import { useEffect, useState } from 'react';
import { Mic, ShieldCheck, Trash2 } from 'lucide-react';
import {
  getRemoteStatus,
  lockWorkstation, openApplication,
  pairRemote, pollRemoteOnce,
  unpairRemote,
  type AllowedApplication,
  type PairingResult,
  type RemotePollResult,
  type RemoteStatus
} from '../../adapters/agent';
import { recordInteraction } from '../../adapters/security';

const remoteApplications: AllowedApplication[] = [
  'calculator', 'notepad', 'explorer', 'task-manager', 'paint', 'snipping-tool',
  'settings', 'bluetooth-settings', 'sound-settings', 'display-settings',
  'notification-settings', 'microphone-privacy', 'windows-update', 'downloads',
  'documents', 'security-settings'
];

export function RemoteAccessPanel({ onChatRequest }: { onChatRequest?: (query: string) => void }) {
  const [status, setStatus] = useState<RemoteStatus>();
  const [deviceName, setDeviceName] = useState('Alexa');
  const [relayUrl, setRelayUrl] = useState('');
  const [pairing, setPairing] = useState<PairingResult>();
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [pending, setPending] = useState<RemotePollResult['command']>();

  async function refresh() {
    setStatus(await getRemoteStatus());
  }

  useEffect(() => {
    void refresh().catch(() => undefined);
    const stop = () => {
      setBusy(false);
      setPending(undefined);
      setStatus(current => current ? { ...current, enabled: false } : current);
      setNotice('Polling Alexa e operazione pendente arrestati localmente. Riattiva aggiornando lo stato.');
    };
    window.addEventListener('aura:stop-all', stop);
    return () => window.removeEventListener('aura:stop-all', stop);
  }, []);

  useEffect(() => {
    if (!status?.paired || !status.enabled) return;
    let active = true;
    const timer = window.setInterval(() => {
      void pollRemoteOnce().then(result => {
        if (active && result.received) {
          setNotice(`${result.command?.command}: ${result.outcome}`);
          if (result.command?.policy === 'confirm') setPending(result.command);
        }
      }).catch(() => undefined);
    }, 15_000);
    return () => { active = false; window.clearInterval(timer); };
  }, [status?.paired, status?.enabled]);

  async function run(operation: () => Promise<void>) {
    setBusy(true);
    setNotice('');
    try {
      await operation();
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function pair() {
    const result = await pairRemote(deviceName.trim(), relayUrl.trim());
    setPairing(result);
    await refresh();
    setNotice('Associazione creata. Copia ora il segreto nel futuro plugin Alexa.');
  }

  async function revoke() {
    const removed = await unpairRemote();
    if (!removed) {
      setNotice('Revoca annullata o nessuna associazione presente.');
      return;
    }
    setPairing(undefined);
    await refresh();
    setNotice('Associazione Alexa revocata e segreto invalidato.');
  }

  async function poll() {
    const result = await pollRemoteOnce();
    setNotice(result.received ? `${result.command?.command}: ${result.outcome}` : result.outcome);
    if (result.command?.policy === 'confirm') setPending(result.command);
  }

  async function executePending() {
    if (!pending) return;
    const payload = JSON.parse(pending.payloadJson) as Record<string, unknown>;
    if (pending.command === 'app.open') {
      const application = typeof payload.application === 'string' ? payload.application : '';
      if (!remoteApplications.includes(application as AllowedApplication)) throw new Error('Applicazione Alexa fuori allowlist.');
      await openApplication(application as AllowedApplication);
    } else if (pending.command === 'system.lock') {
      await lockWorkstation();
    } else if (pending.command === 'chat.ask') {
      const query = typeof payload.query === 'string' ? payload.query.trim().slice(0, 4000) : '';
      if (!query || !onChatRequest) throw new Error('Domanda Alexa non valida o chat non disponibile.');
      onChatRequest(query);
      setNotice('Domanda Alexa copiata nella chat: controllala e inviala quando vuoi.');
    } else {
      throw new Error('Comando remoto non eseguibile.');
    }
    await recordInteraction({
      source: 'alexa', action: `${pending.command}.executed`, content: pending.payloadJson, outcome: 'allow',
      correlationId: pending.id, metadata: { locallyConfirmed: true, signatureLogged: false }
    }).catch(() => false);
    setPending(undefined);
  }

  async function rejectPending() {
    if (!pending) return;
    await recordInteraction({
      source: 'alexa', action: `${pending.command}.rejected`, content: pending.payloadJson, outcome: 'deny',
      correlationId: pending.id, metadata: { locallyConfirmed: false, signatureLogged: false }
    }).catch(() => false);
    setPending(undefined);
    setNotice('Comando Alexa rifiutato localmente e registrato nel log.');
  }

  return <details className="personalization remote-access">
    <summary><Mic /> Controllo vocale remoto Alexa</summary>
    <p className="section-help">
      Flusso obbligatorio: Alexa → AURA → PC. Alexa invia soltanto intenti firmati ad AURA e non accede mai direttamente a Windows.
      Collegamento predisposto tramite richieste HTTPS in uscita: AURA non apre porte sul PC.
      Dopo il pairing AURA interroga automaticamente il relay ogni 15 secondi mentre l'app è aperta.
      Firma HMAC, scadenza, nonce anti-replay e allowlist impediscono al plugin remoto di aggirare i guardrail locali.
    </p>

    <div className="evolution-history">
      <article>
        <span>Stato associazione</span>
        <strong>{status?.paired && status.enabled ? `attiva · ${status.deviceName}` : 'non associata'}</strong>
        <small>{status?.relayHost ? `Relay: ${status.relayHost}` : 'Nessun relay configurato'} · porta in ingresso: {status?.inboundPort ? 'aperta' : 'nessuna'}</small>
      </article>
      <article>
        <span>Comandi consentiti</span>
        <strong>{status?.allowedCommands.join(', ') || 'caricamento…'}</strong>
        <small>Le azioni che modificano il PC richiedono comunque conferma locale.</small>
      </article>
    </div>

    <div className="profile-grid">
      <label>Nome dispositivo<input value={deviceName} onChange={event => setDeviceName(event.target.value)} maxLength={80} /></label>
      <label>Relay HTTPS pubblico<input value={relayUrl} onChange={event => setRelayUrl(event.target.value)} placeholder="https://relay.example.com/aura/" inputMode="url" /></label>
    </div>
    <div className="settings-actions">
      <button type="button" disabled={busy || !deviceName.trim() || !relayUrl.trim()} onClick={() => void run(pair)}><ShieldCheck /> Associa con conferma</button>
      <button type="button" disabled={busy || !status?.paired} className="danger" onClick={() => void run(revoke)}><Trash2 /> Revoca</button>
      <button type="button" disabled={busy} onClick={() => void run(refresh)}>Aggiorna stato</button>
      <button type="button" disabled={busy || !status?.paired} onClick={() => void run(poll)}>Controlla relay ora</button>
    </div>

    {pairing && <div className="tool-output" role="status">
      <strong>Segreto mostrato una sola volta</strong>
      <p>{pairing.warning}</p>
      <code>{pairing.pairingSecret}</code>
      <div className="settings-actions">
        <button type="button" onClick={() => setPairing(undefined)}>Ho copiato: rimuovi dalla schermata</button>
      </div>
    </div>}
    {pending && <div className="tool-output" role="alert">
      <strong>Comando Alexa in attesa: {pending.command}</strong>
      <p>{pending.reason} La conferma nativa Windows resta obbligatoria per le azioni sul PC.</p>
      <div className="settings-actions">
        <button type="button" disabled={busy} onClick={() => void run(executePending)}>Verifica ed esegui</button>
        <button type="button" className="danger" disabled={busy} onClick={() => void run(rejectPending)}>Rifiuta</button>
      </div>
    </div>}
    {notice && <div className="voice-notice">{notice}</div>}
  </details>;
}
