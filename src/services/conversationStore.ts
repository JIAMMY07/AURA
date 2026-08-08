import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ChatMessage } from '../adapters/ollama';

const STORAGE_KEY = 'aura.conversation.v1';
const MAX_MESSAGES = 100;

export type StorageMode = 'encrypted-windows' | 'browser-fallback' | 'memory';

export interface ConversationStore {
  readonly mode: StorageMode;
  load(): Promise<ChatMessage[]>;
  save(messages: ChatMessage[]): Promise<void>;
  clear(): Promise<void>;
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export class LocalConversationStore implements ConversationStore {
  readonly mode: StorageMode;

  constructor(private readonly storage?: StorageLike) {
    this.mode = storage ? 'browser-fallback' : 'memory';
  }

  async load(): Promise<ChatMessage[]> {
    if (!this.storage) return [];
    try {
      const raw = this.storage.getItem(STORAGE_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw) as unknown;
      if (!Array.isArray(parsed)) return [];
      return parsed.filter(isChatMessage).slice(-MAX_MESSAGES);
    } catch {
      return [];
    }
  }

  async save(messages: ChatMessage[]): Promise<void> {
    this.storage?.setItem(STORAGE_KEY, JSON.stringify(messages.filter(isChatMessage).slice(-MAX_MESSAGES)));
  }

  async clear(): Promise<void> {
    this.storage?.removeItem(STORAGE_KEY);
  }
}

class TauriConversationStore implements ConversationStore {
  readonly mode = 'encrypted-windows' as const;
  load(): Promise<ChatMessage[]> { return invoke('storage_load'); }
  save(messages: ChatMessage[]): Promise<void> { return invoke('storage_save', { messages }); }
  clear(): Promise<void> { return invoke('storage_clear'); }
}

export function createConversationStore(): ConversationStore {
  if (isTauri()) return new TauriConversationStore();
  const storage = typeof window !== 'undefined' ? window.localStorage : undefined;
  return new LocalConversationStore(storage);
}

export function isChatMessage(value: unknown): value is ChatMessage {
  if (!value || typeof value !== 'object') return false;
  const item = value as Partial<ChatMessage>;
  return typeof item.id === 'string'
    && (item.role === 'user' || item.role === 'assistant' || item.role === 'system')
    && typeof item.content === 'string'
    && typeof item.createdAt === 'string';
}
