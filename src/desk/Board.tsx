// The board once access is allowed: the setup bar, the List / Desk switch, and the live desk.

import { useCallback, useEffect, useState } from 'react';
import { DeskView } from './DeskView';
import { ListView, type Grouping } from './ListView';
import { act, adoptSetup, forgetSetup, getDesk, renameSetup, watchDesk } from './api';
import type { Action, Desk, SetupSummary } from './types';

type ViewName = 'list' | 'desk';

// Per-viewer conveniences only; the board works without them (private windows, blocked storage).
function remembered<T extends string>(key: string, fallback: T, allowed: readonly T[]): T {
  try {
    const v = localStorage.getItem(key) as T | null;
    return v && allowed.includes(v) ? v : fallback;
  } catch {
    return fallback;
  }
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
    <div className="deskboard">
      {error && (
        <div className="banner danger" role="alert">
          {error}
        </div>
      )}
      <SetupBar
        setup={desk.setup}
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
        <span className="meta stretch">
          {count} {count === 1 ? 'window' : 'windows'} on {connected} {connected === 1 ? 'display' : 'displays'}
        </span>
      </div>
      {view === 'list' ? (
        <ListView desk={desk} grouping={grouping} filter={filter} onAct={onAct} />
      ) : (
        <DeskView desk={desk} onAct={onAct} />
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
            <strong title={setup.displayNames.join(' + ')}>{setup.name}</strong>
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
              {o.name}
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
