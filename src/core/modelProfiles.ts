export type ResourceProfileName = 'Luna' | 'Terra' | 'Sole';

export interface ResourceProfile {
  name: ResourceProfileName;
  contextTokens: number;
  maxOutputTokens: number;
  temperature: number;
  keepAlive: string;
  speed: 'Massima' | 'Bilanciata' | 'Ridotta';
  power: 'Essenziale' | 'Elevata' | 'Massima';
  description: string;
}

export const RESOURCE_PROFILES: Record<ResourceProfileName, ResourceProfile> = {
  Luna: {
    name: 'Luna',
    contextTokens: 4096,
    maxOutputTokens: 768,
    temperature: 0.3,
    keepAlive: '2m',
    speed: 'Massima',
    power: 'Essenziale',
    description: 'Più veloce e leggera: ideale per comandi, domande brevi e attività quotidiane.'
  },
  Terra: {
    name: 'Terra',
    contextTokens: 8192,
    maxOutputTokens: 1536,
    temperature: 0.5,
    keepAlive: '5m',
    speed: 'Bilanciata',
    power: 'Elevata',
    description: 'Equilibrio consigliato tra velocità, qualità e consumo di risorse.'
  },
  Sole: {
    name: 'Sole',
    contextTokens: 12288,
    maxOutputTokens: 2048,
    temperature: 0.6,
    keepAlive: '10m',
    speed: 'Ridotta',
    power: 'Massima',
    description: 'Più potente: usa ragionamento e contesto maggiori senza una scadenza complessiva; resta interrompibile manualmente.'
  }
};

const PROFILE_MODEL_PRIORITY: Record<ResourceProfileName, readonly string[]> = {
  Luna: ['qwen3:4b-instruct', 'llama3.2:3b', 'aura:4b'],
  Terra: ['aura:4b', 'qwen3:4b-instruct', 'qwen3:4b'],
  Sole: ['qwen3:4b', 'aura:4b', 'qwen3:4b-instruct']
};

export const RECOMMENDED_MODELS = [
  { name: 'aura:4b', note: 'specializzazione locale AURA per uso Windows, coding e giudizio critico' },
  { name: 'aura-coder:3b', note: 'specializzazione multilinguaggio per generazione, debugging e test' },
  { name: 'qwen3:4b-instruct', note: 'predefinito: rapido, diretto e ampio margine VRAM' },
  { name: 'qwen3:4b', note: 'variante reasoning per problemi complessi' },
  { name: 'qwen2.5-coder:3b', note: 'specializzato in codice, debugging e moduli sandbox' },
  { name: 'qwen2.5vl:3b', note: 'comprensione di schermate, immagini e documenti' },
  { name: 'llama3.2:3b', note: 'fallback leggero e affidabile' }
] as const;

export function choosePreferredModel(names: string[], profile: ResourceProfileName = 'Terra'): string {
  const priorities = [
    ...PROFILE_MODEL_PRIORITY[profile],
    ...RECOMMENDED_MODELS.map(model => model.name)
  ];
  for (const recommended of priorities) {
    const exact = names.find(name => name === recommended || name.startsWith(`${recommended}:`));
    if (exact) return exact;
  }
  return names[0] ?? '';
}

export function chooseModelForPrompt(prompt: string, selected: string, names: string[]): string {
  const codingIntent = /```|\b(?:java|python|py|c\+\+|cpp|csharp|c#|javascript|typescript|node(?:\.js)?|html|css|php|sql|rust|powershell|golang|kotlin|swift)\b|\b(?:codice|programma|funzione|classe|algoritmo|debug|bug|refactor|compila|compiler|stack trace|api|database)\b/i.test(prompt);
  if (!codingIntent) return selected;
  return names.find(name => name === 'aura-coder:3b')
    ?? names.find(name => name === 'qwen2.5-coder:3b')
    ?? selected;
}
