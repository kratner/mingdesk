// A made-up desk for working on the board in a plain browser (npm run dev, or a screenshot), shaped like a
// real three-display desk: a laptop with two externals, desktops and full-screen remote-desktop sessions.
// Invented titles only: this repo is public. `?mock=ghost` unplugs one display; `?mock=office` is a single
// wide monitor with the lid closed.

import type { Desk, DeskDisplay, Desktop, Rect, WindowInfo } from './types';

const LAPTOP: Rect = { x: 0, y: 0, w: 2048, h: 1280 };
const MIDDLE: Rect = { x: 2048, y: 0, w: 1920, h: 1080 };
const RIGHT: Rect = { x: 3968, y: 0, w: 1920, h: 1080 };

let nextId = 100;
function win(app: string, title: string, frame: Rect, more: Partial<WindowInfo> = {}): WindowInfo {
  const id = nextId++;
  return {
    id,
    pid: 500 + app.length,
    app,
    appId: `com.example.${app.toLowerCase().replace(/\s+/g, '')}`,
    title,
    frame,
    minimized: false,
    fullScreen: false,
    onCurrentDesktop: true,
    desktop: null,
    focused: false,
    order: id,
    ...more,
  };
}

function desktops(display: string, list: [number, boolean, boolean][]): Desktop[] {
  return list.map(([id, fullScreen, current], i) => ({ id, display, index: i + 1, fullScreen, current }));
}

function home(): Desk {
  const laptop: DeskDisplay = {
    id: 'LAPTOP',
    name: 'Built-in Display',
    frame: LAPTOP,
    main: true,
    connected: true,
    desktops: desktops('LAPTOP', [[1, false, true]]),
    windows: [
      win('Mail', 'Inbox – 3 messages', { x: 240, y: 180, w: 1100, h: 760 }, { desktop: 1 }),
      win('Notes', 'Groceries', { x: 860, y: 300, w: 700, h: 640 }, { desktop: 1 }),
      win('Calendar', 'November 2026', { x: 120, y: 90, w: 1300, h: 900 }, { desktop: 1, minimized: true }),
    ],
  };
  const middle: DeskDisplay = {
    id: 'MIDDLE',
    name: 'Studio 27',
    frame: MIDDLE,
    main: false,
    connected: true,
    desktops: desktops('MIDDLE', [
      [59, false, true],
      [70, true, false],
      [66, true, false],
    ]),
    windows: [
      win('Code', 'mingdesk — App.tsx', { x: 2048, y: 30, w: 1920, h: 1050 }, { desktop: 59, focused: true }),
      win('Safari', 'Tauri 2 docs – Window', { x: 2140, y: 60, w: 1500, h: 980 }, { desktop: 59 }),
      win('RustDesk', '111222333@office-pc - Remote Desktop - RustDesk', MIDDLE, {
        desktop: 70,
        fullScreen: true,
        onCurrentDesktop: false,
      }),
      win('RustDesk', '444555666@old-laptop.local - Remote Desktop - RustDesk', MIDDLE, {
        desktop: 66,
        fullScreen: true,
        onCurrentDesktop: false,
      }),
    ],
  };
  const right: DeskDisplay = {
    id: 'RIGHT',
    name: 'Side 24',
    frame: RIGHT,
    main: false,
    connected: true,
    desktops: desktops('RIGHT', [
      [56, false, true],
      [74, true, false],
    ]),
    windows: [
      win('Finder', 'Downloads', { x: 4073, y: 122, w: 994, h: 436 }, { desktop: 56 }),
      win('Terminal', 'zsh — 120×40', { x: 4390, y: 397, w: 1074, h: 649 }, { desktop: 56 }),
      win('Preview', 'Quarterly plan.pdf', { x: 4900, y: 160, w: 900, h: 880 }, { desktop: 56 }),
      win('RustDesk', '111222333@office-pc - Remote Desktop - RustDesk', RIGHT, {
        desktop: 74,
        fullScreen: true,
        onCurrentDesktop: false,
      }),
    ],
  };
  const displays = [laptop, middle, right];
  displays.forEach((d) =>
    d.windows.sort((a, b) => Number(a.minimized) - Number(b.minimized) || a.order - b.order),
  );
  return {
    displays,
    offscreen: [win('TextEdit', 'Left on the unplugged monitor.txt', { x: 9000, y: 200, w: 600, h: 400 })],
    bounds: { x: 0, y: 0, w: RIGHT.x + RIGHT.w, h: LAPTOP.h },
    setup: {
      id: 'LAPTOP,MIDDLE,RIGHT',
      name: 'Home',
      displayNames: ['Built-in Display', 'Studio 27', 'Side 24'],
    },
    otherSetups: [{ id: 'WIDE', name: 'Office', displayNames: ['Wide 49'] }],
  };
}

function ghost(): Desk {
  const desk = home();
  const right = desk.displays[2];
  desk.displays[2] = { ...right, connected: false, windows: [], desktops: [] };
  return desk;
}

function office(): Desk {
  const wide: Rect = { x: 0, y: 0, w: 5120, h: 1440 };
  return {
    displays: [
      {
        id: 'WIDE',
        name: 'Wide 49',
        frame: wide,
        main: true,
        connected: true,
        desktops: desktops('WIDE', [[1, false, true]]),
        windows: [
          win('Code', 'mingdesk — desk.rs', { x: 0, y: 30, w: 2560, h: 1410 }, { desktop: 1, focused: true }),
          win('Safari', 'Pull request #12', { x: 2560, y: 30, w: 1600, h: 1410 }, { desktop: 1 }),
          win('Slack', 'general', { x: 4160, y: 30, w: 960, h: 1410 }, { desktop: 1 }),
        ],
      },
    ],
    offscreen: [],
    bounds: wide,
    setup: { id: 'WIDE', name: 'Office', displayNames: ['Wide 49'] },
    otherSetups: [{ id: 'LAPTOP,MIDDLE,RIGHT', name: 'Home', displayNames: ['Built-in Display', 'Studio 27', 'Side 24'] }],
  };
}

export function mockDesk(kind: string | null): Desk {
  if (kind === 'ghost') return ghost();
  if (kind === 'office') return office();
  return home();
}
