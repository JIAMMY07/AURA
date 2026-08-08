export type ResourceProfileName = 'Leggero' | 'Bilanciato' | 'Qualità';

export interface ResourceProfile {
  name: ResourceProfileName;
  contextTokens: number;
  maxOutputTokens: number;
  temperature: number;
  keepAlive: string;
  description: string;
}

export const RESOURCE_PROFILES: Record<ResourceProfileName, ResourceProfile> = {
  Leggero: {
    name: 'Leggero',
    contextTokens: 4096,
    maxOutputTokens: 768,
    temperature: 0.3,
    keepAlive: '2m',
    description: 'Risposte rapide e minimo uso di VRAM.'
  },
  Bilanciato: {
    name: 'Bilanciato',
    contextTokens: 8192,
    maxOutputTokens: 1536,
    temperature: 0.5,
    keepAlive: '5m',
    description: 'Profilo consigliato per RTX 2060 6 GiB.'
  },
  Qualità: {
    name: 'Qualità',
    contextTokens: 12288,
    maxOutputTokens: 2048,
    temperature: 0.6,
    keepAlive: '10m',
    description: 'Più contesto, con possibile offload sulla RAM.'
  }
};

export const RECOMMENDED_MODELS = [
  { name: 'qwen3:4b-instruct', note: 'predefinito: rapido, diretto e ampio margine VRAM' },
  { name: 'qwen3:4b', note: 'variante reasoning per problemi complessi' },
  { name: 'qwen3:8b', note: 'qualità superiore, vicino al limite GPU' },
  { name: 'llama3.2:3b', note: 'fallback leggero e affidabile' }
] as const;

export function choosePreferredModel(names: string[]): string {
  for (const recommended of RECOMMENDED_MODELS) {
    const exact = names.find(name => name === recommended.name || name.startsWith(`${recommended.name}:`));
    if (exact) return exact;
  }
  return names[0] ?? '';
}
