import { invoke, isTauri } from '@tauri-apps/api/core';

export interface VoiceStatus {
  available: boolean;
  engine: string;
  model: string;
  vad: boolean;
  maxSeconds: number;
  ttsAvailable: boolean;
  ttsEngine: string;
}

export interface VoiceTranscript {
  text: string;
  durationSeconds: number;
  confidence: number;
}

export interface VoiceInputDevice {
  deviceId: string;
  groupId: string;
  label: string;
}

export interface VoiceDevicePreference {
  deviceId?: string;
  label?: string;
}

export interface VoiceCaptureInfo {
  deviceId: string;
  label: string;
  sampleRate: number;
  channelCount: number;
}

export interface VoiceRecording {
  wav: Uint8Array;
  durationSeconds: number;
  rms: number;
  peak: number;
}

export async function getVoiceStatus(): Promise<VoiceStatus> {
  if (!isTauri()) return { available: false, engine: 'whisper.cpp', model: 'base multilingual', vad: false, maxSeconds: 30, ttsAvailable: false, ttsEngine: 'Windows SAPI' };
  return invoke<VoiceStatus>('voice_status');
}

export async function speakText(text: string): Promise<void> {
  if (!isTauri()) throw new Error('Sintesi vocale disponibile solo nell\u2019app desktop.');
  await invoke('voice_speak', { text });
}

export async function stopSpeaking(): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>('voice_stop_speaking');
}

export async function transcribeVoice(wav: Uint8Array): Promise<VoiceTranscript> {
  if (!isTauri()) throw new Error('Trascrizione vocale disponibile solo nell’app desktop.');
  return invoke<VoiceTranscript>('voice_transcribe', { wavBase64: bytesToBase64(wav) });
}

export async function listVoiceInputDevices(requestPermission = false): Promise<VoiceInputDevice[]> {
  if (!navigator.mediaDevices?.enumerateDevices || !navigator.mediaDevices?.getUserMedia) {
    throw new Error('API microfono non disponibile in questa installazione WebView2.');
  }

  let permissionStream: MediaStream | undefined;
  try {
    if (requestPermission) {
      permissionStream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
    }
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices
      .filter(device => device.kind === 'audioinput')
      .map(device => ({ deviceId: device.deviceId, groupId: device.groupId, label: device.label.trim() }));
  } finally {
    permissionStream?.getTracks().forEach(track => track.stop());
  }
}

export function choosePreferredVoiceInput(
  devices: VoiceInputDevice[],
  preference: VoiceDevicePreference = {}
): VoiceInputDevice | undefined {
  if (!devices.length) return undefined;
  const exact = preference.deviceId && devices.find(device => device.deviceId === preference.deviceId);
  if (exact) return exact;

  const preferredLabel = normalizeDeviceLabel(preference.label ?? '');
  if (preferredLabel) {
    const sameLabel = devices.find(device => normalizeDeviceLabel(device.label) === preferredLabel);
    if (sameLabel) return sameLabel;
  }

  const ranked = devices
    .map((device, index) => ({ device, index, score: voiceDeviceScore(device.label) }))
    .sort((left, right) => right.score - left.score || left.index - right.index);
  if (ranked[0].score > 0) return ranked[0].device;
  return devices.find(device => device.deviceId === 'default') ?? devices[0];
}

export function describeMicrophoneError(cause: unknown): string {
  if (!(cause instanceof DOMException)) {
    return cause instanceof Error ? cause.message : 'Microfono non disponibile.';
  }
  switch (cause.name) {
    case 'NotAllowedError':
    case 'SecurityError':
      return 'Accesso al microfono negato. Abilita AURA in Impostazioni Windows > Privacy e sicurezza > Microfono.';
    case 'NotFoundError':
      return 'Nessun microfono attivo. Connetti le cuffie OPPO e verifica il profilo vivavoce.';
    case 'NotReadableError':
    case 'AbortError':
      return 'Il microfono è occupato, disconnesso o non leggibile. Chiudi le altre app audio e riconnetti le cuffie.';
    case 'OverconstrainedError':
      return 'Il microfono selezionato non è più disponibile. Aggiorna l’elenco dei dispositivi.';
    default:
      return `Microfono non disponibile (${cause.name}).`;
  }
}

export class VoiceRecorder {
  private context?: AudioContext;
  private stream?: MediaStream;
  private source?: MediaStreamAudioSourceNode;
  private processor?: ScriptProcessorNode;
  private silentGain?: GainNode;
  private track?: MediaStreamTrack;
  private unexpectedEnd?: () => void;
  private stopping = false;
  private chunks: Float32Array[] = [];
  private sampleRate = 48_000;

  async start(deviceId?: string, onUnexpectedEnd?: () => void): Promise<VoiceCaptureInfo> {
    if (this.stream) throw new Error('Registrazione già attiva.');
    this.stopping = false;
    this.unexpectedEnd = onUnexpectedEnd;
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          ...(deviceId ? { deviceId: { exact: deviceId } } : {}),
          channelCount: { ideal: 1 },
          echoCancellation: { ideal: true },
          noiseSuppression: { ideal: true },
          autoGainControl: { ideal: true }
        },
        video: false
      });
      this.track = this.stream.getAudioTracks()[0];
      if (!this.track) throw new Error('Il dispositivo selezionato non espone una traccia audio.');
      this.track.addEventListener('ended', this.handleTrackEnded);
      this.context = new AudioContext({ latencyHint: 'interactive' });
      if (this.context.state === 'suspended') await this.context.resume();
      this.sampleRate = this.context.sampleRate;
      this.source = this.context.createMediaStreamSource(this.stream);
      this.processor = this.context.createScriptProcessor(4096, 1, 1);
      this.silentGain = this.context.createGain();
      this.silentGain.gain.value = 0;
      this.processor.onaudioprocess = event => {
        this.chunks.push(new Float32Array(event.inputBuffer.getChannelData(0)));
        event.outputBuffer.getChannelData(0).fill(0);
      };
      this.source.connect(this.processor);
      this.processor.connect(this.silentGain);
      this.silentGain.connect(this.context.destination);
      const settings = this.track.getSettings();
      return {
        deviceId: settings.deviceId ?? deviceId ?? '',
        label: this.track.label || 'Microfono predefinito',
        sampleRate: settings.sampleRate ?? this.sampleRate,
        channelCount: settings.channelCount ?? 1
      };
    } catch (cause) {
      await this.release();
      throw cause;
    }
  }

  async stop(): Promise<VoiceRecording> {
    if (!this.stream || !this.context) throw new Error('Registrazione non attiva.');
    this.stopping = true;
    const joined = joinSamples(this.chunks);
    const analysis = analyzeAudio(joined);
    try {
      if (joined.length / this.sampleRate < 0.15) throw new Error('Registrazione troppo breve: parla per almeno mezzo secondo.');
      const samples = resampleMono(joined, this.sampleRate, 16_000);
      const wav = encodePcm16Wav(samples, 16_000);
      if (samples !== joined) samples.fill(0);
      return {
        wav,
        durationSeconds: joined.length / this.sampleRate,
        rms: analysis.rms,
        peak: analysis.peak
      };
    } finally {
      joined.fill(0);
      await this.release();
    }
  }

  async cancel(): Promise<void> {
    this.stopping = true;
    await this.release();
  }

  private readonly handleTrackEnded = () => {
    if (!this.stopping) this.unexpectedEnd?.();
  };

  private async release(): Promise<void> {
    this.processor && (this.processor.onaudioprocess = null);
    this.processor?.disconnect();
    this.silentGain?.disconnect();
    this.source?.disconnect();
    this.track?.removeEventListener('ended', this.handleTrackEnded);
    this.stream?.getTracks().forEach(track => track.stop());
    if (this.context && this.context.state !== 'closed') await this.context.close().catch(() => undefined);
    this.chunks.forEach(chunk => chunk.fill(0));
    this.stream = undefined;
    this.context = undefined;
    this.source = undefined;
    this.processor = undefined;
    this.silentGain = undefined;
    this.track = undefined;
    this.unexpectedEnd = undefined;
    this.chunks = [];
  }
}

export function resampleMono(input: Float32Array, inputRate: number, outputRate: number): Float32Array {
  if (inputRate === outputRate) return input;
  if (!Number.isFinite(inputRate) || !Number.isFinite(outputRate) || inputRate <= 0 || outputRate <= 0) {
    throw new Error('Frequenza audio non valida.');
  }
  if (!input.length) return new Float32Array();
  const ratio = inputRate / outputRate;
  const output = new Float32Array(Math.round(input.length / ratio));
  if (inputRate < outputRate) {
    for (let index = 0; index < output.length; index += 1) {
      const sourcePosition = index * ratio;
      const left = Math.min(input.length - 1, Math.floor(sourcePosition));
      const right = Math.min(input.length - 1, left + 1);
      const fraction = sourcePosition - left;
      output[index] = input[left] * (1 - fraction) + input[right] * fraction;
    }
    return output;
  }
  for (let index = 0; index < output.length; index += 1) {
    const start = Math.floor(index * ratio);
    const end = Math.min(input.length, Math.floor((index + 1) * ratio));
    let sum = 0;
    for (let cursor = start; cursor < end; cursor += 1) sum += input[cursor];
    output[index] = sum / Math.max(1, end - start);
  }
  return output;
}

export function analyzeAudio(samples: Float32Array): { rms: number; peak: number } {
  if (!samples.length) return { rms: 0, peak: 0 };
  let sumSquares = 0;
  let peak = 0;
  for (const sample of samples) {
    const absolute = Math.abs(sample);
    peak = Math.max(peak, absolute);
    sumSquares += sample * sample;
  }
  return { rms: Math.sqrt(sumSquares / samples.length), peak };
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

function normalizeDeviceLabel(label: string): string {
  return label.toLocaleLowerCase().replace(/\s+/g, ' ').trim();
}

function voiceDeviceScore(label: string): number {
  const normalized = normalizeDeviceLabel(label);
  let score = 0;
  if (normalized.includes('oppo')) score += 100;
  if (normalized.includes('hands-free') || normalized.includes('headset') || normalized.includes('vivavoce')) score += 25;
  if (normalized.includes('microphone') || normalized.includes('microfono')) score += 5;
  return score;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}
