import type { SlashCommand } from '../commands'

interface Props {
  commands: SlashCommand[];
  activeIndex: number;
  onPick: (cmd: SlashCommand) => void;
  onHover: (index: number) => void;
}

const KIND_LABEL: Record<string, string> = { builtin: '', tool: 'tool', skill: 'skill' };

/** The `/` autocomplete popup shown above the input, Claude-Code style. */
export function CommandMenu({ commands, activeIndex, onPick, onHover }: Props) {
  if (commands.length === 0) return null;
  return (
    <div className="absolute bottom-full mb-2 left-0 right-0 z-20 max-h-72 overflow-y-auto
                    rounded-lg border border-border bg-surface shadow-xl p-1">
      <div className="px-2 py-1 text-[9px] uppercase tracking-wide text-text-muted">Commands</div>
      {commands.map((c, i) => (
        <button
          key={`${c.kind}:${c.name}`}
          onMouseEnter={() => onHover(i)}
          onClick={() => onPick(c)}
          className={`w-full text-left rounded px-2 py-1.5 flex items-center gap-2 transition-colors
            ${i === activeIndex ? 'bg-accent-dim' : 'hover:bg-white/[0.04]'}`}
        >
          <span className="text-[12px] text-text font-medium font-mono">/{c.name}</span>
          {c.argHint && <span className="text-[10px] text-text-muted font-mono">{c.argHint}</span>}
          <span className="text-[11px] text-text-muted truncate ml-auto pl-2">{c.description}</span>
          {c.kind !== 'builtin' && (
            <span className="text-[9px] text-accent border border-accent/40 rounded px-1 flex-shrink-0">
              {KIND_LABEL[c.kind]}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
