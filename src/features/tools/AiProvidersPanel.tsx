import { useEffect, useState } from 'react';
import { Cpu, RefreshCw } from 'lucide-react';
import { listAiProviders, type AiProviderDescriptor } from '../../adapters/agent';

export function AiProvidersPanel() {
  const [providers, setProviders] = useState<AiProviderDescriptor[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  async function refresh() {
    setBusy(true);
    setNotice('');
    try { setProviders(await listAiProviders()); }
    catch (error) { setNotice(error instanceof Error ? error.message : String(error)); }
    finally { setBusy(false); }
  }

  useEffect(() => { void refresh(); }, []);

  return <details className="personalization ai-providers-panel">
    <summary><Cpu /> Motori IA e provider futuri</summary>
    <p className="section-help">
      La chat usa un contratto indipendente dal runtime. Ollama resta attivo; gli altri motori sono slot locali
      predisposti ma non vengono installati o attivati automaticamente.
    </p>
    <div className="settings-actions">
      <button type="button" onClick={() => void refresh()} disabled={busy}><RefreshCw /> {busy ? 'Verifica…' : 'Verifica provider'}</button>
    </div>
    <div className="provider-grid">
      {providers.map(provider => <article key={provider.id} data-active={provider.active}>
        <div className="provider-heading">
          <strong>{provider.displayName}</strong>
          <span className={`provider-state ${provider.active && provider.reachable ? 'ready' : provider.active ? 'offline' : 'future'}`}>
            {provider.active ? provider.reachable ? 'attivo' : 'offline' : 'predisposto'}
          </span>
        </div>
        <small>Contratto v{provider.contractVersion} · {provider.apiKind} · {provider.localOnly ? 'solo locale' : 'remoto'}</small>
        {provider.endpoint && <code>{provider.endpoint}</code>}
        <p>{provider.notes}</p>
        <div className="provider-capabilities">{provider.capabilities.map(item => <span key={item}>{item}</span>)}</div>
      </article>)}
      {!providers.length && !busy && <p>Nessun catalogo disponibile nell’anteprima web.</p>}
    </div>
    {notice && <div className="voice-notice">{notice}</div>}
  </details>;
}
