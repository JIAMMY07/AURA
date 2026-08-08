import { describe, expect, it } from 'vitest';
import type { ChatMessage } from '../adapters/ollama';
import { LocalConversationStore } from './conversationStore';

class MemoryStorage {
  private values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

function message(index: number): ChatMessage {
  return { id: String(index), role: 'user', content: `messaggio ${index}`, createdAt: new Date(index).toISOString() };
}

describe('conversation store fallback', () => {
  it('round-trips valid messages', async () => {
    const storage = new MemoryStorage();
    const store = new LocalConversationStore(storage);
    await store.save([message(1)]);
    await expect(store.load()).resolves.toEqual([message(1)]);
  });

  it('retains only the newest 100 messages', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    await store.save(Array.from({ length: 105 }, (_, index) => message(index)));
    const loaded = await store.load();
    expect(loaded).toHaveLength(100);
    expect(loaded[0].id).toBe('5');
  });

  it('clears persisted messages', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    await store.save([message(1)]);
    await store.clear();
    await expect(store.load()).resolves.toEqual([]);
  });
});
