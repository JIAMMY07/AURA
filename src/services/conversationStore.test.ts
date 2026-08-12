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

describe('conversation workspace fallback', () => {
  it('creates projects with independent conversations', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    const project = await store.createProject('Progetto demo');
    const first = await store.createConversation(project.id, 'Prima chat');
    const second = await store.createConversation(project.id, 'Seconda chat');
    await store.save(first.id, [message(1)]);
    await store.save(second.id, [message(2)]);
    await expect(store.load(first.id)).resolves.toEqual([message(1)]);
    await expect(store.load(second.id)).resolves.toEqual([message(2)]);
    await expect(store.listConversations(project.id)).resolves.toHaveLength(2);
  });

  it('retains only the newest 100 messages per chat', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    await store.save('default', Array.from({ length: 105 }, (_, index) => message(index)));
    const loaded = await store.load('default');
    expect(loaded).toHaveLength(100);
    expect(loaded[0].id).toBe('5');
  });

  it('clears only the selected conversation', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    const chat = await store.createConversation('default', 'Chat separata');
    await store.save('default', [message(1)]);
    await store.save(chat.id, [message(2)]);
    await expect(store.clear(chat.id)).resolves.toBe(true);
    await expect(store.load(chat.id)).resolves.toEqual([]);
    await expect(store.load('default')).resolves.toEqual([message(1)]);
  });

  it('deletes only the selected conversation and its messages', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    const first = await store.createConversation('default', 'Da eliminare');
    const second = await store.createConversation('default', 'Da conservare');
    await store.save(first.id, [message(1)]);
    await store.save(second.id, [message(2)]);
    await expect(store.deleteConversation(first.id)).resolves.toBe(true);
    await expect(store.load(first.id)).resolves.toEqual([]);
    await expect(store.load(second.id)).resolves.toEqual([message(2)]);
    await expect(store.listConversations('default')).resolves.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: second.id })])
    );
    await expect(store.listConversations('default')).resolves.not.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: first.id })])
    );
  });

  it('keeps an empty project valid after deleting its last conversation', async () => {
    const store = new LocalConversationStore(new MemoryStorage());
    await expect(store.deleteConversation('default')).resolves.toBe(true);
    await expect(store.listConversations('default')).resolves.toEqual([]);
    const replacement = await store.createConversation('default', 'Nuova chat');
    await expect(store.listConversations('default')).resolves.toEqual([replacement]);
  });

  it('migrates the previous single-chat browser format', async () => {
    const storage = new MemoryStorage();
    storage.setItem('aura.conversation.v1', JSON.stringify([message(7)]));
    const store = new LocalConversationStore(storage);
    await expect(store.load('default')).resolves.toEqual([message(7)]);
    await expect(store.listProjects()).resolves.toHaveLength(1);
  });
});
