import { useState, useEffect, useCallback } from 'react'
import { invoke, isTauri } from '../tauri'

/** View & edit the long-term memory.md that's injected into every chat. */
export function MemoryTab() {
  const [content, setContent] = useState('');
  const [saved, setSaved] = useState('');
  const [dir, setDir] = useState('');
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    const m = (await invoke<string>('get_memory')) ?? '';
    const d = (await invoke<string>('get_memory_dir')) ?? '';
    setContent(m);
    setSaved(m);
    setDir(d);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  const dirty = content !== saved;

  async function save() {
    await invoke('set_memory', { content });
    setSaved(content);
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
      <div className="flex-shrink-0 px-4 pt-4 pb-3 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-sm font-semibold text-text">Memory</h1>
            <p className="text-[11px] text-text-muted mt-0.5">
              Durable facts the assistant is told at the start of every chat. Add with{' '}
              <span className="font-mono text-text-secondary">/remember</span>, remove with{' '}
              <span className="font-mono text-text-secondary">/forget</span>, or edit directly here.
            </p>
          </div>
          <button
            onClick={save}
            disabled={!dirty || !isTauri()}
            className={`px-2.5 py-1.5 text-[12px] rounded font-medium transition-colors ${
              dirty && isTauri()
                ? 'bg-accent text-white hover:opacity-90'
                : 'bg-white/[0.04] text-text-muted border border-border cursor-not-allowed'
            }`}
          >
            {dirty ? 'Save' : 'Saved'}
          </button>
        </div>
        {dir && (
          <p className="text-[10px] text-text-muted mt-2 font-mono truncate" title={`${dir}/memory.md`}>
            {dir}/memory.md
          </p>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {loading ? (
          <div className="text-[11px] text-text-muted text-center py-8">Loading…</div>
        ) : (
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder={"- The user's name is …\n- Prefers concise answers\n- Working on a project called …"}
            className="w-full h-full min-h-[300px] bg-bg border border-border rounded-lg px-3 py-2
                       text-[13px] text-text placeholder:text-text-muted focus:border-accent outline-none
                       resize-none leading-relaxed font-mono"
          />
        )}
      </div>
    </div>
  );
}
