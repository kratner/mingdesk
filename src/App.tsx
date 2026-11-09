import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FourOuncesMark } from './FourOuncesMark';

type Permission = 'granted' | 'missing' | 'notNeeded';
interface Status {
  permission: Permission;
  platform: string;
  version: string;
}

// The board asks the core every POLL_MS while access is missing, so it notices the grant without a restart.
const POLL_MS = 1000;

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [justGranted, setJustGranted] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<Status>('status');
      setStatus((prev) => {
        if (prev?.permission === 'missing' && next.permission === 'granted') setJustGranted(true);
        return next;
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    window.addEventListener('focus', refresh);
    return () => window.removeEventListener('focus', refresh);
  }, [refresh]);

  useEffect(() => {
    if (status?.permission !== 'missing') return;
    const id = window.setInterval(refresh, POLL_MS);
    return () => window.clearInterval(id);
  }, [status?.permission, refresh]);

  async function allow() {
    setError(null);
    try {
      setStatus(await invoke<Status>('allow_access'));
    } catch (e) {
      setError(String(e));
    }
  }

  async function openSettings() {
    setError(null);
    try {
      await invoke('open_permission_settings');
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="app">
      <header className="top">
        <img className="app-icon" src="/mingdesk.svg" alt="" width={28} height={28} />
        <h1>MingDesk</h1>
        {status && status.permission !== 'missing' && <span className="pill ok">Ready</span>}
      </header>

      <div className="body">
        <nav className="spaces" aria-label="Spaces">
          <div className="spaces-title">Spaces</div>
          <button className="space active" type="button">
            Desk
          </button>
          <p className="soon">Your spaces arrive with saving (M2).</p>
        </nav>

        <main className="main">
          {error && (
            <div className="banner danger" role="alert">
              {error}
            </div>
          )}
          {justGranted && (
            <div className="banner success" role="status">
              Accessibility is on. MingDesk can now see your windows.
            </div>
          )}
          {status === null && !error && <p className="muted">Starting…</p>}
          {status?.permission === 'missing' && <Onboarding onAllow={allow} onOpenSettings={openSettings} />}
          {status && status.permission !== 'missing' && <EmptyBoard />}
        </main>
      </div>

      <footer className="foot">
        <span>
          MingDesk {status?.version ?? ''} · {status?.platform ?? ''}
        </span>
        <span className="made">
          <FourOuncesMark height={12} /> A Four Ounces tool
        </span>
      </footer>
    </div>
  );
}

function Onboarding({ onAllow, onOpenSettings }: { onAllow: () => void; onOpenSettings: () => void }) {
  return (
    <section className="card onboarding" aria-labelledby="ob-title">
      <h2 id="ob-title">Allow MingDesk to see your windows</h2>
      <p>
        MingDesk needs the Mac's <strong>Accessibility</strong> permission to list your windows, read their titles
        and documents, and focus, move or close them. It doesn't read what's inside your windows, and nothing leaves
        this Mac.
      </p>
      <ol className="steps">
        <li>
          Click <strong>Allow access</strong>. System Settings opens at Privacy &amp; Security → Accessibility (macOS
          may show a prompt first).
        </li>
        <li>
          Turn on the switch next to <strong>MingDesk</strong>. If it isn't in the list, click <strong>+</strong> and
          choose MingDesk from Applications.
        </li>
        <li>Come back here. This page notices by itself.</li>
      </ol>
      <div className="actions">
        <button className="primary" type="button" onClick={onAllow}>
          Allow access
        </button>
        <button type="button" onClick={onOpenSettings}>
          Open System Settings again
        </button>
        <span className="pill waiting" role="status">
          <span className="dot" aria-hidden="true" /> Waiting for Accessibility…
        </span>
      </div>
    </section>
  );
}

function EmptyBoard() {
  return (
    <div className="board">
      <section className="card pane" aria-labelledby="open-title">
        <h2 id="open-title">Open windows</h2>
        <p className="muted">Every window on this computer, grouped by app and by display, arrives in the next step (M1).</p>
      </section>
      <section className="card pane" aria-labelledby="coll-title">
        <h2 id="coll-title">Collections</h2>
        <p className="muted">
          Save a set of windows and bring them back the way they were: <strong>Save these windows</strong> and{' '}
          <strong>Save desk</strong> arrive in M2.
        </p>
      </section>
    </div>
  );
}
