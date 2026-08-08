import { invoke, isTauri } from '@tauri-apps/api/core';

export interface VoiceStatus {
  available: boolean;
  engine: string;
  model: string;
  vad: boolean;
  maxSeconds: number;
}

export interface VoiceTranscript {
  text: string;
  durationSeconds: number;
  confidence: number;
}

export async function getVoiceStatus(): Promise<VoiceStatus> {
  if (!isTauri()) return { available: false, engine: 'whisper.cpp', model: 'base multilingual', vad: false, maxSeconds: 30 };
  return invoke<VoiceStatus>('voice_status');
}

export async function transcribeVoice(wav: Uint8Array): Promise<VoiceTranscript> {
  if (!isTauri()) throw new Error('Trascrizione vocale disponibile solo nell’app desktop.');
  return invoke<VoiceTranscript>('voice_transcribe', { wavBase64: bytesToBase64(wav) });
}

export class VoiceRecorder {
  private context?: AudioContext;
  private stream?: MediaStream;
  private source?: MediaStreamAudioSourceNode;
  private processor?: ScriptProcessorNode;
  private chunks: Float32Array[] = [];
  private sampleRate = 48_000;

  async start(): Promise<void> {
    if (this.stream) throw new Error('Registrazione già attiva.');
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      video: false
    });
    this.context = new AudioContext();
    this.sampleRate = this.context.sampleRate;
    this.source = this.context.createMediaStreamSource(this.stream);
    this.processor = this.context.createScriptProcessor(4096, 1, 1);
    this.processor.onaudioprocess = event => {
      this.chunks.push(new Float32Array(event.inputBuffer.getChannelData(0)));
      event.outputBuffer.getChannelData(0).fill(0);
    };
    this.source.connect(this.processor);
    this.processor.connect(this.context.destination);
  }

  async stop(): Promise<Uint8Array> {
    if (!this.stream || !this.context) throw new Error('Registrazione non attiva.');
    this.processor?.disconnect();
    this.source?.disconnect();
    this.stream.getTracks().forEach(track => track.stop());
    await this.context.close();
    const joined = joinSamples(this.chunks);
    const samples = resampleMono(joined, this.sampleRate, 16_000);
    this.stream = undefined;
    this.context = undefined;
    this.source = undefined;
    this.processor = undefined;
    this.chunks = [];
    const wav = encodePcm16Wav(samples, 16_000);
    joined.fill(0);
    if (samples !== joined) samples.fill(0);
    return wav;
  }
}

export function resampleMono(input: Float32Array, inputRate: number, outputRate: number): Float32Array {
  if (inputRate === outputRate) return input;
  if (inputRate < outputRate || outputRate <= 0) throw new Error('Frequenza audio non supportata.');
  const ratio = inputRate / outputRate;
  const output = new Float32Array(Math.floor(input.length / ratio));
  for (let index = 0; index < output.length; index += 1) {
    const start = Math.floor(index * ratio);
    const end = Math.min(input.length, Math.floor((index + 1) * ratio));
    let sum = 0;
    for (let cursor = start; cursor < end; cursor += 1) sum += input[cursor];
    output[index] = sum / Math.max(1, end - start);
  }
  return output;
}

export function encodePcm16Wav(samples: Float32Array, sampleRate: number): Uint8Array {
  const bytes = new Uint8Array(44 + samples.length * 2);
  const view = new DataView(bytes.buffer);
  writeAscii(bytes, 0, 'RIFF');
  view.setUint32(4, 36 + samples.length * 2, true);
  writeAscii(bytes, 8, 'WAVE');
  writeAscii(bytes, 12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(bytes, 36, 'data');
  view.setUint32(40, samples.length * 2, true);
  samples.forEach((sample, index) => {
    const clamped = Math.max(-1, Math.min(1, sample));
    view.setInt16(44 + index * 2, clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff, true);
  });
  return bytes;
}

function joinSamples(chunks: Float32Array[]): Float32Array {
  const joined = new Float32Array(chunks.reduce((total, chunk) => total + chunk.length, 0));
  let offset = 0;
  chunks.forEach(chunk => { joined.set(chunk, offset); offset += chunk.length; });
  return joined;
}

function writeAscii(target: Uint8Array, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) target[offset + index] = value.charCodeAt(index);
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}
