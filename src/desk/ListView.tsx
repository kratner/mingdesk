// The List view (PLAN.md §8): one section per display, its windows as rows in stacking order. Windows on the
// desktop showing now come first, then each other desktop or full-screen space, then minimized windows.
// "By app" regroups the same rows by app. Dragging rows between displays and in the stack is M2.

import { useEffect, useState } from 'react';
import { appIcon, knownIcon } from './api';
import { appHue, desktopNames, emptyDesktopHint, matches, resolution, windowLabel } from './format';
import { useFolds, type Folds } from './folds';
import type { Action, Desk, DeskDisplay, WindowInfo } from './types';

export type Grouping = 'display' | 'app';

interface Props {
  desk: Desk;
  grouping: Grouping;
  filter: string;
  onAct: (a: Action) => void;
}

export function ListView({ desk, grouping, filter, onAct }: Props) {
  const folds = useFolds(filter.trim() !== '');
  const names = desktopNames(desk);
  const displayOf = new Map<number, DeskDisplay>();
  desk.displays.forEach((d) => d.windows.forEach((w) => displayOf.set(w.id, d)));
  const total = [...desk.displays.flatMap((d) => d.windows), ...desk.offscreen].filter((w) => matches(w, filter));

  if (total.length === 0) {
    return <p className="muted empty">{filter ? `No window matches “${filter}”.` : 'No windows open.'}</p>;
  }

  if (grouping === 'app') {
    const byApp = new Map<number, WindowInfo[]>();
    total.forEach((w) => byApp.set(w.pid, [...(byApp.get(w.pid) ?? []), w]));
    const apps = [...byApp.values()].sort((a, b) => a[0].app.localeCompare(b[0].app));
    return (
      <div className="list columns">
        {apps.map((ws) => {
          const id = `a:${ws[0].appId ?? ws[0].app}`;
          return (
          <section key={ws[0].pid} className="group" aria-label={ws[0].app}>
            <header className="group-head">
              <Fold id={id} folds={folds}>
                <AppBadge app={ws[0].app} appId={ws[0].appId} />
                <h3>{ws[0].app}</h3>
              </Fold>
              <span className="count">{ws.length}</span>
              <button type="button" className="small" onClick={() => onAct({ kind: 'bringAppForward', pid: ws[0].pid })}>
                Bring forward
              </button>
            </header>
            {!folds.isFolded(id) && (
              <ul className="rows">
                {ws.map((w) => (
                  <Row
                    key={w.id}
                    w={w}
                    onAct={onAct}
                    where={[displayOf.get(w.id)?.name ?? 'Off-screen', w.desktop != null && !w.onCurrentDesktop ? names.get(w.desktop) : null]}
                  />
                ))}
              </ul>
            )}
          </section>
          );
        })}
      </div>
    );
  }

  return (
    <div className="list columns">
      {desk.displays.map((d) => (
        <DisplaySection key={d.id} d={d} names={names} filter={filter} onAct={onAct} folds={folds} />
      ))}
      {desk.offscreen.some((w) => matches(w, filter)) && (
        <section className="group wide" aria-label="Off-screen">
          <header className="group-head">
            <Fold id="offscreen" folds={folds}>
              <h3>Off-screen</h3>
            </Fold>
            <span className="hint">On no display, e.g. left behind on a monitor that was unplugged.</span>
          </header>
          {!folds.isFolded('offscreen') && (
            <ul className="rows">
              {desk.offscreen.filter((w) => matches(w, filter)).map((w) => (
                <Row key={w.id} w={w} onAct={onAct} />
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

function DisplaySection({
  d,
  names,
  filter,
  onAct,
  folds,
}: {
  d: DeskDisplay;
  names: Map<number, string>;
  filter: string;
  onAct: (a: Action) => void;
  folds: Folds;
}) {
  if (!d.connected) {
    return (
      <section className="group ghost" aria-label={`${d.name}, not connected`}>
        <header className="group-head">
          <h3>{d.name}</h3>
          <span className="badge">not connected</span>
          <span className="hint">Part of this setup but not connected now.</span>
        </header>
      </section>
    );
  }
  const shown = d.windows.filter((w) => matches(w, filter));
  const here = shown.filter((w) => w.onCurrentDesktop && !w.minimized);
  const minimized = shown.filter((w) => w.minimized);
  const elsewhere = d.desktops
    .filter((x) => !x.current)
    .map((x) => ({ x, ws: shown.filter((w) => w.desktop === x.id && !w.minimized && !w.onCurrentDesktop) }))
    .filter((g) => g.ws.length > 0);
  const unplaced = shown.filter(
    (w) => !w.onCurrentDesktop && !w.minimized && (w.desktop == null || !d.desktops.some((x) => x.id === w.desktop)),
  );
  const current = d.desktops.find((x) => x.current);

  return (
    <section className="group" aria-label={d.name}>
      <header className="group-head">
        <Fold id={`d:${d.id}`} folds={folds}>
          <h3>{d.name}</h3>
        </Fold>
        <span className="meta">{resolution(d)}</span>
        {d.main && <span className="badge">main</span>}
        <span className="count">{d.windows.length}</span>
      </header>
      {!folds.isFolded(`d:${d.id}`) && (
        <>
        {d.desktops.length > 1 && (
          <div className="desktops" aria-label="Desktops">
            {d.desktops.map((x) => {
              const first = d.windows.find((w) => w.desktop === x.id && !w.minimized);
              return (
                <button
                  key={x.id}
                  type="button"
                  className={`chip${x.current ? ' current' : ''}${x.fullScreen ? ' fs' : ''}`}
                  disabled={!first || x.current}
                  title={first ? `Go to ${names.get(x.id)}` : emptyDesktopHint()}
                  onClick={() => first && onAct({ kind: 'focus', window: first.id })}
                >
                  {names.get(x.id)}
                </button>
              );
            })}
          </div>
        )}
        {here.length > 0 && (
          <Sub id={`s:${d.id}:here`} folds={folds} count={here.length} label={current ? `${names.get(current.id)} · showing now` : null}>
            {here.map((w) => (
              <Row key={w.id} w={w} onAct={onAct} />
            ))}
          </Sub>
        )}
        {elsewhere.map(({ x, ws }) => (
          <Sub key={x.id} id={`s:${d.id}:${x.id}`} folds={folds} count={ws.length} label={names.get(x.id) ?? 'Another desktop'} dim>
            {ws.map((w) => (
              <Row key={w.id} w={w} onAct={onAct} />
            ))}
          </Sub>
        ))}
        {unplaced.length > 0 && (
          <Sub id={`s:${d.id}:other`} folds={folds} count={unplaced.length} label="Another desktop" dim>
            {unplaced.map((w) => (
              <Row key={w.id} w={w} onAct={onAct} />
            ))}
          </Sub>
        )}
        {minimized.length > 0 && (
          <Sub id={`s:${d.id}:min`} folds={folds} count={minimized.length} label="Minimized" dim>
            {minimized.map((w) => (
              <Row key={w.id} w={w} onAct={onAct} />
            ))}
          </Sub>
        )}
        {shown.length === 0 && <p className="muted empty">{filter ? 'No match here.' : 'No windows.'}</p>}
        </>
      )}
    </section>
  );
}

function Sub({
  id,
  folds,
  count,
  label,
  dim,
  children,
}: {
  id: string;
  folds: Folds;
  count: number;
  label: string | null;
  dim?: boolean;
  children: React.ReactNode;
}) {
  const folded = label !== null && folds.isFolded(id);
  return (
    <div className={`sub${dim ? ' dim' : ''}`}>
      {label && (
        <div className="sub-label">
          <Fold id={id} folds={folds}>
            {label}
          </Fold>
          {folded && <span className="count">{count}</span>}
        </div>
      )}
      {!folded && <ul className="rows">{children}</ul>}
    </div>
  );
}

/** A group's name as a button that folds and unfolds it. */
function Fold({ id, folds, children }: { id: string; folds: Folds; children: React.ReactNode }) {
  const folded = folds.isFolded(id);
  return (
    <button type="button" className="fold" aria-expanded={!folded} onClick={() => folds.toggle(id)}>
      <span className="chev" aria-hidden="true">
        {folded ? '▸' : '▾'}
      </span>
      {children}
    </button>
  );
}

/** The app's own icon where the OS gives one (Windows so far), else its initial on a colour of its own. */
export function AppBadge({ app, appId }: { app: string; appId?: string | null }) {
  const [icon, setIcon] = useState<string | null | undefined>(() => (appId ? knownIcon(appId) : null));
  useEffect(() => {
    if (!appId) {
      setIcon(null);
      return;
    }
    let live = true;
    appIcon(appId).then((url) => {
      if (live) setIcon(url);
    });
    return () => {
      live = false;
    };
  }, [appId]);
  if (icon) return <img className="app-badge app-icon" src={icon} alt="" aria-hidden="true" draggable={false} />;
  return (
    <span className="app-badge" style={{ '--hue': appHue(app) } as React.CSSProperties} aria-hidden="true">
      {app.slice(0, 1).toUpperCase()}
    </span>
  );
}

function Row({ w, onAct, where }: { w: WindowInfo; onAct: (a: Action) => void; where?: (string | null | undefined)[] }) {
  const label = windowLabel(w);
  return (
    <li className={`row${w.focused ? ' focused' : ''}`}>
      <button type="button" className="row-main" onClick={() => onAct({ kind: 'focus', window: w.id })} title={`Go to ${label}`}>
        <AppBadge app={w.app} appId={w.appId} />
        <span className="row-text">
          <span className="row-title">{label}</span>
          <span className="row-app">
            {w.app}
            {where?.filter(Boolean).map((x) => (
              <span key={x}> · {x}</span>
            ))}
          </span>
        </span>
        {w.fullScreen && <span className="badge">full screen</span>}
        {w.focused && <span className="badge gold">focused</span>}
      </button>
      <span className="row-actions">
        {!w.minimized && !w.fullScreen && (
          <button type="button" className="icon" aria-label={`Minimize ${label}`} title="Minimize" onClick={() => onAct({ kind: 'minimize', window: w.id })}>
            –
          </button>
        )}
        <button type="button" className="icon" aria-label={`Close ${label}`} title="Close" onClick={() => onAct({ kind: 'close', window: w.id })}>
          ×
        </button>
      </span>
    </li>
  );
}
