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
  details?: {
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

export class OllamaClient {
  constructor(private readonly baseUrl = DEFAULT_BASE_URL) {}

  async health(signal?: AbortSignal): Promise<boolean> {
    try {
      const response = await fetch(`${this.baseUrl}/api/tags`, { signal });
      return response.ok;
    } catch {
      return false;
    }
  }

  async listModels(signal?: AbortSignal): Promise<OllamaModel[]> {
    const response = await fetch(`${this.baseUrl}/api/tags`, { signal });
    if (!response.ok) throw new Error(`Ollama non disponibile (${response.status})`);
    const payload = await response.json() as TagsResponse;
    return Array.isArray(payload.models) ? payload.models : [];
  }

  async chat(
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
          ...messages.filter(message => message.role !== 'system').map(({ role, content }) => ({ role, content }))
        ],
        stream: true,
        think: false,
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
