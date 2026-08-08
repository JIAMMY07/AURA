import { describe, expect, it } from 'vitest';
import { choosePreferredModel } from './modelProfiles';

describe('model selection', () => {
  it('prefers the 4B instruct model on the target hardware', () => {
    expect(choosePreferredModel(['qwen3:4b', 'qwen3:4b-instruct'])).toBe('qwen3:4b-instruct');
  });

  it('falls back to the first installed model', () => {
    expect(choosePreferredModel(['custom:latest'])).toBe('custom:latest');
  });
});
