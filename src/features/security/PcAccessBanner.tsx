import { useEffect, useState } from 'react';
import { ChevronDown, ChevronUp, LockKeyhole, Network, ShieldCheck } from 'lucide-react';
import { getPcAccessMode, setPcAccessMode, type PcAccessMode, type PcAccessStatus } from '../../adapters/agent';

export function PcAccessBanner({ onModeChange }: { onModeChange: (mode: PcAccessMode) => void }) {
  const [status, setStatus] = useState<PcAccessStatus>();
  const [busy, setBusy] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [notice, setNotice] = useState('');

  useEffect(() => {
    void getPcAccessMode().then(value => { setStatus(value); onModeChange(value.mode); })
      .catch(error => setNotice(error instanceof Error ? error.message : String(error)));
    // Il backend viene letto una sola volta all'avvio; i cambi successivi passano da changeMode.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function changeMode(mode: PcAccessMode) {
    setBusy(true); setNotice('');
    try {
      const next = await setPcAccessMode(mode);
      setStatus(next);
      onModeChange(next.mode);
      if (next.mode !== mode) setNotice('Modifica del profilo annullata: resta attiva la modalità precedente.');
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally { setBusy(false); }
  }

  const mode = status?.mode ?? 'controlled';
  return <section className={`pc-access-banner ${mode} ${collapsed ? 'collapsed' : ''}`} aria-label="Accesso PC e approvazioni">
    <div className="pc-access-summary">
      <ShieldCheck />
      <div><span>ACCESSO PC</span><strong>{status?.label ?? 'Verifica…'}</strong></div>
      <select aria-label="Modalità accesso PC" value={mode} onChange={event => void changeMode(event.target.value as PcAccessMode)} disabled={busy}>
        <option value="read-only">Sola lettura</option>
        <option value="controlled">Controllato</option>
        <option value="sandbox-auto">Autonomia in sandbox</option>
      </select>
      <button type="button" className="icon-button" onClick={() => setCollapsed(current => !current)} aria-label={collapsed ? 'Mostra dettagli accesso PC' : 'Nascondi dettagli accesso PC'} title={collapsed ? 'Mostra dettagli' : 'Riduci a banner'}>{collapsed ? <ChevronDown /> : <ChevronUp />}</button>
    </div>
    {!collapsed && <div className="pc-access-details">
      <div><LockKeyhole /><span><strong>Confine</strong><small>{status?.sandboxScope ?? 'Verifica policy backend…'}</small></span></div>
      <div><Network /><span><strong>Rete</strong><small>{status?.network ?? 'Verifica policy di rete…'}</small></span></div>
      <div><ShieldCheck /><span><strong>Approvazioni</strong><small>{status?.approvals ?? 'Verifica revisore…'}</small></span></div>
      <p>{status?.warning}</p>
      {notice && <p className="pc-access-notice" role="status">{notice}</p>}
    </div>}
  </section>;
}
