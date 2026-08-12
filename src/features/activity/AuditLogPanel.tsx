import { useEffect, useMemo, useState } from 'react';
import { Download, RefreshCw, ScrollText, ShieldCheck } from 'lucide-react';
import {
  exportAuditLog, listAuditEvents, verifyAudit, type AuditEvent, type AuditStatus
} from '../../adapters/security';

function formatEventTime(value: string): string {
  const numeric = Number(value);
  const date = Number.isFinite(numeric) && numeric > 0 ? new Date(numeric) : new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('it-IT');
}

export function AuditLogPanel() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [status, setStatus] = useState<AuditStatus>();
  const [query, setQuery] = useState('');
  const [decision, setDecision] = useState('');
  const [source, setSource] = useState('');
  const [limit, setLimit] = useState(200);
  const [includePayloads, setIncludePayloads] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  async function refresh() {
    setBusy(true);
    setNotice('');
    try {
      const [nextEvents, nextStatus] = await Promise.all([
        listAuditEvents({ limit, decision: decision || undefined, includePayloads }),
        verifyAudit()
      ]);
      setEvents(nextEvents);
      setStatus(nextStatus);
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function download() {
    setBusy(true);
    setNotice('');
    try {
      const path = await exportAuditLog(includePayloads);
      setNotice(`Log salvati in: ${path}`);
      await refresh();
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
      setBusy(false);
    }
  }

  useEffect(() => { void refresh(); }, []);

  const visibleEvents = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase('it');
    if (!normalized) return events;
    return events.filter(event => (!source || event.eventType === `interaction.${source}`)
      && (!normalized || [event.eventType, event.summary, event.correlationId, JSON.stringify(event.payload ?? '')]
        .some(value => value.toLocaleLowerCase('it').includes(normalized))));
  }, [events, query, source]);

  return <details className="personalization audit-log-panel">
    <summary><ScrollText /> Log e diagnostica AI</summary>
    <p className="section-help">
      Eventi tecnici locali con catena autenticata. Le interazioni voce, Alexa e gesti sono cifrate; audio e fotogrammi non vengono salvati.
    </p>
    <div className="audit-toolbar">
      <label>Cerca<input value={query} onChange={event => setQuery(event.target.value)} placeholder="tipo, riepilogo o correlazione" /></label>
      <label>Esito<select value={decision} onChange={event => setDecision(event.target.value)}>
        <option value="">Tutti</option><option value="allow">Consentito</option><option value="confirm">Conferma</option>
        <option value="caution">Cautela</option><option value="deny">Negato</option><option value="error">Errore</option>
      </select></label>
      <label>Origine<select value={source} onChange={event => setSource(event.target.value)}>
        <option value="">Tutte</option><option value="voice">Voce</option><option value="alexa">Alexa</option><option value="gesture">Gesti</option>
      </select></label>
      <label>Quantità<select value={limit} onChange={event => setLimit(Number(event.target.value))}>
        <option value={50}>50</option><option value={200}>200</option><option value={1000}>1000</option>
      </select></label>
    </div>
    <label className="audit-payload-toggle">
      <input type="checkbox" checked={includePayloads} onChange={event => setIncludePayloads(event.target.checked)} />
      Includi e decifra contenuti delle interazioni e dettagli tecnici: possono contenere trascrizioni, richieste Alexa, percorsi e URL.
    </label>
    <div className="settings-actions">
      <button type="button" disabled={busy} onClick={() => void refresh()}><RefreshCw /> Aggiorna</button>
      <button type="button" disabled={busy} onClick={() => void download()}><Download /> Scarica JSON</button>
      <span className={status?.valid ? 'audit-integrity valid' : 'audit-integrity invalid'}>
        <ShieldCheck /> {status ? `${status.valid ? 'Catena integra' : 'Catena alterata'} · ${status.count} eventi` : 'Verifica in corso'}
      </span>
    </div>
    <div className="audit-list" aria-live="polite">
      {!busy && !visibleEvents.length && <p>Nessun evento corrispondente.</p>}
      {visibleEvents.map(event => <article key={event.sequence}>
        <div className="audit-event-heading">
          <span className={`audit-decision ${event.decision || 'unknown'}`}>{event.decision || 'n/d'}</span>
          <strong>{event.eventType}</strong><time>{formatEventTime(event.createdAt)}</time>
        </div>
        <p>{event.summary}</p>
        <small>#{event.sequence} · correlazione {event.correlationId} · hash {event.eventHash.slice(0, 12)}…</small>
        {includePayloads && event.payload !== undefined && <pre>{JSON.stringify(event.payload, null, 2)}</pre>}
      </article>)}
    </div>
    {notice && <div className="voice-notice">{notice}</div>}
  </details>;
}
