import { isTauri } from '@tauri-apps/api/core';
import Database from '@tauri-apps/plugin-sql';
import type { ChatMessage } from '../adapters/ollama';

const STORAGE_KEY = 'aura.conversation.v1';
const MAX_MESSAGES = 100;
const DATABASE_URL = 'sqlite:aura.db';

export type StorageMode = 'sqlite' | 'browser-fallback' | 'memory';

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

class SqliteConversationStore implements ConversationStore {
  readonly mode = 'sqlite' as const;
  private database?: Database;

  private async db(): Promise<Database> {
    this.database ??= await Database.load(DATABASE_URL);
    return this.database;
  }

  async load(): Promise<ChatMessage[]> {
    const db = await this.db();
    const rows = await db.select<Array<{ id: string; role: ChatMessage['role']; content: string; created_at: string }>>(
      'SELECT id, role, content, created_at FROM messages ORDER BY sequence DESC LIMIT $1',
      [MAX_MESSAGES]
    );
    return rows.reverse().filter(row => isChatMessage({
      id: row.id,
      role: row.role,
      content: row.content,
      createdAt: row.created_at
    })).map(row => ({ id: row.id, role: row.role, content: row.content, createdAt: row.created_at }));
  }

  async save(messages: ChatMessage[]): Promise<void> {
    const db = await this.db();
    for (const message of messages.filter(isChatMessage).slice(-MAX_MESSAGES)) {
      await db.execute(
        `INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES ($1, 'default', $2, $3, $4)
         ON CONFLICT(id) DO UPDATE SET content = excluded.content`,
        [message.id, message.role, message.content, message.createdAt]
      );
    }
    await db.execute(
      `DELETE FROM messages WHERE session_id = 'default' AND id NOT IN
       (SELECT id FROM messages WHERE session_id = 'default' ORDER BY sequence DESC LIMIT $1)`,
      [MAX_MESSAGES]
    );
  }

  async clear(): Promise<void> {
    const db = await this.db();
    await db.execute("DELETE FROM messages WHERE session_id = 'default'");
  }
}

export function createConversationStore(): ConversationStore {
  if (isTauri()) return new SqliteConversationStore();
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
