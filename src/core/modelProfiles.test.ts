import { describe, expect, it } from 'vitest';
import { chooseModelForPrompt, choosePreferredModel, RESOURCE_PROFILES } from './modelProfiles';

describe('model selection', () => {
  it('prefers the 4B instruct model on the target hardware', () => {
    expect(choosePreferredModel(['qwen3:4b', 'qwen3:4b-instruct'])).toBe('qwen3:4b-instruct');
  });

  it('maps Luna, Terra and Sole to progressively stronger engines', () => {
    const models = ['llama3.2:3b', 'aura:4b', 'qwen3:4b'];
    expect(choosePreferredModel(models, 'Luna')).toBe('llama3.2:3b');
    expect(choosePreferredModel(models, 'Terra')).toBe('aura:4b');
    expect(choosePreferredModel(models, 'Sole')).toBe('qwen3:4b');
  });

  it('exposes increasing context and output budgets', () => {
    expect(RESOURCE_PROFILES.Luna.contextTokens).toBeLessThan(RESOURCE_PROFILES.Terra.contextTokens);
    expect(RESOURCE_PROFILES.Terra.contextTokens).toBeLessThan(RESOURCE_PROFILES.Sole.contextTokens);
    expect(RESOURCE_PROFILES.Luna.maxOutputTokens).toBeLessThan(RESOURCE_PROFILES.Sole.maxOutputTokens);
  });

  it('falls back to the first installed model', () => {
    expect(choosePreferredModel(['custom:latest'])).toBe('custom:latest');
  });
});

describe('coding model routing', () => {
  const models = ['aura:4b', 'aura-coder:3b', 'qwen2.5-coder:3b'];

  it('routes common programming languages to the coding specialist', () => {
    expect(chooseModelForPrompt('Scrivi una classe Java', 'aura:4b', models)).toBe('aura-coder:3b');
    expect(chooseModelForPrompt('Correggi questa funzione Python', 'aura:4b', models)).toBe('aura-coder:3b');
    expect(chooseModelForPrompt('Crea una pagina HTML e JavaScript', 'aura:4b', models)).toBe('aura-coder:3b');
    expect(chooseModelForPrompt('Debug di un programma C++', 'aura:4b', models)).toBe('aura-coder:3b');
  });

  it('keeps ordinary conversation on the selected general model', () => {
    expect(chooseModelForPrompt('Come stai oggi?', 'aura:4b', models)).toBe('aura:4b');
  });
});
