import { useState, useRef, useEffect, useMemo, type KeyboardEvent } from 'react'
import type { TierModel } from '../types'
import { ModelPicker } from './ModelPicker'
import { CommandMenu } from './CommandMenu'
import { menuQuery, parseCommand, type SlashCommand } from '../commands'

interface InputBarProps {
  onSend: (text: string) => void;
  /** Run a slash command: name without slash + the raw arg string. */
  onCommand: (name: string, args: string) => void;
  disabled: boolean;
  tiers: TierModel[];
  selectedModel: string;
  onSelectModel: (tag: string) => void;
  onBrowseAll: () => void;
  /** Built-in commands + one per skill. */
  commands: SlashCommand[];
  agentMode: boolean;
  agentPermMode: 'plan' | 'ask' | 'auto' | 'bypass';
  onToggleAgent: () => void;
  onCycleMode: () => void;
}

const AGENT_MODE_LABEL: Record<string, string> = { plan: 'Plan', ask: 'Ask', auto: 'Auto', bypass: 'Bypass' };

export function InputBar({
  onSend, onCommand, disabled, tiers, selectedModel, onSelectModel, onBrowseAll, commands,
  agentMode, agentPermMode, onToggleAgent, onCycleMode,
}: InputBarProps) {
  const [input, setInput] = useState('');
  const [menuIndex, setMenuIndex] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Command menu state: open only while typing a bare `/partial` (no space yet).
  const query = menuQuery(input);
  const filtered = useMemo(
    () => (query === null ? [] : commands.filter((c) => c.name.startsWith(query))),
    [query, commands]
  );
  const menuOpen = query !== null && !dismissed && filtered.length > 0;

  useEffect(() => { setMenuIndex(0); }, [query]);

  // Auto-resize textarea.
  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 200) + 'px';
  }, [input]);

  useEffect(() => { inputRef.current?.focus(); }, []);

  function submit() {
    const text = input.trim();
    if (!text || disabled) return;
    const cmd = parseCommand(text);
    if (cmd) {
      onCommand(cmd.name, cmd.args);
    } else {
      onSend(text);
    }
    setInput('');
    setDismissed(false);
  }

  function pick(cmd: SlashCommand) {
    if (cmd.takesArgs) {
      setInput(`/${cmd.name} `);
      inputRef.current?.focus();
    } else {
      onCommand(cmd.name, '');
      setInput('');
    }
    setDismissed(false);
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (menuOpen) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setMenuIndex((i) => (i + 1) % filtered.length); return; }
      if (e.key === 'ArrowUp') { e.preventDefault(); setMenuIndex((i) => (i - 1 + filtered.length) % filtered.length); return; }
      if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); pick(filtered[Math.min(menuIndex, filtered.length - 1)]); return; }
      if (e.key === 'Escape') { e.preventDefault(); setDismissed(true); return; }
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  return (
    <div className="flex-shrink-0 border-t border-border px-3 py-2">
      {/* Model picker + hint */}
      <div className="flex items-center gap-2 mb-1.5 px-1">
        <ModelPicker tiers={tiers} selected={selectedModel} onSelect={onSelectModel} onBrowseAll={onBrowseAll} />
        <button
          onClick={onToggleAgent}
          title="Agent mode — let the AI control your Mac"
          className={`text-[10px] px-1.5 py-0.5 rounded border transition-colors ${
            agentMode ? 'border-accent text-accent bg-accent-dim' : 'border-border text-text-muted hover:text-text'
          }`}
        >
          🤖 Agent{agentMode ? ' on' : ''}
        </button>
        {agentMode && (
          <button
            onClick={onCycleMode}
            title="Cycle permission mode: Ask → Auto → Bypass → Plan"
            className="text-[10px] px-1.5 py-0.5 rounded border border-accent/40 text-accent hover:bg-accent-dim transition-colors"
          >
            {AGENT_MODE_LABEL[agentPermMode]}
          </button>
        )}
        <span className="text-[10px] text-text-muted ml-auto">
          {agentMode ? 'Agent will act on your Mac' : (<>Type <kbd className="px-1 bg-white/[0.04] border border-border rounded font-mono">/</kbd> for commands · Enter to send</>)}
        </span>
      </div>

      {/* Input row (relative anchor for the command menu) */}
      <div className="relative flex items-end gap-2">
        {menuOpen && (
          <CommandMenu
            commands={filtered}
            activeIndex={menuIndex}
            onPick={pick}
            onHover={setMenuIndex}
          />
        )}
        <textarea
          ref={inputRef}
          value={input}
          onChange={(e) => { setInput(e.target.value); setDismissed(false); }}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          placeholder={disabled ? 'Working…' : agentMode ? 'Describe a task for the agent (e.g. open Chrome and search hi)…' : 'Ask anything, or / for commands…'}
          rows={1}
          className="flex-1 bg-transparent text-[13px] text-text placeholder:text-text-muted
                     resize-none border-none outline-none leading-relaxed disabled:opacity-50"
        />
        <button
          onClick={submit}
          disabled={disabled || !input.trim()}
          className={`flex-shrink-0 px-3 py-1.5 rounded text-[12px] font-medium transition-colors
            ${input.trim() && !disabled
              ? 'bg-accent text-white hover:opacity-90'
              : 'bg-white/[0.04] text-text-muted border border-border cursor-not-allowed'
            }`}
        >
          {disabled ? '···' : 'Send'}
        </button>
      </div>
    </div>
  );
}
