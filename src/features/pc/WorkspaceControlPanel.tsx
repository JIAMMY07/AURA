import { useEffect, useMemo, useState } from 'react';
import { Copy, File, Folder, FolderOpen, Move, RefreshCw, Save, Search, ShieldCheck, Trash2 } from 'lucide-react';
import {
  addWorkspaceGrant, copyWorkspaceItem, createWorkspaceDirectory, listWorkspaceEntries, listWorkspaceGrants,
  moveWorkspaceItem, openWorkspaceItem, readWorkspaceText, removeWorkspaceGrant, searchWorkspace,
  trashWorkspaceItem, writeWorkspaceText, type WorkspaceEntry, type WorkspaceGrant
} from '../../adapters/agent';

function formatSize(bytes: number): string {
  if (!bytes) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MiB`;
}

export function WorkspaceControlPanel() {
  const [grants, setGrants] = useState<WorkspaceGrant[]>([]);
  const [grantId, setGrantId] = useState('');
  const [grantPath, setGrantPath] = useState('');
  const [directory, setDirectory] = useState('');
  const [entries, setEntries] = useState<WorkspaceEntry[]>([]);
  const [selected, setSelected] = useState('');
  const [editorPath, setEditorPath] = useState('');
  const [editorContent, setEditorContent] = useState('');
  const [query, setQuery] = useState('');
  const [newDirectory, setNewDirectory] = useState('');
  const [destination, setDestination] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  const activeGrant = useMemo(() => grants.find(grant => grant.id === grantId), [grants, grantId]);

  async function reloadGrants(preferredId?: string) {
    const next = await listWorkspaceGrants();
    setGrants(next);
    setGrantId(preferredId && next.some(grant => grant.id === preferredId) ? preferredId : next[0]?.id ?? '');
  }

  async function reloadEntries(targetDirectory = directory, targetGrant = grantId) {
    if (!targetGrant) {
      setEntries([]);
      return;
    }
    const next = await listWorkspaceEntries(targetGrant, targetDirectory);
    setDirectory(targetDirectory);
    setEntries(next);
    setSelected('');
  }

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setNotice('');
    try { await action(); }
    catch (cause) { setNotice(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  }

  useEffect(() => { void reloadGrants().catch(() => undefined); }, []);
  useEffect(() => {
    setDirectory('');
    setEditorPath('');
    setEditorContent('');
    setDestination('');
    void reloadEntries('', grantId).catch(cause => setNotice(cause instanceof Error ? cause.message : String(cause)));
    // Il cambio workspace deve azzerare sempre la navigazione.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [grantId]);

  const authorize = () => run(async () => {
    if (!grantPath.trim()) throw new Error('Inserisci il percorso assoluto di una cartella di progetto.');
    const grant = await addWorkspaceGrant(grantPath.trim());
    if (!grant) {
      setNotice('Autorizzazione workspace annullata.');
      return;
    }
    setGrantPath('');
    await reloadGrants(grant.id);
    setNotice(`Workspace autorizzato: ${grant.name}.`);
  });

  const revoke = () => run(async () => {
    if (!grantId) return;
    const removed = await removeWorkspaceGrant(grantId);
    if (removed) {
      await reloadGrants();
      setNotice('Accesso revocato. Nessun file è stato eliminato.');
    } else setNotice('Revoca annullata.');
  });

  const selectEntry = (entry: WorkspaceEntry) => {
    setSelected(entry.relativePath);
    setDestination(entry.relativePath);
  };

  const activateEntry = (entry: WorkspaceEntry) => run(async () => {
    selectEntry(entry);
    if (entry.isDirectory) {
      await reloadEntries(entry.relativePath);
      return;
    }
    const result = await readWorkspaceText(grantId, entry.relativePath);
    setEditorPath(result.relativePath);
    setEditorContent(result.content);
    setNotice(`File caricato nell'editor · ${formatSize(result.sizeBytes)}.`);
  });

  const goParent = () => run(async () => {
    const parent = directory.split('/').slice(0, -1).join('/');
    await reloadEntries(parent);
  });

  const search = () => run(async () => {
    if (!grantId) throw new Error('Autorizza prima un workspace.');
    const results = await searchWorkspace(grantId, query);
    setEntries(results);
    setSelected('');
    setNotice(`${results.length} risultati trovati; massimo 200.`);
  });

  const save = () => run(async () => {
    if (!grantId || !editorPath.trim()) throw new Error('Indica un percorso relativo per il file.');
    const written = await writeWorkspaceText(grantId, editorPath.trim(), editorContent);
    setNotice(written ? 'File scritto con conferma e rollback atomico.' : 'Scrittura annullata.');
    if (written) await reloadEntries(directory);
  });

  const createDirectory = () => run(async () => {
    if (!grantId || !newDirectory.trim()) throw new Error('Indica il percorso relativo della nuova cartella.');
    const created = await createWorkspaceDirectory(grantId, newDirectory.trim());
    setNotice(created ? 'Cartella creata.' : 'Creazione annullata.');
    if (created) {
      setNewDirectory('');
      await reloadEntries(directory);
    }
  });

  const copyOrMove = (move: boolean) => run(async () => {
    if (!selected || !destination.trim()) throw new Error('Seleziona un elemento e indica una nuova destinazione relativa.');
    const completed = move
      ? await moveWorkspaceItem(grantId, selected, destination.trim())
      : await copyWorkspaceItem(grantId, selected, destination.trim());
    setNotice(completed ? (move ? 'Elemento spostato.' : 'File copiato.') : 'Operazione annullata.');
    if (completed) await reloadEntries(directory);
  });

  const trash = () => run(async () => {
    if (!selected) throw new Error('Seleziona prima un file o una cartella.');
    const completed = await trashWorkspaceItem(grantId, selected);
    setNotice(completed ? 'Elemento spostato nel Cestino di Windows.' : 'Eliminazione annullata.');
    if (completed) {
      setEditorPath('');
      setEditorContent('');
      await reloadEntries(directory);
    }
  });

  const open = () => run(async () => {
    if (!selected) throw new Error('Seleziona prima un elemento.');
    const opened = await openWorkspaceItem(grantId, selected);
    setNotice(opened ? 'Elemento aperto con Windows.' : 'Apertura annullata.');
  });

  return <details className="personalization workspace-control">
    <summary><FolderOpen /> Workspace, file e workflow</summary>
    <p className="section-help">Controllo esteso nello stile Codex: AURA opera soltanto nelle cartelle che autorizzi. Letture senza conferma; modifiche, spostamenti, aperture e Cestino richiedono conferma nativa Windows.</p>
    <div className="workspace-grant-row">
      <input value={grantPath} onChange={event => setGrantPath(event.target.value)} placeholder="C:\percorso\cartella-progetto" aria-label="Percorso workspace da autorizzare" />
      <button type="button" onClick={() => void authorize()} disabled={busy || !grantPath.trim()}><ShieldCheck /> Autorizza cartella</button>
    </div>
    <div className="workspace-toolbar">
      <select value={grantId} onChange={event => setGrantId(event.target.value)} aria-label="Workspace autorizzato" disabled={busy}>
        {!grants.length && <option value="">Nessun workspace autorizzato</option>}
        {grants.map(grant => <option key={grant.id} value={grant.id}>{grant.name} · {grant.path}</option>)}
      </select>
      <button type="button" className="icon-button" disabled={!grantId || busy} onClick={() => void run(() => reloadEntries())} title="Aggiorna"><RefreshCw /></button>
      <button type="button" className="danger" disabled={!grantId || busy} onClick={() => void revoke()}>Revoca</button>
    </div>
    {activeGrant && <small className="workspace-root">Radice concessa: {activeGrant.path}</small>}
    <div className="workspace-browser-grid">
      <section className="workspace-files" aria-label="File del workspace">
        <div className="workspace-pathbar">
          <button type="button" disabled={!directory || busy} onClick={() => void goParent()}>↑</button>
          <span>/{directory}</span>
        </div>
        <div className="workspace-search">
          <input value={query} onChange={event => setQuery(event.target.value)} placeholder="Cerca per nome" />
          <button type="button" className="icon-button" disabled={!grantId || query.trim().length < 2 || busy} onClick={() => void search()} title="Cerca"><Search /></button>
        </div>
        <div className="workspace-entry-list">
          {!entries.length && <p>Nessun elemento da mostrare.</p>}
          {entries.map(entry => <button
            type="button"
            key={entry.relativePath}
            className={selected === entry.relativePath ? 'chosen' : ''}
            onClick={() => selectEntry(entry)}
            onDoubleClick={() => void activateEntry(entry)}
            title="Doppio clic per aprire la cartella o caricare il testo"
          >{entry.isDirectory ? <Folder /> : <File />}<span><strong>{entry.name}</strong><small>{entry.isDirectory ? 'Cartella' : formatSize(entry.sizeBytes)}</small></span></button>)}
        </div>
      </section>
      <section className="workspace-editor" aria-label="Editor file controllato">
        <label>Percorso relativo<input value={editorPath} onChange={event => setEditorPath(event.target.value)} placeholder="src/nuovo-file.txt" disabled={!grantId} /></label>
        <textarea value={editorContent} onChange={event => setEditorContent(event.target.value)} placeholder="Contenuto UTF-8, massimo 2 MiB" disabled={!grantId} />
        <button type="button" disabled={!grantId || !editorPath.trim() || busy} onClick={() => void save()}><Save /> Salva con conferma</button>
      </section>
    </div>
    <div className="workspace-operations">
      <label>Nuova cartella<input value={newDirectory} onChange={event => setNewDirectory(event.target.value)} placeholder="cartella/sottocartella" /></label>
      <button type="button" disabled={!grantId || !newDirectory.trim() || busy} onClick={() => void createDirectory()}><Folder /> Crea</button>
      <label>Destinazione relativa<input value={destination} onChange={event => setDestination(event.target.value)} placeholder="archivio/file.txt" /></label>
      <button type="button" disabled={!selected || !destination.trim() || busy} onClick={() => void copyOrMove(false)}><Copy /> Copia</button>
      <button type="button" disabled={!selected || !destination.trim() || busy} onClick={() => void copyOrMove(true)}><Move /> Sposta</button>
      <button type="button" disabled={!selected || busy} onClick={() => void open()}>Apri</button>
      <button type="button" className="danger" disabled={!selected || busy} onClick={() => void trash()}><Trash2 /> Cestino</button>
    </div>
    <small>Programmi, script e collegamenti non vengono eseguiti da questo pannello. Per il codice resta obbligatorio il runner sandbox.</small>
    {notice && <div className="voice-notice" role="status">{notice}</div>}
  </details>;
}

