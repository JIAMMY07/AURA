import { describe, expect, it } from 'vitest';
import { isVoiceMutationBlocked, parseLocalCommand } from './localCommands';

describe('local command parser', () => {
  it('recognizes bounded PC commands', () => {
    expect(parseLocalCommand('Cosa sai fare?')).toEqual({ kind: 'help' });
    expect(parseLocalCommand('chi sei')).toEqual({ kind: 'identity' });
    expect(parseLocalCommand('puoi generare codice?')).toEqual({ kind: 'coding-capabilities' });
    expect(parseLocalCommand('perché il pc è lento?')).toEqual({ kind: 'performance-diagnosis' });
    expect(parseLocalCommand('controlla la sicurezza del pc')).toEqual({ kind: 'security-diagnosis' });
    expect(parseLocalCommand('Controlla utilizzo del PC')?.kind).toBe('system-status');
    expect(parseLocalCommand('apri la calcolatrice')).toMatchObject({ kind: 'open-app', application: 'calculator' });
    expect(parseLocalCommand('apri impostazioni bluetooth')).toMatchObject({ kind: 'open-app', application: 'bluetooth-settings' });
    expect(parseLocalCommand('termina processo 1234')).toEqual({ kind: 'terminate-process', pid: 1234 });
    expect(parseLocalCommand('blocca il pc')).toEqual({ kind: 'lock-workstation' });
    expect(parseLocalCommand('alza volume')).toMatchObject({ kind: 'media', action: 'volume-up' });
    expect(parseLocalCommand('apri download')).toMatchObject({ kind: 'open-app', application: 'downloads' });
    expect(parseLocalCommand('mostra i file del workspace')).toEqual({ kind: 'workspace-list', relativePath: '' });
    expect(parseLocalCommand('leggi file README.md')).toEqual({ kind: 'workspace-read', relativePath: 'README.md' });
    expect(parseLocalCommand('crea cartella docs/nuova')).toEqual({ kind: 'workspace-mkdir', relativePath: 'docs/nuova' });
    expect(parseLocalCommand('crea file note.txt con contenuto Ciao Mondo')).toEqual({ kind: 'workspace-write', relativePath: 'note.txt', content: 'Ciao Mondo' });
    expect(parseLocalCommand('cestina vecchio.txt')).toEqual({ kind: 'workspace-trash', relativePath: 'vecchio.txt' });
    expect(parseLocalCommand('vai su GitHub e apri questo link https://github.com/openai/codex')).toMatchObject({
      kind: 'browser-goal', url: 'https://github.com/openai/codex'
    });
    expect(parseLocalCommand('prendi il controllo del browser')).toEqual({ kind: 'browser-takeover' });
    expect(parseLocalCommand('fai una ricerca online Ollama Windows')).toEqual({ kind: 'web-research', query: 'Ollama Windows' });
    expect(parseLocalCommand('apri Chrome e vai su YouTube')).toMatchObject({ kind: 'browser-goal', url: 'https://www.youtube.com/' });
    expect(parseLocalCommand('naviga su YouTube')).toMatchObject({ kind: 'browser-goal', url: 'https://www.youtube.com/' });
    expect(parseLocalCommand('apri Chrome')).toEqual({ kind: 'open-installed-app', query: 'Google Chrome', label: 'Google Chrome' });
    expect(parseLocalCommand('leggi lo schermo')).toEqual({ kind: 'screen-read', prompt: 'leggi lo schermo' });
    expect(parseLocalCommand('riduci la finestra a icona')).toEqual({ kind: 'window-control', action: 'minimize', label: 'Finestra ridotta a icona' });
    expect(parseLocalCommand('chiudi applicazione attiva')).toEqual({ kind: 'window-control', action: 'close', label: 'Finestra chiusa' });
    expect(parseLocalCommand('apri applicazione Discord')).toEqual({ kind: 'open-installed-app', query: 'Discord', label: 'Discord' });
    expect(parseLocalCommand('quali sono le ultime notizie su Ollama')).toEqual({ kind: 'web-research', query: 'quali sono le ultime notizie su Ollama' });
    expect(parseLocalCommand('crea un modello 3d da "C:\\immagini\\oggetto.png" e aprilo in Blender')).toEqual({
      kind: 'creation-3d', inputImage: 'C:\\immagini\\oggetto.png', openInBlender: true
    });
    expect(parseLocalCommand('Evolution Lab: aggiungi test al parser')).toMatchObject({
      kind: 'evolution-run', objective: 'aggiungi test al parser'
    });
  });

  it('does not interpret arbitrary shell-like text', () => {
    expect(parseLocalCommand('apri powershell ed esegui Remove-Item')).toBeUndefined();
    expect(parseLocalCommand('cancella tutti i processi')).toBeUndefined();
    expect(parseLocalCommand('termina processo -1')).toBeUndefined();
  });

  it('blocks disk and code mutations from voice while allowing screen and window control', () => {
    expect(isVoiceMutationBlocked({ kind: 'workspace-trash', relativePath: 'note.txt' })).toBe(true);
    expect(isVoiceMutationBlocked({ kind: 'workspace-write', relativePath: 'note.txt', content: 'x' })).toBe(true);
    expect(isVoiceMutationBlocked({ kind: 'evolution-run', objective: 'installa modulo' })).toBe(true);
    expect(isVoiceMutationBlocked({ kind: 'screen-read', prompt: 'leggi lo schermo' })).toBe(false);
    expect(isVoiceMutationBlocked({ kind: 'window-control', action: 'close', label: 'Finestra chiusa' })).toBe(false);
    expect(isVoiceMutationBlocked({ kind: 'open-installed-app', query: 'Discord', label: 'Discord' })).toBe(false);
  });
});
