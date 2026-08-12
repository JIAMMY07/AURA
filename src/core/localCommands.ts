import type { AllowedApplication, MediaAction } from '../adapters/agent';

export type LocalCommand =
  | { kind: 'help' }
  | { kind: 'identity' }
  | { kind: 'coding-capabilities' }
  | { kind: 'performance-diagnosis' }
  | { kind: 'security-diagnosis' }
  | { kind: 'system-status' }
  | { kind: 'process-list' }
  | { kind: 'open-app'; application: AllowedApplication; label: string }
  | { kind: 'open-installed-app'; query: string; label: string }
  | { kind: 'screen-read'; prompt: string }
  | { kind: 'window-control'; action: 'minimize' | 'maximize-restore' | 'close'; label: string }
  | { kind: 'terminate-process'; pid: number }
  | { kind: 'lock-workstation' }
  | { kind: 'media'; action: MediaAction; label: string }
  | { kind: 'workspace-list'; relativePath: string }
  | { kind: 'workspace-read'; relativePath: string }
  | { kind: 'workspace-write'; relativePath: string; content: string }
  | { kind: 'workspace-mkdir'; relativePath: string }
  | { kind: 'workspace-open'; relativePath: string }
  | { kind: 'workspace-trash'; relativePath: string }
  | { kind: 'browser-goal'; url: string; goal: string; autonomous: boolean }
  | { kind: 'web-research'; query: string }
  | { kind: 'browser-takeover' }
  | { kind: 'browser-close' }
  | { kind: 'creation-3d'; inputImage: string; openInBlender: boolean }
  | { kind: 'evolution-run'; objective: string; projectPath?: string }
  | { kind: 'sandbox-status' };

export function isVoiceMutationBlocked(command: LocalCommand): boolean {
  return ['workspace-write', 'workspace-mkdir', 'workspace-trash', 'evolution-run', 'creation-3d'].includes(command.kind);
}

export function parseLocalCommand(input: string): LocalCommand | undefined {
  const normalized = input
    .toLocaleLowerCase('it')
    .normalize('NFKC')
    .replace(/[.!?]+$/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  if (['cosa sai fare', 'che cosa sai fare', 'quali funzioni hai', 'mostra le funzioni', 'mostrami le funzioni', 'mostra comandi', 'comandi', 'aiuto', 'help', '/help'].includes(normalized)) {
    return { kind: 'help' };
  }
  if (['chi sei', 'come ti chiami', 'ciao aura', 'presentati'].includes(normalized)) {
    return { kind: 'identity' };
  }
  if (['sai programmare', 'puoi scrivere codice', 'puoi generare codice', 'puoi eseguire codice', 'che codice sai scrivere'].includes(normalized)) {
    return { kind: 'coding-capabilities' };
  }
  if (['perché il pc è lento', 'perche il pc e lento', 'analizza le prestazioni del pc', 'analizza prestazioni pc', 'cosa rallenta il pc', 'diagnostica prestazioni', 'ottimizza il pc'].includes(normalized)) {
    return { kind: 'performance-diagnosis' };
  }
  if (['controlla la sicurezza del pc', 'controlla sicurezza pc', 'il pc è sicuro', 'il pc e sicuro', 'verifica defender', 'diagnostica sicurezza'].includes(normalized)) {
    return { kind: 'security-diagnosis' };
  }
  if (['/pc stato', 'stato del pc', 'controlla utilizzo del pc', 'controlla lo stato del pc'].includes(normalized)) {
    return { kind: 'system-status' };
  }
  if (['/pc processi', 'mostra i processi', 'elenca i processi'].includes(normalized)) {
    return { kind: 'process-list' };
  }
  if (['/apri calcolatrice', 'apri calcolatrice', 'apri la calcolatrice'].includes(normalized)) {
    return { kind: 'open-app', application: 'calculator', label: 'Calcolatrice' };
  }
  if (['/apri blocco note', 'apri blocco note', 'apri il blocco note'].includes(normalized)) {
    return { kind: 'open-app', application: 'notepad', label: 'Blocco note' };
  }
  if (['/apri esplora file', 'apri esplora file'].includes(normalized)) {
    return { kind: 'open-app', application: 'explorer', label: 'Esplora file' };
  }
  const applications: Array<[string[], AllowedApplication, string]> = [
    [['apri gestione attivita', 'apri gestione attività', '/apri gestione attivita'], 'task-manager', 'Gestione attivita'],
    [['apri paint', '/apri paint'], 'paint', 'Paint'],
    [['apri strumento di cattura', '/apri cattura'], 'snipping-tool', 'Strumento di cattura'],
    [['apri impostazioni', '/apri impostazioni'], 'settings', 'Impostazioni Windows'],
    [['apri impostazioni bluetooth', '/apri bluetooth'], 'bluetooth-settings', 'Impostazioni Bluetooth'],
    [['apri impostazioni audio', '/apri audio'], 'sound-settings', 'Impostazioni audio'],
    [['apri impostazioni schermo', '/apri schermo'], 'display-settings', 'Impostazioni schermo'],
    [['apri impostazioni notifiche', '/apri notifiche'], 'notification-settings', 'Impostazioni notifiche'],
    [['apri privacy microfono', '/apri privacy microfono'], 'microphone-privacy', 'Privacy microfono'],
    [['apri windows update', '/apri aggiornamenti'], 'windows-update', 'Windows Update'],
    [['apri download', '/apri download'], 'downloads', 'Download'],
    [['apri documenti', '/apri documenti'], 'documents', 'Documenti'],
    [['apri sicurezza windows', '/apri sicurezza'], 'security-settings', 'Sicurezza di Windows']
  ];
  for (const [phrases, application, label] of applications) {
    if (phrases.includes(normalized)) return { kind: 'open-app', application, label };
  }
  const terminate = normalized.match(/^(?:termina processo|chiudi processo|\/pc termina)\s+(\d{1,10})$/);
  if (terminate) {
    const pid = Number(terminate[1]);
    if (Number.isSafeInteger(pid) && pid > 0 && pid <= 0xffff_ffff) return { kind: 'terminate-process', pid };
  }
  if (['blocca il pc', 'blocca pc', '/pc blocca'].includes(normalized)) return { kind: 'lock-workstation' };
  const media: Array<[string[], MediaAction, string]> = [
    [['alza volume', 'aumenta volume', '/media volume su'], 'volume-up', 'Volume aumentato'],
    [['abbassa volume', 'diminuisci volume', '/media volume giu'], 'volume-down', 'Volume diminuito'],
    [['muto', 'silenzia audio', 'attiva muto', '/media muto'], 'mute', 'Muto commutato'],
    [['pausa', 'riprendi musica', 'play pausa', '/media pausa'], 'play-pause', 'Riproduzione commutata'],
    [['traccia successiva', '/media avanti'], 'next', 'Traccia successiva'],
    [['traccia precedente', '/media indietro'], 'previous', 'Traccia precedente']
  ];
  for (const [phrases, action, label] of media) {
    if (phrases.includes(normalized)) return { kind: 'media', action, label };
  }
  if (['/sandbox stato', 'stato della sandbox', 'controlla la sandbox'].includes(normalized)) {
    return { kind: 'sandbox-status' };
  }
  const raw = input.normalize('NFKC').replace(/[.!?]+$/g, '').trim();
  if (/^(?:leggi|analizza|descrivi|guarda)(?: il| lo| questo| cosa c'è sul)? schermo(?: e (.+))?$/i.test(raw)
    || /^(?:cosa (?:c'è|vedi)|che cosa (?:c'è|vedi)) (?:sullo|nello) schermo$/i.test(raw)) {
    return { kind: 'screen-read', prompt: raw.slice(0, 2_000) };
  }
  if (/^(?:riduci|minimizza)(?: a icona)?(?: la| questa)? (?:finestra|applicazione|app|pagina)(?: attiva)?(?: a icona)?$/i.test(raw)) {
    return { kind: 'window-control', action: 'minimize', label: 'Finestra ridotta a icona' };
  }
  if (/^(?:massimizza|ingrandisci|ripristina)(?: la| questa)? (?:finestra|applicazione|app|pagina)(?: attiva)?$/i.test(raw)) {
    return { kind: 'window-control', action: 'maximize-restore', label: 'Finestra massimizzata o ripristinata' };
  }
  if (/^chiudi(?: la| questa)? (?:finestra|applicazione|app|pagina)(?: attiva)?$/i.test(raw)) {
    return { kind: 'window-control', action: 'close', label: 'Finestra chiusa' };
  }
  if (/^(?:prendi|riprendi|passa a me)(?: il)? controllo(?: del)? browser$/i.test(raw)) {
    return { kind: 'browser-takeover' };
  }
  if (/^(?:chiudi|arresta)(?: il)? browser(?: controllato)?$/i.test(raw)) {
    return { kind: 'browser-close' };
  }
  const explicitUrl = raw.match(/https:\/\/[^\s<>"']+/i)?.[0]?.replace(/[),.;]+$/g, '');
  const bareUrl = raw.match(/(?:^|\s)((?:www\.)?[a-z0-9.-]+\.[a-z]{2,}(?:\/[^\s<>"']*)?)/i)?.[1]?.replace(/[),.;]+$/g, '');
  if ((explicitUrl || bareUrl) && /(?:browser|vai su|apri|naviga|visita|cerca|github|link)/i.test(raw)) {
    const url = explicitUrl ?? `https://${bareUrl}`;
    const autonomous = /(?:cerca|trova|analizza|leggi|apri (?:la |il )?pagina|vai (?:a|su)|esegui|autonom)/i.test(raw.replace(url, ''));
    return { kind: 'browser-goal', url, goal: raw.slice(0, 2_000), autonomous };
  }
  const knownSites: Array<[RegExp, string]> = [
    [/\byoutube\b/i, 'https://www.youtube.com/'],
    [/\bgithub\b/i, 'https://github.com/'],
    [/\bgoogle\b/i, 'https://www.google.com/'],
    [/\bwikipedia\b/i, 'https://it.wikipedia.org/'],
    [/\breddit\b/i, 'https://www.reddit.com/']
  ];
  if (/(?:apri|vai|naviga|visita|portami)(?:\s+(?:con|nel|su|a|al|il|la|lo|browser|chrome)){0,5}\s/i.test(raw)) {
    const destination = knownSites.find(([pattern]) => pattern.test(raw));
    if (destination) return { kind: 'browser-goal', url: destination[1], goal: raw.slice(0, 2_000), autonomous: /(?:cerca|trova|analizza|guarda|riproduci)/i.test(raw) };
  }
  const installedBrowser = raw.match(/^(?:apri|avvia)(?: il)?\s+(google chrome|chrome|microsoft edge|edge|mozilla firefox|firefox|brave)(?: browser)?$/i);
  if (installedBrowser) {
    const query = installedBrowser[1].toLowerCase().includes('chrome') ? 'Google Chrome'
      : installedBrowser[1].toLowerCase().includes('edge') ? 'Microsoft Edge'
      : installedBrowser[1].toLowerCase().includes('firefox') ? 'Mozilla Firefox' : 'Brave';
    return { kind: 'open-installed-app', query, label: query };
  }
  const installedApplication = raw.match(/^(?:apri|avvia) (?:l['’]applicazione|applicazione|il programma|programma|l['’]app|app)\s+(.{2,80})$/i);
  if (installedApplication) {
    const query = installedApplication[1].trim();
    return { kind: 'open-installed-app', query, label: query };
  }
  const webSearch = raw.match(/^(?:cerca|trova|fai una ricerca|verifica|controlla)(?: su internet| sul web| online)(?: in tempo reale)?\s+(.+)$/i)
    ?? raw.match(/^(?:cerca|trova|fai una ricerca)(?: su internet| sul web| online)?\s+(.+)$/i);
  if (webSearch) {
    return { kind: 'web-research', query: webSearch[1].trim().slice(0, 500) };
  }
  if (/(?:ultime notizie|notizie di oggi|prezzo attuale|meteo (?:di|a|per)|versione (?:più )?recente|attualmente chi|in tempo reale)/i.test(raw)) {
    return { kind: 'web-research', query: raw.slice(0, 500) };
  }
  if (/^(?:crea|genera|costruisci|realizza).{0,30}(?:file|modello|mesh|oggetto)?\s*3d\b/i.test(raw)) {
    const image = raw.match(/"([^"]+\.(?:png|jpe?g|webp|bmp))"/i)?.[1]
      ?? raw.match(/([A-Za-z]:\\[^\r\n]+?\.(?:png|jpe?g|webp|bmp))/i)?.[1]
      ?? '';
    return { kind: 'creation-3d', inputImage: image, openInBlender: /(?:apri|mostra).{0,20}blender/i.test(raw) };
  }
  const evolution = raw.match(/^(?:\/evolution|usa evolution(?: lab)?(?: per)?|evolution(?: lab)?\s*[:\-]?|migliora (?:il )?(?:codice|progetto|repository)|modifica (?:il )?(?:codice|progetto|repository)|correggi (?:il )?(?:codice|progetto|repository))\s+(.+)$/i);
  if (evolution) {
    const quotedProject = evolution[1].match(/(?:repo|repository|progetto)\s+"([A-Za-z]:\\[^"]+)"/i)?.[1];
    return { kind: 'evolution-run', objective: evolution[1].slice(0, 4_000), ...(quotedProject ? { projectPath: quotedProject } : {}) };
  }
  if (/^(?:mostra|elenca) (?:i )?file(?: del workspace)?$/i.test(raw)) {
    return { kind: 'workspace-list', relativePath: '' };
  }
  const listPath = raw.match(/^(?:mostra|elenca) (?:i )?file (?:in|dentro)\s+(.+)$/i);
  if (listPath) return { kind: 'workspace-list', relativePath: listPath[1].trim() };
  const read = raw.match(/^leggi file\s+(.+)$/i);
  if (read) return { kind: 'workspace-read', relativePath: read[1].trim() };
  const write = raw.match(/^crea file\s+(.+?)\s+con contenuto\s+([\s\S]+)$/i);
  if (write) return { kind: 'workspace-write', relativePath: write[1].trim(), content: write[2] };
  const mkdir = raw.match(/^crea cartella\s+(.+)$/i);
  if (mkdir) return { kind: 'workspace-mkdir', relativePath: mkdir[1].trim() };
  const openFile = raw.match(/^apri (?:file|cartella)\s+(.+)$/i);
  if (openFile) return { kind: 'workspace-open', relativePath: openFile[1].trim() };
  const trash = raw.match(/^(?:sposta nel cestino|cestina)\s+(.+)$/i);
  if (trash) return { kind: 'workspace-trash', relativePath: trash[1].trim() };
  return undefined;
}
