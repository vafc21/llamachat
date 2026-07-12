import { useState, useEffect, useCallback } from 'react'
import { invoke, isTauri, listen } from '../tauri'
import type { DownloadProgress } from '../types'

interface Perms { accessibility: boolean; screen_recording: boolean; ollama: boolean }

/** Checklist of what Agent mode needs, with live ✅/❌ status + grant buttons. */
export function AgentSetup({ onDone }: { onDone?: () => void }) {
  const [perms, setPerms] = useState<Perms>({ accessibility: false, screen_recording: false, ollama: false });
  const [llava, setLlava] = useState<'idle' | 'downloading' | 'done'>('idle');
  const [llavaPct, setLlavaPct] = useState(0);

  const refresh = useCallback(async () => {
    const p = await invoke<Perms>('check_permissions');
    if (p) setPerms(p);
  }, []);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 2500);
    return () => clearInterval(t);
  }, [refresh]);

  useEffect(() => {
    let un: (() => void) | null = null;
    listen<DownloadProgress>('download_progress', (p) => {
      if (p.tag !== 'llava:7b') return;
      if (p.status === 'done') { setLlava('done'); setLlavaPct(100); }
      else if (p.status === 'error') setLlava('idle');
      else setLlavaPct(p.pct ?? 0);
    }).then((u) => { un = u; });
    return () => un?.();
  }, []);

  async function grantAccessibility() {
    await invoke('request_accessibility');
    await invoke('open_settings_pane', { pane: 'accessibility' });
    setTimeout(refresh, 800);
  }
  async function grantScreenRecording() {
    // Pops the macOS prompt AND registers the app in the Screen Recording list.
    await invoke('request_screen_recording');
    await invoke('open_settings_pane', { pane: 'screen_recording' });
    setTimeout(refresh, 800);
  }
  function downloadLlava() {
    setLlava('downloading');
    invoke('download_model', { tag: 'llava:7b' });
  }

  const rows: { label: string; desc: string; ok: boolean | null; action?: { label: string; run: () => void } }[] = [
    { label: 'Ollama running', desc: 'The local model server that powers chat and the agent.', ok: perms.ollama },
    {
      label: 'Accessibility permission',
      desc: 'Lets the agent move the mouse, press keys, and read the screen.',
      ok: perms.accessibility,
      action: perms.accessibility ? undefined : { label: 'Grant', run: grantAccessibility },
    },
    {
      label: 'Screen Recording',
      desc: 'Only needed for screenshot-vision perception — not the default. Grant only if you turn that on.',
      ok: perms.screen_recording,
      action: perms.screen_recording ? undefined : { label: 'Grant', run: grantScreenRecording },
    },
    {
      label: 'Vision model (LLaVA)',
      desc: 'Optional — lets the agent "look" at the screen. ~4.7 GB.',
      ok: llava === 'done' ? true : null,
      action: llava === 'done' ? undefined : {
        label: llava === 'downloading' ? `${Math.round(llavaPct)}%` : 'Download',
        run: downloadLlava,
      },
    },
  ];

  return (
    <div className="space-y-2">
      {rows.map((r) => (
        <div key={r.label} className="flex items-start gap-3 border border-border rounded-lg p-3 bg-surface">
          <StatusIcon ok={r.ok} />
          <div className="min-w-0 flex-1">
            <div className="text-[13px] text-text font-medium">{r.label}</div>
            <p className="text-[11px] text-text-muted mt-0.5">{r.desc}</p>
          </div>
          {r.action && isTauri() && (
            <button
              onClick={r.action.run}
              className="flex-shrink-0 px-2.5 py-1 text-[12px] rounded border border-accent/40 text-accent
                         hover:bg-accent-dim transition-colors"
            >
              {r.action.label}
            </button>
          )}
        </div>
      ))}
      {onDone && (
        <button
          onClick={onDone}
          className="w-full mt-1 py-2 bg-accent text-white text-[13px] font-medium rounded-lg hover:opacity-90 transition-opacity"
        >
          Done
        </button>
      )}
    </div>
  );
}

function StatusIcon({ ok }: { ok: boolean | null }) {
  if (ok === true) {
    return (
      <span className="flex-shrink-0 w-5 h-5 rounded-full bg-success/15 text-success flex items-center justify-center">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M3 8l3.5 3.5L13 5" stroke="currentColor" strokeWidth="2" /></svg>
      </span>
    );
  }
  if (ok === false) {
    return (
      <span className="flex-shrink-0 w-5 h-5 rounded-full bg-error/15 text-error flex items-center justify-center">
        <svg width="10" height="10" viewBox="0 0 16 16" fill="none"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" /></svg>
      </span>
    );
  }
  return (
    <span className="flex-shrink-0 w-5 h-5 rounded-full bg-white/[0.05] text-text-muted flex items-center justify-center">
      <svg width="10" height="10" viewBox="0 0 16 16" fill="none"><path d="M4 8h8" stroke="currentColor" strokeWidth="2" /></svg>
    </span>
  );
}
