// The board's line to the Rust core. Outside Tauri (a plain browser: npm run dev, screenshots) it works on the
// made-up desk in mock.ts instead, so the board can be developed and checked without touching real windows.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { mockDesk } from './mock';
import type { Action, Desk } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

let mock: Desk | null = null;
const mockListeners = new Set<(d: Desk) => void>();
function mockState(): Desk {
  if (!mock) mock = mockDesk(new URLSearchParams(window.location.search).get('mock'));
  return mock;
}
function mockChanged() {
  const d = structuredClone(mockState());
  mockListeners.forEach((f) => f(d));
}

export async function getDesk(): Promise<Desk> {
  if (inTauri) return invoke<Desk>('desk');
  return structuredClone(mockState());
}

/** Calls `onChange` with every new desk the core reports. Returns a function that stops listening. */
export async function watchDesk(onChange: (d: Desk) => void): Promise<() => void> {
  if (inTauri) return listen<Desk>('desk', (e) => onChange(e.payload));
  mockListeners.add(onChange);
  return () => mockListeners.delete(onChange);
}

export async function act(action: Action): Promise<void> {
  if (inTauri) return invoke('act', { action });
  const desk = mockState();
  const all = [...desk.displays.flatMap((d) => d.windows), ...desk.offscreen];
  if (action.kind === 'bringAppForward') return;
  const w = all.find((x) => x.id === action.window);
  if (!w) throw new Error('That window has closed.');
  if (action.kind === 'minimize') w.minimized = true;
  if (action.kind === 'focus') {
    all.forEach((x) => (x.focused = false));
    Object.assign(w, { minimized: false, focused: true, order: -1 });
  }
  if (action.kind === 'close') {
    desk.displays.forEach((d) => (d.windows = d.windows.filter((x) => x.id !== w.id)));
    desk.offscreen = desk.offscreen.filter((x) => x.id !== w.id);
  }
  mockChanged();
}

export async function adoptSetup(): Promise<void> {
  if (inTauri) return invoke('adopt_setup');
  const desk = mockState();
  desk.otherSetups.unshift(desk.setup!);
  desk.displays = desk.displays.filter((d) => d.connected);
  desk.setup = { id: 'adopted', name: desk.displays.map((d) => d.name).join(' + '), displayNames: desk.displays.map((d) => d.name) };
  mockChanged();
}

export async function renameSetup(id: string, name: string): Promise<void> {
  if (inTauri) return invoke('rename_setup', { id, name });
  const desk = mockState();
  [desk.setup, ...desk.otherSetups].forEach((s) => s && s.id === id && (s.name = name.trim() || s.name));
  mockChanged();
}

export async function forgetSetup(id: string): Promise<void> {
  if (inTauri) return invoke('forget_setup', { id });
  const desk = mockState();
  desk.otherSetups = desk.otherSetups.filter((s) => s.id !== id);
  mockChanged();
}
