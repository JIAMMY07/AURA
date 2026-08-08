import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { ResourceProfile } from '../core/modelProfiles';

export type ChatRole = 'system' | 'user' | 'assistant';

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  createdAt: string;
}

export interface OllamaModel {
  name: string;
  model: string;
  size: number;
  modified_at: string;
  capabilities?: string[];
  details?: {
    format?: string;
    family?: string;
    parameter_size?: string;
    quantization_level?: string;
  };
}

interface TagsResponse { models: OllamaModel[] }
interface ChatChunk {
  message?: { role?: string; content?: string };
  done?: boolean;
  done_reason?: string;
  eval_count?: number;
  eval_duration?: number;
  error?: string;
}

interface BackendStreamEvent {
  event: 'token';
  data: { token: string };
}

export interface StreamResult {
  text: string;
  tokensPerSecond?: number;
  doneReason?: string;
}

const DEFAULT_BASE_URL = 'http://127.0.0.1:11434';
const AURA_SYSTEM_PROMPT = 'Sei AURA, un assistente Windows local-first. Rispondi nella lingua dell’utente in modo chiaro e conciso. Non affermare di aver eseguito azioni sul PC se non hai ricevuto un risultato verificato da uno strumento. Non mostrare ragionamenti interni.';

export function parseNdjsonBuffer(buffer: string): { values: ChatChunk[]; remainder: string } {
  const lines = buffer.split('\n');
  const remainder = lines.pop() ?? '';
  const values = lines
    .map(line => line.trim())
    .filter(Boolean)
    .map(line => JSON.parse(line) as ChatChunk);
  return { values, remainder };
}

export function isChatCapableLocalModel(model: OllamaModel): boolean {
  const lower = model.name.toLowerCase();
  return model.size >= 1024 * 1024
    && model.details?.format?.toLowerCase() === 'gguf'
    && (model.capabilities ?? []).some(capability => capability.toLowerCase() === 'completion')
    && !lower.endsWith('-cloud')
    && !lower.endsWith(':cloud')
    && !lower.includes('-cloud:')
    && !lower.includes('guardian')
    && !lower.includes('shieldgemma');
}

export function trimMessagesForContext(messages: ChatMessage[], profile: ResourceProfile): ChatMessage[] {
  const reservedTokens = profile.maxOutputTokens + 512;
  const characterBudget = Math.max(1024, (profile.contextTokens - reservedTokens) * 4);
  const selected: ChatMessage[] = [];
  let used = 0;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    const cost = message.content.length + 32;
    if (selected.length && used + cost > characterBudget) break;
    if (!selected.length && cost > characterBudget) {
      selected.push({ ...message, content: message.content.slice(-characterBudget) });
      break;
    }
    selected.push(message);
    used += cost;
  }
  return selected.reverse();
}

export class OllamaClient {
  private readonly thinkingModels = new Set<string>();

  constructor(private readonly baseUrl = DEFAULT_BASE_URL) {}

  async health(signal?: AbortSignal): Promise<boolean> {
    if (isTauri()) return invoke<boolean>('ollama_health');
    try {
      const response = await fetch(`${this.baseUrl}/api/tags`, { signal });
      return response.ok;
    } catch {
      return false;
    }
  }

  async listModels(signal?: AbortSignal): Promise<OllamaModel[]> {
    if (isTauri()) return invoke<OllamaModel[]>('ollama_list_models');
    const response = await fetch(`${this.baseUrl}/api/tags`, { signal });
    if (!response.ok) throw new Error(`Ollama non disponibile (${response.status})`);
    const payload = await response.json() as TagsResponse;
    const models = Array.isArray(payload.models) ? payload.models.filter(isChatCapableLocalModel) : [];
    this.thinkingModels.clear();
    for (const model of models) {
      if ((model.capabilities ?? []).some(capability => capability.toLowerCase() === 'thinking')) {
        this.thinkingModels.add(model.name);
      }
    }
    return models;
  }

  async chat(
    model: string,
    messages: ChatMessage[],
    profile: ResourceProfile,
    requestId: string,
    onToken: (token: string) => void,
    signal?: AbortSignal
  ): Promise<StreamResult> {
    if (isTauri()) {
      return this.chatThroughBackend(model, messages, profile, requestId, onToken, signal);
    }
    return this.chatThroughWeb(model, messages, profile, onToken, signal);
  }

  private async chatThroughBackend(
    model: string,
    messages: ChatMessage[],
    profile: ResourceProfile,
    requestId: string,
    onToken: (token: string) => void,
    signal?: AbortSignal
  ): Promise<StreamResult> {
    const onEvent = new Channel<BackendStreamEvent>();
    let text = '';
    onEvent.onmessage = message => {
      if (message.event === 'token' && message.data.token) {
        text += message.data.token;
        onToken(message.data.token);
      }
    };
    const cancel = () => { void invoke('ollama_cancel', { requestId }); };
    signal?.addEventListener('abort', cancel, { once: true });
    try {
      const result = await invoke<{ tokensPerSecond?: number; doneReason?: string }>('ollama_chat', {
        request: {
          requestId,
          model,
          messages: trimMessagesForContext(messages, profile).map(({ role, content }) => ({ role, content })),
          profile: profile.name
        },
        onEvent
      });
      if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
      return { text, ...result };
    } catch (cause) {
      if (signal?.aborted || cause === 'Generazione annullata.') {
        throw new DOMException('Aborted', 'AbortError');
      }
      throw cause instanceof Error ? cause : new Error(String(cause));
    } finally {
      signal?.removeEventListener('abort', cancel);
    }
  }

  private async chatThroughWeb(
    model: string,
    messages: ChatMessage[],
    profile: ResourceProfile,
    onToken: (token: string) => void,
    signal?: AbortSignal
  ): Promise<StreamResult> {
    const response = await fetch(`${this.baseUrl}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model,
        messages: [
          { role: 'system', content: AURA_SYSTEM_PROMPT },
          ...trimMessagesForContext(messages, profile)
            .filter(message => message.role !== 'system')
            .map(({ role, content }) => ({ role, content }))
        ],
        stream: true,
        think: this.thinkingModels.has(model),
        keep_alive: profile.keepAlive,
        options: {
          num_ctx: profile.contextTokens,
          num_predict: profile.maxOutputTokens,
          temperature: profile.temperature
        }
      }),
      signal
    });

    if (!response.ok) throw new Error(`Richiesta Ollama fallita (${response.status})`);
    if (!response.body) throw new Error('Ollama non ha restituito uno stream.');

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let text = '';
    let finalChunk: ChatChunk | undefined;

    const consume = (chunk: ChatChunk) => {
      if (chunk.error) throw new Error(`Ollama: ${chunk.error}`);
      finalChunk = chunk;
      const token = chunk.message?.content ?? '';
      if (token) {
        text += token;
        onToken(token);
      }
    };

    while (true) {
      const { value, done } = await reader.read();
      buffer += decoder.decode(value, { stream: !done });
      const parsed = parseNdjsonBuffer(buffer);
      buffer = parsed.remainder;
      parsed.values.forEach(consume);
      if (done) break;
    }

    if (buffer.trim()) consume(JSON.parse(buffer) as ChatChunk);
    const durationSeconds = finalChunk?.eval_duration ? finalChunk.eval_duration / 1_000_000_000 : 0;
    return {
      text,
      doneReason: finalChunk?.done_reason,
      tokensPerSecond: durationSeconds && finalChunk?.eval_count
        ? finalChunk.eval_count / durationSeconds
        : undefined
    };
  }
}
