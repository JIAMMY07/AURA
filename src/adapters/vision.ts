import {
  FaceLandmarker, FilesetResolver, GestureRecognizer,
  type NormalizedLandmark
} from '@mediapipe/tasks-vision';

export interface VisionConnection { start: number; end: number }

export interface VisionInputDevice {
  deviceId: string;
  groupId: string;
  label: string;
}

export interface VisionGesture {
  name: string;
  score: number;
  handedness: string;
}

export interface VisionFrame {
  faces: NormalizedLandmark[][];
  hands: NormalizedLandmark[][];
  gestures: VisionGesture[];
  timestamp: number;
}

export type HandControlAction = 'volume-up' | 'volume-down' | 'mute' | 'play-pause' | 'next' | 'previous';
export type DesktopWindowAction = 'minimize' | 'maximize-restore' | 'switch-next' | 'close';
export type DesktopControlEvent =
  | { kind: 'move'; x: number; y: number }
  | { kind: 'pointer'; action: 'left-click' | 'left-down' | 'left-up' | 'right-click' }
  | { kind: 'scroll'; delta: number }
  | { kind: 'window'; action: DesktopWindowAction };

const HAND_COMMANDS: Readonly<Record<string, HandControlAction>> = {
  Thumb_Up: 'volume-up',
  Thumb_Down: 'volume-down',
  Closed_Fist: 'mute',
  Open_Palm: 'play-pause',
  Victory: 'next',
  Pointing_Up: 'previous'
};

export class GestureCommandGate {
  private candidate = '';
  private candidateSince = 0;
  private neutralSince = 0;
  private lastActionAt = Number.NEGATIVE_INFINITY;
  private armed = true;

  constructor(
    private readonly stableMs = 900,
    private readonly cooldownMs = 2_000,
    private readonly neutralMs = 450,
    private readonly minimumScore = 0.82
  ) {}

  reset() {
    this.candidate = '';
    this.candidateSince = 0;
    this.neutralSince = 0;
    this.lastActionAt = Number.NEGATIVE_INFINITY;
    this.armed = true;
  }

  update(gestures: VisionGesture[], timestamp: number): HandControlAction | undefined {
    const accepted = gestures.length === 1 && gestures[0].score >= this.minimumScore
      ? HAND_COMMANDS[gestures[0].name]
      : undefined;
    if (!accepted) {
      this.candidate = '';
      this.candidateSince = 0;
      if (!this.neutralSince) this.neutralSince = timestamp;
      if (timestamp - this.neutralSince >= this.neutralMs) this.armed = true;
      return undefined;
    }
    this.neutralSince = 0;
    if (!this.armed || timestamp - this.lastActionAt < this.cooldownMs) return undefined;
    if (this.candidate !== accepted) {
      this.candidate = accepted;
      this.candidateSince = timestamp;
      return undefined;
    }
    if (timestamp - this.candidateSince < this.stableMs) return undefined;
    this.armed = false;
    this.lastActionAt = timestamp;
    this.candidate = '';
    this.candidateSince = 0;
    return accepted;
  }
}

type StableDesktopAction = DesktopWindowAction | 'right-click';

const WINDOW_GESTURES: Readonly<Record<string, StableDesktopAction>> = {
  Thumb_Down: 'minimize',
  Thumb_Up: 'maximize-restore',
  Open_Palm: 'switch-next',
  Closed_Fist: 'right-click',
  ILoveYou: 'close'
};

function distance(a: NormalizedLandmark, b: NormalizedLandmark): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function activeArea(value: number, minimum: number, maximum: number): number {
  return Math.max(0, Math.min(1, (value - minimum) / (maximum - minimum)));
}

export class HandDesktopController {
  private smoothedX?: number;
  private smoothedY?: number;
  private pinchSince = 0;
  private dragging = false;
  private scrollY?: number;
  private windowCandidate = '';
  private windowSince = 0;
  private windowArmed = true;
  private neutralSince = 0;

  reset(): DesktopControlEvent[] {
    const release = this.dragging ? [{ kind: 'pointer', action: 'left-up' } as DesktopControlEvent] : [];
    this.smoothedX = undefined;
    this.smoothedY = undefined;
    this.pinchSince = 0;
    this.dragging = false;
    this.scrollY = undefined;
    this.windowCandidate = '';
    this.windowSince = 0;
    this.windowArmed = true;
    this.neutralSince = 0;
    return release;
  }

  update(frame: VisionFrame, timestamp: number): DesktopControlEvent[] {
    const events: DesktopControlEvent[] = [];
    if (frame.hands.length !== 1) {
      if (this.dragging) events.push({ kind: 'pointer', action: 'left-up' });
      this.dragging = false;
      this.pinchSince = 0;
      this.scrollY = undefined;
      this.updateWindowGesture(undefined, timestamp, events);
      return events;
    }
    const hand = frame.hands[0];
    const gesture = frame.gestures.length === 1 && frame.gestures[0].score >= 0.78
      ? frame.gestures[0].name : undefined;
    const indexTip = hand[8];
    const thumbTip = hand[4];
    const wrist = hand[0];
    const middleBase = hand[9];
    if (!indexTip || !thumbTip || !wrist || !middleBase) return events;

    const rawX = activeArea(1 - indexTip.x, 0.08, 0.92);
    const rawY = activeArea(indexTip.y, 0.08, 0.9);
    this.smoothedX = this.smoothedX === undefined ? rawX : this.smoothedX * 0.68 + rawX * 0.32;
    this.smoothedY = this.smoothedY === undefined ? rawY : this.smoothedY * 0.68 + rawY * 0.32;
    if (gesture === 'Pointing_Up' || this.pinchSince > 0) {
      events.push({ kind: 'move', x: this.smoothedX, y: this.smoothedY });
    }

    const palmScale = Math.max(0.025, distance(wrist, middleBase));
    const pinching = distance(indexTip, thumbTip) / palmScale < 0.42;
    if (pinching) {
      if (!this.pinchSince) this.pinchSince = timestamp;
      if (!this.dragging && timestamp - this.pinchSince >= 420) {
        this.dragging = true;
        events.push({ kind: 'pointer', action: 'left-down' });
      }
    } else if (this.pinchSince) {
      if (this.dragging) events.push({ kind: 'pointer', action: 'left-up' });
      else if (timestamp - this.pinchSince >= 70) events.push({ kind: 'pointer', action: 'left-click' });
      this.dragging = false;
      this.pinchSince = 0;
    }

    if (gesture === 'Victory' && !pinching) {
      if (this.scrollY !== undefined) {
        const delta = Math.round((this.scrollY - indexTip.y) * 1_600);
        if (Math.abs(delta) >= 8) events.push({ kind: 'scroll', delta: Math.max(-360, Math.min(360, delta)) });
      }
      this.scrollY = indexTip.y;
    } else {
      this.scrollY = undefined;
    }
    this.updateWindowGesture(pinching || gesture === 'Pointing_Up' || gesture === 'Victory' ? undefined : gesture, timestamp, events);
    return events;
  }

  private updateWindowGesture(gesture: string | undefined, timestamp: number, events: DesktopControlEvent[]) {
    const action = gesture ? WINDOW_GESTURES[gesture] : undefined;
    if (!action) {
      this.windowCandidate = '';
      this.windowSince = 0;
      if (!this.neutralSince) this.neutralSince = timestamp;
      if (timestamp - this.neutralSince >= 500) this.windowArmed = true;
      return;
    }
    this.neutralSince = 0;
    if (!this.windowArmed) return;
    if (this.windowCandidate !== action) {
      this.windowCandidate = action;
      this.windowSince = timestamp;
      return;
    }
    const required = action === 'close' ? 2_000 : action === 'right-click' ? 850 : 1_050;
    if (timestamp - this.windowSince < required) return;
    if (action === 'right-click') events.push({ kind: 'pointer', action: 'right-click' });
    else events.push({ kind: 'window', action });
    this.windowArmed = false;
    this.windowCandidate = '';
    this.windowSince = 0;
  }
}

export const FACE_CONNECTIONS: VisionConnection[] = [
  ...FaceLandmarker.FACE_LANDMARKS_FACE_OVAL,
  ...FaceLandmarker.FACE_LANDMARKS_LEFT_EYE,
  ...FaceLandmarker.FACE_LANDMARKS_RIGHT_EYE,
  ...FaceLandmarker.FACE_LANDMARKS_LEFT_EYEBROW,
  ...FaceLandmarker.FACE_LANDMARKS_RIGHT_EYEBROW,
  ...FaceLandmarker.FACE_LANDMARKS_LIPS
];

export const HAND_CONNECTIONS: VisionConnection[] = GestureRecognizer.HAND_CONNECTIONS;

export async function listVisionInputDevices(requestPermission = false): Promise<VisionInputDevice[]> {
  if (!navigator.mediaDevices?.enumerateDevices || !navigator.mediaDevices?.getUserMedia) {
    throw new Error('API videocamera non disponibile in questa installazione WebView2.');
  }
  let permissionStream: MediaStream | undefined;
  try {
    if (requestPermission) permissionStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices
      .filter(device => device.kind === 'videoinput')
      .map(device => ({ deviceId: device.deviceId, groupId: device.groupId, label: device.label.trim() }));
  } finally {
    permissionStream?.getTracks().forEach(track => track.stop());
  }
}

export function chooseVisionInput(devices: VisionInputDevice[], preferredId?: string): VisionInputDevice | undefined {
  if (!devices.length) return undefined;
  return devices.find(device => device.deviceId === preferredId)
    ?? devices.find(device => device.deviceId === 'default')
    ?? devices[0];
}

export function describeCameraError(cause: unknown): string {
  if (!(cause instanceof DOMException)) return cause instanceof Error ? cause.message : 'Videocamera non disponibile.';
  switch (cause.name) {
    case 'NotAllowedError':
    case 'SecurityError':
      return 'Accesso alla videocamera negato. Abilita AURA in Impostazioni Windows > Privacy e sicurezza > Fotocamera.';
    case 'NotFoundError':
      return 'Nessuna videocamera attiva. Collegane una oppure abilita quella integrata.';
    case 'NotReadableError':
    case 'AbortError':
      return 'La videocamera è occupata o non leggibile. Chiudi le altre applicazioni che la stanno usando.';
    case 'OverconstrainedError':
      return 'La videocamera selezionata non è più disponibile. Aggiorna l’elenco dei dispositivi.';
    default:
      return `Videocamera non disponibile (${cause.name}).`;
  }
}

export class LocalVisionEngine {
  private constructor(
    private readonly faceLandmarker: FaceLandmarker,
    private readonly gestureRecognizer: GestureRecognizer
  ) {}

  static async create(): Promise<LocalVisionEngine> {
    const wasmRoot = new URL('mediapipe/wasm/', window.location.href).toString();
    const fileset = await FilesetResolver.forVisionTasks(wasmRoot);
    const [faceLandmarker, gestureRecognizer] = await Promise.all([
      FaceLandmarker.createFromOptions(fileset, {
        baseOptions: { modelAssetPath: new URL('models/vision/face_landmarker.task', window.location.href).toString(), delegate: 'CPU' },
        runningMode: 'VIDEO',
        numFaces: 2,
        minFaceDetectionConfidence: 0.55,
        minFacePresenceConfidence: 0.55,
        minTrackingConfidence: 0.5,
        outputFaceBlendshapes: false,
        outputFacialTransformationMatrixes: false
      }),
      GestureRecognizer.createFromOptions(fileset, {
        baseOptions: { modelAssetPath: new URL('models/vision/gesture_recognizer.task', window.location.href).toString(), delegate: 'CPU' },
        runningMode: 'VIDEO',
        numHands: 2,
        minHandDetectionConfidence: 0.55,
        minHandPresenceConfidence: 0.55,
        minTrackingConfidence: 0.5,
        cannedGesturesClassifierOptions: { scoreThreshold: 0.6 }
      })
    ]);
    return new LocalVisionEngine(faceLandmarker, gestureRecognizer);
  }

  analyze(video: HTMLVideoElement, timestamp: number): VisionFrame {
    const faceResult = this.faceLandmarker.detectForVideo(video, timestamp);
    const handResult = this.gestureRecognizer.recognizeForVideo(video, timestamp);
    const gestures: VisionGesture[] = handResult.gestures.map((categories, index) => {
      const best = categories[0];
      const handedness = handResult.handedness[index]?.[0]?.categoryName ?? '';
      return { name: best?.categoryName ?? 'Unknown', score: best?.score ?? 0, handedness };
    });
    return {
      faces: faceResult.faceLandmarks,
      hands: handResult.landmarks,
      gestures,
      timestamp
    };
  }

  close(): void {
    this.faceLandmarker.close();
    this.gestureRecognizer.close();
  }
}
