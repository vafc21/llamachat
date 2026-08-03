import { useState, useRef, useCallback, useEffect, useMemo } from 'react'
import { Sidebar, type NavView } from './components/Sidebar'
import { ChatArea } from './components/ChatArea'
import { InputBar } from './components/InputBar'
import { SetupWizard } from './components/SetupWizard'
import { ModelLibrary } from './components/ModelLibrary'
import { Settings } from './components/Settings'
import { loadPrefs, savePrefs, type UiPrefs } from './prefs'
import { SkillsTab } from './components/SkillsTab'
import { MemoryTab } from './components/MemoryTab'
import { WelcomeSteps } from './components/WelcomeSteps'
import { PersonaChoice } from './components/PersonaChoice'
import { CodeWorkspace } from './components/CodeWorkspace'
import { CoworkPane, type CoworkTask } from './components/CoworkPane'
import { StatusBar } from './components/StatusBar'
import { IconSprite, Icon } from './components/Icon'
import { TOOL_MARK } from './components/MessageBubble'
import { invoke, listen, isTauri } from './tauri'
import { MOCK_HARDWARE, tiersFromPlan, mockTiers } from './models'
import { loadSkills, saveSkills } from './skills'
import { allCommands } from './commands'
import {
  loadPersona, savePersona, loadMode, saveMode, modesFor, greeting,
  type Persona, type Mode,
} from './persona'
import { route, describeRoute } from './router'
import { estimateContext, estimateTokens, type TurnStats, type ActivityEntry } from './runtime'
import type { Message, Conversation, HardwareProfile, LevelPlan, TierModel, DownloadProgress, Skill, ConvDto } from './types'

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

/**
 * First-run order:
 *   persona  — the startup question (R14). Asked once, before anything else.
 *   profiling/setup — hardware detection + tier downloads (unchanged).
 *   welcome  — the optional memory/permissions steps (unchanged).
 *   ready    — the app.
 */
type Phase = 'persona' | 'profiling' | 'setup' | 'welcome' | 'ready'

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

/** Shown in a plain browser dev build, where there is no Tauri backend. */
const MOCK_REPLY =
  "I'm running locally. (Browser dev build — no Tauri backend is attached, so this is a canned reply " +
  'streamed word by word so the runtime counters have something real to measure.)';

/** Compact one-line summary of a tool call's args for the chat. */
function summarizeArgs(args: Record<string, unknown> | undefined): string {
  if (!args) return '';
  const a = args as Record<string, unknown>;
  const pick = a.target ?? a.command ?? a.url ?? a.query ?? a.path ?? a.action ?? '';
  const s = typeof pick === 'string' ? pick : JSON.stringify(pick);
  return s ? `\`${s.length > 80 ? s.slice(0, 80) + '…' : s}\`` : '';
}

/** Plain-text arg summary for the Code activity log (no markdown backticks). */
function plainArgs(args: Record<string, unknown> | undefined): string {
  return summarizeArgs(args).replace(/`/g, '');
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
  const storedPersona = useRef(loadPersona());
  const [persona, setPersona] = useState<Persona>(storedPersona.current ?? 'simple');
  const [mode, setMode] = useState<Mode>(loadMode());
  const [prefs, setPrefs] = useState<UiPrefs>(() => loadPrefs());
  const [phase, setPhase] = useState<Phase>(storedPersona.current ? 'profiling' : 'persona');
  const [welcomed, setWelcomed] = useState(() => {
    try { return localStorage.getItem('llamachat.welcomed') === '1'; } catch { return false; }
  });
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  const [activeId, setActiveId] = useState(() => {
    try { return localStorage.getItem('llamachat.activeId') || INITIAL_CONVERSATIONS[0].id; }
    catch { return INITIAL_CONVERSATIONS[0].id; }
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  /** Nav destination overlaying the mode pane, or null when in a mode. */
  const [nav, setNav] = useState<NavView | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [tiers, setTiers] = useState<TierModel[]>([]);
  const [selectedModel, setSelectedModel] = useState('llama3.2:3b');
  const [userPicked, setUserPicked] = useState(false);
  const [skills, setSkills] = useState<Skill[]>(() => loadSkills());
  const [agentPermMode, setAgentPermMode] = useState<AgentPermMode>('ask');
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown> } | null>(null);
  /** Measured per-reply runtime, keyed by assistant message id. */
  const [turnStats, setTurnStats] = useState<Record<string, TurnStats>>({});
  /** "Thought for N seconds", keyed by assistant message id (simple persona). */
  const [simpleStatus, setSimpleStatus] = useState<Record<string, string>>({});
  /** Real tool calls from the agent loop — the Code activity log. */
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  /** Cowork runs in flight. */
  const [tasks, setTasks] = useState<CoworkTask[]>([]);
  const chatRef = useRef<HTMLDivElement>(null);
  const setupStarted = useRef(false);
  const agentConvId = useRef('');
  const agentTaskId = useRef('');

  // Cowork and Code both drive the tool loop; Chat is a plain completion.
  const modes = modesFor(persona);

  useEffect(() => {
    const ua = navigator.platform || navigator.userAgent || '';
    if (ua.includes('Mac')) setPlatform('macos');
    else if (ua.includes('Win')) setPlatform('windows');
    else setPlatform('linux');
  }, []);

  // v6's architecture: persona/mode live on <html> and CSS does the gating.
  useEffect(() => {
    const el = document.documentElement;
    el.setAttribute('data-platform', platform);
    el.setAttribute('data-persona', persona);
    el.setAttribute('data-mode', mode);
  }, [platform, persona, mode]);

  useEffect(() => { savePersona(persona); }, [persona]);
  useEffect(() => { saveMode(mode); }, [mode]);
  useEffect(() => { savePrefs(prefs); }, [prefs]);

  // Code is developer-only (R2) — dropping to the simple persona leaves it.
  useEffect(() => {
    if (persona === 'simple' && mode === 'code') setMode('chat');
  }, [persona, mode]);

  // Persist skills whenever they change.
  useEffect(() => { saveSkills(skills); }, [skills]);

  // Load saved conversations (markdown files) on startup.
  useEffect(() => {
    (async () => {
      const saved = await invoke<ConvDto[]>('list_conversations');
      if (saved && saved.length) {
        const convs = saved.map((d) => dtoToConversation(d, uid));
        setConversations(convs);
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

  // A stored activeId can outlive the conversation it pointed at (deleted, or
  // a fresh browser session where nothing was restored). Left dangling, every
  // send silently no-ops because no conversation matches the id. Reconcile.
  useEffect(() => {
    if (conversations.length === 0) return;
    if (conversations.some((c) => c.id === activeId)) return;
    setActiveId(conversations[0].id);
  }, [conversations, activeId]);

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

  // Keep the thread pinned to the newest turn.
  useEffect(() => {
    const el = chatRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [conversations, activeId]);

  // ── First-run orchestration ──────────────────────────────
  useEffect(() => {
    if (phase === 'persona') return;
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
  }, [phase]);

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

  // When the Quick model is ready: first run → welcome steps, else → the app.
  useEffect(() => {
    if (phase !== 'setup') return;
    if (tiers[0]?.status !== 'ready') return;
    if (welcomed) { setPhase('ready'); return; }
    // Persist "seen onboarding" NOW, before showing it — so if the user quits
    // mid-onboarding (e.g. to grant Screen Recording, which requires an app
    // restart) it doesn't re-run the flow and re-append their memory.
    try { localStorage.setItem('llamachat.welcomed', '1'); } catch { /* ignore */ }
    setPhase('welcome');
  }, [phase, tiers, welcomed]);

  // ── Agent-mode events → chat messages + real activity log ─────────
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
    const touchTask = (detail: string, state?: CoworkTask['state']) =>
      setTasks((prev) => prev.map((t) =>
        t.id === agentTaskId.current
          ? { ...t, detail, updatedAt: Date.now(), state: state ?? t.state }
          : t));

    (async () => {
      uns.push(await listen<{ tool: string; args: Record<string, unknown> }>('agent_step', (p) => {
        add(`${TOOL_MARK}**${p.tool}** ${summarizeArgs(p.args)}`);
        setActivity((prev) => [...prev, {
          id: uid(), tool: p.tool, detail: plainArgs(p.args), state: 'running', startedAt: Date.now(),
        }]);
        touchTask(`Working · ${p.tool}`);
      }));
      uns.push(await listen<{ ok: boolean; text: string }>('agent_result', (p) => {
        add('```\n' + (p.text || '(done)') + '\n```');
        setActivity((prev) => {
          const i = prev.map((a) => a.state).lastIndexOf('running');
          if (i < 0) return prev;
          const next = [...prev];
          next[i] = { ...next[i], state: p.ok ? 'ok' : 'error', endedAt: Date.now() };
          return next;
        });
      }));
      uns.push(await listen<{ text: string }>('agent_answer', (p) => { if (p.text?.trim()) add(p.text); }));
      uns.push(await listen<{ text: string }>('agent_plan', (p) => add('**Plan**\n\n' + p.text)));
      uns.push(await listen<{ error: string }>('agent_error', (p) => {
        add('**Error** — ' + p.error);
        touchTask(p.error.slice(0, 80), 'error');
      }));
      uns.push(await listen<{ tool: string; args: Record<string, unknown> }>('agent_approval', (p) => {
        setPendingApproval(p);
        touchTask('Waiting for you · confirm the tool call', 'waiting');
      }));
      uns.push(await listen('agent_done', () => {
        setStreaming(false);
        setPendingApproval(null);
        touchTask('Finished', 'done');
      }));
    })();
    return () => uns.forEach((u) => u?.());
  }, [activeId]);

  const active = conversations.find((c) => c.id === activeId) ?? conversations[0];
  const commands = allCommands(skills);
  const currentRec = tiers.find((t) => t.rec.ollama_pull === selectedModel)?.rec ?? null;
  const ctxTotal = currentRec?.context_comfortable ?? 8192;
  const ctxUsed = useMemo(() => estimateContext(active?.messages ?? []), [active]);
  const readyModels = tiers.filter((t) => t.status === 'ready').length;
  const lastTps = useMemo(() => {
    const vals = Object.values(turnStats);
    const last = vals[vals.length - 1];
    return last && last.seconds > 0 ? last.tokensOut / last.seconds : null;
  }, [turnStats]);
  const greetLine = greeting('Vlad');

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

  /** Leave whatever nav overlay is open and go back to the current mode. */
  function toMode(m?: Mode) {
    setNav(null);
    if (m) setMode(m);
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

    // R18: the simple persona never picks a model — the router does, per
    // message. Developers keep their manual choice (R16) unless they haven't
    // touched the picker.
    const useRouter = persona === 'simple' ? prefs.autoRoute : !userPicked;
    const decision = useRouter ? route(userText, tiers, selectedModel) : null;
    const model = decision?.tag ?? selectedModel;

    setStreaming(true);
    const assistantId = uid();
    addMessage({ id: assistantId, role: 'assistant', content: '', timestamp: new Date().toISOString() }, convId);

    if (decision && persona === 'dev' && prefs.explainRouter) {
      setTurnStats((prev) => ({
        ...prev,
        [assistantId]: {
          model, quant: tiers.find((t) => t.rec.ollama_pull === model)?.rec.quant,
          tokensOut: 0, seconds: 0, ctxUsed, ctxTotal, router: describeRoute(decision, tiers),
        },
      }));
    }

    streamResponse(history, assistantId, convId, opts?.system ?? conv.systemPrompt, model);
  }

  // ── Agent mode (Cowork + Code) ───────────────────────────
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

    // Cowork surfaces the run in its Active list.
    const taskId = uid();
    agentTaskId.current = taskId;
    setTasks((prev) => [{
      id: taskId, title: text.slice(0, 80), detail: 'Starting…',
      state: 'working', startedAt: Date.now(), updatedAt: Date.now(),
    }, ...prev]);

    setStreaming(true);
    setNav(null);
    if (!isTauri()) {
      addNote('_(The tool loop runs only in the desktop app.)_', convId);
      setStreaming(false);
      setTasks((prev) => prev.map((t) => t.id === taskId ? { ...t, state: 'error', detail: 'Desktop app only' } : t));
      return;
    }
    invoke('run_agent', { messages: history, model: selectedModel, mode: agentPermMode });
  }

  function approveAgent(approved: boolean) {
    invoke('approve_agent', { approved });
    setPendingApproval(null);
  }
  function stopAgent() {
    invoke('stop_agent');
    setStreaming(false);
  }
  function cycleAgentMode() {
    const order: AgentPermMode[] = ['ask', 'auto', 'bypass', 'plan'];
    setAgentPermMode((m) => order[(order.indexOf(m) + 1) % order.length]);
  }

  async function streamResponse(
    messages: { role: string; content: string }[],
    msgId: string,
    convId: string,
    system: string | undefined,
    model: string
  ) {
    // Measured, not estimated: one `chat_token` event is one token.
    const startedAt = Date.now();
    let tokens = 0;
    let chars = 0;

    const appendToken = (token: string) => {
      tokens += 1;
      chars += token.length;
      setConversations((prev) =>
        prev.map((c) =>
          c.id === convId
            ? { ...c, messages: c.messages.map((m) => (m.id === msgId ? { ...m, content: m.content + token } : m)) }
            : c
        )
      );
    };

    const finish = () => {
      const seconds = (Date.now() - startedAt) / 1000;
      setTurnStats((prev) => ({
        ...prev,
        [msgId]: {
          ...prev[msgId],
          model,
          quant: tiers.find((t) => t.rec.ollama_pull === model)?.rec.quant,
          tokensOut: tokens,
          seconds,
          ctxUsed: ctxUsed + estimateTokens(String(chars)) + tokens,
          ctxTotal,
          router: prev[msgId]?.router,
        },
      }));
      // R17/R18: the simple persona sees only the outcome, in plain words.
      const secs = Math.max(1, Math.round(seconds));
      setSimpleStatus((prev) => ({ ...prev, [msgId]: `Thought for ${secs} second${secs === 1 ? '' : 's'}` }));
    };

    if (!isTauri()) {
      // Stream the canned reply word by word so the dev build's counters are
      // measured the same way the real ones are, rather than reporting zero.
      const words = MOCK_REPLY.split(' ');
      let i = 0;
      const tick = setInterval(() => {
        appendToken((i === 0 ? '' : ' ') + words[i]);
        if (++i >= words.length) {
          clearInterval(tick);
          finish();
          setStreaming(false);
        }
      }, 18);
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
      unlistenDone = await listen<boolean>('chat_done', () => { cleanup(); finish(); setStreaming(false); });
      // Big models that spill to CPU load and generate slowly — give them a much
      // longer leash before timing out.
      const selRec = tiers.find((t) => t.rec.ollama_pull === model)?.rec;
      const timeoutMs = selRec?.memory_fit.offload ? 900000 : 180000;
      timeout = setTimeout(() => {
        cleanup();
        appendToken('\n\n**Timed out** waiting for a response. The model may still be loading — try again.');
        finish();
        setStreaming(false);
      }, timeoutMs);
      await invoke('send_message', { messages, model, system: system ?? null });
    } catch {
      cleanup();
      appendToken('\n\n**Could not start the model.** Make sure Ollama is installed and running.');
      finish();
      setStreaming(false);
    }
  }

  // ── Slash commands ───────────────────────────────────────
  function handleCommand(name: string, args: string) {
    const skill = skills.find((s) => s.name === name);
    if (skill) {
      toMode();
      sendChat(args || skill.title, { system: skill.instructions });
      return;
    }
    switch (name) {
      case 'new': handleNewConversation(); toMode(); break;
      case 'clear': setConversations((prev) => prev.map((c) => (c.id === activeId ? { ...c, messages: [] } : c))); break;
      case 'help': showHelp(); break;
      case 'model': switchModelByTier(args); break;
      // R17: no model library for the simple persona, by any route.
      case 'models':
        if (persona === 'dev') setNav('library');
        else addNote('LlamaChat manages models for you. Switch to the developer setup in Settings to see the library.');
        break;
      case 'skills': setNav('skills'); break;
      case 'memory': setNav('memory'); break;
      case 'remember': rememberFact(args); break;
      case 'forget': forgetFact(args); break;
      case 'settings': setNav('settings'); break;
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
    toMode();
  }

  function switchModelByTier(arg: string) {
    // R17: the simple persona has no concept of a model to switch to.
    if (persona === 'simple') {
      addNote('LlamaChat picks the model for you. Switch to the developer setup in Settings to choose by hand.');
      toMode();
      return;
    }
    const a = arg.trim().toLowerCase();
    if (!a) {
      const lines = tiers.map((t) => `${t.label} · ${t.rec.display_name}${t.status === 'ready' ? '' : ` (${t.status})`}`);
      addNote('Models — use `/model quick|smart|best`:\n```\n' + lines.join('\n') + '\n```');
      toMode();
      return;
    }
    const t = tiers.find((x) => x.tier === a);
    if (!t) { addNote(`No \`${a}\` tier — use quick, smart, or best.`); return; }
    if (t.status !== 'ready') { addNote(`${t.label} isn't ready yet (${t.status}).`); return; }
    pickModel(t.rec.ollama_pull);
    addNote(`Switched to ${t.label} · ${t.rec.display_name}.`);
    toMode();
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
    toMode();
    streamResponse(history, assistantId, activeId, conv.systemPrompt, selectedModel);
  }

  function setSystemPromptCmd(args: string) {
    const p = args.trim();
    setConversations((prev) => prev.map((c) => (c.id === activeId ? { ...c, systemPrompt: p || undefined } : c)));
    addNote(p ? "Updated this chat's system prompt." : 'Cleared the custom system prompt.');
    toMode();
  }

  async function rememberFact(fact: string) {
    const f = fact.trim();
    if (!f) { addNote('Usage: `/remember <fact>`'); return; }
    const cur = (await invoke<string>('get_memory')) ?? '';
    const next = `${cur.trimEnd()}\n- ${f}\n`.replace(/^\n+/, '');
    await invoke('set_memory', { content: next });
    addNote(`Remembered: ${f}`);
    toMode();
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
    toMode();
  }

  // ── Tools ────────────────────────────────────────────────
  async function runTool(toolName: string, args: Record<string, unknown>, display: string) {
    toMode();
    const convId = activeId;
    addMessage({ id: uid(), role: 'user', content: display, timestamp: new Date().toISOString() }, convId);

    if (!isTauri()) { addNote('_(Tools run only in the desktop app.)_', convId); return; }

    const entryId = uid();
    setActivity((prev) => [...prev, {
      id: entryId, tool: toolName, detail: plainArgs(args), state: 'running', startedAt: Date.now(),
    }]);

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
    setActivity((prev) => prev.map((a) =>
      a.id === entryId ? { ...a, state: res?.ok ? 'ok' : 'error', endedAt: Date.now() } : a));
    if (!res) { addNote('**Tool call failed.**', convId); return; }
    const body = res.ok ? (res.output || '_(no output)_') : `Error: ${res.error || 'failed'}`;
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
    setNav(null);
  }

  function handleDeleteConversation(id: string) {
    invoke('delete_conversation', { id });
    setConversations((prev) => {
      const next = prev.filter((c) => c.id !== id);
      if (activeId === id && next.length > 0) setActiveId(next[0].id);
      return next;
    });
  }

  // ── First-run screens ────────────────────────────────────
  if (phase === 'persona') {
    return (
      <>
        <IconSprite />
        <div className="lc-shell">
          <PersonaChoice
            onPick={(p) => {
              setPersona(p);
              savePersona(p);
              setPhase('profiling');
            }}
          />
        </div>
      </>
    );
  }
  if (phase === 'welcome') {
    return (
      <>
        <IconSprite />
        <WelcomeSteps
          onFinish={() => {
            try { localStorage.setItem('llamachat.welcomed', '1'); } catch { /* ignore */ }
            setWelcomed(true);
            setPhase('ready');
          }}
        />
      </>
    );
  }
  if (phase === 'profiling' || phase === 'setup') {
    return (
      <>
        <IconSprite />
        <SetupWizard
          phase={phase}
          hardware={hardware}
          tiers={tiers}
          persona={persona}
          onContinue={() => setPhase('ready')}
          onBrowseAll={() => { setPhase('ready'); setNav('library'); }}
        />
      </>
    );
  }

  // ── The app ──────────────────────────────────────────────
  const composerCommon = {
    onCommand: handleCommand,
    disabled: streaming,
    commands,
    persona,
    mode,
    modes,
    onMode: (m: Mode) => toMode(m),
    tiers,
    selectedModel,
    onSelectModel: pickModel,
    onBrowseAll: () => setNav('library'),
    ctxUsed,
    ctxTotal,
    onStop: streaming ? stopAgent : undefined,
  };

  const hasThread = active.messages.length > 0;

  return (
    <>
      <IconSprite />
      <div className="lc-shell">
        {/* Top bar — the chromeless strip from the reference shots. */}
        <div className="tb">
          <button type="button" title="Toggle sidebar" onClick={() => setSidebarOpen((o) => !o)}>
            <Icon name="panel" />
          </button>
          <button type="button" title="New conversation" onClick={handleNewConversation}>
            <Icon name="plus" />
          </button>
          {nav !== null && (
            <button type="button" title="Back" onClick={() => setNav(null)}>
              <Icon name="back" />
            </button>
          )}
          <span className="title">{nav === null ? active.title : NAV_TITLE[nav]}</span>
          <div className="sp" />
        </div>

        <div className="lc-body">
          <Sidebar
            open={sidebarOpen}
            mode={mode}
            conversations={conversations}
            activeId={activeId}
            nav={nav}
            readyModels={readyModels}
            onSelect={(id) => { setActiveId(id); setNav(null); }}
            onNew={handleNewConversation}
            onDelete={handleDeleteConversation}
            onNav={(v) => setNav((cur) => (cur === v ? null : v))}
          />

          <main className="main">
            {/* Nav overlays. These are destinations, not modes (R1). */}
            {nav === 'library' && (
              <ModelLibrary selectedModel={selectedModel} onUseModel={(tag) => { pickModel(tag); setNav(null); }} />
            )}
            {nav === 'settings' && (
              <Settings
                hardware={hardware}
                persona={persona}
                onPersona={setPersona}
                prefs={prefs}
                onPrefs={setPrefs}
                tiers={tiers}
              />
            )}
            {nav === 'skills' && <SkillsTab skills={skills} onChange={setSkills} />}
            {nav === 'memory' && <MemoryTab />}

            {/* ── Chat ─────────────────────────────────────── */}
            {nav === null && mode === 'chat' && (
              <div className="pane">
                {!hasThread ? (
                  <div className="center">
                    {/* R11 — the greeting Vlad liked. */}
                    <div className="greet"><Icon name="llama" />{greetLine}</div>
                    <InputBar {...composerCommon} variant="centered" onSend={sendChat} />
                    <div className="hint simple-only">
                      Runs entirely on your computer. Nothing leaves this machine.
                    </div>
                    <div className="hint dev-only">
                      {currentRec
                        ? `${currentRec.ollama_pull} · ${currentRec.quant} · ${readyModels} model${readyModels === 1 ? '' : 's'} ready`
                        : 'No model loaded yet.'}
                    </div>
                  </div>
                ) : (
                  <div className="pane">
                    <ChatArea
                      ref={chatRef}
                      messages={active.messages}
                      streaming={streaming}
                      persona={persona}
                      stats={turnStats}
                      simpleStatus={simpleStatus}
                    />
                    <div className="dockchat">
                      <InputBar {...composerCommon} variant="docked" onSend={sendChat} />
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* ── Cowork ───────────────────────────────────── */}
            {nav === null && mode === 'cowork' && (
              <div className="pane">
                {hasThread ? (
                  <>
                    <ChatArea
                      ref={chatRef}
                      messages={active.messages}
                      streaming={streaming}
                      persona={persona}
                      stats={turnStats}
                      simpleStatus={simpleStatus}
                    />
                    {pendingApproval && <ApprovalRow pending={pendingApproval} onAnswer={approveAgent} />}
                    <div className="dockchat">
                      <InputBar
                        {...composerCommon}
                        variant="docked"
                        onSend={runAgent}
                        secondRow={<CoworkRow permMode={agentPermMode} onCycle={cycleAgentMode} />}
                      />
                    </div>
                  </>
                ) : (
                  <CoworkPane
                    greeting={greetLine}
                    composer={
                      <InputBar
                        {...composerCommon}
                        variant="centered"
                        onSend={runAgent}
                        secondRow={<CoworkRow permMode={agentPermMode} onCycle={cycleAgentMode} />}
                      />
                    }
                    tasks={tasks}
                    onClear={() => setTasks([])}
                    onOpen={() => setNav(null)}
                  />
                )}
              </div>
            )}

            {/* ── Code (developer only) ────────────────────── */}
            {nav === null && mode === 'code' && persona === 'dev' && (
              <div className="pane">
                <CodeWorkspace
                  greeting={greetLine}
                  hardware={hardware}
                  tiers={tiers}
                  selectedModel={selectedModel}
                  onUseModel={pickModel}
                  onManageLibrary={() => setNav('library')}
                  activity={activity}
                  ctxUsed={ctxUsed}
                  ctxTotal={ctxTotal}
                />
                <div className="dock">
                  {pendingApproval && <ApprovalRow pending={pendingApproval} onAnswer={approveAgent} />}
                  <div className="dctx">
                    <span className="chip"><Icon name="host" /> local</span>
                    <span className="chip"><Icon name="folder" /> {hardware?.storage.models_dir?.split('/').pop() || 'workspace'}</span>
                    <span className="chip"><Icon name="term" /> {hardware?.os.name ?? 'local'}</span>
                  </div>
                  <InputBar
                    {...composerCommon}
                    variant="code"
                    onSend={runAgent}
                    agentPermMode={agentPermMode}
                    onCyclePermMode={cycleAgentMode}
                  />
                </div>
              </div>
            )}
          </main>
        </div>

        {/* R16/R19 — the developer status bar. `.dev-only` hides it for simple. */}
        {prefs.statusBar && (
          <StatusBar
            hardware={hardware}
            tiers={tiers}
            selectedModel={selectedModel}
            tokensPerSec={lastTps}
            ctxUsed={ctxUsed}
            ctxTotal={ctxTotal}
          />
        )}
      </div>
    </>
  );
}

const NAV_TITLE: Record<NavView, string> = {
  library: 'Model library',
  skills: 'Skills',
  memory: 'Memory',
  settings: 'Settings',
};

/** Cowork's second composer row: scope, tools, and the permission control. */
function CoworkRow({ permMode, onCycle }: { permMode: AgentPermMode; onCycle: () => void }) {
  const label: Record<AgentPermMode, string> = {
    plan: 'Plan only',
    ask: 'Ask before changes',
    auto: 'Auto-approve safe',
    bypass: 'No prompts',
  };
  return (
    <div className="crow2">
      {/* TODO(scope): there is no per-run working-directory concept in the
          backend yet — `run_agent` takes no scope argument — so this reports
          the whole machine rather than pretending a folder is selected. */}
      <span className="chip"><Icon name="folder" /> This computer</span>
      <span className="chip"><Icon name="tool" /> Shell · Files · Browser</span>
      <div className="sp" style={{ flex: 1 }} />
      <button type="button" className="chip" onClick={onCycle} title="Cycle permission mode">
        {label[permMode]} <Icon name="chev" size={12} />
      </button>
    </div>
  );
}

function ApprovalRow({
  pending, onAnswer,
}: {
  pending: { tool: string; args: Record<string, unknown> };
  onAnswer: (ok: boolean) => void;
}) {
  return (
    <div className="crow2" style={{ marginTop: 0, borderTop: 'none', marginBottom: 8 }}>
      <span className="chip warn">
        <Icon name="tool" /> Run {pending.tool} {plainArgs(pending.args)}?
      </span>
      <div className="sp" style={{ flex: 1 }} />
      <button type="button" className="chip ok" onClick={() => onAnswer(true)}>
        <Icon name="check" /> Approve
      </button>
      <button type="button" className="chip" onClick={() => onAnswer(false)}>
        <Icon name="x" /> Deny
      </button>
    </div>
  );
}
