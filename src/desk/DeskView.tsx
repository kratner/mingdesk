// The Desk view (PLAN.md §8): every display in its real arrangement, scaled to fit, with the windows of the
// desktop showing on each drawn in stacking order. Trackpad first: pinch to zoom, two-finger scroll to pan,
// double-click a display to zoom to it, Fit all to go back. Moving and resizing windows here is M2.

import { Fragment, useEffect, useRef, useState } from 'react';
import { AppBadge } from './ListView';
import { desktopNames, resolution, windowLabel } from './format';
import type { Action, Desk, DeskDisplay, Rect } from './types';

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

export function DeskView({ desk, onAct }: { desk: Desk; onAct: (a: Action) => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 500 });
  const [view, setView] = useState<View>(FIT);
  const names = desktopNames(desk);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setSize({ w: e.contentRect.width, h: e.contentRect.height }));
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
          const away = d.desktops.filter((x) => !x.current);
          const minimized = d.windows.filter((w) => w.minimized);
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
              {(away.length > 0 || minimized.length > 0) && (
                <div className="screen-tray">
                  {away.map((x) => {
                    const first = d.windows.find((w) => w.desktop === x.id && !w.minimized);
                    return (
                      <button
                        key={x.id}
                        type="button"
                        className={`chip${x.fullScreen ? ' fs' : ''}`}
                        disabled={!first}
                        onClick={() => first && onAct({ kind: 'focus', window: first.id })}
                        title={first ? `Go to ${names.get(x.id)}` : 'Empty desktop'}
                      >
                        {names.get(x.id)}
                      </button>
                    );
                  })}
                  {minimized.map((w) => (
                    <button key={w.id} type="button" className="chip min" onClick={() => onAct({ kind: 'focus', window: w.id })} title={`Restore ${windowLabel(w)}`}>
                      ▁ {windowLabel(w)}
                    </button>
                  ))}
                </div>
              )}
            </div>
            </Fragment>
          );
        })}
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
