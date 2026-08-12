import { useEffect, useMemo, useState } from 'react';
import { AppWindow, RefreshCw, Search } from 'lucide-react';
import { listInstalledApplications, openInstalledApplication, type InstalledApplication } from '../../adapters/agent';

export function InstalledAppsPanel() {
  const [applications, setApplications] = useState<InstalledApplication[]>([]);
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (needle ? applications.filter(application => application.label.toLowerCase().includes(needle)) : applications).slice(0, 80);
  }, [applications, query]);

  async function refresh() {
    setBusy(true);
    setNotice('');
    try {
      const next = await listInstalledApplications();
      setApplications(next);
      setNotice(next.length ? `${next.length} applicazioni indicizzate dal Menu Start.` : 'Catalogo disponibile nell’app desktop Windows.');
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function open(application: InstalledApplication) {
    if (busy) return;
    setBusy(true);
    setNotice('');
    try {
      const opened = await openInstalledApplication(application.id);
      setNotice(opened ? `${application.label} avviata dal collegamento Windows verificato.` : 'Avvio annullato.');
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => { void refresh(); }, []);

  return <details className="personalization installed-apps">
    <summary><AppWindow /> Applicazioni installate</summary>
    <p className="section-help">Catalogo ricavato dai Menu Start utente e di sistema. Ogni collegamento ha un ID HMAC; AURA non accetta eseguibili o argomenti arbitrari.</p>
    <div className="installed-app-toolbar">
      <Search />
      <input value={query} onChange={event => setQuery(event.target.value)} placeholder="Cerca applicazione" aria-label="Cerca applicazione installata" />
      <button type="button" className="icon-button" onClick={() => void refresh()} disabled={busy} title="Aggiorna applicazioni"><RefreshCw /></button>
    </div>
    <div className="installed-app-list">
      {!filtered.length && <p>Nessuna applicazione da mostrare.</p>}
      {filtered.map(application => <button type="button" key={application.id} onClick={() => void open(application)} disabled={busy}>
        <AppWindow /><span><strong>{application.label}</strong><small>{application.source}</small></span>
      </button>)}
    </div>
    {applications.length > filtered.length && <small>Mostrate le prime {filtered.length} applicazioni; usa la ricerca per restringere.</small>}
    {notice && <div className="voice-notice" role="status">{notice}</div>}
  </details>;
}
