import { useEffect, useState } from 'react';
import { Mic, ShieldCheck, Trash2 } from 'lucide-react';
import {
  getRemoteStatus,
  pairRemote,
  unpairRemote,
  type PairingResult,
  type RemoteStatus
} from '../../adapters/agent';

export function RemoteAccessPanel() {
  const [status, setStatus] = useState<RemoteStatus>();
  const [deviceName, setDeviceName] = useState('Alexa');
  const [relayUrl, setRelayUrl] = useState('');
  const [pairing, setPairing] = useState<PairingResult>();
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  async function refresh() {
    setStatus(await getRemoteStatus());
  }

  useEffect(() => {
    void refresh().catch(() => undefined);
  }, []);

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

  return <details className="personalization remote-access">
    <summary><Mic /> Controllo vocale remoto Alexa</summary>
    <p className="section-help">
      Collegamento predisposto tramite richieste HTTPS in uscita: AURA non apre porte sul PC.
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
    </div>

    {pairing && <div className="tool-output" role="status">
      <strong>Segreto mostrato una sola volta</strong>
      <p>{pairing.warning}</p>
      <code>{pairing.pairingSecret}</code>
      <div className="settings-actions">
        <button type="button" onClick={() => setPairing(undefined)}>Ho copiato: rimuovi dalla schermata</button>
      </div>
    </div>}
    {notice && <div className="voice-notice">{notice}</div>}
  </details>;
}
