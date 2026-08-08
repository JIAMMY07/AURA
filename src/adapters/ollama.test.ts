import { describe, expect, it } from 'vitest';
import { parseNdjsonBuffer } from './ollama';

describe('Ollama NDJSON parser', () => {
  it('parses complete lines and preserves an incomplete remainder', () => {
    const result = parseNdjsonBuffer('{"message":{"content":"Ciao"}}\n{"message":');
    expect(result.values[0].message?.content).toBe('Ciao');
    expect(result.remainder).toBe('{"message":');
  });

  it('ignores empty lines', () => {
    expect(parseNdjsonBuffer('\n\n').values).toEqual([]);
  });

  it('parses a streamed Ollama error object', () => {
    const result = parseNdjsonBuffer('{"error":"modello terminato"}\n');
    expect(result.values[0].error).toBe('modello terminato');
  });
});
