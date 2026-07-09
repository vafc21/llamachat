import { useState, useRef, useEffect, type KeyboardEvent } from 'react'

interface InputBarProps {
  onSend: (text: string) => void;
  onToolCall: (tool: string) => void;
  disabled: boolean;
}

export function InputBar({ onSend, onToolCall, disabled }: InputBarProps) {
  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 200) + 'px';
  }, [input]);

  function handleSend() {
    if (!input.trim() || disabled) return;
    onSend(input.trim());
    setInput('');
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (input.startsWith('/') && !input.includes(' ')) {
        onToolCall(input.slice(1));
        setInput('');
      } else {
        handleSend();
      }
    }
    // Shift+Enter for newline
  }

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="flex-shrink-0 border-t border-border px-3 py-2">
      {/* Available tools hint */}
      <div className="flex gap-3 mb-1.5 px-1">
        {['shell', 'file', 'browser'].map((tool) => (
          <button
            key={tool}
            onClick={() => {
              onToolCall(tool);
              inputRef.current?.focus();
            }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors
                       flex items-center gap-1"
            title={`/${tool}`}
          >
            <kbd className="px-1 py-0.5 bg-white/[0.03] border border-border rounded text-[9px]
                          text-text-muted font-mono">
              /{tool}
            </kbd>
          </button>
        ))}
        <span className="text-[10px] text-text-muted ml-auto">
          Enter to send · Shift+Enter newline
        </span>
      </div>

      {/* Input row */}
      <div className="flex items-end gap-2">
        <textarea
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          placeholder={disabled ? 'Waiting for response…' : 'Ask anything or /shell, /file, /browser…'}
          rows={1}
          className="flex-1 bg-transparent text-[13px] text-text placeholder:text-text-muted
                     resize-none border-none outline-none leading-relaxed
                     disabled:opacity-50"
        />
        <button
          onClick={handleSend}
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
