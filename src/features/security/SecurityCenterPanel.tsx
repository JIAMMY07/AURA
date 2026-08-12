import { useEffect, useState } from 'react';
import { ShieldCheck } from 'lucide-react';
import {
  getSecurityCenterStatus,
  getIncidentStatus,
  remediateDefenderThreats,
  runDefenderQuickScan,
  updateDefenderDefinitions,
  type IncidentStatus,
  type SecurityCenterStatus
} from '../../adapters/agent';

export function SecurityCenterPanel() {
  const [status, setStatus] = useState<SecurityCenterStatus>();
  const [incident, setIncident] = useState<IncidentStatus>();
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  async function run(operation: () => Promise<void>) {
    setBusy(true);
    setNotice('');
    try { await operation(); }
    catch (error) { setNotice(error instanceof Error ? error.message : String(error)); }
    finally { setBusy(false); }
  }

  async function refresh() {
    const [center, incidentStatus] = await Promise.all([getSecurityCenterStatus(), getIncidentStatus()]);
    setStatus(center);
    setIncident(incidentStatus);
  }
  useEffect(() => {
    void refresh().catch(() => undefined);
    const stop = () => { setBusy(false); setNotice('Task Security Center arrestata dal comando globale.'); };
    window.addEventListener('aura:stop-all', stop);
    return () => window.removeEventListener('aura:stop-all', stop);
  }, []);

  return <details className="personalization security-center">
    <summary><ShieldCheck /> Security Center AURA</summary>
    <p className="section-help">
      Copertura diagnostica: <strong>{status?.coverageScore ?? '…'}%</strong> · postura del PC: <strong>{status?.postureScore ?? '…'}/100</strong>.
      I controlli sono in sola lettura; scansione e aggiornamento Defender richiedono conferma nativa.
    </p>
    {status && <div className="evolution-history">
      {status.checks.map(check => <article key={check.id}>
        <span>{check.label}</span>
        <strong className={`security-${check.status}`}>{check.status}</strong>
        <small>{check.detail}</small>
      </article>)}
      <article>
        <span>Firma applicazione</span>
        <strong>{status.publicSignature ? 'pubblica e timestamped' : status.localSignature ? 'locale verificabile' : 'assente'}</strong>
        <small>{status.signatureStatus} · {status.signer || 'publisher assente'} · timestamp {status.timestamped ? 'presente' : 'assente'} · SHA-256 {status.executableSha256}</small>
      </article>
      {status.recommendations.length > 0 && <article>
        <span>Azioni consigliate</span>
        <small>{status.recommendations.join(' · ')}</small>
      </article>}
    </div>}
    {incident && <div className="evolution-history">
      <article>
        <span>Incident Response</span>
        <strong className={`security-${incident.severity === 'healthy' ? 'healthy' : 'warning'}`}>{incident.severity}</strong>
        <small>{incident.summary}</small>
      </article>
      <article>
        <span>Protezione AURA e rete</span>
        <strong>{incident.auraIntegrity}</strong>
        <small>Defender {incident.defenderHealthy ? 'attivo' : 'da ripristinare'} · Firewall {incident.firewallHealthy ? 'attivo' : 'da verificare'} · contenimento automatico {incident.automaticContainment ? 'attivo' : 'disattivato per sicurezza'}</small>
      </article>
      {incident.recommendations.length > 0 && <article>
        <span>Playbook consigliato</span>
        <small>{incident.recommendations.join(' · ')}</small>
      </article>}
    </div>}
    <div className="settings-actions">
      <button type="button" disabled={busy} onClick={() => void run(async () => { await refresh(); setNotice('Postura Windows aggiornata.'); })}>Aggiorna controlli</button>
      <button type="button" disabled={busy} onClick={() => void run(async () => { await runDefenderQuickScan(); await refresh(); setNotice('Scansione rapida Defender completata.'); })}>Scansione rapida</button>
      <button type="button" disabled={busy} onClick={() => void run(async () => { await updateDefenderDefinitions(); await refresh(); setNotice('Definizioni Defender aggiornate.'); })}>Aggiorna definizioni</button>
      <button type="button" disabled={busy || !incident?.activeThreats} onClick={() => void run(async () => {
        const result = await remediateDefenderThreats();
        await refresh();
        setNotice(result.message);
      })}>Contieni minacce rilevate</button>
    </div>
    {notice && <div className="voice-notice">{notice}</div>}
  </details>;
}
