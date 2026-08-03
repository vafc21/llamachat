import { useState, useEffect, type ReactNode } from 'react'
import { Icon, type IconName } from './Icon'
import { invoke, isTauri, listen } from '../tauri'
import { usePermissions, type ReadyState, type PermissionsApi } from '../permissions'
import { hasSystemPermissions, type Platform } from '../platform'
import type { Persona } from '../persona'
import type { DownloadProgress } from '../types'

/**
 * The readiness checklist — "can LlamaChat actually do what it says".
 *
 * Visual language is v6's persona cards (`.choice` / `.mc`): one bordered
 * surface card per row, a check glyph (`#i-check`) when satisfied, a dash
 * glyph (`#i-dash`) when not. No emoji (R7), no new colours, no glow.
 *
 * Three rules this component exists to enforce:
 *
 *  1. **Never claim a state we did not verify.** The Rust side returns `true`
 *     unconditionally for accessibility / screen recording on Windows and
 *     Linux because they are not permission-gated there. Rendering that as a
 *     granted permission would be a lie, so those rows simply do not exist off
 *     macOS. In a browser dev build there is no backend at all, so every row
 *     reads "can't check here" rather than green.
 *
 *  2. **Only a real blocker blocks.** Ollama being down breaks everything, so
 *     it is called out as broken — but the user is still told what is wrong and
 *     still allowed past. Accessibility and Screen Recording are optional and
 *     say so.
 *
 *  3. **Say what the persona needs, in that persona's language.** A simple user
 *     never sees Screen Recording or TCC jargon; they get Cowork's plain
 *     "click and type for you" framing. Developers get precise wording.
 */

type RowKey = 'ollama' | 'accessibility' | 'screen_recording' | 'vision';

interface Row {
  key: RowKey;
  icon: IconName;
  title: string;
  /** One or two sentences. Persona-specific. */
  blurb: string;
  state: ReadyState;
  /** Shown under the blurb only when the row is not satisfied. */
  caveat?: ReactNode;
  /** True when failing this row breaks the product, not just a feature. */
  blocking?: boolean;
  /** Overrides the default granted/not-granted wording. */
  label?: Partial<Record<ReadyState, string>>;
  actions: Action[];
}

interface Action {
  label: string;
  run: () => void;
  /** Renders as the amber/warning chip rather than the neutral one. */
  warn?: boolean;
}

interface Props {
  persona: Persona;
  platform: Platform;
  /** Offer the optional vision-model download (a ~4.7 GB pull). */
  showVision?: boolean;
  /** Reuse a caller's poller instead of starting a second one. */
  api?: PermissionsApi;
}

export function ReadinessChecklist({ persona, platform, showVision = false, api }: Props) {
  const own = usePermissions(!api);
  const { state, refresh, polled } = api ?? own;
  const mac = hasSystemPermissions(platform);
  const dev = persona === 'dev';
  const live = isTauri();

  /** Set once a Screen Recording grant has been attempted this session. */
  const [srAttempted, setSrAttempted] = useState(false);
  const [llava, setLlava] = useState<'idle' | 'downloading' | 'done'>('idle');
  const [llavaPct, setLlavaPct] = useState(0);

  useEffect(() => {
    if (!showVision) return;
    let un: (() => void) | null = null;
    listen<DownloadProgress>('download_progress', (p) => {
      if (p.tag !== 'llava:7b') return;
      if (p.status === 'done') { setLlava('done'); setLlavaPct(100); }
      else if (p.status === 'error') setLlava('idle');
      else setLlavaPct(p.pct ?? 0);
    }).then((u) => { un = u; });
    return () => un?.();
  }, [showVision]);

  const srState = state('screen_recording');

  async function grantAccessibility() {
    await invoke('request_accessibility');
    // The prompt's "Open System Settings" button is easy to miss — take them there.
    await invoke('open_settings_pane', { pane: 'accessibility' });
    await refresh();
  }

  async function grantScreenRecording() {
    setSrAttempted(true);
    // Pops the prompt AND registers the app under Privacy ▸ Screen Recording.
    await invoke('request_screen_recording');
    await invoke('open_settings_pane', { pane: 'screen_recording' });
    // Deliberately re-poll rather than trusting the return value: macOS caches
    // the per-process decision, so this will usually still read false until a
    // relaunch. The row then shows the restart caveat instead of a fake tick.
    await refresh();
  }

  async function openPane(pane: string) {
    await invoke('open_settings_pane', { pane });
    await refresh();
  }

  function downloadLlava() {
    setLlava('downloading');
    invoke('download_model', { tag: 'llava:7b' });
  }

  const recheck: Action = { label: 'Re-check', run: () => { refresh(); } };

  const rows: Row[] = [];

  // ── Ollama. The only genuine blocker, on every platform, every persona. ──
  const ollama = state('ollama');
  rows.push({
    key: 'ollama',
    icon: 'box',
    title: dev ? 'Ollama is running' : 'The local AI engine is running',
    blurb: dev
      ? 'The local model server behind chat, the router and the agent loop. Everything in LlamaChat goes through it.'
      : 'This is what actually does the thinking, on this computer. Without it LlamaChat cannot answer anything.',
    state: ollama,
    blocking: true,
    caveat: ollama === 'no' ? <OllamaHelp persona={persona} platform={platform} /> : undefined,
    actions: [recheck],
  });

  // ── macOS-only system permissions. Absent everywhere else, on purpose. ──
  if (mac) {
    const acc = state('accessibility');
    rows.push({
      key: 'accessibility',
      icon: 'shield',
      title: dev ? 'Accessibility' : 'Allowed to use this computer',
      blurb: dev
        ? 'Required for the agent to move the mouse, press keys, and read the accessibility tree. Cowork and Code both need it.'
        : 'Lets LlamaChat click and type for you when you ask it to do something on this Mac. Only used while a Cowork task is running.',
      state: acc,
      caveat: acc === 'no'
        ? <span className="rnote">Optional. Chat works without it; Cowork can&apos;t act on your Mac until it&apos;s on.</span>
        : undefined,
      actions: acc === 'ok'
        ? [{ label: 'Open Settings', run: () => openPane('accessibility') }, recheck]
        : [{ label: 'Grant', run: grantAccessibility }, { label: 'Open Settings', run: () => openPane('accessibility') }],
    });

    // Screen Recording only matters for the screenshot-vision perception path,
    // which is a developer setting. A simple user never turns it on, so they
    // are never asked about it.
    if (dev) {
      rows.push({
        key: 'screen_recording',
        icon: 'eye',
        title: 'Screen Recording',
        blurb:
          'Only needed if you switch perception to screenshot vision. The default accessibility-tree path does not use it.',
        state: srState,
        caveat: srState === 'no'
          ? (
            <span className="rnote warn">
              <Icon name="alert" size={12} />
              macOS caches this per process. After you grant it, this row keeps reading &quot;not
              granted&quot; until LlamaChat relaunches.
            </span>
          )
          : undefined,
        actions: srState === 'ok'
          ? [{ label: 'Open Settings', run: () => openPane('screen_recording') }, recheck]
          : [
            { label: 'Grant', run: grantScreenRecording },
            { label: 'Restart to apply', run: () => invoke('restart_app'), warn: srAttempted },
          ],
      });
    }
  }

  // ── Optional vision model. Settings only — not a 4.7 GB first-run gate. ──
  if (showVision && dev) {
    rows.push({
      key: 'vision',
      icon: 'eye',
      title: 'Vision model (LLaVA)',
      blurb: 'Optional. Describes screenshots to the agent when perception is set to screenshot vision. About 4.7 GB.',
      state: llava === 'done' ? 'ok' : live ? 'no' : 'unknown',
      // Not a permission — don't borrow permission wording for it.
      label: {
        ok: 'Installed',
        no: llava === 'downloading' ? 'Downloading' : 'Not installed',
        unknown: "Can't check here",
      },
      actions: [{
        label: llava === 'downloading' ? `${Math.round(llavaPct)}%` : llava === 'done' ? 'Re-download' : 'Download',
        run: downloadLlava,
      }],
    });
  }

  return (
    <div className="ready">
      {rows.map((r) => <ReadyRow key={r.key} row={r} live={live} />)}

      {/* Platforms without TCC: say so once, rather than faking two green rows. */}
      {!mac && (
        <p className="rfoot">
          {platform === 'windows' ? 'Windows' : 'Linux'} doesn&apos;t gate mouse, keyboard or screen
          access behind a system permission, so there is nothing else to approve here.
        </p>
      )}

      {/* Recovery affordance for stale TCC entries. macOS only — tccutil is a
          no-op elsewhere, so offering it would be theatre. */}
      {mac && (
        <div className="rrecover">
          <p>
            Granted something but it still shows as off? Unsigned builds leave stale macOS privacy
            entries behind &mdash; resetting clears them so a fresh Grant sticks.
          </p>
          <div className="ract">
            <button
              type="button"
              className="chip"
              disabled={!live}
              onClick={async () => { await invoke('reset_permissions'); await refresh(); }}
            >
              <Icon name="refresh" size={13} /> Reset permissions
            </button>
            <button type="button" className="chip" disabled={!live} onClick={() => invoke('restart_app')}>
              <Icon name="run" size={13} /> Restart LlamaChat
            </button>
          </div>
        </div>
      )}

      {!live && (
        <p className="rfoot">
          Browser dev build &mdash; the desktop backend isn&apos;t attached, so nothing above can be
          checked or granted. These rows show real state only in the app.
        </p>
      )}
      {live && !polled && <p className="rfoot">Checking&hellip;</p>}
    </div>
  );
}

/** One card. Check glyph when satisfied, dash glyph otherwise (v6's treatment). */
function ReadyRow({ row, live }: { row: Row; live: boolean }) {
  const cls =
    row.state === 'ok' ? ' on'
      : row.state === 'no' && row.blocking ? ' bad'
        : '';
  return (
    <div className={`rrow${cls}`}>
      <span className="rst" aria-hidden="true">
        <Icon name={row.state === 'ok' ? 'check' : 'dash'} size={13} />
      </span>
      <div className="rtx">
        <b>
          <Icon name={row.icon} size={13} /> {row.title}
          <em>{row.label?.[row.state] ?? stateLabel(row.state, row.blocking)}</em>
        </b>
        <p>{row.blurb}</p>
        {row.caveat}
      </div>
      <div className="ract">
        {row.actions.map((a) => (
          <button
            key={a.label}
            type="button"
            className={`chip${a.warn ? ' warn' : ''}`}
            disabled={!live}
            title={live ? undefined : 'Desktop app only'}
            onClick={a.run}
          >
            {a.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function stateLabel(s: ReadyState, blocking?: boolean): string {
  if (s === 'ok') return 'Ready';
  if (s === 'checking') return 'Checking';
  if (s === 'unknown') return "Can't check here";
  return blocking ? 'Not running' : 'Not granted';
}

/** Plain instructions for the one thing that genuinely stops the app working. */
function OllamaHelp({ persona, platform }: { persona: Persona; platform: Platform }) {
  if (persona === 'dev') {
    return (
      <span className="rnote warn">
        <Icon name="alert" size={12} />
        <span>
          Start it with <code>ollama serve</code>
          {platform === 'macos' ? ', or launch the Ollama app.' : '.'} Install it from
          ollama.com/download. LlamaChat also tries to start it itself the first time it pulls a
          model, so this may clear on its own.
        </span>
      </span>
    );
  }
  return (
    <span className="rnote warn">
      <Icon name="alert" size={12} />
      <span>
        {platform === 'macos'
          ? 'Open the Ollama app from your Applications folder, then come back and re-check.'
          : 'Start Ollama, then come back and re-check. Get it from ollama.com/download.'}
      </span>
    </span>
  );
}

interface StepProps {
  persona: Persona;
  platform: Platform;
  /** The app's poller, so the step and the checklist agree on one state. */
  api: PermissionsApi;
  onContinue: () => void;
  onBack?: () => void;
}

/**
 * The startup readiness step. Uses v6's `.setup` shell so it reads as part of
 * the same first-run sequence as the persona question and the download screen.
 *
 * Always skippable — Continue is never disabled. If Ollama is down we say
 * exactly what will be broken and let the user through anyway, rather than
 * trapping them behind a checklist they may not be able to satisfy right now.
 */
export function ReadinessStep({ persona, platform, api, onContinue, onBack }: StepProps) {
  const blocked = api.state('ollama') === 'no';

  return (
    <div className="setup">
      <div className="mk"><Icon name="llama" /></div>
      <h1>{blocked ? 'One thing is missing.' : 'Quick readiness check.'}</h1>
      <p className="sub">
        {persona === 'dev'
          ? 'What LlamaChat needs on this machine, and what each thing unlocks. Everything except the model server is optional.'
          : 'A quick look at what LlamaChat needs on this computer. You can skip anything optional and come back later.'}
      </p>

      <div style={{ width: '100%', maxWidth: 660 }}>
        <ReadinessChecklist persona={persona} platform={platform} api={api} />
      </div>

      <div className="go">
        <button type="button" onClick={onContinue}>Continue</button>
        {onBack && <button type="button" onClick={onBack}>Back</button>}
        <span className="note">
          {blocked
            ? 'You can continue, but replies will fail until the model server is running.'
            : 'You can change all of this later in Settings.'}
        </span>
      </div>
    </div>
  );
}
