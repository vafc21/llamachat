import { useState, useRef, useEffect, useMemo, type KeyboardEvent, type ReactNode } from 'react'
import type { TierModel } from '../types'
import { ModelPicker } from './ModelPicker'
import { CommandMenu } from './CommandMenu'
import { Icon } from './Icon'
import { ContextRing } from './ContextRing'
import { PermMenu } from './PermMenu'
import type { AgentPermMode } from '../perm'
import { MODE_LABEL, type Mode, type Persona } from '../persona'
import { menuQuery, parseCommand, type SlashCommand } from '../commands'

/**
 * The composer — ported from v6.
 *
 * It owns the segmented Chat | Cowork | Code control, which is the application's
 * ONLY mode switcher (R1). The sidebar deliberately does not get a second one.
 *
 * Three shapes, all the same component:
 *   'centered' — the empty state, greeting above it
 *   'docked'   — pinned under an in-progress conversation
 *   'code'     — bottom-docked, context chip row above, tool affordance inside
 */

export type ComposerVariant = 'centered' | 'docked' | 'code';

interface InputBarProps {
  onSend: (text: string) => void;
  /** Run a slash command: name without slash + the raw arg string. */
  onCommand: (name: string, args: string) => void;
  disabled: boolean;
  /** Built-in commands + one per skill. */
  commands: SlashCommand[];

  persona: Persona;
  mode: Mode;
  modes: Mode[];
  onMode: (m: Mode) => void;

  tiers: TierModel[];
  selectedModel: string;
  onSelectModel: (tag: string) => void;
  onBrowseAll: () => void;

  variant?: ComposerVariant;
  placeholder?: string;
  /** Estimated tokens currently in the window (dev persona only). */
  ctxUsed?: number;
  ctxTotal?: number;

  /** Agent permission mode — the amber chip in the Code composer. */
  agentPermMode?: AgentPermMode;
  onPermMode?: (m: AgentPermMode) => void;
  /** Extra row under the control row (Cowork's scope + tools + permission). */
  secondRow?: ReactNode;
  /** Stop an in-flight run. */
  onStop?: () => void;
}

export function InputBar({
  onSend, onCommand, disabled, commands,
  persona, mode, modes, onMode,
  tiers, selectedModel, onSelectModel, onBrowseAll,
  variant = 'centered', placeholder,
  ctxUsed = 0, ctxTotal = 0,
  agentPermMode, onPermMode, secondRow, onStop,
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

  useEffect(() => { inputRef.current?.focus(); }, [mode, variant]);

  function submit() {
    const text = input.trim();
    if (!text || disabled) return;
    const cmd = parseCommand(text);
    if (cmd) onCommand(cmd.name, cmd.args);
    else onSend(text);
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

  const isCode = variant === 'code';
  const showRing = persona === 'dev' && (variant !== 'centered' || isCode) && ctxTotal > 0;

  const defaultPlaceholder =
    isCode ? 'Describe a task — tools are on, I can read and change files here'
      : mode === 'cowork' ? 'Give me something to work on'
        : variant === 'docked' ? 'Reply'
          : 'Ask anything';

  return (
    <div className={isCode ? 'dcomp' : 'comp'}>
      <div style={{ position: 'relative' }}>
        {menuOpen && (
          <CommandMenu commands={filtered} activeIndex={menuIndex} onPick={pick} onHover={setMenuIndex} />
        )}
        <textarea
          ref={inputRef}
          value={input}
          onChange={(e) => { setInput(e.target.value); setDismissed(false); }}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          rows={1}
          placeholder={disabled ? 'Working…' : (placeholder ?? defaultPlaceholder)}
        />
      </div>

      <div className="crow">
        {/* R1: the single mode switcher. */}
        <div className="seg" role="tablist" aria-label="Mode">
          {modes.map((m) => (
            <button
              key={m}
              type="button"
              role="tab"
              aria-selected={m === mode}
              className={m === mode ? 'on' : undefined}
              onClick={() => onMode(m)}
            >
              {MODE_LABEL[m]}
            </button>
          ))}
        </div>

        {/* R10: Code mode says, in the composer, that the model is tool-equipped. */}
        {isCode && (
          <div className="tools" title="This mode gives the model shell, file and browser tools">
            <Icon name="tool" /> Tool mode
          </div>
        )}
        {isCode && agentPermMode && onPermMode && (
          <PermMenu mode={agentPermMode} onPick={onPermMode} />
        )}

        <div className="sp" />

        {/* R17: simple persona never sees a model name — it gets an Auto pill. */}
        {persona === 'dev' ? (
          <ModelPicker tiers={tiers} selected={selectedModel} onSelect={onSelectModel} onBrowseAll={onBrowseAll} />
        ) : (
          <span className="modelpick" title="LlamaChat picks the model for each message">
            <Icon name="spark" size={14} /><b>Auto</b>
          </span>
        )}

        {/* R5: the context-window meter. Dev persona only. */}
        {showRing && <ContextRing used={ctxUsed} total={ctxTotal} />}


        {disabled && onStop ? (
          <button type="button" className="sendb" onClick={onStop} title="Stop">
            <Icon name="stop" size={15} />
          </button>
        ) : (
          <button
            type="button"
            className="sendb"
            onClick={submit}
            disabled={disabled || !input.trim()}
            title="Send"
          >
            <Icon name="send" size={15} />
          </button>
        )}
      </div>

      {secondRow}
    </div>
  );
}
