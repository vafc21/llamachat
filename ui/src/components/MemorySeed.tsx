import { useState } from 'react'
import { invoke } from '../tauri'

/**
 * Memory-transfer prompt — mirrors Claude's "import memory" flow: it asks the
 * user's EXISTING assistant to dump everything it already knows about them, with
 * no interview. Paste the result and it lands in memory.md.
 */
const SEED_PROMPT =
  "I'm moving to a new personal AI assistant and want to bring my context with me. " +
  "Based only on what you already know and remember about me — our past conversations, saved memories, and any custom instructions — " +
  "write a single summary I can hand to my new assistant. Do NOT ask me any questions; just use what you already know. " +
  "Cover: who I am (name, role, location if known), what I'm currently working on (projects and goals), the tools and technologies I use, " +
  "how I like an assistant to communicate (tone, length, format), and any other durable facts or preferences worth remembering. " +
  'Write it as a concise bulleted list in the second person (e.g. "- Your name is …", "- You prefer …"). ' +
  "Skip anything you're unsure about, and output only the list.";

/** First-run: transfer your memory from another assistant into memory.md. No questions asked. */
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
        <p className="text-sm text-text font-medium">Bring your memory over <span className="text-text-muted font-normal">· optional</span></p>
        <p className="text-[11px] text-text-muted mt-1 leading-relaxed">
          Already use <span className="text-text-secondary">ChatGPT, Claude, or Gemini</span>? Copy the prompt below into it — it'll
          summarize what it already knows about you (no questions to answer) and hand back a list. Paste that here and LlamaChat
          remembers it in every chat. No other assistant? Just skip — you can add memories anytime with <span className="text-text-secondary">/remember</span>.
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
        <label className="text-[11px] text-text font-medium">Paste what it gives you back</label>
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
          Save &amp; continue
        </button>
        <button onClick={onSkip} className="px-4 py-2 rounded-lg text-[13px] text-text-secondary hover:text-text border border-border">
          Skip
        </button>
      </div>
    </div>
  );
}
