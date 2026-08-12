import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ChatMessage } from '../adapters/ollama';

const LEGACY_STORAGE_KEY = 'aura.conversation.v1';
const WORKSPACE_STORAGE_KEY = 'aura.workspace.v2';
const MAX_MESSAGES = 100;

export type StorageMode = 'encrypted-windows' | 'browser-fallback' | 'memory';

export interface ProjectSummary {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationSummary {
  id: string;
  projectId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationStore {
  readonly mode: StorageMode;
  listProjects(): Promise<ProjectSummary[]>;
  createProject(name: string): Promise<ProjectSummary>;
  listConversations(projectId: string): Promise<ConversationSummary[]>;
  createConversation(projectId: string, title: string): Promise<ConversationSummary>;
  renameConversation(sessionId: string, title: string): Promise<boolean>;
  deleteConversation(sessionId: string): Promise<boolean>;
  load(sessionId: string): Promise<ChatMessage[]>;
  save(sessionId: string, messages: ChatMessage[]): Promise<void>;
  clear(sessionId: string): Promise<boolean>;
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

interface LocalWorkspace {
  projects: ProjectSummary[];
  conversations: ConversationSummary[];
  messages: Record<string, ChatMessage[]>;
}

function now(): string { return new Date().toISOString(); }

function initialWorkspace(legacyMessages: ChatMessage[] = []): LocalWorkspace {
  const createdAt = now();
  return {
    projects: [{ id: 'default', name: 'Progetto principale', createdAt, updatedAt: createdAt }],
    conversations: [{ id: 'default', projectId: 'default', title: 'Conversazione principale', createdAt, updatedAt: createdAt }],
    messages: { default: legacyMessages.filter(isChatMessage).slice(-MAX_MESSAGES) }
  };
}

function isProject(value: unknown): value is ProjectSummary {
  if (!value || typeof value !== 'object') return false;
  const item = value as Partial<ProjectSummary>;
  return typeof item.id === 'string' && typeof item.name === 'string'
    && typeof item.createdAt === 'string' && typeof item.updatedAt === 'string';
}

function isConversation(value: unknown): value is ConversationSummary {
  if (!value || typeof value !== 'object') return false;
  const item = value as Partial<ConversationSummary>;
  return typeof item.id === 'string' && typeof item.projectId === 'string' && typeof item.title === 'string'
    && typeof item.createdAt === 'string' && typeof item.updatedAt === 'string';
}

export class LocalConversationStore implements ConversationStore {
  readonly mode: StorageMode;
  private memory = initialWorkspace();

  constructor(private readonly storage?: StorageLike) {
    this.mode = storage ? 'browser-fallback' : 'memory';
  }

  private read(): LocalWorkspace {
    if (!this.storage) return this.memory;
    try {
      const raw = this.storage.getItem(WORKSPACE_STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<LocalWorkspace>;
        const projects = Array.isArray(parsed.projects) ? parsed.projects.filter(isProject) : [];
        const conversations = Array.isArray(parsed.conversations) ? parsed.conversations.filter(isConversation) : [];
        if (projects.length) {
          const messages = typeof parsed.messages === 'object' && parsed.messages ? parsed.messages : {};
          return { projects, conversations, messages: messages as Record<string, ChatMessage[]> };
        }
      }
      const legacyRaw = this.storage.getItem(LEGACY_STORAGE_KEY);
      const legacy = legacyRaw ? JSON.parse(legacyRaw) as unknown : [];
      const migrated = initialWorkspace(Array.isArray(legacy) ? legacy.filter(isChatMessage) : []);
      this.write(migrated);
      this.storage.removeItem(LEGACY_STORAGE_KEY);
      return migrated;
    } catch {
      return initialWorkspace();
    }
  }

  private write(workspace: LocalWorkspace) {
    if (this.storage) this.storage.setItem(WORKSPACE_STORAGE_KEY, JSON.stringify(workspace));
    else this.memory = workspace;
  }

  async listProjects(): Promise<ProjectSummary[]> {
    return [...this.read().projects].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  async createProject(name: string): Promise<ProjectSummary> {
    const workspace = this.read();
    const createdAt = now();
    const project = { id: crypto.randomUUID(), name: name.trim(), createdAt, updatedAt: createdAt };
    workspace.projects.push(project);
    this.write(workspace);
    return project;
  }

  async listConversations(projectId: string): Promise<ConversationSummary[]> {
    return this.read().conversations.filter(chat => chat.projectId === projectId)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  async createConversation(projectId: string, title: string): Promise<ConversationSummary> {
    const workspace = this.read();
    if (!workspace.projects.some(project => project.id === projectId)) throw new Error('Progetto non trovato.');
    const createdAt = now();
    const conversation = { id: crypto.randomUUID(), projectId, title: title.trim(), createdAt, updatedAt: createdAt };
    workspace.conversations.push(conversation);
    workspace.messages[conversation.id] = [];
    const project = workspace.projects.find(item => item.id === projectId);
    if (project) project.updatedAt = createdAt;
    this.write(workspace);
    return conversation;
  }

  async renameConversation(sessionId: string, title: string): Promise<boolean> {
    const workspace = this.read();
    const conversation = workspace.conversations.find(item => item.id === sessionId);
    if (!conversation) return false;
    conversation.title = title.trim();
    conversation.updatedAt = now();
    this.write(workspace);
    return true;
  }

  async deleteConversation(sessionId: string): Promise<boolean> {
    const workspace = this.read();
    const conversation = workspace.conversations.find(item => item.id === sessionId);
    if (!conversation) return false;
    if (typeof globalThis.confirm === 'function'
      && !globalThis.confirm('Cancellare definitivamente questa chat e tutti i suoi messaggi?')) return false;
    workspace.conversations = workspace.conversations.filter(item => item.id !== sessionId);
    delete workspace.messages[sessionId];
    const project = workspace.projects.find(item => item.id === conversation.projectId);
    if (project) project.updatedAt = now();
    this.write(workspace);
    return true;
  }

  async load(sessionId: string): Promise<ChatMessage[]> {
    const messages = this.read().messages[sessionId];
    return Array.isArray(messages) ? messages.filter(isChatMessage).slice(-MAX_MESSAGES) : [];
  }

  async save(sessionId: string, messages: ChatMessage[]): Promise<void> {
    const workspace = this.read();
    workspace.messages[sessionId] = messages.filter(isChatMessage).slice(-MAX_MESSAGES);
    const timestamp = now();
    const conversation = workspace.conversations.find(item => item.id === sessionId);
    if (conversation) {
      conversation.updatedAt = timestamp;
      const project = workspace.projects.find(item => item.id === conversation.projectId);
      if (project) project.updatedAt = timestamp;
    }
    this.write(workspace);
  }

  async clear(sessionId: string): Promise<boolean> {
    const workspace = this.read();
    workspace.messages[sessionId] = [];
    this.write(workspace);
    return true;
  }
}

class TauriConversationStore implements ConversationStore {
  readonly mode = 'encrypted-windows' as const;
  listProjects(): Promise<ProjectSummary[]> { return invoke('storage_list_projects'); }
  createProject(name: string): Promise<ProjectSummary> { return invoke('storage_create_project', { name }); }
  listConversations(projectId: string): Promise<ConversationSummary[]> { return invoke('storage_list_conversations', { projectId }); }
  createConversation(projectId: string, title: string): Promise<ConversationSummary> {
    return invoke('storage_create_conversation', { projectId, title });
  }
  renameConversation(sessionId: string, title: string): Promise<boolean> {
    return invoke('storage_rename_conversation', { sessionId, title });
  }
  deleteConversation(sessionId: string): Promise<boolean> {
    return invoke('storage_delete_conversation', { sessionId });
  }
  load(sessionId: string): Promise<ChatMessage[]> { return invoke('storage_load', { sessionId }); }
  save(sessionId: string, messages: ChatMessage[]): Promise<void> { return invoke('storage_save', { sessionId, messages }); }
  clear(sessionId: string): Promise<boolean> { return invoke('storage_clear', { sessionId }); }
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
