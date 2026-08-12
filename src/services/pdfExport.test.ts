import { describe, expect, it } from 'vitest';
import { createConversationPdf } from './pdfExport';

describe('conversation PDF export', () => {
  it('creates a valid PDF with multiple messages', async () => {
    const bytes = await createConversationPdf({
      projectName: 'Progetto test',
      conversationTitle: 'Analisi AURA',
      modelLabel: 'Terra',
      messages: [
        { id: '1', role: 'user', content: 'Spiegami il progetto.', createdAt: '2026-08-09T10:00:00Z' },
        { id: '2', role: 'assistant', content: 'Ecco una risposta critica e verificabile.\nSeconda riga.', createdAt: '2026-08-09T10:01:00Z' }
      ]
    });
    expect(new TextDecoder().decode(bytes.slice(0, 8))).toMatch(/^%PDF-/);
    expect(bytes.byteLength).toBeGreaterThan(1500);
  });

  it('paginates long conversations', async () => {
    const bytes = await createConversationPdf({
      projectName: 'Test', conversationTitle: 'Chat lunga',
      messages: Array.from({ length: 45 }, (_, index) => ({
        id: String(index), role: index % 2 ? 'assistant' as const : 'user' as const,
        content: `Messaggio ${index}: ${'contenuto '.repeat(30)}`,
        createdAt: new Date(index * 1000).toISOString()
      }))
    });
    expect(bytes.byteLength).toBeGreaterThan(5000);
  });
});
