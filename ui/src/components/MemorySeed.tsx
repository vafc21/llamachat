import { useState } from 'react'
import { invoke } from '../tauri'

const SEED_PROMPT =
  "You're helping set up my personal local AI assistant, which remembers facts about me across conversations. " +
  "Interview me: ask 6–8 short questions, ONE at a time, covering my name, what I do, current projects, the tools/tech I use, " +
  "how I like an assistant to respond (tone, length), and anything else worth remembering. " +
  'After I answer, output a concise bullet list written in the second person (e.g. "- Your name is …", "- You prefer …") ' +
  "that I can paste straight into my assistant's memory. Only output the bullet list at the end.";

/** First-run: copy a prompt into ChatGPT/Claude, paste the result back to seed memory.md. */
export function MemorySeed({ onNext, onSkip }: { onNext: () => void; onSkip: () => void }) {
  const [copied, setCopied] = useState(false);
  const [pasted, setPasted] = useState('');
  const [saving, setSaving] = useState(false);

  function copy() {
    navigator.clipboard.writeText(SEED_PROMPT)
      .then(() => { setCopied(true); setTimeout(() => setCopied(false), 1500); })
      .catch(() => {});
  }

  async function save() {
    const text = pasted.trim();
    if (!text) return;
    setSaving(true);
    const cur = (await invoke<string>('get_memory')) ?? '';
    const next = (cur.trim() ? cur.trimEnd() + '\n' : '') + text + '\n';
    await invoke('set_memory', { content: next });
    onNext();
  }

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm text-text font-medium">Seed your assistant's memory <span className="text-text-muted font-normal">· optional</span></p>
        <p className="text-[11px] text-text-muted mt-1 leading-relaxed">
          Copy the prompt below into <span className="text-text-secondary">ChatGPT or Claude</span>. It'll interview you and hand back
          a summary. Paste that summary here and LlamaChat will remember it in every chat. You can skip this and add memories anytime.
        </p>
      </div>

      <div className="rounded-lg border border-border bg-surface overflow-hidden">
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-border bg-white/[0.02]">
          <span className="text-[10px] text-text-muted uppercase tracking-wide">Prompt to copy</span>
          <button onClick={copy} className="text-[11px] text-accent hover:opacity-80">{copied ? 'Copied ✓' : 'Copy'}</button>
        </div>
        <div className="px-3 py-2 text-[11px] text-text-secondary leading-relaxed max-h-32 overflow-y-auto">
          {SEED_PROMPT}
        </div>
      </div>

      <div>
        <label className="text-[11px] text-text font-medium">Paste the summary here</label>
        <textarea
          value={pasted}
          onChange={(e) => setPasted(e.target.value)}
          rows={4}
          placeholder="- Your name is …&#10;- You're building …&#10;- You prefer concise answers"
          className="w-full mt-1 bg-bg border border-border rounded-lg px-3 py-2 text-[12px] text-text
                     placeholder:text-text-muted focus:border-accent outline-none resize-none leading-relaxed"
        />
      </div>

      <div className="flex gap-2">
        <button
          onClick={save}
          disabled={!pasted.trim() || saving}
          className={`flex-1 py-2 rounded-lg text-[13px] font-medium transition-opacity ${
            pasted.trim() && !saving ? 'bg-accent text-white hover:opacity-90' : 'bg-white/[0.04] text-text-muted border border-border cursor-not-allowed'
          }`}
        >
          Save & continue
        </button>
        <button onClick={onSkip} className="px-4 py-2 rounded-lg text-[13px] text-text-secondary hover:text-text border border-border">
          Skip
        </button>
      </div>
    </div>
  );
}
