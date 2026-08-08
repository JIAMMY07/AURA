export type LocalCommand =
  | { kind: 'system-status' }
  | { kind: 'process-list' }
  | { kind: 'open-app'; application: 'calculator' | 'notepad' | 'explorer'; label: string }
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
  if (['/sandbox stato', 'stato della sandbox', 'controlla la sandbox'].includes(normalized)) {
    return { kind: 'sandbox-status' };
  }
  return undefined;
}
