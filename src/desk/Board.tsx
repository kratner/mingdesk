// The board once access is allowed: the setup bar, the List / Desk switch, and the live desk.

import { useCallback, useEffect, useState } from 'react';
import { DeskView, FILL_KEY, initialFill } from './DeskView';
import { ListView, type Grouping } from './ListView';
import { act, adoptSetup, forgetSetup, getDesk, renameSetup, watchDesk } from './api';
import type { Action, Desk, SetupSummary } from './types';

type ViewName = 'list' | 'desk';
const DENSITIES = ['compact', 'normal', 'large'] as const;
type Density = (typeof DENSITIES)[number];
const DENSITY_LABEL: Record<Density, string> = { compact: 'Compact', normal: 'Normal', large: 'Large' };

// Per-viewer conveniences only; the board works without them (private windows, blocked storage).
function remembered<T extends string>(key: string, fallback: T, allowed: readonly T[]): T {
  try {
    const v = localStorage.getItem(key) as T | null;
    return v && allowed.includes(v) ? v : fallback;
  } catch {
    return fallback;
  }
}
/** The Desk view's window fill, in words. */
function fillName(v: number): string {
  return v === 0 ? 'Outlines' : v === 100 ? 'Solid' : `${v}% solid`;
}
function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* not kept; fine */
  }
}

export function Board() {
  const [desk, setDesk] = useState<Desk | null>(null);
  const [error, setError] = useState<string | null>(null);
  // `?view=desk` opens a view directly (screenshots); otherwise the last one used.
  const [view, setView] = useState<ViewName>(() => {
    const asked = new URLSearchParams(window.location.search).get('view');
    return asked === 'list' || asked === 'desk' ? asked : remembered('mingdesk.view', 'list', ['list', 'desk'] as const);
  });
  const [grouping, setGrouping] = useState<Grouping>(() => remembered('mingdesk.grouping', 'display', ['display', 'app'] as const));
  const [filter, setFilter] = useState('');
  const [fill, setFill] = useState(initialFill);
  const [density, setDensity] = useState<Density>(() => {
    const asked = new URLSearchParams(window.location.search).get('density') as Density | null;
    return asked && DENSITIES.includes(asked) ? asked : remembered('mingdesk.density', 'normal', DENSITIES);
  });

  const refresh = useCallback(async () => {
    try {
      setDesk(await getDesk());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    let stop: (() => void) | undefined;
    watchDesk(setDesk).then((f) => (stop = f));
    window.addEventListener('focus', refresh);
    return () => {
      stop?.();
      window.removeEventListener('focus', refresh);
    };
  }, [refresh]);

  const onAct = useCallback(async (a: Action) => {
    setError(null);
    try {
      await act(a);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const run = (f: () => Promise<void>) => async () => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    }
  };

  function choose(v: ViewName) {
    setView(v);
    remember('mingdesk.view', v);
  }
  function group(g: Grouping) {
    setGrouping(g);
    remember('mingdesk.grouping', g);
  }

  if (!desk) return <p className="muted">Reading your desk…</p>;

  const count = desk.displays.reduce((n, d) => n + d.windows.length, 0) + desk.offscreen.length;
  const connected = desk.displays.filter((d) => d.connected).length;
  const ghosts = desk.displays.filter((d) => !d.connected);

  return (
    <div className={`deskboard density-${density}`}>
      {error && (
        <div className="banner danger" role="alert">
          {error}
        </div>
      )}
      <div className="board-head">
      <SetupBar
          setup={desk.setup && { ...desk.setup, displayNames: leftToRight(desk, desk.setup.displayNames) }}
          others={desk.otherSetups}
          ghosts={ghosts.map((g) => g.name)}
          onAdopt={run(adoptSetup)}
          onRename={(id, name) => run(() => renameSetup(id, name))()}
          onForget={(id) => run(() => forgetSetup(id))()}
        />
        <div className="toolbar">
          <div className="seg" role="group" aria-label="View">
            <button type="button" aria-pressed={view === 'list'} onClick={() => choose('list')}>
              List
            </button>
            <button type="button" aria-pressed={view === 'desk'} onClick={() => choose('desk')}>
              Desk
            </button>
          </div>
          {view === 'list' && (
            <>
              <div className="seg" role="group" aria-label="Group by">
                <button type="button" aria-pressed={grouping === 'display'} onClick={() => group('display')}>
                  By display
                </button>
                <button type="button" aria-pressed={grouping === 'app'} onClick={() => group('app')}>
                  By app
                </button>
              </div>
              <input
                className="filter"
                type="search"
                placeholder="Filter windows"
                aria-label="Filter windows"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              />
            </>
          )}
          <label className="density" title="How much fits on screen">
          <span className="meta">Density</span>
          <input
            type="range"
            min={0}
            max={2}
            step={1}
            value={DENSITIES.indexOf(density)}
            aria-valuetext={DENSITY_LABEL[density]}
            onChange={(e) => {
              const d = DENSITIES[Number(e.target.value)];
              setDensity(d);
              remember('mingdesk.density', d);
            }}
          />
          <span className="meta density-name">{DENSITY_LABEL[density]}</span>
        </label>
        {view === 'desk' && (
          <label className="density" title="Outlines show where every window sits; solid shows only what's in front">
            <span className="meta">Windows</span>
            <input
              type="range"
              min={0}
              max={100}
              step={5}
              value={fill}
              aria-valuetext={fillName(fill)}
              onChange={(e) => {
                const v = Number(e.target.value);
                setFill(v);
                remember(FILL_KEY, String(v));
              }}
            />
            <span className="meta fill-name">{fillName(fill)}</span>
          </label>
        )}
        <span className="meta stretch">
            {count} {count === 1 ? 'window' : 'windows'} on {connected} {connected === 1 ? 'display' : 'displays'}
          </span>
        </div>
      </div>
      {view === 'list' ? (
        <ListView desk={desk} grouping={grouping} filter={filter} onAct={onAct} />
      ) : (
        <DeskView desk={desk} fill={fill} onAct={onAct} />
      )}
    </div>
  );
}

function SetupBar({
  setup,
  others,
  ghosts,
  onAdopt,
  onRename,
  onForget,
}: {
  setup: SetupSummary | null;
  others: SetupSummary[];
  ghosts: string[];
  onAdopt: () => void;
  onRename: (id: string, name: string) => void;
  onForget: (id: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState('');
  // Forgetting asks once more inline: MingDesk learns the setup again when its displays come back.
  const [forgetting, setForgetting] = useState<string | null>(null);
  if (!setup) return null;

  function save() {
    if (setup && name.trim()) onRename(setup.id, name);
    setEditing(false);
  }

  return (
    <div className="setupbar" aria-label="Display setup">
      <span className="setup-current">
        <span className="label">Setup</span>
        {editing ? (
          <form
            className="rename"
            onSubmit={(e) => {
              e.preventDefault();
              save();
            }}
          >
            <input
              autoFocus
              aria-label="Setup name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === 'Escape' && setEditing(false)}
            />
            <button type="submit" className="small primary">
              Save
            </button>
          </form>
        ) : (
          <>
            <strong>{setup.name}</strong>
            {/* A setup named after its one display ("Display 1") doesn't say the name twice. */}
            {setup.displayNames.join(' + ') !== setup.name && (
              <span className="setup-displays">{setup.displayNames.join(' + ')}</span>
            )}
            <button
              type="button"
              className="link"
              onClick={() => {
                setName(setup.name);
                setEditing(true);
              }}
            >
              Rename
            </button>
          </>
        )}
      </span>
      {ghosts.length > 0 && (
        <span className="setup-ghosts">
          {ghosts.join(', ')} {ghosts.length === 1 ? "isn't" : "aren't"} connected.
          <button type="button" className="small" onClick={onAdopt}>
            This is a setup of its own
          </button>
        </span>
      )}
      {others.length > 0 && (
        <span className="setup-others">
          <span className="label">Other setups</span>
          {others.map((o) => (
            <span key={o.id} className="other" title={o.displayNames.join(' + ')}>
              <span>{o.name}</span>
              <span className="setup-displays">({o.displayNames.length} {o.displayNames.length === 1 ? 'display' : 'displays'})</span>
              {forgetting === o.id ? (
                <>
                  <button type="button" className="small danger" onClick={() => onForget(o.id)}>
                    Forget it
                  </button>
                  <button type="button" className="link" onClick={() => setForgetting(null)}>
                    Keep
                  </button>
                </>
              ) : (
                <button type="button" className="link" aria-label={`Forget the ${o.name} setup`} onClick={() => setForgetting(o.id)}>
                  Forget
                </button>
              )}
            </span>
          ))}
        </span>
      )}
    </div>
  );
}

/** The setup's display names in the order they sit on the desk, left to right (the stored order is by display id). */
function leftToRight(desk: Desk, stored: string[]): string[] {
  const here = desk.displays
    .filter((d) => d.connected)
    .sort((a, b) => a.frame.x - b.frame.x || a.frame.y - b.frame.y)
    .map((d) => d.name);
  return here.length === stored.length && here.every((n) => stored.includes(n)) ? here : stored;
}
