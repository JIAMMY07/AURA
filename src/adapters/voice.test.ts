import { describe, expect, it } from 'vitest';
import { analyzeAudio, choosePreferredVoiceInput, encodePcm16Wav, resampleMono } from './voice';

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

  it('upsamples an 8 kHz Bluetooth hands-free stream', () => {
    const input = Float32Array.from([0, 0.5, 1, 0.5]);
    const output = resampleMono(input, 8_000, 16_000);
    expect(output).toHaveLength(8);
    expect(output[1]).toBeCloseTo(0.25);
    expect(output[2]).toBeCloseTo(0.5);
  });

  it('prefers a remembered device and otherwise follows the Windows default', () => {
    const devices = [
      { deviceId: 'default', groupId: 'a', label: 'Microfono predefinito' },
      { deviceId: 'oppo', groupId: 'b', label: 'OPPO Enco Buds2 Pro Hands-Free' }
    ];
    expect(choosePreferredVoiceInput(devices)?.deviceId).toBe('default');
    expect(choosePreferredVoiceInput(devices, { deviceId: 'default' })?.deviceId).toBe('default');
    expect(choosePreferredVoiceInput(devices, { deviceId: 'old', label: 'OPPO Enco Buds2 Pro Hands-Free' })?.deviceId).toBe('oppo');
  });

  it('supports any input and falls back safely when the remembered one is disconnected', () => {
    const devices = [
      { deviceId: 'usb', groupId: 'a', label: 'Microfono USB' },
      { deviceId: 'webcam', groupId: 'b', label: 'Webcam HD Audio' }
    ];
    expect(choosePreferredVoiceInput(devices, { deviceId: 'webcam' })?.deviceId).toBe('webcam');
    expect(choosePreferredVoiceInput(devices, { deviceId: 'bluetooth-disconnected', label: 'Vecchio headset' })?.deviceId).toBe('usb');
    expect(choosePreferredVoiceInput([], { deviceId: 'usb' })).toBeUndefined();
  });

  it('calculates level diagnostics for weak and clipped audio', () => {
    const analysis = analyzeAudio(Float32Array.from([0, 0.5, -1]));
    expect(analysis.rms).toBeCloseTo(Math.sqrt(1.25 / 3), 6);
    expect(analysis.peak).toBe(1);
  });
});
