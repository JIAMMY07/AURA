import { useEffect, useRef, useState } from 'react';
import { Camera, CameraOff, ChevronDown, ChevronUp, Hand, RefreshCw, ShieldCheck } from 'lucide-react';
import {
  chooseVisionInput, describeCameraError, FACE_CONNECTIONS, HAND_CONNECTIONS,
  GestureCommandGate, HandDesktopController, listVisionInputDevices, LocalVisionEngine, type DesktopControlEvent, type HandControlAction,
  type VisionFrame, type VisionInputDevice
} from '../../adapters/vision';
import { moveHandPointer, performHandPointerAction, performHandScroll, performHandWindowAction, setHandDesktopControl } from '../../adapters/agent';
import { recordInteraction } from '../../adapters/security';

const CAMERA_KEY = 'aura.vision-camera.v1';
const FRAME_INTERVAL_MS = 55;

function loadCameraPreference(): string {
  try { return window.localStorage.getItem(CAMERA_KEY) ?? ''; } catch { return ''; }
}

function saveCameraPreference(deviceId: string) {
  try {
    if (deviceId) window.localStorage.setItem(CAMERA_KEY, deviceId);
    else window.localStorage.removeItem(CAMERA_KEY);
  } catch { /* Preferenza facoltativa. */ }
}

function gestureLabel(name: string): string {
  const labels: Record<string, string> = {
    Closed_Fist: 'Pugno chiuso', Open_Palm: 'Palmo aperto', Pointing_Up: 'Indice alzato',
    Thumb_Down: 'Pollice giù', Thumb_Up: 'Pollice su', Victory: 'Vittoria', ILoveYou: 'Ti voglio bene',
    None: 'Nessun gesto', Unknown: 'Gesto non riconosciuto'
  };
  return labels[name] ?? name;
}

function drawConnections(
  context: CanvasRenderingContext2D,
  landmarks: VisionFrame['faces'][number],
  connections: { start: number; end: number }[],
  width: number,
  height: number,
  color: string
) {
  context.strokeStyle = color;
  context.lineWidth = 2;
  context.beginPath();
  connections.forEach(connection => {
    const start = landmarks[connection.start];
    const end = landmarks[connection.end];
    if (!start || !end) return;
    context.moveTo(start.x * width, start.y * height);
    context.lineTo(end.x * width, end.y * height);
  });
  context.stroke();
}

function drawVisionFrame(canvas: HTMLCanvasElement, frame: VisionFrame, width: number, height: number) {
  if (!width || !height) return;
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const context = canvas.getContext('2d');
  if (!context) return;
  context.clearRect(0, 0, width, height);
  frame.faces.forEach(face => drawConnections(context, face, FACE_CONNECTIONS, width, height, '#75efd0'));
  frame.hands.forEach(hand => {
    drawConnections(context, hand, HAND_CONNECTIONS, width, height, '#f6c76b');
    context.fillStyle = '#fff3be';
    hand.forEach(point => {
      context.beginPath();
      context.arc(point.x * width, point.y * height, 2.8, 0, Math.PI * 2);
      context.fill();
    });
  });
}

const HAND_ACTION_LABELS: Record<HandControlAction, string> = {
  'volume-up': 'Volume aumentato', 'volume-down': 'Volume ridotto', mute: 'Audio attivato/disattivato',
  'play-pause': 'Riproduzione/pausa', next: 'Brano successivo', previous: 'Brano precedente'
};

function desktopEventInstruction(event: DesktopControlEvent): string {
  if (event.kind === 'pointer') return `pointer:${event.action}`;
  if (event.kind === 'scroll') return `scroll:${event.delta}`;
  if (event.kind === 'window') return `window:${event.action}`;
  return 'pointer-move';
}

export function VisionPanel({ onHandCommand }: { onHandCommand?: (action: HandControlAction) => Promise<boolean> }) {
  const [devices, setDevices] = useState<VisionInputDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState(loadCameraPreference);
  const [active, setActive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('Videocamera spenta. Nessun fotogramma viene acquisito.');
  const [frame, setFrame] = useState<VisionFrame>();
  const [collapsed, setCollapsed] = useState(false);
  const [handControl, setHandControl] = useState(false);
  const [desktopControl, setDesktopControl] = useState(false);
  const [handCommandBusy, setHandCommandBusy] = useState(false);
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const streamRef = useRef<MediaStream | undefined>(undefined);
  const engineRef = useRef<LocalVisionEngine | undefined>(undefined);
  const animationRef = useRef<number | undefined>(undefined);
  const activeRef = useRef(false);
  const lastFrameRef = useRef(0);
  const handControlRef = useRef(false);
  const desktopControlRef = useRef(false);
  const desktopBusyRef = useRef(false);
  const handCommandBusyRef = useRef(false);
  const gestureGateRef = useRef(new GestureCommandGate());
  const desktopControllerRef = useRef(new HandDesktopController());

  async function executeDesktopEvents(events: DesktopControlEvent[]) {
    if (!events.length || desktopBusyRef.current || !desktopControlRef.current) return;
    desktopBusyRef.current = true;
    try {
      for (const event of events) {
        if (event.kind === 'move') await moveHandPointer(event.x, event.y);
        else if (event.kind === 'pointer') await performHandPointerAction(event.action);
        else if (event.kind === 'scroll') await performHandScroll(event.delta);
        else await performHandWindowAction(event.action);
        if (event.kind !== 'move') {
          await recordInteraction({
            source: 'gesture', action: event.kind, content: desktopEventInstruction(event), outcome: 'allow',
            metadata: { mode: 'desktop', frameStored: false }
          }).catch(() => false);
        }
      }
      const action = [...events].reverse().find(event => event.kind !== 'move');
      if (action?.kind === 'window') {
        const labels = { minimize: 'Finestra ridotta', 'maximize-restore': 'Massimizza/ripristina', 'switch-next': 'Finestra successiva', close: 'Chiusura finestra inviata' };
        setNotice(`Controllo mani: ${labels[action.action]}.`);
      }
    } catch (cause) {
      setNotice(`Controllo desktop arrestato: ${cause instanceof Error ? cause.message : String(cause)}`);
      desktopControlRef.current = false;
      setDesktopControl(false);
      void setHandDesktopControl(false).catch(() => undefined);
    } finally {
      desktopBusyRef.current = false;
    }
  }

  async function executeHandCommand(action: HandControlAction) {
    if (handCommandBusyRef.current || !handControlRef.current) return;
    handCommandBusyRef.current = true;
    setHandCommandBusy(true);
    try {
      if (!onHandCommand) throw new Error('Controllo PC disponibile soltanto nell’app desktop AURA.');
      const executed = await onHandCommand(action);
      await recordInteraction({
        source: 'gesture', action: 'media', content: action, outcome: executed ? 'allow' : 'deny',
        metadata: { mode: 'media', frameStored: false }
      }).catch(() => false);
      setNotice(executed ? `Comando mani eseguito: ${HAND_ACTION_LABELS[action]}.` : 'Comando mani non eseguito.');
    } catch (cause) {
      setNotice(`Controllo mani arrestato: ${cause instanceof Error ? cause.message : String(cause)}`);
      handControlRef.current = false;
      setHandControl(false);
    } finally {
      handCommandBusyRef.current = false;
      setHandCommandBusy(false);
    }
  }

  async function refreshDevices(requestPermission: boolean): Promise<VisionInputDevice | undefined> {
    const available = await listVisionInputDevices(requestPermission);
    setDevices(available);
    const selected = chooseVisionInput(available, selectedDeviceId || loadCameraPreference());
    setSelectedDeviceId(selected?.deviceId ?? '');
    if (requestPermission) setNotice(selected
      ? `Videocamera pronta: ${selected.label || 'dispositivo predefinito'}.`
      : 'Nessuna videocamera rilevata.');
    return selected;
  }

  function releaseVision() {
    activeRef.current = false;
    if (animationRef.current) cancelAnimationFrame(animationRef.current);
    animationRef.current = undefined;
    streamRef.current?.getTracks().forEach(track => { track.onended = null; track.stop(); });
    streamRef.current = undefined;
    engineRef.current?.close();
    engineRef.current = undefined;
    if (videoRef.current) videoRef.current.srcObject = null;
    const canvas = canvasRef.current;
    canvas?.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height);
    setActive(false);
    setFrame(undefined);
    handControlRef.current = false;
    setHandControl(false);
    gestureGateRef.current.reset();
    const wasDesktopControl = desktopControlRef.current;
    const releaseEvents = desktopControllerRef.current.reset();
    desktopControlRef.current = false;
    setDesktopControl(false);
    if (wasDesktopControl) void (async () => {
      try {
        if (releaseEvents.some(event => event.kind === 'pointer' && event.action === 'left-up')) await performHandPointerAction('left-up');
      } finally {
        await setHandDesktopControl(false).catch(() => false);
      }
    })();
  }

  function stopVision(message = 'Videocamera spenta. Nessun fotogramma viene acquisito.') {
    releaseVision();
    setNotice(message);
  }

  function scheduleAnalysis() {
    if (!activeRef.current) return;
    animationRef.current = requestAnimationFrame(timestamp => {
      const video = videoRef.current;
      const canvas = canvasRef.current;
      const engine = engineRef.current;
      if (video && canvas && engine && video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
        && timestamp - lastFrameRef.current >= FRAME_INTERVAL_MS) {
        lastFrameRef.current = timestamp;
        try {
          const result = engine.analyze(video, timestamp);
          drawVisionFrame(canvas, result, video.videoWidth, video.videoHeight);
          setFrame(result);
          if (desktopControlRef.current) {
            const events = desktopControllerRef.current.update(result, timestamp);
            if (events.length) void executeDesktopEvents(events);
          } else if (handControlRef.current) {
            const action = gestureGateRef.current.update(result.gestures, timestamp);
            if (action) void executeHandCommand(action);
          }
        } catch (cause) {
          stopVision(`Analisi visiva arrestata: ${cause instanceof Error ? cause.message : 'errore sconosciuto'}`);
          return;
        }
      }
      scheduleAnalysis();
    });
  }

  async function startVision() {
    if (activeRef.current || busy) return;
    setBusy(true);
    setNotice('Caricamento dei modelli visivi locali…');
    try {
      const device = await refreshDevices(true);
      if (!device) throw new DOMException('No camera', 'NotFoundError');
      const stream = await navigator.mediaDevices.getUserMedia({
        video: {
          ...(device.deviceId ? { deviceId: { exact: device.deviceId } } : {}),
          width: { ideal: 640 }, height: { ideal: 480 }, frameRate: { ideal: 15, max: 20 }
        },
        audio: false
      });
      streamRef.current = stream;
      const track = stream.getVideoTracks()[0];
      if (!track) throw new Error('La videocamera non espone una traccia video.');
      track.onended = () => stopVision('Videocamera disconnessa. Analisi visiva arrestata in sicurezza.');
      const video = videoRef.current;
      if (!video) throw new Error('Anteprima videocamera non disponibile.');
      video.srcObject = stream;
      await video.play();
      engineRef.current = await LocalVisionEngine.create();
      activeRef.current = true;
      lastFrameRef.current = 0;
      setActive(true);
      setNotice(`Analisi locale attiva da ${track.label || device.label || 'videocamera'}. Nessuna immagine viene salvata.`);
      scheduleAnalysis();
    } catch (cause) {
      releaseVision();
      setNotice(describeCameraError(cause));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refreshDevices(false).catch(() => undefined);
    const onDeviceChange = () => { if (!activeRef.current) void refreshDevices(false).catch(() => undefined); };
    navigator.mediaDevices?.addEventListener?.('devicechange', onDeviceChange);
    const emergencyStop = () => stopVision('Visione e controllo gestuale arrestati dal comando globale.');
    window.addEventListener('aura:stop-all', emergencyStop);
    return () => {
      navigator.mediaDevices?.removeEventListener?.('devicechange', onDeviceChange);
      window.removeEventListener('aura:stop-all', emergencyStop);
      releaseVision();
    };
    // La preferenza iniziale viene letta una volta; gli aggiornamenti passano dal selettore.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectDevice = (deviceId: string) => {
    setSelectedDeviceId(deviceId);
    saveCameraPreference(deviceId);
    const device = devices.find(candidate => candidate.deviceId === deviceId);
    setNotice(`Videocamera selezionata: ${device?.label || 'dispositivo predefinito'}.`);
  };

  const toggleHandControl = (enabled: boolean) => {
    gestureGateRef.current.reset();
    handControlRef.current = enabled;
    setHandControl(enabled);
    setNotice(enabled
      ? 'Controllo mani armato: mantieni un solo gesto stabile per circa un secondo.'
      : 'Controllo mani disattivato. Il riconoscimento resta solo informativo.');
  };

  const toggleDesktopControl = async (enabled: boolean) => {
    const releaseEvents = desktopControllerRef.current.reset();
    try {
      if (enabled) {
        const armed = await setHandDesktopControl(true);
        if (!armed) {
          setNotice('Abilitazione del controllo desktop annullata.');
          return;
        }
        handControlRef.current = false;
        setHandControl(false);
        desktopControlRef.current = true;
        setDesktopControl(true);
        setNotice('Controllo desktop completo armato. Indice: puntatore · pizzico: click/trascina · vittoria: scroll.');
      } else {
        if (releaseEvents.some(event => event.kind === 'pointer' && event.action === 'left-up')) await performHandPointerAction('left-up');
        desktopControlRef.current = false;
        setDesktopControl(false);
        await setHandDesktopControl(false);
        setNotice('Controllo desktop con le mani disattivato.');
      }
    } catch (cause) {
      desktopControlRef.current = false;
      setDesktopControl(false);
      setNotice(`Controllo desktop non disponibile: ${cause instanceof Error ? cause.message : String(cause)}`);
    }
  };

  return <section className={`vision-panel ${active ? 'active' : ''} ${collapsed ? 'collapsed' : ''}`} aria-label="Riconoscimento locale di mani e volto">
    <header>
      <div><Camera /><strong>Visione locale</strong><span className={`vision-status ${active ? 'on' : ''}`}>{active ? 'ATTIVA' : 'SPENTA'}</span></div>
      <div className="vision-controls">
        <select aria-label="Videocamera" value={selectedDeviceId} onChange={event => selectDevice(event.target.value)} disabled={active || busy}>
          {!devices.length && <option value="">Autorizza e rileva videocamere</option>}
          {devices.map((device, index) => <option value={device.deviceId} key={device.deviceId || `${device.groupId}-${index}`}>
            {device.label || `Videocamera ${index + 1}`}
          </option>)}
        </select>
        <button type="button" className="icon-button" onClick={() => void refreshDevices(true).catch(cause => setNotice(describeCameraError(cause)))} disabled={active || busy} title="Aggiorna videocamere"><RefreshCw /></button>
        <button type="button" className={active ? 'danger' : 'secondary'} onClick={() => active ? stopVision() : void startVision()} disabled={busy}>
          {active ? <><CameraOff /> Arresta</> : <><Camera /> Avvia</>}
        </button>
        <button type="button" className="icon-button vision-collapse" onClick={() => setCollapsed(current => !current)} title={collapsed ? 'Mostra pannello videocamera' : 'Riduci videocamera a banner'} aria-label={collapsed ? 'Mostra pannello videocamera' : 'Riduci videocamera a banner'}>
          {collapsed ? <ChevronDown /> : <ChevronUp />}
        </button>
      </div>
    </header>
    {!collapsed && <><div className="vision-content">
      <div className="vision-preview">
        <video ref={videoRef} muted playsInline aria-label="Anteprima videocamera locale" />
        <canvas ref={canvasRef} aria-hidden="true" />
        {!active && <div className="vision-placeholder"><CameraOff /><span>La videocamera è spenta</span></div>}
      </div>
      <div className="vision-results" aria-live="polite">
        <div><span>Volti rilevati</span><strong>{frame?.faces.length ?? 0}</strong></div>
        <div><span>Mani rilevate</span><strong>{frame?.hands.length ?? 0}</strong></div>
        <div className="gesture-result"><span>Gesti</span><strong>{frame?.gestures.length
          ? frame.gestures.map(gesture => `${gestureLabel(gesture.name)} ${Math.round(gesture.score * 100)}%`).join(' · ')
          : '—'}</strong></div>
        <label className={`hand-control-toggle ${desktopControl ? 'enabled' : ''}`}><Hand /><span><strong>Controllo desktop completo</strong><small>Puntatore, click, trascinamento, scroll e finestre; si disarma con la videocamera</small></span><input type="checkbox" checked={desktopControl} onChange={event => void toggleDesktopControl(event.target.checked)} disabled={!active || handCommandBusy || busy} /></label>
        <small className="gesture-map">☝ puntatore · 🤏 click, due volte per doppio click, mantieni per trascinare · ✊ click destro · ✌ scroll verticale · 👎 riduci · 👍 massimizza/ripristina · 🖐 cambia finestra · 🤟 mantieni 2 s per chiudere</small>
        <label className={`hand-control-toggle compact ${handControl ? 'enabled' : ''}`}><Hand /><span><strong>Scorciatoie multimediali</strong><small>Modalità alternativa al controllo desktop</small></span><input type="checkbox" checked={handControl} onChange={event => { if (event.target.checked && desktopControl) void toggleDesktopControl(false); toggleHandControl(event.target.checked); }} disabled={!active || handCommandBusy || desktopControl} /></label>
        <p><ShieldCheck /> Elaborazione sul PC: i frame non vengono salvati né inviati a Ollama o Internet.</p>
        <small>Il volto viene rilevato tramite landmark, non usato per identificare una persona. Un comando richiede una sola mano, confidenza elevata, gesto stabile, pausa neutra e cooldown.</small>
      </div>
    </div>
    <div className="vision-notice" role="status">{notice}</div></>}
  </section>;
}
