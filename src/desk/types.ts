// The desk as the Rust core sends it (src-tauri/src/desk.rs, platform/mod.rs). Field names are the camelCase
// serde gives them; keep the two in step.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface WindowInfo {
  id: number;
  pid: number;
  app: string;
  appId: string | null;
  /** Empty when macOS won't say. */
  title: string;
  frame: Rect;
  minimized: boolean;
  fullScreen: boolean;
  /** False for a window on another Mission Control desktop. */
  onCurrentDesktop: boolean;
  desktop: number | null;
  focused: boolean;
  /** Stacking order, 0 = front-most. */
  order: number;
}

/** A Mission Control desktop or a window's full-screen space. */
export interface Desktop {
  id: number;
  display: string;
  index: number;
  fullScreen: boolean;
  current: boolean;
}

export interface DeskDisplay {
  id: string;
  name: string;
  frame: Rect;
  main: boolean;
  /** False for a ghost: part of the current setup, not connected now. */
  connected: boolean;
  windows: WindowInfo[];
  desktops: Desktop[];
}

export interface SetupSummary {
  id: string;
  name: string;
  displayNames: string[];
}

export interface Desk {
  displays: DeskDisplay[];
  offscreen: WindowInfo[];
  bounds: Rect;
  setup: SetupSummary | null;
  otherSetups: SetupSummary[];
}

export type Action =
  | { kind: 'focus'; window: number }
  | { kind: 'minimize'; window: number }
  | { kind: 'close'; window: number }
  | { kind: 'bringAppForward'; pid: number };
