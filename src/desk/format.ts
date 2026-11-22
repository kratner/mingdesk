// Words and numbers the List and Desk views share.

import type { Desk, DeskDisplay, Desktop, WindowInfo } from './types';

export function windowLabel(w: WindowInfo): string {
  return w.title.trim() || `${w.app} window`;
}

export function resolution(d: DeskDisplay): string {
  return `${Math.round(d.frame.w)} × ${Math.round(d.frame.h)}`;
}

const lastFullScreenNames = new Map<number, string>();

/**
 * What Mission Control calls each desktop: ordinary desktops are numbered across all displays ("Desktop 1" on
 * the first display, "Desktop 2" on the next…); a full-screen space is named after its window.
 */
export function desktopNames(desk: Desk): Map<number, string> {
  const names = new Map<number, string>();
  const byDesktop = new Map<number, WindowInfo>();
  desk.displays.forEach((d) => d.windows.forEach((w) => w.desktop != null && !byDesktop.has(w.desktop) && byDesktop.set(w.desktop, w)));
  let n = 0;
  for (const d of desk.displays) {
    for (const x of d.desktops) {
      if (x.fullScreen) {
        const w = byDesktop.get(x.id);
        // Mid-swipe the window server briefly doesn't say which window a full-screen space holds; keep the name
        // it had rather than flashing "Full screen".
        const name = w ? `${w.app}: ${shortTitle(w)}` : (lastFullScreenNames.get(x.id) ?? 'Full screen');
        if (w) lastFullScreenNames.set(x.id, name);
        names.set(x.id, name);
      } else {
        names.set(x.id, `Desktop ${++n}`);
      }
    }
  }
  return names;
}

/** A remote-desktop title like "111222333@office-pc - Remote Desktop - RustDesk" shortened to "111222333@office-pc". */
export function shortTitle(w: WindowInfo): string {
  const t = windowLabel(w);
  const cut = t.indexOf(' - ');
  return cut > 0 ? t.slice(0, cut) : t;
}

export function windowsOn(d: DeskDisplay, x: Desktop): WindowInfo[] {
  return d.windows.filter((w) => w.desktop === x.id && !w.minimized);
}

export function matches(w: WindowInfo, filter: string): boolean {
  const f = filter.trim().toLowerCase();
  if (!f) return true;
  return f.split(/\s+/).every((word) => `${w.app} ${w.title}`.toLowerCase().includes(word));
}

/** A steady colour per app for its badge, from the app's name. */
export function appHue(app: string): number {
  let h = 0;
  for (const c of app) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}
