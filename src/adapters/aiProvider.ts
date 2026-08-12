import type { ResourceProfile } from '../core/modelProfiles';
import {
  OllamaClient,
  type ChatMessage,
  type OllamaModel,
  type StreamResult
} from './ollama';

/** Contratto stabile della chat, indipendente dal runtime che esegue il modello. */
export interface AiProvider {
  readonly id: string;
  readonly displayName: string;
  health(signal?: AbortSignal): Promise<boolean>;
  listModels(signal?: AbortSignal): Promise<OllamaModel[]>;
  chat(
    model: string,
    messages: ChatMessage[],
    profile: ResourceProfile,
    requestId: string,
    onToken: (token: string) => void,
    signal?: AbortSignal
  ): Promise<StreamResult>;
}

/** Provider predefinito. Cambiarlo non richiede modifiche alla UI della chat. */
export function createDefaultAiProvider(): AiProvider {
  return new OllamaClient();
}

