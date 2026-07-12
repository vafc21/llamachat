import { useState, useRef, useCallback, useEffect } from 'react'
import { Sidebar } from './components/Sidebar'
import { ChatArea } from './components/ChatArea'
import { InputBar } from './components/InputBar'
import { SetupWizard } from './components/SetupWizard'
import { ModelLibrary } from './components/ModelLibrary'
import { Settings } from './components/Settings'
import { SkillsTab } from './components/SkillsTab'
import { MemoryTab } from './components/MemoryTab'
import { WelcomeSteps } from './components/WelcomeSteps'
import { invoke, listen, isTauri } from './tauri'
import { MOCK_HARDWARE, tiersFromPlan, mockTiers } from './models'
import { loadSkills, saveSkills } from './skills'
import { allCommands } from './commands'
import type { Message, Conversation, HardwareProfile, LevelPlan, TierModel, DownloadProgress, Skill, ConvDto } from './types'

type View = 'chat' | 'library' | 'settings' | 'skills' | 'memory'

/** Conversation ⇄ persisted DTO (markdown transcript). */
function conversationToDto(c: Conversation): ConvDto {
  return {
    id: c.id, title: c.title, createdAt: c.createdAt, systemPrompt: c.systemPrompt,
    messages: c.messages
      .filter((m) => (m.role === 'user' || m.role === 'assistant') && m.content.trim() !== '')
      .map((m) => ({ role: m.role, content: m.content, timestamp: m.timestamp })),
  };
}
function dtoToConversation(d: ConvDto, mkId: () => string): Conversation {
  return {
    id: d.id,
    title: d.title || 'Conversation',
    createdAt: d.createdAt || new Date().toISOString(),
    systemPrompt: d.systemPrompt,
    messages: d.messages.map((m) => ({
      id: mkId(), role: m.role as Message['role'], content: m.content,
      timestamp: m.timestamp || new Date().toISOString(),
    })),
  };
}
type Phase = 'profiling' | 'setup' | 'welcome' | 'ready'

function uid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

const INITIAL_CONVERSATIONS: Conversation[] = [
  { id: uid(), title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
];

export type AgentPermMode = 'plan' | 'ask' | 'auto' | 'bypass';

/** Compact one-line summary of a tool call's args for the chat. */
function summarizeArgs(args: Record<string, unknown> | undefined): string {
  if (!args) return '';
  const a = args as Record<string, unknown>;
  const pick = a.target ?? a.command ?? a.url ?? a.query ?? a.path ?? a.action ?? '';
  const s = typeof pick === 'string' ? pick : JSON.stringify(pick);
  return s ? `\`${s.length > 80 ? s.slice(0, 80) + '…' : s}\`` : '';
}

/** Best model to auto-select: prefer Smart, then Quick, then any ready one. */
function preferredTag(tiers: TierModel[]): string | null {
  const ready = tiers.filter((t) => t.status === 'ready');
  const pick =
    ready.find((t) => t.tier === 'smart') ??
    ready.find((t) => t.tier === 'quick') ??
    ready[0] ??
    tiers[0];
  return pick?.rec.ollama_pull ?? null;
}

export default function App() {
  type Platform = 'linux' | 'macos' | 'windows';
  const [platform, setPlatform] = useState<Platform>('linux');
  const [phase, setPhase] = useState<Phase>('profiling');
  const [welcomed, setWelcomed] = useState(() => {
    try { return localStorage.getItem('llamachat.welcomed') === '1'; } catch { return false; }
  });
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  const [activeId, setActiveId] = useState(() => {
    try { return localStorage.getItem('llamachat.activeId') || INITIAL_CONVERSATIONS[0].id; }
    catch { return INITIAL_CONVERSATIONS[0].id; }
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [view, setView] = useState<View>('chat');
  const [streaming, setStreaming] = useState(false);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [tiers, setTiers] = useState<TierModel[]>([]);
  const [selectedModel, setSelectedModel] = useState('llama3.2:3b');
  const [userPicked, setUserPicked] = useState(false);
  const [skills, setSkills] = useState<Skill[]>(() => loadSkills());
  const [agentMode, setAgentMode] = useState(false);
  const [agentPermMode, setAgentPermMode] = useState<AgentPermMode>('ask');
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown> } | null>(null);
  const chatRef = useRef<HTMLDivElement>(null);
  const setupStarted = useRef(false);
  const agentConvId = useRef('');

  useEffect(() => {
    const ua = navigator.platform || navigator.userAgent || '';
    if (ua.includes('Mac')) setPlatform('macos');
    else if (ua.includes('Win')) setPlatform('windows');
    else setPlatform('linux');
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute('data-platform', platform);
  }, [platform]);

  // Persist skills whenever they change.
  useEffect(() => { saveSkills(skills); }, [skills]);

  // Load saved conversations (markdown files) on startup.
  useEffect(() => {
    (async () => {
      const saved = await invoke<ConvDto[]>('list_conversations');
      if (saved && saved.length) {
        const convs = saved.map((d) => dtoToConversation(d, uid));
        setConversations(convs);
        // Restore the conversation the user was last on, if it still exists.
        let want = convs[0].id;
        try {
          const s = localStorage.getItem('llamachat.activeId');
          if (s && convs.some((c) => c.id === s)) want = s;
        } catch { /* ignore */ }
        setActiveId(want);
      }
    })();
  }, []);

  // Remember the active conversation across restarts.
  useEffect(() => {
    try { localStorage.setItem('llamachat.activeId', activeId); } catch { /* ignore */ }
  }, [activeId]);

  // Auto-save conversations to markdown (debounced; only non-empty ones).
  useEffect(() => {
    if (!isTauri()) return;
    const t = setTimeout(() => {
      for (const c of conversations) {
        if (c.messages.some((m) => m.content.trim())) {
          invoke('save_conversation', { conversation: conversationToDto(c) });
        }
      }
    }, 800);
    return () => clearTimeout(t);
  }, [conversations]);

  // ── First-run orchestration ──────────────────────────────
  useEffect(() => {
    if (setupStarted.current) return;
    setupStarted.current = true;

    (async () => {
      const hw = (await invoke<HardwareProfile>('get_hardware_profile')) ?? MOCK_HARDWARE;
      setHardware(hw);

      const plan = await invoke<LevelPlan>('get_benchmark_plan');
      let built = plan ? tiersFromPlan(plan) : [];
      if (built.length === 0) built = mockTiers();

      const installed = (await invoke<string[]>('list_installed_models')) ?? [];
      const installedSet = new Set(installed);
      built = built.map((t) =>
        installedSet.has(t.rec.ollama_pull) ? { ...t, status: 'ready' as const, pct: 100 } : t
      );

      setTiers(built);
      setPhase('setup');

      if (!isTauri()) {
        setTiers((prev) => prev.map((t) => ({ ...t, status: 'ready', pct: 100 })));
        return;
      }
      for (const t of built) {
        if (t.status !== 'ready') invoke('download_model', { tag: t.rec.ollama_pull });
      }
    })();
  }, []);

  // Live download progress → tier status.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<DownloadProgress>('download_progress', (p) => {
        setTiers((prev) =>
          prev.map((t): TierModel => {
            if (t.rec.ollama_pull !== p.tag) return t;
            if (p.status === 'done') return { ...t, status: 'ready', pct: 100, detail: p.detail };
            if (p.status === 'error') return { ...t, status: 'error', detail: p.detail };
            return { ...t, status: 'downloading', pct: p.pct ?? t.pct, detail: p.detail };
          })
        );
      });
    })();
    return () => unlisten?.();
  }, []);

  // Default model = Smart (auto-upgrades to Smart once ready) until the user picks.
  useEffect(() => {
    if (userPicked) return;
    const tag = preferredTag(tiers);
    if (tag) setSelectedModel(tag);
  }, [tiers, userPicked]);

  // When the Quick model is ready: first run → welcome steps, else → chat.
  useEffect(() => {
    if (phase !== 'setup') return;
    if (tiers[0]?.status !== 'ready') return;
    if (welcomed) { setPhase('ready'); return; }
    // Persist "seen onboarding" NOW, before showing it — so if the user quits
    // mid-onboarding (e.g. to grant Screen Recording, which requires an app
    // restart) it doesn't re-run the flow and re-append their memory. We still
    // show the welcome once this session (don't flip in-memory `welcomed`).
    try { localStorage.setItem('llamachat.welcomed', '1'); } catch { /* ignore */ }
    setPhase('welcome');
  }, [phase, tiers, welcomed]);

  // Agent-mode events → chat messages on the run's conversation.
  useEffect(() => {
    const uns: Array<(() => void) | null> = [];
    const add = (content: string) =>
      setConversations((prev) =>
        prev.map((c) =>
          c.id === (agentConvId.current || activeId)
            ? { ...c, messages: [...c.messages, { id: uid(), role: 'assistant' as const, content, timestamp: new Date().toISOString() }] }
            : c
        )
      );
    (async () => {
      uns.push(await listen<{ tool: string; args: Record<string, unknown> }>('agent_step', (p) => add(`🔧 **${p.tool}** ${summarizeArgs(p.args)}`)));
      uns.push(await listen<{ ok: boolean; text: string }>('agent_result', (p) => add((p.ok ? '' : '⚠️ ') + '```\n' + (p.text || '(done)') + '\n```')));
      uns.push(await listen<{ text: string }>('agent_answer', (p) => { if (p.text?.trim()) add(p.text); }));
      uns.push(await listen<{ text: string }>('agent_plan', (p) => add('**Plan**\n\n' + p.text)));
      uns.push(await listen<{ error: string }>('agent_error', (p) => add('⚠️ ' + p.error)));
      uns.push(await listen<{ tool: string; args: Record<string, unknown> }>('agent_approval', (p) => setPendingApproval(p)));
      uns.push(await listen('agent_done', () => { setStreaming(false); setPendingApproval(null); }));
    })();
    return () => uns.forEach((u) => u?.());
  }, []);

  const active = conversations.find((c) => c.id === activeId) ?? conversations[0];
  const commands = allCommands(skills);

  function pickModel(tag: string) {
    setUserPicked(true);
    setSelectedModel(tag);
  }

  const addMessage = useCallback((msg: Message, convId: string) => {
    setConversations((prev) =>
      prev.map((c) => (c.id === convId ? { ...c, messages: [...c.messages, msg] } : c))
    );
  }, []);

  /** Append an assistant "note" (help text, tool output, confirmations). */
  function addNote(content: string, convId: string = activeId) {
    addMessage({ id: uid(), role: 'assistant', content, timestamp: new Date().toISOString() }, convId);
  }

  // ── Chat ─────────────────────────────────────────────────
  function sendChat(userText: string, opts?: { system?: string }) {
    if (!userText.trim() || streaming) return;
    const convId = activeId;
    const conv = conversations.find((c) => c.id === convId) ?? active;

    // Full history (prior turns + this message) for conversation memory.
    const history = conv.messages
      .filter((m) => (m.role === 'user' || m.role === 'assistant') && m.content.trim() !== '')
      .map((m) => ({ role: m.role, content: m.content }));
    history.push({ role: 'user', content: userText });

    addMessage({ id: uid(), role: 'user', content: userText, timestamp: new Date().toISOString() }, convId);
    if (conv.messages.length === 0) {
      const title = userText.slice(0, 60) + (userText.length > 60 ? '…' : '');
      setConversations((prev) => prev.map((c) => (c.id === convId ? { ...c, title } : c)));
    }

    setStreaming(true);
    const assistantId = uid();
    addMessage({ id: assistantId, role: 'assistant', content: '', timestamp: new Date().toISOString() }, convId);

    streamResponse(history, assistantId, convId, opts?.system ?? conv.systemPrompt);
  }

  // ── Agent mode ───────────────────────────────────────────
  function runAgent(text: string) {
    if (!text.trim() || streaming) return;
    const convId = activeId;
    agentConvId.current = convId;
    const conv = conversations.find((c) => c.id === convId) ?? active;
    const history = conv.messages
      .filter((m) => (m.role === 'user' || m.role === 'assistant') && m.content.trim() !== '')
      .map((m) => ({ role: m.role, content: m.content }));
    history.push({ role: 'user', content: text });

    addMessage({ id: uid(), role: 'user', content: text, timestamp: new Date().toISOString() }, convId);
    if (conv.messages.length === 0) {
      setConversations((prev) => prev.map((c) => (c.id === convId ? { ...c, title: text.slice(0, 60) } : c)));
    }
    setStreaming(true);
    setView('chat');
    if (!isTauri()) { addMessage({ id: uid(), role: 'assistant', content: '_(Agent runs only in the desktop app.)_', timestamp: new Date().toISOString() }, convId); setStreaming(false); return; }
    invoke('run_agent', { messages: history, model: selectedModel, mode: agentPermMode });
  }

  function approveAgent(approved: boolean) {
    invoke('approve_agent', { approved });
    setPendingApproval(null);
  }
  function stopAgent() {
    invoke('stop_agent');
  }
  function cycleAgentMode() {
    const order: AgentPermMode[] = ['ask', 'auto', 'bypass', 'plan'];
    setAgentPermMode((m) => order[(order.indexOf(m) + 1) % order.length]);
  }

  async function streamResponse(
    messages: { role: string; content: string }[],
    msgId: string,
    convId: string,
    system?: string
  ) {
    const appendToken = (token: string) => {
      setConversations((prev) =>
        prev.map((c) =>
          c.id === convId
            ? { ...c, messages: c.messages.map((m) => (m.id === msgId ? { ...m, content: m.content + token } : m)) }
            : c
        )
      );
    };

    if (!isTauri()) {
      mockResponse(msgId, convId);
      setStreaming(false);
      return;
    }

    let unlistenToken: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    let timeout: ReturnType<typeof setTimeout> | null = null;
    const cleanup = () => {
      unlistenToken?.(); unlistenDone?.();
      unlistenToken = null; unlistenDone = null;
      if (timeout) clearTimeout(timeout);
      timeout = null;
    };

    try {
      unlistenToken = await listen<string>('chat_token', (token) => appendToken(token));
      unlistenDone = await listen<boolean>('chat_done', () => { cleanup(); setStreaming(false); });
      // Big models that spill to CPU load and generate slowly — give them a much
      // longer leash before timing out.
      const selRec = tiers.find((t) => t.rec.ollama_pull === selectedModel)?.rec;
      const timeoutMs = selRec?.memory_fit.offload ? 900000 : 180000;
      timeout = setTimeout(() => {
        cleanup();
        appendToken('\n\n⚠️ Timed out waiting for a response. The model may still be loading — try again.');
        setStreaming(false);
      }, timeoutMs);
      await invoke('send_message', { messages, model: selectedModel, system: system ?? null });
    } catch {
      cleanup();
      appendToken('\n\n⚠️ Could not start the model. Make sure Ollama is installed and running.');
      setStreaming(false);
    }
  }

  function mockResponse(msgId: string, convId: string) {
    const chunk = "I'm running locally. (Browser dev build — no Tauri backend, so this is a canned reply.)";
    setConversations((prev) =>
      prev.map((c) =>
        c.id === convId
          ? { ...c, messages: c.messages.map((m) => (m.id === msgId ? { ...m, content: chunk } : m)) }
          : c
      )
    );
  }

  // ── Slash commands ───────────────────────────────────────
  function handleCommand(name: string, args: string) {
    const skill = skills.find((s) => s.name === name);
    if (skill) {
      setView('chat');
      sendChat(args || skill.title, { system: skill.instructions });
      return;
    }
    switch (name) {
      case 'new': handleNewConversation(); setView('chat'); break;
      case 'clear': setConversations((prev) => prev.map((c) => (c.id === activeId ? { ...c, messages: [] } : c))); break;
      case 'help': showHelp(); break;
      case 'model': switchModelByTier(args); break;
      case 'models': setView('library'); break;
      case 'skills': setView('skills'); break;
      case 'memory': setView('memory'); break;
      case 'remember': rememberFact(args); break;
      case 'forget': forgetFact(args); break;
      case 'settings': setView('settings'); break;
      case 'copy': copyLast(); break;
      case 'retry': retryLast(); break;
      case 'system': setSystemPromptCmd(args); break;
      case 'shell': runTool('shell', { command: args }, `/shell ${args}`); break;
      case 'file': runFileTool(args); break;
      case 'browser': runBrowser(args); break;
      default: addNote(`Unknown command: \`/${name}\`. Type \`/help\` to see all commands.`);
    }
  }

  function showHelp() {
    const lines = commands.map((c) => `/${c.name}${c.argHint ? ' ' + c.argHint : ''}  —  ${c.description}`);
    addNote('Commands:\n```\n' + lines.join('\n') + '\n```');
    setView('chat');
  }

  function switchModelByTier(arg: string) {
    const a = arg.trim().toLowerCase();
    if (!a) {
      const lines = tiers.map((t) => `${t.label} · ${t.rec.display_name}${t.status === 'ready' ? '' : ` (${t.status})`}`);
      addNote('Models — use `/model quick|smart|best`:\n```\n' + lines.join('\n') + '\n```');
      setView('chat');
      return;
    }
    const t = tiers.find((x) => x.tier === a);
    if (!t) { addNote(`No \`${a}\` tier — use quick, smart, or best.`); return; }
    if (t.status !== 'ready') { addNote(`${t.label} isn't ready yet (${t.status}).`); return; }
    pickModel(t.rec.ollama_pull);
    addNote(`Switched to ${t.label} · ${t.rec.display_name}.`);
    setView('chat');
  }

  async function copyLast() {
    const conv = conversations.find((c) => c.id === activeId);
    const last = [...(conv?.messages ?? [])].reverse().find((m) => m.role === 'assistant' && m.content.trim());
    if (!last) { addNote('Nothing to copy yet.'); return; }
    try { await navigator.clipboard.writeText(last.content); addNote('Copied the last reply to your clipboard.'); }
    catch { addNote('Could not access the clipboard.'); }
  }

  function retryLast() {
    const conv = conversations.find((c) => c.id === activeId);
    if (!conv || streaming) return;
    const idx = conv.messages.map((m) => m.role).lastIndexOf('user');
    if (idx < 0) { addNote('Nothing to retry yet.'); return; }
    const kept = conv.messages.slice(0, idx + 1);
    const history = kept
      .filter((m) => (m.role === 'user' || m.role === 'assistant') && m.content.trim() !== '')
      .map((m) => ({ role: m.role, content: m.content }));
    const assistantId = uid();
    setConversations((prev) =>
      prev.map((c) =>
        c.id === activeId
          ? { ...c, messages: [...kept, { id: assistantId, role: 'assistant' as const, content: '', timestamp: new Date().toISOString() }] }
          : c
      )
    );
    setStreaming(true);
    setView('chat');
    streamResponse(history, assistantId, activeId, conv.systemPrompt);
  }

  function setSystemPromptCmd(args: string) {
    const p = args.trim();
    setConversations((prev) => prev.map((c) => (c.id === activeId ? { ...c, systemPrompt: p || undefined } : c)));
    addNote(p ? "Updated this chat's system prompt." : 'Cleared the custom system prompt.');
    setView('chat');
  }

  async function rememberFact(fact: string) {
    const f = fact.trim();
    if (!f) { addNote('Usage: `/remember <fact>`'); return; }
    const cur = (await invoke<string>('get_memory')) ?? '';
    const next = `${cur.trimEnd()}\n- ${f}\n`.replace(/^\n+/, '');
    await invoke('set_memory', { content: next });
    addNote(`🧠 Remembered: ${f}`);
    setView('chat');
  }

  async function forgetFact(fragment: string) {
    const q = fragment.trim().toLowerCase();
    if (!q) { addNote('Usage: `/forget <text>`'); return; }
    const cur = (await invoke<string>('get_memory')) ?? '';
    const before = cur.split('\n');
    const kept = before.filter((line) => !line.toLowerCase().includes(q));
    await invoke('set_memory', { content: kept.join('\n') });
    const removed = before.length - kept.length;
    addNote(removed > 0 ? `Forgot ${removed} line${removed === 1 ? '' : 's'} matching "${q}".` : `Nothing in memory matched "${q}".`);
    setView('chat');
  }

  // ── Tools ────────────────────────────────────────────────
  async function runTool(toolName: string, args: Record<string, unknown>, display: string) {
    setView('chat');
    const convId = activeId;
    addMessage({ id: uid(), role: 'user', content: display, timestamp: new Date().toISOString() }, convId);

    if (!isTauri()) { addNote('_(Tools run only in the desktop app.)_', convId); return; }

    // The user explicitly invoked a tool — grant consent for destructive tools.
    try {
      const needs = await invoke<boolean>('tool_needs_approval', { toolName });
      if (needs) {
        const granted = await invoke<boolean>('get_consent');
        if (!granted) {
          await invoke('set_consent', { granted: true });
          addNote('_Enabled tool execution (shell/file). You can disable it in Settings._', convId);
        }
      }
    } catch { /* best effort */ }

    const res = await invoke<{ ok: boolean; output?: string; error?: string }>('execute_tool', {
      request: { name: toolName, args },
    });
    if (!res) { addNote('⚠️ Tool call failed.', convId); return; }
    const body = res.ok ? (res.output || '_(no output)_') : `⚠️ ${res.error || 'failed'}`;
    addNote('```\n' + body + '\n```', convId);
  }

  function runFileTool(args: string) {
    const parts = args.trim().split(/\s+/);
    const action = (parts[0] || '').toLowerCase();
    if (action === 'read' && parts[1]) {
      runTool('file', { action: 'read', path: parts.slice(1).join(' ') }, `/file read ${parts.slice(1).join(' ')}`);
    } else if (action === 'write' && parts[1]) {
      const path = parts[1];
      const content = parts.slice(2).join(' ');
      runTool('file', { action: 'write', path, content }, `/file write ${path}`);
    } else {
      addNote('Usage: `/file read <path>` or `/file write <path> <text>`');
    }
  }

  function runBrowser(url: string) {
    const u = url.trim();
    if (!u) { addNote('Usage: `/browser <url>`'); return; }
    const safe = u.replace(/'/g, '');
    runTool('shell', { command: `open '${safe}'` }, `/browser ${u}`);
  }

  function handleNewConversation() {
    const id = uid();
    setConversations((prev) => [
      { id, title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
      ...prev,
    ]);
    setActiveId(id);
  }

  function handleDeleteConversation(id: string) {
    invoke('delete_conversation', { id });
    setConversations((prev) => {
      const next = prev.filter((c) => c.id !== id);
      if (activeId === id && next.length > 0) setActiveId(next[0].id);
      return next;
    });
  }

  if (phase === 'welcome') {
    return (
      <WelcomeSteps
        onFinish={() => {
          try { localStorage.setItem('llamachat.welcomed', '1'); } catch { /* ignore */ }
          setWelcomed(true);
          setPhase('ready');
        }}
      />
    );
  }
  if (phase === 'profiling' || phase === 'setup') {
    return (
      <SetupWizard
        phase={phase}
        hardware={hardware}
        tiers={tiers}
        onContinue={() => setPhase('ready')}
        onBrowseAll={() => { setPhase('ready'); setView('library'); }}
      />
    );
  }

  return (
    <div className="h-full flex bg-bg">
      <Sidebar
        open={sidebarOpen}
        conversations={conversations}
        activeId={activeId}
        hardware={hardware}
        onSelect={(id) => { setActiveId(id); setView('chat'); }}
        onNew={handleNewConversation}
        onDelete={handleDeleteConversation}
        onToggle={() => setSidebarOpen((o) => !o)}
      />

      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex-shrink-0 h-9 border-b border-border flex items-center px-3 gap-2">
          {!sidebarOpen && (
            <button onClick={() => setSidebarOpen(true)} className="text-text-secondary hover:text-text p-0.5" title="Toggle sidebar">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M2 3h12M2 8h12M2 13h12" stroke="currentColor" strokeWidth="1.5" />
              </svg>
            </button>
          )}
          {view === 'chat' ? (
            <>
              <span className="text-[11px] text-text-muted truncate">{active.title}</span>
              <span className="text-[10px] text-accent ml-auto">{selectedModel}</span>
              {hardware && (
                <span className="text-[10px] text-text-muted">
                  &middot; {hardware.cpu.model.split(' ').slice(0, 2).join(' ')}
                </span>
              )}
            </>
          ) : (
            <button onClick={() => setView('chat')} className="text-[11px] text-text-secondary hover:text-text flex items-center gap-1" title="Back to chat">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M10 3L5 8l5 5" stroke="currentColor" strokeWidth="1.5" />
              </svg>
              Back to chat
            </button>
          )}

          <div className={`flex items-center gap-1 ${view === 'chat' ? '' : 'ml-auto'}`}>
            <NavButton active={view === 'memory'} onClick={() => setView(view === 'memory' ? 'chat' : 'memory')} title="Memory">
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
                <path d="M8 2.2c-1.3 0-2.4.9-2.6 2.1C4.2 4.6 3.4 5.6 3.4 6.8c0 .5.1.9.4 1.3-.4.4-.6 1-.6 1.6 0 1.2.9 2.1 2.1 2.2.3.9 1.1 1.5 2.1 1.5s1.8-.6 2.1-1.5c1.2-.1 2.1-1 2.1-2.2 0-.6-.2-1.2-.6-1.6.3-.4.4-.8.4-1.3 0-1.2-.8-2.2-2-2.5C10.4 3.1 9.3 2.2 8 2.2z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
                <path d="M8 2.2v9.6" stroke="currentColor" strokeWidth="1.1" />
              </svg>
            </NavButton>
            <NavButton active={view === 'skills'} onClick={() => setView(view === 'skills' ? 'chat' : 'skills')} title="Skills">
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
                <path d="M8 1.5l1.6 3.9 3.9 1.6-3.9 1.6L8 12.5 6.4 8.6 2.5 7l3.9-1.6L8 1.5z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
              </svg>
            </NavButton>
            <NavButton active={view === 'library'} onClick={() => setView(view === 'library' ? 'chat' : 'library')} title="Model library">
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
                <path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                <circle cx="4.5" cy="4" r="1" fill="currentColor" />
                <circle cx="4.5" cy="8" r="1" fill="currentColor" />
                <circle cx="4.5" cy="12" r="1" fill="currentColor" />
              </svg>
            </NavButton>
            <NavButton active={view === 'settings'} onClick={() => setView(view === 'settings' ? 'chat' : 'settings')} title="Settings">
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
                <circle cx="8" cy="8" r="2.2" stroke="currentColor" strokeWidth="1.5" />
                <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
              </svg>
            </NavButton>
          </div>
        </div>

        {view === 'chat' && (
          <>
            <ChatArea ref={chatRef} messages={active.messages} streaming={streaming} />
            {pendingApproval && (
              <div className="flex-shrink-0 mx-3 mb-2 rounded-lg border border-warning/40 bg-warning/5 px-3 py-2 flex items-center gap-2">
                <span className="text-[12px] text-text">
                  Run <span className="font-mono text-warning">{pendingApproval.tool}</span> {summarizeArgs(pendingApproval.args)}?
                </span>
                <div className="ml-auto flex gap-1.5">
                  <button onClick={() => approveAgent(true)} className="px-2.5 py-1 text-[12px] rounded bg-accent text-white hover:opacity-90">Approve</button>
                  <button onClick={() => approveAgent(false)} className="px-2.5 py-1 text-[12px] rounded border border-border text-text-secondary hover:text-text">Deny</button>
                </div>
              </div>
            )}
            {streaming && agentMode && !pendingApproval && (
              <div className="flex-shrink-0 mx-3 mb-2 flex items-center gap-2 text-[11px] text-text-muted">
                <span className="w-3 h-3 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                Agent working…
                <button onClick={stopAgent} className="ml-auto px-2.5 py-1 text-[12px] rounded border border-border text-text-secondary hover:text-error">Stop</button>
              </div>
            )}
            <InputBar
              onSend={(t) => (agentMode ? runAgent(t) : sendChat(t))}
              onCommand={handleCommand}
              disabled={streaming}
              tiers={tiers}
              selectedModel={selectedModel}
              onSelectModel={pickModel}
              onBrowseAll={() => setView('library')}
              commands={commands}
              agentMode={agentMode}
              agentPermMode={agentPermMode}
              onToggleAgent={() => setAgentMode((a) => !a)}
              onCycleMode={cycleAgentMode}
            />
          </>
        )}
        {view === 'library' && (
          <ModelLibrary selectedModel={selectedModel} onUseModel={(tag) => { pickModel(tag); setView('chat'); }} />
        )}
        {view === 'settings' && <Settings hardware={hardware} />}
        {view === 'skills' && <SkillsTab skills={skills} onChange={setSkills} />}
        {view === 'memory' && <MemoryTab />}
      </div>
    </div>
  );
}

function NavButton({ active, onClick, title, children }: { active: boolean; onClick: () => void; title: string; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`p-1 rounded transition-colors ${active ? 'text-accent bg-accent-dim' : 'text-text-muted hover:text-text'}`}
    >
      {children}
    </button>
  );
}
