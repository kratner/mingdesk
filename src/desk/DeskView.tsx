// The Desk view (PLAN.md §8): every display in its real arrangement, scaled to fit, with the windows of the
// desktop showing on each drawn in stacking order. Trackpad first: pinch to zoom, two-finger scroll to pan,
// double-click a display to zoom to it, Fit all to go back. Moving and resizing windows here is M2.

import { Fragment, useEffect, useRef, useState } from 'react';
import { AppBadge } from './ListView';
import { desktopNames, resolution, windowLabel } from './format';
import type { Action, Desk, DeskDisplay, Rect, WindowInfo } from './types';

const PAD = 24;
/** Room above each display for its name. */
const LABEL_H = 20;
const MAX_ZOOM = 8;

interface View {
  zoom: number;
  x: number;
  y: number;
}
const FIT: View = { zoom: 1, x: 0, y: 0 };
/** Up to this many of a kind show as chips on the display; more collapse to one count chip with a list. */
const TRAY_SINGLES = 2;
/** Past this many entries the tray list gets a filter box. */
const LIST_FILTER_FROM = 8;

type TrayKind = 'away' | 'behind' | 'min';
interface TrayItem {
  key: string;
  label: string;
  app: string | null;
  /** The window's own title, with the app's name trimmed off the end, for the list. */
  short: string;
  title: string;
  /** The window to focus; null for an empty desktop. */
  window: number | null;
  /** Where the window sits, for the list's little map; null for a desktop. */
  frame: Rect | null;
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
  onPeek,
  onPick,
}: {
  group: TrayGroup;
  title: string;
  screen: Rect;
  onPeek: (id: number | null) => void;
  onPick: (id: number) => void;
}) {
  const [filter, setFilter] = useState('');
  const f = filter.trim().toLowerCase();
  const items = group.items.filter((it) => !f || it.label.toLowerCase().includes(f));
  const apps = [...new Set(items.map((it) => it.app))];
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
          const mine = items.filter((it) => it.app === app);
          return (
            <Fragment key={app ?? 'desktops'}>
              {app && apps.length > 1 && (
                <li className="tray-list-app">
                  {app} <span className="count">{mine.length}</span>
                </li>
              )}
              {mine.map((it) => (
                <li key={it.key}>
                  <button
                    type="button"
                    disabled={it.window === null}
                    title={it.title}
                    onMouseEnter={() => onPeek(it.window)}
                    onFocus={() => onPeek(it.window)}
                    onClick={() => it.window !== null && onPick(it.window)}
                  >
                    {it.app && <AppBadge app={it.app} />}
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

export function DeskView({ desk, onAct }: { desk: Desk; onAct: (a: Action) => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 500 });
  const [view, setView] = useState<View>(FIT);
  // A covered window whose "behind" chip or list entry is hovered: drawn as an outline where it sits.
  const [peek, setPeek] = useState<number | null>(null);
  // The tray list open, if any: one display's other desktops, hidden windows or minimized windows.
  const [open, setOpen] = useState<{ display: string; kind: TrayKind } | null>(null);

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
              short: `${x.fullScreen ? '⛶' : '▭'} ${names.get(x.id)}`,
              frame: null,
              title: first ? `Go to ${names.get(x.id)}` : 'macOS gives no way to switch to an empty desktop',
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
            label: `◫ ${w.app}: ${windowLabel(w)}`,
            app: w.app,
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
            label: `▁ ${w.app}: ${windowLabel(w)}`,
            app: w.app,
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
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(null);
    const onDown = (e: MouseEvent) => {
      const t = e.target as Element;
      if (!popRef.current?.contains(t) && !t.closest('.count-chip')) setOpen(null);
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('mousedown', onDown);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('mousedown', onDown);
    };
  }, [open]);
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
  const s = fit * view.zoom;
  // Centre the whole desk, then apply the user's pan.
  const ox = (size.w - b.w * fit) / 2 + view.x;
  const oy = (size.h - b.h * fit + LABEL_H) / 2 + view.y;
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

  // Pinch (ctrl+wheel in WebKit) zooms around the pointer; two-finger scroll pans.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      // The tray list scrolls itself; the canvas pans and zooms everywhere else.
      if ((e.target as Element).closest('.tray-list')) return;
      e.preventDefault();
      if (e.ctrlKey) {
        const rect = el.getBoundingClientRect();
        const px = e.clientX - rect.left;
        const py = e.clientY - rect.top;
        setView((v) => {
          const zoom = Math.min(MAX_ZOOM, Math.max(1, v.zoom * Math.exp(-e.deltaY / 100)));
          const k = zoom / v.zoom;
          if (zoom === 1) return FIT;
          // Keep the point under the pointer where it is.
          const cx = (size.w - b.w * fit) / 2;
          const cy = (size.h - b.h * fit) / 2;
          return { zoom, x: px - cx - (px - cx - v.x) * k, y: py - cy - (py - cy - v.y) * k };
        });
      } else {
        setView((v) => (v.zoom === 1 ? v : { ...v, x: v.x - e.deltaX, y: v.y - e.deltaY }));
      }
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [size, b.w, b.h, fit]);

  function zoomTo(d: DeskDisplay) {
    const zoom = Math.min(MAX_ZOOM, Math.min((size.w - PAD * 2) / d.frame.w, (size.h - PAD * 2) / d.frame.h) / fit);
    const cx = (size.w - b.w * fit) / 2;
    const cy = (size.h - b.h * fit) / 2;
    // Put the display's centre in the middle of the box.
    const dx = (d.frame.x - b.x + d.frame.w / 2) * fit * zoom;
    const dy = (d.frame.y - b.y + d.frame.h / 2) * fit * zoom;
    setView({ zoom, x: size.w / 2 - cx - dx, y: size.h / 2 - cy - dy });
  }

  return (
    <div className="deskview">
      <div className="deskview-tools">
        <span className="hint">Pinch to zoom · two-finger scroll to pan · double-click a display to zoom to it</span>
        <button type="button" className="small" disabled={view.zoom === 1} onClick={() => setView(FIT)}>
          Fit all
        </button>
      </div>
      <div className="canvas" ref={box} onKeyDown={(e) => e.key === '0' && setView(FIT)}>
        {desk.displays.map((d) => {
          const current = d.desktops.find((x) => x.current);
          const drawn = d.windows
            .filter((w) => w.onCurrentDesktop && !w.minimized)
            .sort((a, b2) => b2.order - a.order); // back to front
          const peeking = d.windows.find((w) => w.id === peek && d.covered.includes(w.id));
          return (
            <Fragment key={d.id}>
            <div className="screen-label" style={{ ...labelAt(d.frame) }} onDoubleClick={() => zoomTo(d)}>
              {d.name} <span className="meta">{resolution(d)}</span>
              {d.main && <span className="badge">main</span>}
              {!d.connected && <span className="badge">not connected</span>}
              {current && d.desktops.length > 1 && <span className="meta"> · {names.get(current.id)}</span>}
            </div>
            <div
              className={`screen${d.connected ? '' : ' ghost'}`}
              style={place(d.frame)}
              onDoubleClick={() => zoomTo(d)}
              aria-label={`${d.name}${d.connected ? '' : ', not connected'}`}
            >
              {drawn.map((w, i) => {
                const r = w.frame;
                return (
                  <button
                    key={w.id}
                    type="button"
                    className={`win${w.focused ? ' focused' : ''}`}
                    style={{
                      left: (r.x - d.frame.x) * s,
                      top: (r.y - d.frame.y) * s,
                      width: Math.max(r.w * s, 8),
                      height: Math.max(r.h * s, 8),
                      zIndex: i + 1,
                    }}
                    title={`${windowLabel(w)} — ${w.app}`}
                    onClick={() => onAct({ kind: 'focus', window: w.id })}
                    onDoubleClick={(e) => e.stopPropagation()}
                  >
                    <span className="win-bar">
                      <AppBadge app={w.app} />
                      <span className="win-title">{windowLabel(w)}</span>
                    </span>
                  </button>
                );
              })}
              {peeking && (
                <div
                  className="peek"
                  style={{
                    left: (peeking.frame.x - d.frame.x) * s,
                    top: (peeking.frame.y - d.frame.y) * s,
                    width: peeking.frame.w * s,
                    height: peeking.frame.h * s,
                  }}
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
        {open &&
          (() => {
            const d = desk.displays.find((x) => x.id === open.display);
            const g = d && trayFor(d).find((x) => x.kind === open.kind);
            if (!d || !g) return null;
            const p = place(d.frame);
            const width = Math.min(340, size.w - 16);
            const left = Math.max(8, Math.min((p.left as number) + 4, size.w - width - 8));
            const top = Math.max(8, Math.min((p.top as number) + 24, size.h - 220));
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
      </div>
      {desk.offscreen.length > 0 && (
        <div className="offscreen-note">
          <strong>Off-screen:</strong>{' '}
          {desk.offscreen.map((w) => (
            <button key={w.id} type="button" className="chip" onClick={() => onAct({ kind: 'focus', window: w.id })}>
              {w.app}: {windowLabel(w)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
