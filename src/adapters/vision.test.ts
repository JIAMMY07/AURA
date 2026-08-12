import { describe, expect, it } from 'vitest';
import { chooseVisionInput, GestureCommandGate, HandDesktopController, type VisionFrame, type VisionGesture } from './vision';

describe('vision input selection', () => {
  const devices = [
    { deviceId: 'default', groupId: 'system', label: 'Videocamera predefinita' },
    { deviceId: 'usb', groupId: 'external', label: 'USB Camera' }
  ];

  it('uses the explicitly selected camera when available', () => {
    expect(chooseVisionInput(devices, 'usb')?.deviceId).toBe('usb');
  });

  it('falls back to the Windows default and handles no camera', () => {
    expect(chooseVisionInput(devices, 'disconnected')?.deviceId).toBe('default');
    expect(chooseVisionInput([], 'usb')).toBeUndefined();
  });
});

describe('desktop hand controller', () => {
  const frame = (gesture: string, thumbX = 0.7, indexX = 0.45, indexY = 0.4): VisionFrame => {
    const hand = Array.from({ length: 21 }, () => ({ x: 0.5, y: 0.55, z: 0, visibility: 1 }));
    hand[0] = { x: 0.5, y: 0.8, z: 0, visibility: 1 };
    hand[9] = { x: 0.5, y: 0.55, z: 0, visibility: 1 };
    hand[4] = { x: thumbX, y: indexY, z: 0, visibility: 1 };
    hand[8] = { x: indexX, y: indexY, z: 0, visibility: 1 };
    return { faces: [], hands: [hand], gestures: [{ name: gesture, score: 0.95, handedness: 'Right' }], timestamp: 0 };
  };

  it('maps the mirrored index finger to bounded pointer coordinates', () => {
    const controller = new HandDesktopController();
    expect(controller.update(frame('Pointing_Up'), 0)).toEqual([
      expect.objectContaining({ kind: 'move', x: expect.any(Number), y: expect.any(Number) })
    ]);
  });

  it('turns a short pinch into one click and a held pinch into drag', () => {
    const controller = new HandDesktopController();
    controller.update(frame('None', 0.46), 100);
    expect(controller.update(frame('None', 0.46), 250)).toContainEqual({ kind: 'move', x: expect.any(Number), y: expect.any(Number) });
    expect(controller.update(frame('None'), 300)).toContainEqual({ kind: 'pointer', action: 'left-click' });
    controller.update(frame('None', 0.46), 500);
    expect(controller.update(frame('None', 0.46), 950)).toContainEqual({ kind: 'pointer', action: 'left-down' });
    expect(controller.update(frame('None'), 1_000)).toContainEqual({ kind: 'pointer', action: 'left-up' });
  });

  it('requires a prolonged unique gesture to close a window', () => {
    const controller = new HandDesktopController();
    controller.update(frame('ILoveYou'), 0);
    expect(controller.update(frame('ILoveYou'), 1_999)).not.toContainEqual({ kind: 'window', action: 'close' });
    expect(controller.update(frame('ILoveYou'), 2_000)).toContainEqual({ kind: 'window', action: 'close' });
    expect(controller.update(frame('ILoveYou'), 4_500)).not.toContainEqual({ kind: 'window', action: 'close' });
  });
});

describe('gesture command gate', () => {
  const gesture = (name: string, score = 0.9): VisionGesture => ({ name, score, handedness: 'Right' });

  it('requires one stable high-confidence gesture', () => {
    const gate = new GestureCommandGate();
    expect(gate.update([gesture('Thumb_Up')], 100)).toBeUndefined();
    expect(gate.update([gesture('Thumb_Up')], 999)).toBeUndefined();
    expect(gate.update([gesture('Thumb_Up')], 1_000)).toBe('volume-up');
  });

  it('requires a neutral pause before repeating and rejects ambiguous input', () => {
    const gate = new GestureCommandGate();
    gate.update([gesture('Closed_Fist')], 0);
    expect(gate.update([gesture('Closed_Fist')], 900)).toBe('mute');
    expect(gate.update([gesture('Closed_Fist')], 3_000)).toBeUndefined();
    expect(gate.update([], 3_100)).toBeUndefined();
    expect(gate.update([], 3_550)).toBeUndefined();
    expect(gate.update([gesture('Closed_Fist'), gesture('Thumb_Up')], 3_600)).toBeUndefined();
    expect(gate.update([gesture('Thumb_Down', 0.7)], 4_000)).toBeUndefined();
    expect(gate.update([gesture('Thumb_Down')], 4_100)).toBeUndefined();
    expect(gate.update([gesture('Thumb_Down')], 5_000)).toBe('volume-down');
  });
});
