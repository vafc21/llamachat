import type { Conversation } from '../types'
import { Icon } from './Icon'
import type { Mode } from '../persona'

/** Non-mode destinations. These are NOT modes — see R1: the composer's
 *  segmented control is the only mode switcher. Opening one of these overlays
 *  the main pane and returns you to the mode you were in. */
export type NavView = 'library' | 'skills' | 'memory' | 'settings';

interface SidebarProps {
  open: boolean;
  mode: Mode;
  conversations: Conversation[];
  activeId: string;
  /** Currently open nav destination, or null when in a mode pane. */
  nav: NavView | null;
  /** Count of models ready to run — shown on the Library pill (dev only). */
  readyModels: number;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onNav: (v: NavView) => void;
}

export function Sidebar({
  open, mode, conversations, activeId, nav, readyModels,
  onSelect, onNew, onDelete, onNav,
}: SidebarProps) {
  if (!open) return null;

  // v6 relabels the recents list in Code mode ("Tasks").
  const listHeading = mode === 'code' ? 'Tasks' : 'Recents';
  const shown = conversations.slice(0, 12);

  return (
    <aside className="side">
      <div className="brand">
        <div className="m"><Icon name="llama" /></div>
        <b>LlamaChat</b>
      </div>

      <button type="button" className="newbtn" onClick={onNew}>
        <Icon name="plus" size={15} /> New
        <span style={{ flex: 1 }} />
        <span className="kbd">⌘N</span>
      </button>

      <div className="nav">
        {/* R17: no model library in the sidebar for the simple persona. */}
        <button
          type="button"
          className={`dev-only${nav === 'library' ? ' on' : ''}`}
          onClick={() => onNav('library')}
        >
          <Icon name="lib" /> Library
          {readyModels > 0 && <span className="pill">{readyModels} ready</span>}
        </button>
        <button type="button" className={nav === 'skills' ? 'on' : undefined} onClick={() => onNav('skills')}>
          <Icon name="tool" /> Skills
        </button>
        <button type="button" className={nav === 'memory' ? 'on' : undefined} onClick={() => onNav('memory')}>
          <Icon name="brain" /> Memory
        </button>
        <button type="button" className={nav === 'settings' ? 'on' : undefined} onClick={() => onNav('settings')}>
          <Icon name="set" /> Settings
        </button>
      </div>

      <div className="shead">{listHeading}</div>
      <div className="slist">
        {shown.map((c) => (
          <div
            key={c.id}
            role="button"
            tabIndex={0}
            className={`sitem${c.id === activeId && nav === null ? ' on' : ''}`}
            onClick={() => onSelect(c.id)}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect(c.id); } }}
          >
            <i />
            <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{c.title}</span>
            <button
              type="button"
              className="del"
              title="Delete conversation"
              onClick={(e) => { e.stopPropagation(); onDelete(c.id); }}
            >
              <Icon name="x" size={12} />
            </button>
          </div>
        ))}
        {shown.length === 0 && (
          <div style={{ fontSize: 12, color: 'var(--text3)', padding: '8px 10px' }}>
            Nothing yet.
          </div>
        )}
      </div>
      {conversations.length > shown.length && (
        <button type="button" className="viewall" onClick={() => onSelect(conversations[0].id)}>
          {conversations.length - shown.length} more
        </button>
      )}

      <div className="sp" />

      <div className="acct">
        <div className="av">VP</div>
        <div>
          <b>Vlad</b>
          {/* R17: the simple persona doesn't get a model count here either. */}
          <span className="dev-only">
            Developer{readyModels > 0 ? ` · ${readyModels} model${readyModels === 1 ? '' : 's'}` : ''}
          </span>
          <span className="simple-only">Local</span>
        </div>
      </div>
    </aside>
  );
}
