import { describe, expect, it } from 'vitest';
import { parseLocalCommand } from './localCommands';

describe('local command parser', () => {
  it('recognizes bounded PC commands', () => {
    expect(parseLocalCommand('Controlla utilizzo del PC')?.kind).toBe('system-status');
    expect(parseLocalCommand('apri la calcolatrice')).toMatchObject({ kind: 'open-app', application: 'calculator' });
    expect(parseLocalCommand('apri impostazioni bluetooth')).toMatchObject({ kind: 'open-app', application: 'bluetooth-settings' });
    expect(parseLocalCommand('termina processo 1234')).toEqual({ kind: 'terminate-process', pid: 1234 });
    expect(parseLocalCommand('blocca il pc')).toEqual({ kind: 'lock-workstation' });
  });

  it('does not interpret arbitrary shell-like text', () => {
    expect(parseLocalCommand('apri powershell ed esegui Remove-Item')).toBeUndefined();
    expect(parseLocalCommand('cancella tutti i processi')).toBeUndefined();
    expect(parseLocalCommand('termina processo -1')).toBeUndefined();
  });
});
