import { useState, useRef, useLayoutEffect } from 'react'
import { Icon } from './Icon'
import { PERM_MODES, PERM_LABEL, PERM_EXPLAIN, RISKY_MODES, type AgentPermMode } from '../perm'

/** Roughly the menu's height; used only to decide which way to open. */
const MENU_H = 300;

interface Props {
  mode: AgentPermMode;
  onPick: (m: AgentPermMode) => void;
}

/**
 * The permission dropdown (Cowork + Code).
 *
 * This replaces a chip that cycled through modes on click: the modes were
 * undiscoverable, you could not jump straight to one, and nothing said what
 * any of them did. The menu shows all five with their consequences.
 */
export function PermMenu({ mode, onPick }: Props) {
  const [open, setOpen] = useState(false);
  // Which way to open. The composer is centred in Cowork and docked at the
  // bottom in Code, so a fixed direction sends the menu off-screen in one of
  // them; decide from the trigger's actual position instead.
  const [drop, setDrop] = useState<'up' | 'down'>('up');
  const triggerRef = useRef<HTMLButtonElement>(null);
  const risky = RISKY_MODES.includes(mode);

  useLayoutEffect(() => {
    if (!open) return;
    const r = triggerRef.current?.getBoundingClientRect();
    if (!r) return;
    setDrop(r.top < MENU_H && window.innerHeight - r.bottom > r.top ? 'down' : 'up');
  }, [open]);

  return (
    <div style={{ position: 'relative' }}>
      <button
        ref={triggerRef}
        type="button"
        className={`chip${risky ? ' warn' : ''}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        title={PERM_EXPLAIN[mode]}
      >
        <Icon name="shield" size={13} />
        {PERM_LABEL[mode]}
        <Icon name="chev" size={12} />
      </button>

      {open && (
        <>
          <div style={{ position: 'fixed', inset: 0, zIndex: 10 }} onClick={() => setOpen(false)} />
          <div
            className="card permmenu"
            style={{
              position: 'absolute', right: 0, zIndex: 20, width: 290,
              background: 'var(--surface2)',
              ...(drop === 'up'
                ? { bottom: '100%', marginBottom: 8 }
                : { top: '100%', marginTop: 8 }),
            }}
            role="listbox"
            aria-label="Permission mode"
          >
            <div className="cardh"><Icon name="shield" /><h2>What the agent may do</h2></div>
            {PERM_MODES.map((m) => (
              <button
                key={m}
                type="button"
                role="option"
                aria-selected={m === mode}
                className={`permrow${m === mode ? ' on' : ''}`}
                onClick={() => { onPick(m); setOpen(false); }}
              >
                <div className="pm">
                  <b>{PERM_LABEL[m]}</b>
                  {m === mode && <Icon name="check" size={14} />}
                </div>
                <span>{PERM_EXPLAIN[m]}</span>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
