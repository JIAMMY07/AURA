import type { AllowedApplication } from '../adapters/agent';

export type LocalCommand =
  | { kind: 'system-status' }
  | { kind: 'process-list' }
  | { kind: 'open-app'; application: AllowedApplication; label: string }
  | { kind: 'terminate-process'; pid: number }
  | { kind: 'lock-workstation' }
  | { kind: 'sandbox-status' };

export function parseLocalCommand(input: string): LocalCommand | undefined {
  const normalized = input
    .toLocaleLowerCase('it')
    .normalize('NFKC')
    .replace(/[.!?]+$/g, '')
    .replace(/\s+/g, ' ')
    .trim();
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
  if (['/sandbox stato', 'stato della sandbox', 'controlla la sandbox'].includes(normalized)) {
    return { kind: 'sandbox-status' };
  }
  return undefined;
}
