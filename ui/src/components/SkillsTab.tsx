import { useState, useEffect, useRef } from 'react'
import type { Skill } from '../types'
import { skillUid, slugify } from '../skills'

interface Props {
  skills: Skill[];
  onChange: (skills: Skill[]) => void;
}

/** Create & manage skills. Each skill is invocable in chat as /<name>. */
export function SkillsTab({ skills, onChange }: Props) {
  const [editing, setEditing] = useState<Skill | null>(null);

  function upsert(skill: Skill) {
    const exists = skills.some((s) => s.id === skill.id);
    onChange(exists ? skills.map((s) => (s.id === skill.id ? skill : s)) : [...skills, skill]);
    setEditing(null);
  }
  function remove(id: string) {
    onChange(skills.filter((s) => s.id !== id));
    if (editing?.id === id) setEditing(null);
  }

  return (
    <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
      <div className="flex-shrink-0 px-4 pt-4 pb-3 border-b border-border">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-sm font-semibold text-text">Skills</h1>
            <p className="text-[11px] text-text-muted mt-0.5">
              Reusable instructions you invoke in chat by typing <span className="font-mono text-text-secondary">/name</span>.
            </p>
          </div>
          <button
            onClick={() => setEditing({ id: skillUid(), name: '', title: '', instructions: '', description: '' })}
            className="px-2.5 py-1.5 text-[12px] rounded border border-border text-text-secondary
                       hover:border-border-strong hover:text-text transition-colors"
          >
            + New skill
          </button>
        </div>
      </div>

      {editing && (
        <SkillForm
          key={editing.id}
          initial={editing}
          existingNames={skills.filter((s) => s.id !== editing.id).map((s) => s.name)}
          onSave={upsert}
          onCancel={() => setEditing(null)}
        />
      )}

      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
        {skills.length === 0 && !editing && (
          <div className="text-[11px] text-text-muted text-center py-8">
            No skills yet. Create one and call it in chat with <span className="font-mono">/name</span>.
          </div>
        )}
        {skills.map((s) => (
          <div key={s.id} className="border border-border rounded-lg p-3 bg-surface">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium text-text">{s.title || s.name}</span>
                  <span className="text-[10px] font-mono text-accent">/{s.name}</span>
                </div>
                {s.description && <p className="text-[11px] text-text-muted mt-0.5">{s.description}</p>}
                <p className="text-[11px] text-text-secondary mt-1 line-clamp-2">{s.instructions}</p>
              </div>
              <div className="flex items-center gap-1.5 flex-shrink-0">
                <button onClick={() => setEditing(s)} className="text-[11px] text-text-muted hover:text-text px-2 py-1">Edit</button>
                <button onClick={() => remove(s.id)} title="Delete" className="text-text-muted hover:text-error p-1">
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                    <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" />
                  </svg>
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function SkillForm({
  initial, existingNames, onSave, onCancel,
}: {
  initial: Skill;
  existingNames: string[];
  onSave: (s: Skill) => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState(initial.title);
  const [name, setName] = useState(initial.name);
  const [nameEdited, setNameEdited] = useState(!!initial.name);
  const [description, setDescription] = useState(initial.description ?? '');
  const [instructions, setInstructions] = useState(initial.instructions);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => { titleRef.current?.focus(); }, []);

  // Auto-derive the slug from the title until the user edits it directly.
  const effectiveName = nameEdited ? name : slugify(title);
  const clash = existingNames.includes(effectiveName);
  const valid = title.trim() && effectiveName && instructions.trim() && !clash;

  function save() {
    if (!valid) return;
    onSave({ id: initial.id, name: effectiveName, title: title.trim(), description: description.trim() || undefined, instructions: instructions.trim() });
  }

  return (
    <div className="flex-shrink-0 px-4 py-3 border-b border-border bg-surface/50 space-y-3">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <Field label="Title" hint="Shown in the list">
          <input ref={titleRef} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Summarize"
            className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text placeholder:text-text-muted focus:border-accent outline-none" />
        </Field>
        <Field label="Command" hint="Invoke as /name">
          <input value={effectiveName} onChange={(e) => { setNameEdited(true); setName(slugify(e.target.value)); }} placeholder="summarize"
            className={`w-full bg-bg border rounded px-2 py-1.5 text-[12px] text-text placeholder:text-text-muted outline-none font-mono ${clash ? 'border-error' : 'border-border focus:border-accent'}`} />
        </Field>
      </div>
      <Field label="Description" hint="One line, shown in the / menu">
        <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Summarize the text you provide"
          className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text placeholder:text-text-muted focus:border-accent outline-none" />
      </Field>
      <Field label="Instructions" hint="Sent as the system prompt when the skill runs">
        <textarea value={instructions} onChange={(e) => setInstructions(e.target.value)} rows={4} placeholder="You are a precise summarizer. Produce 3-5 tight bullet points…"
          className="w-full bg-bg border border-border rounded px-2 py-1.5 text-[12px] text-text placeholder:text-text-muted focus:border-accent outline-none resize-none leading-relaxed" />
      </Field>
      {clash && <p className="text-[10px] text-error">A skill named /{effectiveName} already exists.</p>}
      <div className="flex gap-2">
        <button onClick={save} disabled={!valid}
          className={`px-3 py-1.5 rounded text-[12px] font-medium transition-colors ${valid ? 'bg-accent text-white hover:opacity-90' : 'bg-white/[0.04] text-text-muted border border-border cursor-not-allowed'}`}>
          Save skill
        </button>
        <button onClick={onCancel} className="px-3 py-1.5 rounded text-[12px] text-text-secondary hover:text-text border border-border">Cancel</button>
      </div>
    </div>
  );
}

function Field({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-[11px] text-text font-medium">{label}</span>
      <span className="block text-[10px] text-text-muted mb-1">{hint}</span>
      {children}
    </label>
  );
}
