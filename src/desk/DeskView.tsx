// The Desk view (PLAN.md §8): every display in its real arrangement, scaled to fit, with the windows of the
// desktop showing on each drawn in stacking order, as outlines. A window's title shows only where its top-left
// corner isn't covered, so labels never pile up; hovering names the window under the pointer, and a click lists
// every window stacked at that spot, front to back. Zoom with the toolbar's Zoom slider, a pinch, or ⌘+/⌘−; once
// zoomed the desk scrolls (scrollbars, wheel, two-finger scroll); double-click a display to zoom to it, Fit all to
// go back. Moving and resizing windows here is M2.

import { Fragment, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { AppBadge } from './ListView';
import { appAndTitle, desktopNames, emptyDesktopHint, resolution, windowLabel } from './format';
import type { Action, Desk, DeskDisplay, Rect, WindowInfo } from './types';

const PAD = 24;
/** Room above each display for its name. */
const LABEL_H = 20;
export const MAX_ZOOM = 8;
export const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(1, z));
/** A desk point and where in the viewport it should stay when the zoom changes. */
interface Anchor {
  dx: number;
  dy: number;
  px: number;
  py: number;
}
/** Up to this many of a kind show as chips on the display; more collapse to one count chip with a list. */
const TRAY_SINGLES = 2;
/** Past this many entries the tray list gets a filter box. */
const LIST_FILTER_FROM = 8;
/** A window's title label, in canvas pixels: shown only where this much of its top-left is clear. */
const TAG_H = 18;
const TAG_MIN_W = 48;
const TAG_MAX_W = 220;
/** How solid windows are drawn, 0 (outlines: every window's position shows) to 100 (solid: only what's in
 * front shows). Kept per viewer; `?fill=` sets it for a screenshot. */
export const FILL_KEY = 'mingdesk.windowFill';
export function initialFill(): number {
  const asked = Number(new URLSearchParams(window.location.search).get('fill'));
  if (asked >= 0 && asked <= 100 && new URLSearchParams(window.location.search).has('fill')) return asked;
  try {
    const v = Number(localStorage.getItem(FILL_KEY));
    return localStorage.getItem(FILL_KEY) !== null && v >= 0 && v <= 100 ? v : 0;
  } catch {
    return 0;
  }
}

type TrayKind = 'away' | 'behind' | 'min';
interface TrayItem {
  key: string;
  label: string;
  app: string | null;
  /** The app's id, for its icon. */
  appId: string | null;
  /** The window's own title, with the app's name trimmed off the end, for the list. */
  short: string;
  title: string;
  /** The window to focus; null for an empty desktop. */
  window: number | null;
  /** Where the window sits, for the list's little map; null for a desktop. */
  frame: Rect | null;
  /** A heading shown above this entry (the stack list's sections). */
  heading?: string;
}
interface TrayGroup {
  kind: TrayKind;
  noun: string;
  items: TrayItem[];
}

/** A display's hidden windows (or other desktops, or minimized windows) as a scrollable list, grouped by app,
 * each with a little map of where its window sits, since titles often repeat ("New Tab"). */
function TrayList({
  group,
  title,
  screen,
  ordered = false,
  onPeek,
  onPick,
}: {
  group: TrayGroup;
  title: string;
  screen: Rect;
  /** Keep the given order (front to back) under the items' headings instead of grouping by app. */
  ordered?: boolean;
  onPeek: (id: number | null) => void;
  onPick: (id: number) => void;
}) {
  const [filter, setFilter] = useState('');
  const f = filter.trim().toLowerCase();
  const items = group.items.filter((it) => !f || it.label.toLowerCase().includes(f));
  const apps = ordered ? [null] : [...new Set(items.map((it) => it.app))];
  return (
    <>
      <div className="tray-list-head">{title}</div>
      {group.items.length > LIST_FILTER_FROM && (
        <input
          className="filter"
          type="search"
          placeholder="Filter by title"
          aria-label="Filter this list"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      )}
      <ul className="tray-list-rows" onMouseLeave={() => onPeek(null)}>
        {apps.map((app) => {
          const mine = ordered ? items : items.filter((it) => it.app === app);
          return (
            <Fragment key={app ?? 'desktops'}>
              {app && apps.length > 1 && (
                <li className="tray-list-app">
                  {app} <span className="count">{mine.length}</span>
                </li>
              )}
              {mine.map((it) => (
                <Fragment key={it.key}>
                {it.heading && <li className="tray-list-app">{it.heading}</li>}
                <li>
                  <button
                    type="button"
                    disabled={it.window === null}
                    title={it.title}
                    onMouseEnter={() => onPeek(it.window)}
                    onFocus={() => onPeek(it.window)}
                    onClick={() => it.window !== null && onPick(it.window)}
                  >
                    {it.app && <AppBadge app={it.app} appId={it.appId} />}
                    <span className="tray-list-label">{it.short}</span>
                    {it.frame && (
                      <>
                        <span className="tray-list-size">
                          {Math.round(it.frame.w)}×{Math.round(it.frame.h)}
                        </span>
                        <MiniMap screen={screen} frame={it.frame} />
                      </>
                    )}
                  </button>
                </li>
                </Fragment>
              ))}
            </Fragment>
          );
        })}
        {items.length === 0 && <li className="muted empty">No match.</li>}
      </ul>
    </>
  );
}

/** A display drawn small with one window in it: tells apart windows whose titles match. */
function MiniMap({ screen, frame }: { screen: Rect; frame: Rect }) {
  const W = 40;
  const H = Math.max(14, Math.round((W * screen.h) / Math.max(screen.w, 1)));
  const k = W / Math.max(screen.w, 1);
  return (
    <svg className="minimap" width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      <rect x="0.5" y="0.5" width={W - 1} height={H - 1} rx="2" className="minimap-screen" />
      <rect
        x={Math.max(0, (frame.x - screen.x) * k)}
        y={Math.max(0, (frame.y - screen.y) * k)}
        width={Math.max(2, Math.min(W, frame.w * k))}
        height={Math.max(2, Math.min(H, frame.h * k))}
        rx="1"
        className="minimap-window"
      />
    </svg>
  );
}

/** "New Tab - Google Chrome - Keith (Work)" → "New Tab": the app is already shown. */
function trimApp(title: string, app: string): string {
  const at = title.indexOf(` - ${app}`);
  const cut = at > 0 ? title.slice(0, at) : title;
  return cut.trim() || title;
}

export function DeskView({
  desk,
  fill,
  zoom,
  onZoom,
  onAct,
}: {
  desk: Desk;
  fill: number;
  /** 1 fits the whole desk; up to MAX_ZOOM. Owned by the board, whose toolbar has the Zoom slider. */
  zoom: number;
  onZoom: (z: number) => void;
  onAct: (a: Action) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 500 });
  // A covered window whose "behind" chip or list entry is hovered: drawn as an outline where it sits.
  const [peek, setPeek] = useState<number | null>(null);
  // The tray list open, if any: one display's other desktops, hidden windows or minimized windows.
  const [open, setOpen] = useState<{ display: string; kind: TrayKind } | null>(null);
  // The window under the pointer, outlined and named.
  const [hot, setHot] = useState<{ id: number; x: number; y: number } | null>(null);
  // The stack list open, if any: every window at the spot clicked (desk coordinates), front to back.
  const [stack, setStack] = useState<{ display: string; x: number; y: number; left: number; top: number } | null>(null);

  function trayFor(d: DeskDisplay): TrayGroup[] {
    const byApp = (a: WindowInfo, b: WindowInfo) => a.app.localeCompare(b.app) || windowLabel(a).localeCompare(windowLabel(b), undefined, { numeric: true });
    const groups: TrayGroup[] = [
      {
        kind: 'away',
        noun: 'other desktops',
        items: d.desktops
          .filter((x) => !x.current)
          .map((x) => {
            const first = d.windows.find((w) => w.desktop === x.id && !w.minimized);
            return {
              key: `x${x.id}`,
              label: `${x.fullScreen ? '⛶' : '▭'} ${names.get(x.id)}`,
              app: null,
              appId: null,
              short: `${x.fullScreen ? '⛶' : '▭'} ${names.get(x.id)}`,
              frame: null,
              title: first ? `Go to ${names.get(x.id)}` : emptyDesktopHint(),
              window: first?.id ?? null,
            };
          }),
      },
      {
        kind: 'behind',
        noun: 'behind',
        items: d.covered
          .map((id) => d.windows.find((w) => w.id === id))
          .filter((w): w is WindowInfo => w !== undefined)
          .sort(byApp)
          .map((w) => ({
            key: `w${w.id}`,
            label: `◫ ${appAndTitle(w)}`,
            app: w.app,
            appId: w.appId,
            short: trimApp(windowLabel(w), w.app),
            frame: w.frame,
            title: `Behind other windows: ${windowLabel(w)}. Click to bring it forward.`,
            window: w.id,
          })),
      },
      {
        kind: 'min',
        noun: 'minimized',
        items: d.windows
          .filter((w) => w.minimized)
          .sort(byApp)
          .map((w) => ({
            key: `w${w.id}`,
            label: `▁ ${appAndTitle(w)}`,
            app: w.app,
            appId: w.appId,
            short: trimApp(windowLabel(w), w.app),
            title: `Restore ${windowLabel(w)}`,
            window: w.id,
            frame: w.frame,
          })),
      },
    ];
    return groups.filter((g) => g.items.length > 0);
  }

  // Esc or a click outside closes the tray list.
  const popRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open && !stack) return;
    const close = () => {
      setOpen(null);
      setStack(null);
      setPeek(null);
    };
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && close();
    const onDown = (e: MouseEvent) => {
      const t = e.target as Element;
      if (!popRef.current?.contains(t) && !t.closest('.count-chip')) close();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('mousedown', onDown);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('mousedown', onDown);
    };
  }, [open, stack]);
  const names = desktopNames(desk);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    // ResizeObserver needs Safari 13.1; older WebKit (a Mojave Mac on Safari 12) follows the window instead.
    if (typeof ResizeObserver === 'undefined') {
      measure();
      window.addEventListener('resize', measure);
      return () => window.removeEventListener('resize', measure);
    }
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const b = desk.bounds;
  const fit = Math.min((size.w - PAD * 2) / Math.max(b.w, 1), (size.h - PAD * 2 - LABEL_H) / Math.max(b.h, 1));
  const s = fit * zoom;
  // Zoomed in, the drawing is bigger than the canvas and scrolls; it's centred while it fits.
  const worldW = Math.max(size.w, b.w * s + PAD * 2);
  const worldH = Math.max(size.h, b.h * s + PAD * 2 + LABEL_H);
  const ox = (worldW - b.w * s) / 2;
  const oy = (worldH - b.h * s + LABEL_H) / 2;
  const place = (r: Rect): React.CSSProperties => ({
    left: ox + (r.x - b.x) * s,
    top: oy + (r.y - b.y) * s,
    width: r.w * s,
    height: r.h * s,
  });

  // Each display's name sits just above it, clear of the windows drawn inside.
  const labelAt = (r: Rect): React.CSSProperties => {
    const p = place(r);
    return { left: p.left, top: (p.top as number) - LABEL_H, width: p.width };
  };

  // Zooming keeps a point of the desk still: the one under the pointer for a pinch, the display's centre for a
  // double-click, otherwise whatever is in the middle of the canvas. The middle is tracked as the canvas scrolls
  // because zooming out shrinks the drawing, and the browser clamps the scroll before we could read it.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;
  const geo = useRef({ s, ox, oy, mid: { dx: 0, dy: 0 } });
  const pending = useRef<Anchor | null>(null);
  const deskAt = (px: number, py: number) => {
    const sc = scroller.current!;
    const g = geo.current;
    return { dx: (sc.scrollLeft + px - g.ox) / g.s, dy: (sc.scrollTop + py - g.oy) / g.s };
  };
  const trackMiddle = () => scroller.current && (geo.current.mid = deskAt(size.w / 2, size.h / 2));
  const zoomAround = (z: number, px: number, py: number) => {
    pending.current = { ...deskAt(px, py), px, py };
    onZoom(clampZoom(z));
  };
  useLayoutEffect(() => {
    const sc = scroller.current;
    if (!sc) return;
    const a = pending.current ?? (geo.current.s !== s ? { ...geo.current.mid, px: size.w / 2, py: size.h / 2 } : null);
    pending.current = null;
    if (a) {
      sc.scrollLeft = a.dx * s + ox - a.px;
      sc.scrollTop = a.dy * s + oy - a.py;
    }
    geo.current = { s, ox, oy, mid: geo.current.mid };
    trackMiddle();
  });

  // A mouse's Ctrl+wheel comes in notches (deltaY about 100 each), which the pinch formula below turned into a
  // ×2.7 jump per notch (Keith on Windows, 2026-11-23). A notch is a 20% step instead, and the zoom glides there
  // over a few frames; notches in quick succession add up. Each frame keeps the point under the pointer still.
  const glide = useRef<{ target: number; z: number; px: number; py: number; frame: number } | null>(null);
  const glideBy = (notches: number, px: number, py: number) => {
    const g = glide.current;
    const from = g ? g.target : zoomRef.current;
    const target = clampZoom(from * Math.pow(1.2, notches));
    if (g) {
      Object.assign(g, { target, px, py });
      return;
    }
    const step = () => {
      const s = glide.current;
      if (!s) return;
      s.z += (s.target - s.z) * 0.35;
      const done = Math.abs(s.target - s.z) / s.target < 0.003;
      zoomAround(done ? s.target : s.z, s.px, s.py);
      if (done) glide.current = null;
      else s.frame = requestAnimationFrame(step);
    };
    glide.current = { target, z: zoomRef.current, px, py, frame: requestAnimationFrame(step) };
  };
  useEffect(
    () => () => {
      if (glide.current) cancelAnimationFrame(glide.current.frame);
    },
    [],
  );

  // A pinch zooms around the pointer: newer WebKit sends it as ctrl+wheel, older WebKit (Mojave's) only as
  // Safari's own gesture events. Plain scrolling is left to the browser.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let start = 1;
    let pinching = false;
    const at = (e: { clientX: number; clientY: number }) => {
      const r = el.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top] as const;
    };
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey || (e.target as Element).closest('.tray-list')) return;
      e.preventDefault();
      if (pinching) return;
      // A mouse notch: line-based, or a big pixel step a pinch never sends.
      if (e.deltaMode !== 0 || Math.abs(e.deltaY) >= 40) {
        const lines = e.deltaMode === 0 ? e.deltaY / 100 : e.deltaY / 3;
        const notches = -Math.sign(lines) * Math.max(1, Math.round(Math.abs(lines)));
        glideBy(notches, ...at(e));
        return;
      }
      zoomAround(zoomRef.current * Math.exp(-e.deltaY / 100), ...at(e));
    };
    type Gesture = Event & { scale: number; clientX: number; clientY: number };
    const onStart = (e: Event) => {
      e.preventDefault();
      pinching = true;
      start = zoomRef.current;
    };
    const onChange = (e: Event) => {
      e.preventDefault();
      const g = e as Gesture;
      zoomAround(start * g.scale, ...at(g));
    };
    const onEnd = (e: Event) => {
      e.preventDefault();
      pinching = false;
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    el.addEventListener('gesturestart', onStart);
    el.addEventListener('gesturechange', onChange);
    el.addEventListener('gestureend', onEnd);
    return () => {
      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('gesturestart', onStart);
      el.removeEventListener('gesturechange', onChange);
      el.removeEventListener('gestureend', onEnd);
    };
  });

  /** The windows drawn on a display, front to back. */
  const drawnOn = (d: DeskDisplay) =>
    d.windows.filter((w) => w.onCurrentDesktop && !w.minimized).sort((a, c) => a.order - c.order);
  const inside = (r: Rect, x: number, y: number) => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
  /** A point on a display, in desk coordinates, from a mouse event on it. */
  const deskPoint = (d: DeskDisplay, e: React.MouseEvent) => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: d.frame.x + (e.clientX - r.left) / s, y: d.frame.y + (e.clientY - r.top) / s };
  };
  /** How wide a title label can be at a window's top-left corner before a window in front covers it (canvas px). */
  function tagWidth(w: WindowInfo, front: WindowInfo[]): number {
    const r = w.frame;
    const h = TAG_H / s;
    let free = Math.min(r.w, TAG_MAX_W / s);
    for (const f of front) {
      const g = f.frame;
      if (g.y >= r.y + h || g.y + g.h <= r.y) continue; // misses the label's row
      if (g.x <= r.x && g.x + g.w > r.x) return 0; // covers the corner
      if (g.x > r.x) free = Math.min(free, g.x - r.x);
    }
    return free * s;
  }

  /** A window's box inside its display's drawing, clipped to the display so a maximized window's outline shows on
   * all four sides (the display's 1px border takes 2px off the room inside). */
  function inDisplay(r: Rect, d: DeskDisplay): React.CSSProperties {
    const W = d.frame.w * s - 2;
    const H = d.frame.h * s - 2;
    const x0 = Math.max(0, (r.x - d.frame.x) * s);
    const y0 = Math.max(0, (r.y - d.frame.y) * s);
    const x1 = Math.min(W, (r.x + r.w - d.frame.x) * s);
    const y1 = Math.min(H, (r.y + r.h - d.frame.y) * s);
    return { left: x0, top: y0, width: Math.max(x1 - x0, 8), height: Math.max(y1 - y0, 8) };
  }

  // Dragging moves around a zoomed desk, for a mouse, which has no two-finger scroll (Keith on Windows,
  // 2026-11-23). Ctrl+drag or a middle-button drag work anywhere; a plain drag only from empty space, because in
  // M2 a plain drag on a window moves the window. A drag isn't a click: it doesn't open the spot's list.
  const pan = useRef<{ x: number; y: number; left: number; top: number; moved: boolean } | null>(null);
  const justPanned = useRef(false);
  const [panning, setPanning] = useState(false);
  const startPan = (e: React.PointerEvent) => {
    const sc = scroller.current;
    if (zoom <= 1 || !sc || (e.button !== 0 && e.button !== 1)) return;
    const t = e.target as Element;
    if (t.closest('button')) return;
    if (e.button === 0 && !e.ctrlKey && t.closest('.win')) return;
    pan.current = { x: e.clientX, y: e.clientY, left: sc.scrollLeft, top: sc.scrollTop, moved: false };
  };
  const movePan = (e: React.PointerEvent) => {
    const p = pan.current;
    const sc = scroller.current;
    if (!p || !sc) return;
    const dx = e.clientX - p.x;
    const dy = e.clientY - p.y;
    if (!p.moved) {
      if (Math.hypot(dx, dy) < 4) return; // a click that wobbled, not a drag
      p.moved = true;
      setPanning(true);
      sc.setPointerCapture(e.pointerId);
    }
    sc.scrollLeft = p.left - dx;
    sc.scrollTop = p.top - dy;
  };
  const endPan = (e: React.PointerEvent) => {
    if (pan.current?.moved) {
      justPanned.current = true;
      scroller.current?.releasePointerCapture(e.pointerId);
    }
    pan.current = null;
    setPanning(false);
  };

  function zoomTo(d: DeskDisplay) {
    setStack(null);
    const z = clampZoom(Math.min((size.w - PAD * 2) / d.frame.w, (size.h - PAD * 2 - LABEL_H) / d.frame.h) / fit);
    // Put the display's centre in the middle of the canvas.
    pending.current = { dx: d.frame.x - b.x + d.frame.w / 2, dy: d.frame.y - b.y + d.frame.h / 2, px: size.w / 2, py: size.h / 2 };
    onZoom(z);
  }

  return (
    <div className="deskview">
      <div className={`canvas${zoom > 1 ? ' zoomed' : ''}`} ref={box} style={{ '--win-fill': fill / 100 } as React.CSSProperties} onKeyDown={(e) => {
        if (e.key === '0') onZoom(1);
        else if (e.key === '+' || e.key === '=') zoomAround(zoomRef.current * 1.25, size.w / 2, size.h / 2);
        else if (e.key === '-') zoomAround(zoomRef.current / 1.25, size.w / 2, size.h / 2);
      }}>
        {/* The canvas's own controls sit on it, as on a map: the hint while it's wide enough, Fit all once zoomed. */}
        {size.w >= 560 && (
          <div className="canvas-hint" aria-hidden="true">
            Click a spot to list its windows · pinch or slide Zoom{zoom > 1 ? ' · scroll or Ctrl+drag to move around' : ''} · double-click a display to fit it
          </div>
        )}
        {zoom > 1 && (
          <button type="button" className="small canvas-fit" onClick={() => onZoom(1)}>
            Fit all
          </button>
        )}
        <div
          className={`canvas-scroll${zoom > 1 ? ' zoomed' : ''}${panning ? ' panning' : ''}`}
          ref={scroller}
          onPointerDown={startPan}
          onPointerMove={movePan}
          onPointerUp={endPan}
          onPointerCancel={endPan}
          // The middle button would otherwise start the browser's own autoscroll.
          onMouseDown={(e) => zoom > 1 && e.button === 1 && e.preventDefault()}
          onClickCapture={(e) => {
            if (!justPanned.current) return;
            justPanned.current = false;
            e.stopPropagation();
          }}
          onScroll={() => {
            trackMiddle();
            // Lists and names pinned to a spot would drift off it: close them.
            setHot(null);
            setOpen(null);
            setStack(null);
          }}
        >
        <div className="canvas-world" style={{ width: worldW, height: worldH }}>
        {desk.displays.map((d) => {
          const current = d.desktops.find((x) => x.current);
          const front = drawnOn(d);
          const drawn = [...front].reverse(); // back to front
          const peeking = d.windows.find((w) => w.id === peek);
          const hotWin = hot && front.find((w) => w.id === hot.id);
          return (
            <Fragment key={d.id}>
            <div className="screen-label" style={{ ...labelAt(d.frame) }} onDoubleClick={() => zoomTo(d)}>
              <span>{d.name}</span>
              <span className="meta">{resolution(d)}</span>
              {d.main && <span className="badge">main</span>}
              {!d.connected && <span className="badge">not connected</span>}
              {current && d.desktops.length > 1 && <span className="meta"> · {names.get(current.id)}</span>}
            </div>
            <div
              className={`screen${d.connected ? '' : ' ghost'}`}
              style={place(d.frame)}
              onDoubleClick={() => zoomTo(d)}
              onMouseMove={(e) => {
                const p = deskPoint(d, e);
                const w = front.find((x) => inside(x.frame, p.x, p.y));
                setHot(w ? { id: w.id, x: (p.x - d.frame.x) * s, y: (p.y - d.frame.y) * s } : null);
              }}
              onMouseLeave={() => setHot(null)}
              onClick={(e) => {
                if ((e.target as Element).closest('button')) return;
                const p = deskPoint(d, e);
                if (!front.some((w) => inside(w.frame, p.x, p.y))) return;
                const c = box.current!.getBoundingClientRect();
                setOpen(null);
                setStack({ display: d.id, x: p.x, y: p.y, left: e.clientX - c.left, top: e.clientY - c.top });
              }}
              aria-label={`${d.name}${d.connected ? '' : ', not connected'}`}
            >
              {drawn.map((w, i) => {
                const r = w.frame;
                const tag = tagWidth(w, front.slice(0, front.length - 1 - i));
                return (
                  <div
                    key={w.id}
                    className={`win${w.focused ? ' focused' : ''}${hotWin?.id === w.id ? ' hot' : ''}`}
                    style={{ ...inDisplay(r, d), zIndex: i + 1 }}
                  >
                    {tag >= TAG_MIN_W && (
                      <button
                        type="button"
                        className="win-tag"
                        style={{ maxWidth: tag }}
                        title={`${windowLabel(w)} — ${w.app}. Click to bring it forward.`}
                        onClick={() => onAct({ kind: 'focus', window: w.id })}
                        onDoubleClick={(e) => e.stopPropagation()}
                      >
                        <AppBadge app={w.app} appId={w.appId} />
                        <span className="win-title">{trimApp(windowLabel(w), w.app)}</span>
                      </button>
                    )}
                  </div>
                );
              })}
              {hotWin && !stack && (
                <div
                  className="hot-name"
                  style={{ left: Math.min(hot!.x + 12, d.frame.w * s - 40), top: hot!.y + 14 }}
                  aria-hidden="true"
                >
                  {appAndTitle(hotWin)}
                </div>
              )}
              {peeking && (
                <div
                  className="peek"
                  style={inDisplay(peeking.frame, d)}
                  aria-hidden="true"
                />
              )}
              {trayFor(d).length > 0 && (
                <div className="screen-tray">
                  {trayFor(d).map((g) =>
                    g.items.length <= TRAY_SINGLES ? (
                      g.items.map((it) => (
                        <button
                          key={it.key}
                          type="button"
                          className={`chip ${g.kind}`}
                          disabled={it.window === null}
                          onClick={() => it.window !== null && onAct({ kind: 'focus', window: it.window })}
                          onMouseEnter={() => g.kind === 'behind' && setPeek(it.window)}
                          onMouseLeave={() => setPeek(null)}
                          onFocus={() => g.kind === 'behind' && setPeek(it.window)}
                          onBlur={() => setPeek(null)}
                          title={it.title}
                        >
                          {it.label}
                        </button>
                      ))
                    ) : (
                      <button
                        key={g.kind}
                        type="button"
                        className={`chip ${g.kind} count-chip`}
                        aria-expanded={open?.display === d.id && open.kind === g.kind}
                        onClick={() =>
                          setOpen((o) => (o?.display === d.id && o.kind === g.kind ? null : { display: d.id, kind: g.kind }))
                        }
                      >
                        {g.items.length} {g.noun}
                      </button>
                    ),
                  )}
                </div>
              )}
            </div>
            </Fragment>
          );
        })}
        </div>
        </div>
        {open &&
          (() => {
            const d = desk.displays.find((x) => x.id === open.display);
            const g = d && trayFor(d).find((x) => x.kind === open.kind);
            if (!d || !g) return null;
            // The list sits over the canvas, not in the scrolling drawing: place it where the display shows now.
            const p = place(d.frame);
            const sx = scroller.current?.scrollLeft ?? 0;
            const sy = scroller.current?.scrollTop ?? 0;
            const width = Math.min(340, size.w - 16);
            const left = Math.max(8, Math.min((p.left as number) - sx + 4, size.w - width - 8));
            const top = Math.max(8, Math.min((p.top as number) - sy + 24, size.h - 220));
            return (
              <>
              <div className="scrim" aria-hidden="true" />
              <div ref={popRef} className="tray-list" style={{ left, top, width, maxHeight: size.h - top - 8 }} role="dialog" aria-label={`${d.name}: ${g.items.length} ${g.noun}`}>
                <TrayList
                  group={g}
                  screen={d.frame}
                  title={`${d.name} · ${g.items.length} ${g.noun}`}
                  onPeek={(id) => g.kind === 'behind' && setPeek(id)}
                  onPick={(id) => {
                    setOpen(null);
                    setPeek(null);
                    onAct({ kind: 'focus', window: id });
                  }}
                />
              </div>
              </>
            );
          })()}
        {stack &&
          (() => {
            const d = desk.displays.find((x) => x.id === stack.display);
            if (!d) return null;
            const front = drawnOn(d);
            const here = front.filter((w) => inside(w.frame, stack.x, stack.y));
            const rest = front.filter((w) => !here.includes(w));
            const item = (w: WindowInfo, heading?: string): TrayItem => ({
              key: `w${w.id}`,
              label: `${appAndTitle(w)}`,
              app: w.app,
              appId: w.appId,
              short: `${w.app} · ${trimApp(windowLabel(w), w.app)}`,
              title: `Bring forward: ${windowLabel(w)}`,
              window: w.id,
              frame: w.frame,
              heading,
            });
            const g: TrayGroup = {
              kind: 'behind',
              noun: 'windows',
              items: [
                ...here.map((w, i) => item(w, i === 0 ? 'Here, front to back' : undefined)),
                ...rest.map((w, i) => item(w, i === 0 ? 'Elsewhere on this display' : undefined)),
              ],
            };
            const width = Math.min(360, size.w - 16);
            const left = Math.max(8, Math.min(stack.left + 8, size.w - width - 8));
            const top = Math.max(8, Math.min(stack.top + 8, size.h - 220));
            return (
              <>
              <div className="scrim" aria-hidden="true" />
              <div ref={popRef} className="tray-list" style={{ left, top, width, maxHeight: size.h - top - 8 }} role="dialog" aria-label={`${d.name}: windows here`}>
                <TrayList
                  group={g}
                  ordered
                  screen={d.frame}
                  title={`${d.name} · ${here.length} ${here.length === 1 ? 'window' : 'windows'} here`}
                  onPeek={setPeek}
                  onPick={(id) => {
                    setStack(null);
                    setPeek(null);
                    onAct({ kind: 'focus', window: id });
                  }}
                />
              </div>
              </>
            );
          })()}
      </div>
      {desk.offscreen.length > 0 && (
        <div className="offscreen-note">
          <strong>Off-screen:</strong>{' '}
          {desk.offscreen.map((w) => (
            <button key={w.id} type="button" className="chip" onClick={() => onAct({ kind: 'focus', window: w.id })}>
              {appAndTitle(w)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
