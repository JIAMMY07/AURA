import { describe, expect, it } from 'vitest';
import { isChatCapableLocalModel, parseNdjsonBuffer, shouldUseThinking, trimMessagesForContext, type ChatMessage } from './ollama';
import { RESOURCE_PROFILES } from '../core/modelProfiles';

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

  it('keeps the newest messages inside the context budget', () => {
    const messages: ChatMessage[] = Array.from({ length: 20 }, (_, index) => ({
      id: String(index), role: 'user', content: 'x'.repeat(1000), createdAt: new Date(index).toISOString()
    }));
    const selected = trimMessagesForContext(messages, RESOURCE_PROFILES.Luna);
    expect(selected.at(-1)?.id).toBe('19');
    expect(selected.length).toBeLessThan(messages.length);
  });
});

describe('Ollama model filtering', () => {
  it('keeps local completion models and hides embedding or cloud models', () => {
    const base = {
      model: 'model',
      size: 2_000_000,
      modified_at: 'now',
      details: { format: 'gguf' }
    };

    expect(isChatCapableLocalModel({ ...base, name: 'qwen3:4b', capabilities: ['completion', 'tools'] })).toBe(true);
    expect(isChatCapableLocalModel({ ...base, name: 'qwen3-embedding:0.6b', capabilities: ['embedding'] })).toBe(false);
    expect(isChatCapableLocalModel({ ...base, name: 'gpt-oss:cloud', capabilities: ['completion'] })).toBe(false);
    expect(isChatCapableLocalModel({ ...base, name: 'granite3-guardian:2b', capabilities: ['completion'] })).toBe(false);
  });

  it('enables hidden reasoning only for the explicitly selected reasoning model', () => {
    const base = { model: 'model', size: 2_000_000, modified_at: 'now', details: { format: 'gguf' }, capabilities: ['completion', 'thinking'] };
    expect(shouldUseThinking({ ...base, name: 'qwen3:4b' })).toBe(true);
    expect(shouldUseThinking({ ...base, name: 'qwen3:4b-instruct' })).toBe(false);
    expect(shouldUseThinking({ ...base, name: 'aura:4b' })).toBe(false);
  });
});
