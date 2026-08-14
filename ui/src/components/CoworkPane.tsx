import type { ReactNode } from 'react'
import { Icon } from './Icon'

/** A Cowork run — one agent task the user handed over. */
export interface CoworkTask {
  id: string;
  /** What the user asked for. */
  title: string;
  /** Live one-line progress, e.g. "Working · 3 tool calls". */
  detail: string;
  state: 'working' | 'done' | 'waiting' | 'error';
  startedAt: number;
  updatedAt: number;
}

const DOT: Record<CoworkTask['state'], string> = {
  working: 'var(--accent)',
  done: 'var(--ok)',
  waiting: 'var(--warn)',
  error: 'var(--err)',
};

function ago(ts: number, now: number): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 60) return 'now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.round(h / 24);
  return d === 1 ? 'yesterday' : `${d}d`;
}

interface Props {
  greeting: string;
  composer: ReactNode;
  tasks: CoworkTask[];
  onClear: () => void;
}

/**
 * Cowork (R4): the assistant uses tools, but nothing on this screen is
 * developer-facing — no model names, no token counts, no runtime. Just the
 * greeting, the composer with its scope/permission row, and the list of work
 * in flight, ported from v6's `.active`.
 */
export function CoworkPane({ greeting, composer, tasks, onClear }: Props) {
  const now = Date.now();
  return (
    <div className="center top">
      <div className="greet"><Icon name="llama" />{greeting}</div>
      {composer}

      <div className="active">
        <div className="ahead">
          <h3>Active</h3>
          {tasks.length > 0 && <button type="button" onClick={onClear}>Clear active</button>}
        </div>
        {tasks.length === 0 && (
          <div className="aempty">Nothing running. Hand over a task above and it shows up here.</div>
        )}
        {tasks.map((t) => (
          // A task row is a live status readout, not a link: there is no
          // per-task detail view to open, so it must not look clickable.
          <div className="arow" key={t.id}>
            <i className="dot" style={{ background: DOT[t.state] }} />
            <div className="tx">
              <b>{t.title}</b>
              <span>{t.detail}</span>
            </div>
            <div className="meta">{ago(t.updatedAt, now)}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
