import { useEffect, useState } from 'react';
import { FlaskConical } from 'lucide-react';
import {
  approveEvolutionPatch, detectEvolutionProjectRoot, generateEvolutionPatch, getSandboxStatus, installSandboxRuntime, listEvolutionProposals,
  promoteEvolutionPatch, recordEvolutionProposal, runEvolutionSandbox,
  type EvolutionReportView, type SandboxStatus
} from '../../adapters/agent';

export function EvolutionLabPanel({ projectName }: { projectName: string }) {
  const [sandbox, setSandbox] = useState<SandboxStatus>();
  const [objective, setObjective] = useState('');
  const [notice, setNotice] = useState('');
  const [reports, setReports] = useState<EvolutionReportView[]>([]);
  const [projectPath, setProjectPath] = useState('');
  const [testProfile, setTestProfile] = useState<'Static' | 'Node' | 'Rust'>('Node');
  const [busy, setBusy] = useState(false);

  async function refresh() {
    const [nextSandbox, nextReports] = await Promise.all([getSandboxStatus(), listEvolutionProposals()]);
    setSandbox(nextSandbox); setReports(nextReports);
  }
  useEffect(() => {
    void refresh().catch(() => undefined);
    void detectEvolutionProjectRoot().then(root => { if (root) setProjectPath(root); }).catch(() => undefined);
    const refreshFromChat = () => { void refresh().catch(() => undefined); };
    const stop = () => { setBusy(false); setNotice('Task Evolution arrestata dal comando globale.'); };
    window.addEventListener('aura:evolution-updated', refreshFromChat);
    window.addEventListener('aura:stop-all', stop);
    return () => {
      window.removeEventListener('aura:evolution-updated', refreshFromChat);
      window.removeEventListener('aura:stop-all', stop);
    };
  }, []);

  async function createProposal() {
    if (!objective.trim()) return;
    setBusy(true); setNotice('');
    try {
      const proposal = await recordEvolutionProposal(projectName.trim() || 'Progetto non specificato', objective.trim());
      setNotice(`Proposta ${proposal.id} registrata senza eseguire codice. Report: ${proposal.reportPath}`);
      setReports(await listEvolutionProposals()); setObjective('');
    } catch (error) { setNotice(error instanceof Error ? error.message : String(error)); }
    finally { setBusy(false); }
  }

  async function runStage(report: EvolutionReportView, stage: 'generate' | 'approve' | 'test' | 'promote') {
    if (!projectPath.trim() || busy) return;
    setBusy(true); setNotice('');
    try {
      const operation = stage === 'generate'
        ? await generateEvolutionPatch(report.id, projectPath.trim())
        : stage === 'approve'
          ? await approveEvolutionPatch(report.id)
        : stage === 'test'
          ? await runEvolutionSandbox(report.id, projectPath.trim(), testProfile)
          : await promoteEvolutionPatch(report.id, projectPath.trim(), report.artifactPath);
      setNotice(`${operation.status}. Artefatto: ${operation.artifactPath}`);
      await refresh();
    } catch (error) { setNotice(`Fase non completata: ${error instanceof Error ? error.message : String(error)}`); }
    finally { setBusy(false); }
  }

  async function runAutonomousCycle() {
    if (!objective.trim() || !projectPath.trim() || busy) return;
    setBusy(true); setNotice('');
    try {
      const proposal = await recordEvolutionProposal(projectName.trim() || 'AURA', objective.trim());
      const generated = await generateEvolutionPatch(proposal.id, projectPath.trim());
      setNotice(`AURA Forge ha generato e analizzato il diff: ${generated.artifactPath}. Esaminalo e approvalo esplicitamente prima dei test; nessun codice è stato eseguito.`);
      setObjective('');
      await refresh();
    } catch (error) {
      setNotice(`Ciclo autonomo interrotto in sicurezza: ${error instanceof Error ? error.message : String(error)}`);
      await refresh().catch(() => undefined);
    } finally { setBusy(false); }
  }

  return <details className="personalization">
    <summary><FlaskConical /> Evolution Lab</summary>
    <p className="section-help">{sandbox?.reason ?? 'Verifica sandbox…'} L’accesso completo è concesso solo dentro un runtime isolato verificato; la promozione sul progetto reale resta manuale.</p>
    {sandbox && <div className="browser-status" data-online={sandbox.executionAllowed ? 'true' : 'false'}>
      <span>Test isolati {sandbox.executionAllowed ? 'operativi' : 'bloccati'} · canary {sandbox.canaryAvailable ? 'operativo' : 'in attesa runtime'}</span>
      <small>Attestazione v{sandbox.attestationVersion} · rollback automatico {sandbox.automaticRollback ? 'attivo' : 'non disponibile'}</small>
    </div>}
    <div className="evolution-config">
      <label>Repository Git<input value={projectPath} onChange={event => setProjectPath(event.target.value)} placeholder="C:\percorso\progetto" /></label>
      <label>Profilo test<select value={testProfile} onChange={event => setTestProfile(event.target.value as typeof testProfile)}><option>Node</option><option>Rust</option><option>Static</option></select></label>
    </div>
    <textarea value={objective} onChange={event => setObjective(event.target.value)} placeholder="Descrivi la funzione o il miglioramento da simulare…" rows={3} />
    <div className="settings-actions">
      <button type="button" onClick={() => void createProposal()} disabled={!objective.trim() || busy}>Registra proposta isolata</button>
      <button type="button" onClick={() => void runAutonomousCycle()} disabled={!objective.trim() || !projectPath.trim() || busy}>Genera patch e prepara revisione</button>
      <button type="button" disabled={busy} onClick={() => void refresh()}>Aggiorna stato sandbox</button>
      <button type="button" disabled={busy || sandbox?.executionAllowed} onClick={() => void (async () => { setBusy(true); try { const result = await installSandboxRuntime(); setNotice(result.message); await refresh(); } catch (error) { setNotice(error instanceof Error ? error.message : String(error)); } finally { setBusy(false); } })()}>Installa runtime (UAC)</button>
    </div>
    {notice && <div className="voice-notice">{notice}</div>}
    {reports.length > 0 && <div className="evolution-history">
      <strong>Proposte cifrate verificabili</strong>
      {reports.map(report => <article key={report.id}>
        <span>{report.projectName} · {report.createdAt ? new Date(Number(report.createdAt)).toLocaleString('it-IT') : 'data legacy'}</span>
        <p>{report.objective}</p>
        <small>{report.state} · tentativo {report.attempts}/3 · esecuzione {report.executionAllowed ? 'abilitata' : 'bloccata'} · rollback {report.rollbackStatus || 'non armato'}</small>
        {report.attestation && <div className="forge-inspection">
          <span className={`badge ${report.attestation.canaryPassed ? 'safe' : 'critical'}`}>canary {report.attestation.canaryPassed ? 'superato' : 'fallito'}</span>
          <small>{report.attestation.testProfile} · rete test {report.attestation.networkDuringTest ? 'attiva' : 'isolata'} · namespace canary {report.attestation.canaryFreshNamespace ? 'nuovo' : 'riutilizzato'}</small>
          <small>Attestazione SHA-256 {report.attestation.patchSha256.slice(0, 16)}…</small>
        </div>}
        {report.inspection && <div className="forge-inspection">
          <span className={`badge ${report.inspection.risk}`}>rischio {report.inspection.risk}</span>
          <small>Capability: {report.inspection.capabilities.join(', ')} · SHA-256 {report.inspection.patchSha256.slice(0, 16)}…</small>
          <small>File: {report.inspection.changedFiles.join(', ') || 'nessuno'}</small>
          {report.inspection.securityFindings.map(finding => <p key={finding} className="security-warning">{finding}</p>)}
          <details><summary>Mostra diff generato</summary><pre className="tool-output forge-diff">{report.inspection.diffExcerpt}</pre></details>
        </div>}
        <div className="settings-actions evolution-actions">
          <button type="button" disabled={busy || report.attempts >= 3 || !['proposal-only', 'awaiting-diff-approval', 'diff-approved', 'generation-failed'].includes(report.state)} onClick={() => void runStage(report, 'generate')}>{report.attempts ? '1. Rigenera patch' : '1. Genera patch'}</button>
          <button type="button" disabled={busy || report.state !== 'awaiting-diff-approval'} onClick={() => void runStage(report, 'approve')}>2. Approva diff</button>
          <button type="button" disabled={busy || !sandbox?.executionAllowed || report.state !== 'diff-approved'} onClick={() => void runStage(report, 'test')}>3. Testa in sandbox</button>
          <button type="button" disabled={busy || !sandbox?.executionAllowed || report.state !== 'canary-validated' || !report.artifactPath} onClick={() => void runStage(report, 'promote')}>4. Promuovi con rollback</button>
        </div>
      </article>)}
    </div>}
  </details>;
}
