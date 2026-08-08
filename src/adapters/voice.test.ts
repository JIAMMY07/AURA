import { describe, expect, it } from 'vitest';
import { encodePcm16Wav, resampleMono } from './voice';

describe('voice audio preparation', () => {
  it('resamples mono audio to 16 kHz', () => {
    const input = new Float32Array(48_000).fill(0.25);
    const output = resampleMono(input, 48_000, 16_000);
    expect(output).toHaveLength(16_000);
    expect(output[100]).toBeCloseTo(0.25);
  });

  it('encodes a valid mono PCM16 WAV header', () => {
    const wav = encodePcm16Wav(new Float32Array(16_000), 16_000);
    expect(new TextDecoder().decode(wav.slice(0, 4))).toBe('RIFF');
    expect(new TextDecoder().decode(wav.slice(8, 12))).toBe('WAVE');
    expect(new DataView(wav.buffer).getUint32(24, true)).toBe(16_000);
    expect(wav).toHaveLength(32_044);
  });
});
