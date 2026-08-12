import { useEffect, useState } from 'react';
import { BookOpen, Globe2, Plug } from 'lucide-react';
import {
  actInControlledBrowser, callApi, closeControlledBrowser, controlBrowser, deleteApiConnector, deleteRagDocument,
  fetchBrowserPage, getBrowserAgentStatus, getBrowserStatus, importRagDocument, listApiConnectors, listRagDocuments,
  navigateControlledBrowser, openControlledBrowser, saveApiConnector, searchRag, snapshotControlledBrowser,
  pauseBrowserAgent, quarantineBrowserDownload, startBrowserAgent,
  type ApiConnector, type BrowserAgentStatus, type BrowserPage, type BrowserSnapshot, type BrowserStatus, type RagDocument, type RagHit
} from '../../adapters/agent';

function normalizeBrowserTarget(value: string): string {
  const target = value.trim();
  if (/^https:\/\//i.test(target)) return target;
  if (/^[\w.-]+\.[a-z]{2,}(?:[/:?#].*)?$/i.test(target)) return `https://${target}`;
  return `https://duckduckgo.com/?q=${encodeURIComponent(target)}`;
}

export function KnowledgeToolsPanel({ onUseRag }: { onUseRag: (citation: string, excerpt: string) => void }) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [browserStatus, setBrowserStatus] = useState<BrowserStatus>();
  const [browserUrl, setBrowserUrl] = useState('https://example.com');
  const [browserPage, setBrowserPage] = useState<BrowserPage>();
  const [browserSnapshot, setBrowserSnapshot] = useState<BrowserSnapshot>();
  const [browserBusy, setBrowserBusy] = useState(false);
  const [browserNotice, setBrowserNotice] = useState('');
  const [browserSelector, setBrowserSelector] = useState('');
  const [browserAction, setBrowserAction] = useState<'click'|'type'|'submit'|'scroll'>('click');
  const [browserValue, setBrowserValue] = useState('');
  const [browserAgent, setBrowserAgent] = useState<BrowserAgentStatus>();
  const [browserGoal, setBrowserGoal] = useState('');
  const [browserMaxSteps, setBrowserMaxSteps] = useState(8);
  const [downloadUrl, setDownloadUrl] = useState('');
  const [apiConnectors, setApiConnectors] = useState<ApiConnector[]>([]);
  const [apiName, setApiName] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiSecret, setApiSecret] = useState('');
  const [apiAuthKind, setApiAuthKind] = useState<'none'|'bearer'|'header'|'basic'|'query'>('none');
  const [apiAuthHeader, setApiAuthHeader] = useState('Authorization');
  const [apiSelected, setApiSelected] = useState('');
  const [apiMethod, setApiMethod] = useState<'GET'|'POST'|'PUT'|'PATCH'|'DELETE'|'HEAD'|'OPTIONS'>('GET');
  const [apiPath, setApiPath] = useState('');
  const [apiBody, setApiBody] = useState('');
  const [apiOutput, setApiOutput] = useState('');
  const [ragDocuments, setRagDocuments] = useState<RagDocument[]>([]);
  const [ragPath, setRagPath] = useState('');
  const [ragQuery, setRagQuery] = useState('');
  const [ragHits, setRagHits] = useState<RagHit[]>([]);

  async function run(action: () => Promise<void>) {
    setBusy(true); setNotice('');
    try { await action(); }
    catch (error) { setNotice(error instanceof Error ? error.message : String(error)); }
    finally { setBusy(false); }
  }
  async function runBrowser(action: () => Promise<void>) {
    setBrowserBusy(true); setBrowserNotice('');
    try { await action(); }
    catch (error) { setBrowserNotice(error instanceof Error ? error.message : String(error)); }
    finally { setBrowserBusy(false); }
  }
  async function refreshBrowserStatus() { setBrowserStatus(await getBrowserStatus()); }
  async function refreshBrowserAgent() { setBrowserAgent(await getBrowserAgentStatus()); }
  async function refreshConnectors(selected?: string) {
    const connectors = await listApiConnectors();
    setApiConnectors(connectors); setApiSelected(selected ?? connectors[0]?.id ?? '');
  }
  async function refreshDocuments() { setRagDocuments(await listRagDocuments()); }

  useEffect(() => {
    void refreshBrowserStatus().catch(() => undefined);
    void refreshBrowserAgent().catch(() => undefined);
    void refreshConnectors().catch(() => undefined);
    void refreshDocuments().catch(() => undefined);
    const refreshFromChat = () => {
      void refreshBrowserStatus().catch(() => undefined);
      void refreshBrowserAgent().catch(() => undefined);
    };
    window.addEventListener('aura:browser-updated', refreshFromChat);
    return () => window.removeEventListener('aura:browser-updated', refreshFromChat);
  }, []);

  useEffect(() => {
    const stop = () => {
      setBusy(false);
      setBrowserBusy(false);
      setBrowserAgent(current => current ? { ...current, running: false, phase: 'stopped', pauseRequested: true, message: 'Arrestato dal comando globale.' } : current);
      setBrowserStatus(current => current ? { ...current, interactive: false, currentUrl: undefined } : current);
      setBrowserNotice('Task browser arrestata e finestra chiusa dal comando globale.');
    };
    window.addEventListener('aura:stop-all', stop);
    return () => window.removeEventListener('aura:stop-all', stop);
  }, []);
  useEffect(() => {
    if (!browserStatus?.interactive) return;
    const timer = window.setInterval(() => {
      void refreshBrowserStatus().catch(() => undefined);
      void refreshBrowserAgent().catch(() => undefined);
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [browserStatus?.interactive]);

  return <>
    <details className="personalization browser-panel">
      <summary><Globe2 /> Browser autonomo controllato</summary>
      <p className="section-help">Inserisci un URL HTTPS, un dominio oppure una ricerca. La finestra WebView2 resta sempre controllabile con mouse e tastiera; password, file e JavaScript arbitrario sono esclusi dall’automazione.</p>
      <div className="browser-status" data-online={browserStatus?.interactive ? 'true' : 'false'}>
        <span>{browserStatus?.interactive ? 'Finestra aperta' : 'Finestra chiusa'}</span>
        <small>{browserStatus?.currentUrl ?? browserStatus?.networkPolicy ?? 'Verifica stato…'}</small>
      </div>
      <div className="evolution-config browser-agent-control">
        <label>Obiettivo multi-step<textarea aria-label="Obiettivo browser autonomo" value={browserGoal} onChange={event => setBrowserGoal(event.target.value)} placeholder="Esempio: cerca la documentazione ufficiale di Rust e apri la pagina sull'ownership" /></label>
        <label>Limite passi<input type="number" min={1} max={20} value={browserMaxSteps} onChange={event => setBrowserMaxSteps(Math.max(1, Math.min(20, Number(event.target.value) || 1)))} /></label>
        <div className="settings-actions">
          <button type="button" disabled={browserBusy || !browserStatus?.interactive || !browserGoal.trim()} onClick={() => {
            setBrowserBusy(true); setBrowserNotice('');
            void startBrowserAgent(browserGoal.trim(), browserMaxSteps).then(status => { setBrowserAgent(status); setBrowserNotice(status.message); }).catch(error => setBrowserNotice(error instanceof Error ? error.message : String(error))).finally(() => setBrowserBusy(false));
          }}>Esegui obiettivo</button>
          <button type="button" className="danger" disabled={!browserAgent?.running} onClick={() => void pauseBrowserAgent().then(status => { setBrowserAgent(status); setBrowserNotice(status.message); }).catch(error => setBrowserNotice(String(error)))}>Prendi controllo ora</button>
        </div>
        {browserAgent && <div className="browser-status" data-online={browserAgent.running ? 'true' : 'false'}><span>{browserAgent.phase} · {browserAgent.steps}/{browserAgent.maxSteps} passi · rev. {browserAgent.pageRevision}</span><small>{browserAgent.message} · profilo {browserAgent.profileIsolated ? 'isolato' : 'condiviso'}</small></div>}
      </div>
      <label className="browser-address">URL o ricerca<input aria-label="URL o ricerca browser" value={browserUrl} onChange={event => setBrowserUrl(event.target.value)} /></label>
      <div className="settings-actions browser-actions">
        <button type="button" disabled={browserBusy || !browserUrl.trim()} onClick={() => void runBrowser(async () => { const target = normalizeBrowserTarget(browserUrl); setBrowserUrl(target); const page = await fetchBrowserPage(target); setBrowserPage(page); setBrowserNotice(`Pagina acquisita: ${page.title}`); })}>Leggi senza aprire</button>
        <button type="button" disabled={browserBusy || !browserUrl.trim()} onClick={() => void runBrowser(async () => { const target = normalizeBrowserTarget(browserUrl); setBrowserUrl(target); const opened = await openControlledBrowser(target); await refreshBrowserStatus(); setBrowserNotice(opened ? 'Browser aperto: puoi prenderne il controllo.' : 'Apertura annullata.'); })}>Apri e prendi controllo</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive || !browserUrl.trim()} onClick={() => void runBrowser(async () => { const target = normalizeBrowserTarget(browserUrl); setBrowserUrl(target); const navigated = await navigateControlledBrowser(target); await refreshBrowserStatus(); setBrowserNotice(navigated ? 'Navigazione completata.' : 'Navigazione annullata.'); })}>Naviga</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive} onClick={() => void runBrowser(async () => { const done = await controlBrowser('back'); setBrowserNotice(done ? 'Navigazione indietro inviata.' : 'Operazione annullata.'); })}>Indietro</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive} onClick={() => void runBrowser(async () => { const done = await controlBrowser('forward'); setBrowserNotice(done ? 'Navigazione avanti inviata.' : 'Operazione annullata.'); })}>Avanti</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive} onClick={() => void runBrowser(async () => { const done = await controlBrowser('reload'); setBrowserNotice(done ? 'Pagina ricaricata.' : 'Operazione annullata.'); })}>Ricarica</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive} onClick={() => void runBrowser(async () => { const snapshot = await snapshotControlledBrowser(); setBrowserSnapshot(snapshot); setBrowserPage(snapshot); setBrowserUrl(snapshot.url); setBrowserNotice(`${snapshot.elements.length} elementi interattivi rilevati.`); })}>Ispeziona finestra</button>
        <button type="button" disabled={browserBusy} onClick={() => void runBrowser(refreshBrowserStatus)}>Aggiorna stato</button>
        <button type="button" disabled={browserBusy || !browserStatus?.interactive} onClick={() => void runBrowser(async () => { const closed = await closeControlledBrowser(); await refreshBrowserStatus(); setBrowserSnapshot(undefined); setBrowserNotice(closed ? 'Browser chiuso.' : 'La finestra era già chiusa.'); })}>Chiudi</button>
      </div>
      <div className="evolution-config">
        <label>URL download<input aria-label="URL download in quarantena" value={downloadUrl} onChange={event => setDownloadUrl(event.target.value)} placeholder="https://example.com/file.zip" /></label>
        <button type="button" disabled={browserBusy || !downloadUrl.trim()} onClick={() => void runBrowser(async () => { const result = await quarantineBrowserDownload(downloadUrl.trim()); setBrowserNotice(`${result.originalName}: ${result.bytes} byte salvati in quarantena · SHA-256 ${result.sha256.slice(0, 16)}…${result.executable ? ' · contenuto eseguibile rilevato' : ''}`); })}>Scarica in quarantena</button>
      </div>
      <div className="evolution-config">
        <label>Azione DOM<select value={browserAction} onChange={event => setBrowserAction(event.target.value as typeof browserAction)}><option value="click">Click</option><option value="type">Scrivi</option><option value="submit">Invia form</option><option value="scroll">Scorri a elemento</option></select></label>
        {browserSnapshot?.elements.length ? <label>Elemento rilevato<select aria-label="Elemento browser rilevato" value={browserSelector} onChange={event => setBrowserSelector(event.target.value)}><option value="">Seleziona…</option>{browserSnapshot.elements.map((element, index) => <option key={`${element.selector}-${index}`} value={element.selector} disabled={element.disabled}>{element.label || element.kind} · {element.kind}</option>)}</select></label> : null}
        <label>Selettore CSS<input value={browserSelector} onChange={event => setBrowserSelector(event.target.value)} placeholder="button[type=submit]" /></label>
        {browserAction === 'type' && <label>Testo non sensibile<input value={browserValue} onChange={event => setBrowserValue(event.target.value)} /></label>}
        <button type="button" disabled={browserBusy || !browserStatus?.interactive || !browserSelector.trim()} onClick={() => void runBrowser(async () => { await actInControlledBrowser(browserAction, browserSelector.trim(), browserAction === 'type' ? browserValue : ''); setBrowserNotice('Azione DOM verificata; puoi intervenire manualmente nella finestra.'); })}>Esegui con conferma</button>
      </div>
      {browserPage && <article className="message assistant"><span>{browserPage.title}</span><p>{browserPage.text.slice(0, 4000)}</p><small>{browserPage.url}{browserPage.truncated ? ' · contenuto limitato' : ''}</small></article>}
      {browserNotice && <div className="voice-notice" role="status">{browserNotice}</div>}
    </details>

    <details className="personalization">
      <summary><Plug /> Connettori API REST</summary>
      <p className="section-help">HTTPS generico, sette metodi e cinque schemi di autenticazione. Credenziali cifrate, redirect bloccati e corpo escluso dall’audit.</p>
      <div className="profile-grid">
        <label>Nome<input value={apiName} onChange={event => setApiName(event.target.value)} placeholder="La mia API" /></label>
        <label>Base URL<input value={apiBaseUrl} onChange={event => setApiBaseUrl(event.target.value)} placeholder="https://api.example.com/v1/" /></label>
        <label>Autenticazione<select value={apiAuthKind} onChange={event => setApiAuthKind(event.target.value as typeof apiAuthKind)}><option value="none">Nessuna</option><option value="bearer">Bearer</option><option value="header">Header API key</option><option value="query">Query API key</option><option value="basic">Basic</option></select></label>
        {['header','query'].includes(apiAuthKind) && <label>{apiAuthKind === 'query' ? 'Parametro query' : 'Nome header'}<input value={apiAuthHeader} onChange={event => setApiAuthHeader(event.target.value)} /></label>}
        <label>Credenziale<input type="password" value={apiSecret} onChange={event => setApiSecret(event.target.value)} autoComplete="off" /></label>
      </div>
      <button type="button" disabled={busy || !apiName.trim() || !apiBaseUrl.trim()} onClick={() => void run(async () => { const id = await saveApiConnector({ name: apiName.trim(), baseUrl: apiBaseUrl.trim(), authKind: apiAuthKind, authHeader: apiAuthHeader.trim(), secret: apiSecret }); await refreshConnectors(id); setApiSecret(''); setNotice('Connettore salvato con credenziale cifrata.'); })}>Salva cifrato</button>
      {apiConnectors.length > 0 && <div className="evolution-config">
        <label>Connettore<select value={apiSelected} onChange={event => setApiSelected(event.target.value)}>{apiConnectors.map(connector => <option key={connector.id} value={connector.id}>{connector.name} · {connector.baseUrl}</option>)}</select></label>
        <label>Metodo<select value={apiMethod} onChange={event => setApiMethod(event.target.value as typeof apiMethod)}><option>GET</option><option>POST</option><option>PUT</option><option>PATCH</option><option>DELETE</option><option>HEAD</option><option>OPTIONS</option></select></label>
        <label>Percorso relativo<input value={apiPath} onChange={event => setApiPath(event.target.value)} placeholder="status" /></label>
        <label>Body JSON<textarea value={apiBody} onChange={event => setApiBody(event.target.value)} placeholder='{"campo":"valore"}' /></label>
        <button type="button" disabled={busy || !apiSelected} onClick={() => void run(async () => { const result = await callApi(apiSelected, apiMethod, apiPath, apiBody); setApiOutput(`HTTP ${result.status}\n${result.body}`); setNotice('Chiamata API completata.'); })}>{apiMethod} con conferma</button>
        <button type="button" className="danger" disabled={busy || !apiSelected} onClick={() => void run(async () => { await deleteApiConnector(apiSelected); await refreshConnectors(); })}>Elimina</button>
      </div>}
      {apiOutput && <pre className="tool-output">{apiOutput.slice(0, 8000)}</pre>}
    </details>

    <details className="personalization">
      <summary><BookOpen /> RAG documentale locale</summary>
      <p className="section-help">PDF, DOCX, testo, dati e codice con ranking ibrido qwen3-embedding. Percorso, testo ed embeddings sono cifrati.</p>
      <div className="evolution-config">
        <label>Percorso documento<input value={ragPath} onChange={event => setRagPath(event.target.value)} placeholder="C:\documenti\manuale.pdf" /></label>
        <button type="button" disabled={busy || !ragPath.trim()} onClick={() => void run(async () => { const result = await importRagDocument(ragPath.trim()); await refreshDocuments(); setNotice(`${result.sourceName}: ${result.chunkCount} frammenti cifrati indicizzati.`); })}>Importa e indicizza</button>
      </div>
      {ragDocuments.length > 0 && <div className="evolution-history">{ragDocuments.map(document => <article key={document.id}><span>{document.sourceName} · {document.chunkCount} frammenti</span><button type="button" className="danger" disabled={busy} onClick={() => void run(async () => { await deleteRagDocument(document.id); await refreshDocuments(); })}>Elimina</button></article>)}</div>}
      <div className="evolution-config"><label>Domanda<input value={ragQuery} onChange={event => setRagQuery(event.target.value)} /></label><button type="button" disabled={busy || !ragQuery.trim()} onClick={() => void run(async () => { const hits = await searchRag(ragQuery.trim(), 5); setRagHits(hits); setNotice(`${hits.length} fonti locali recuperate.`); })}>Cerca fonti</button></div>
      {ragHits.map(hit => <article key={`${hit.documentId}-${hit.ordinal}`} className="message assistant"><span>{hit.citation} · {(hit.score * 100).toFixed(1)}%</span><p>{hit.excerpt}</p><button type="button" onClick={() => onUseRag(hit.citation, hit.excerpt)}>Usa nella chat</button></article>)}
    </details>
    {notice && <div className="voice-notice">{notice}</div>}
  </>;
}
