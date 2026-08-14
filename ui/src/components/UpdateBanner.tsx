import { useEffect, useState } from 'react'
import { isTauri } from '../tauri'
import { Icon } from './Icon'

type Phase =
  | { k: 'idle' }
  | { k: 'available'; version: string; notes?: string }
  | { k: 'downloading'; pct: number }
  | { k: 'ready' }
  | { k: 'failed'; msg: string };

/**
 * Update notice.
 *
 * Deliberately a dismissible bar and not a modal: this is a local-first app,
 * and an update prompt that blocks the window is the kind of interruption the
 * rest of the product avoids. Checking is silent, and a failed check stays
 * silent too -- someone offline by choice should never see a network error
 * they did not ask for.
 */
export function UpdateBanner() {
  const [phase, setPhase] = useState<Phase>({ k: 'idle' });
  const [hidden, setHidden] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;

    (async () => {
      try {
        const { check } = await import('@tauri-apps/plugin-updater');
        const update = await check();
        if (!cancelled && update?.available) {
          setPhase({ k: 'available', version: update.version, notes: update.body });
        }
      } catch {
        // Offline, or GitHub unreachable. Not worth a word to the user.
      }
    })();

    return () => { cancelled = true; };
  }, []);

  async function install() {
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (!update?.available) return;

      let total = 0;
      let got = 0;
      setPhase({ k: 'downloading', pct: 0 });

      await update.downloadAndInstall((e) => {
        if (e.event === 'Started') {
          total = e.data.contentLength ?? 0;
        } else if (e.event === 'Progress') {
          got += e.data.chunkLength;
          setPhase({ k: 'downloading', pct: total ? Math.round((got / total) * 100) : 0 });
        } else if (e.event === 'Finished') {
          setPhase({ k: 'ready' });
        }
      });

      setPhase({ k: 'ready' });
    } catch (e) {
      setPhase({ k: 'failed', msg: e instanceof Error ? e.message : String(e) });
    }
  }

  async function relaunch() {
    const { relaunch: go } = await import('@tauri-apps/plugin-process');
    await go();
  }

  if (hidden || phase.k === 'idle') return null;

  return (
    <div className="updbar">
      <Icon name="spark" size={14} />
      <div className="updtx">
        {phase.k === 'available' && (
          <>
            <b>LlamaChat {phase.version} is available.</b>
            <span>Downloads in the background. Your chats, models and settings are kept.</span>
          </>
        )}
        {phase.k === 'downloading' && (
          <>
            <b>Downloading update… {phase.pct}%</b>
            <span>You can keep working. It installs when you restart.</span>
          </>
        )}
        {phase.k === 'ready' && (
          <>
            <b>Update ready.</b>
            <span>Restart to finish. Nothing is lost.</span>
          </>
        )}
        {phase.k === 'failed' && (
          <>
            <b>Update failed.</b>
            <span>{phase.msg}</span>
          </>
        )}
      </div>

      {phase.k === 'available' && (
        <button type="button" className="chip" onClick={install}>Update</button>
      )}
      {phase.k === 'ready' && (
        <button type="button" className="chip" onClick={relaunch}>Restart now</button>
      )}
      {phase.k !== 'downloading' && (
        <button type="button" className="updx" title="Dismiss" onClick={() => setHidden(true)}>
          <Icon name="x" size={12} />
        </button>
      )}
    </div>
  );
}
