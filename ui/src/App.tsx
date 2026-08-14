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
import { ReadinessStep } from './components/Readiness'
import { CodeWorkspace } from './components/CodeWorkspace'
import { CoworkPane, type CoworkTask } from './components/CoworkPane'
import { StatusBar } from './components/StatusBar'
import { PermMenu } from './components/PermMenu'
import { asPermMode, type AgentPermMode } from './perm'
import { IconSprite, Icon } from './components/Icon'
import { TOOL_MARK } from './components/MessageBubble'
import { invoke, listen, isTauri } from './tauri'
import { MOCK_HARDWARE, tiersFromPlan, mockTiers } from './models'
import { loadSkills, saveSkills } from './skills'
import { allCommands } from './commands'
import {
  loadPersona, savePersona, loadMode, saveMode, modesFor, greeting, PERSONA_KEY,
  type Persona, type Mode,
} from './persona'
import { route, describeRoute } from './router'
import { detectPlatform, hasSystemPermissions, type Platform } from './platform'
import { usePermissions } from './permissions'
import { estimateContext, type TurnStats, type ActivityEntry } from './runtime'
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
 *   persona   — the startup question (R14). Asked once, before anything else.
 *   readiness — can this machine actually run the thing? Placed BEFORE the
 *               downloads on purpose: Ollama being down is what makes the next
 *               screen sit at 0% forever, so it is worth catching first.
 *               Auto-skipped when there is nothing to report or act on.
 *   profiling/setup — hardware detection + tier downloads.
 *   welcome   — the optional memory-transfer step.
 *   ready     — the app.
 */
type Phase = 'persona' | 'readiness' | 'profiling' | 'setup' | 'welcome' | 'ready'

/** Set when the readiness step has been passed, so a restart mid-grant returns to it. */
const READY_SEEN_KEY = 'llamachat.readinessSeen';

function readinessSeen(): boolean {
  try { return localStorage.getItem(READY_SEEN_KEY) === '1'; } catch { return false; }
}
function markReadinessSeen() {
  try { localStorage.setItem(READY_SEEN_KEY, '1'); } catch { /* ignore */ }
}

function uid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

const INITIAL_CONVERSATIONS: Conversation[] = [
  { id: uid(), title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
];



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
  const [platform, setPlatform] = useState<Platform>(() => detectPlatform());
  const storedPersona = useRef(loadPersona());
  const [persona, setPersona] = useState<Persona>(storedPersona.current ?? 'simple');
  const [mode, setMode] = useState<Mode>(loadMode());
  const [prefs, setPrefs] = useState<UiPrefs>(() => loadPrefs());
  const [phase, setPhase] = useState<Phase>(
    // Quitting during onboarding must not silently skip a step. Persona is
    // stored the moment it is answered, so the readiness flag is what decides
    // whether the rest of the first run still has to happen — which matters a
    // lot here, because granting Screen Recording restarts the app mid-flow.
    !storedPersona.current ? 'persona' : readinessSeen() ? 'profiling' : 'readiness'
  );
  const [welcomed, setWelcomed] = useState(() => {
    try { return localStorage.getItem('llamachat.welcomed') === '1'; } catch { return false; }
  });
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  /** Live mirror of `conversations` for async handlers (see `finish()` below). */
  const conversationsRef = useRef(conversations);
  const [activeId, setActiveId] = useState(() => {
    try { return localStorage.getItem('llamachat.activeId') || INITIAL_CONVERSATIONS[0].id; }
    catch { return INITIAL_CONVERSATIONS[0].id; }
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  /** Nav destination overlaying the mode pane, or null when in a mode. */
  const [nav, setNav] = useState<NavView | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  /** Folder the agent is scoped to, or null for the whole machine. */
  const [workspaceDir, setWorkspaceDir] = useState<string | null>(null);
  const [tiers, setTiers] = useState<TierModel[]>([]);
  const [selectedModel, setSelectedModel] = useState('llama3.2:3b');
  const [userPicked, setUserPicked] = useState(false);
  const [skills, setSkills] = useState<Skill[]>(() => loadSkills());
  // Mirrors the backend's own default (`state.rs`: PermMode::Manual). The
  // backend is the source of truth; `syncPermMode` pushes any change to it.
  const [agentPermMode, setAgentPermMode] = useState<AgentPermMode>('manual');
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

  useEffect(() => { setPlatform(detectPlatform()); }, []);

  // Readiness is polled only while its step is on screen, so it can be skipped
  // when there is genuinely nothing to say. A one-row all-green checklist is
  // noise, not reassurance.
  const readiness = usePermissions(phase === 'readiness');
  useEffect(() => {
    if (phase !== 'readiness') return;
    if (!isTauri()) return; // browser dev build: nothing is verifiable, so show it
    if (!readiness.polled || !readiness.perms) return;
    const p = readiness.perms;
    const macGates = hasSystemPermissions(platform);
    const clean =
      p.ollama &&
      (!macGates || (p.accessibility && (persona === 'simple' || p.screen_recording)));
    if (clean) { markReadinessSeen(); setPhase('profiling'); }
  }, [phase, readiness.polled, readiness.perms, platform, persona]);

  // v6's architecture: persona/mode live on <html> and CSS does the gating.
  useEffect(() => {
    const el = document.documentElement;
    el.setAttribute('data-platform', platform);
    el.setAttribute('data-persona', persona);
    el.setAttribute('data-mode', mode);
  }, [platform, persona, mode]);

  // NOTE: persona is deliberately NOT persisted by an effect. An effect keyed on
  // `persona` fires once on mount with the default ('simple'), which writes the
  // key before the user has answered anything — so the very next launch or page
  // reload sees a stored persona and skips the startup question entirely,
  // silently locking the user into the simple side. Persona is only ever written
  // by `choosePersona`, i.e. by an actual human click.
  useEffect(() => { saveMode(mode); }, [mode]);
  useEffect(() => { savePrefs(prefs); }, [prefs]);

  /** The only path that writes the persona: an explicit answer (startup or Settings). */
  const choosePersona = useCallback((p: Persona) => {
    setPersona(p);
    savePersona(p);
    storedPersona.current = p;
  }, []);

  /**
   * Back to the first screen. Only the onboarding flags are cleared — chats,
   * memory, skills, models and prefs are left alone, which is what "show the
   * setup again" has to mean if it is going to be safe to press.
   */
  const replayOnboarding = useCallback(() => {
    try {
      localStorage.removeItem(PERSONA_KEY);
      localStorage.removeItem(READY_SEEN_KEY);
      localStorage.removeItem('llamachat.welcomed');
    } catch { /* ignore */ }
    storedPersona.current = null;
    setWelcomed(false);
    setNav(null);
    setupStarted.current = false;
    setPhase('persona');
  }, []);

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
    if (phase === 'persona' || phase === 'readiness') return;
    if (setupStarted.current) return;
    setupStarted.current = true;

    (async () => {
      // The profiling screen has no controls on it at all — it is a pure
      // spinner. If anything in here throws, the user is stranded on it with
      // no button, no message and no way out, which is exactly the kind of
      // dead end onboarding cannot afford. Fall through to mock tiers instead:
      // a wrong-but-usable model list still lets them reach the app.
      let built: TierModel[] = [];
      try {
        const hw = (await invoke<HardwareProfile>('get_hardware_profile')) ?? MOCK_HARDWARE;
        setHardware(hw);

        const plan = await invoke<LevelPlan>('get_benchmark_plan');
        built = plan ? tiersFromPlan(plan) : [];
        if (built.length === 0) built = mockTiers();

        const installed = (await invoke<string[]>('list_installed_models')) ?? [];
        const installedSet = new Set(installed);
        built = built.map((t) =>
          installedSet.has(t.rec.ollama_pull) ? { ...t, status: 'ready' as const, pct: 100 } : t
        );
      } catch (e) {
        console.error('hardware profiling failed:', e);
        setHardware((h) => h ?? MOCK_HARDWARE);
        if (built.length === 0) built = mockTiers();
      }

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

  /**
   * Leave the setup screen. Shared by "Quick is ready", "Continue anyway" and
   * "Browse all models" so all three land in the same place — previously the
   * two manual exits jumped straight to the app and the welcome step then
   * ambushed the user on their *second* launch instead.
   */
  const leaveSetup = useCallback((then?: () => void) => {
    then?.();
    if (welcomed) { setPhase('ready'); return; }
    // Persist "seen onboarding" NOW, before showing it — so if the user quits
    // mid-onboarding it doesn't re-run the flow and re-append their memory.
    try { localStorage.setItem('llamachat.welcomed', '1'); } catch { /* ignore */ }
    setPhase('welcome');
  }, [welcomed]);

  // When the Quick model is ready: first run → welcome steps, else → the app.
  useEffect(() => {
    if (phase !== 'setup') return;
    if (tiers[0]?.status !== 'ready') return;
    leaveSetup();
  }, [phase, tiers, leaveSetup]);

  // ── Agent-mode events → chat messages + real activity log ─────────
  //
  // `listen()` is async, so a re-run of this effect could tear down before the
  // previous registrations resolved: the cleanup saw a still-empty array, and
  // every listener that landed afterwards stayed subscribed forever. Switching
  // conversation stacked a second, third… copy of every handler, so one
  // `agent_step` was written to the transcript once per leaked listener
  // (observed on a real run: a single shell call logged three times).
  //
  // `cancelled` closes that window — anything resolving late unsubscribes itself.
  useEffect(() => {
    let cancelled = false;
    const uns: Array<(() => void) | null> = [];
    const track = (u: (() => void) | null) => {
      if (cancelled) u?.();
      else uns.push(u);
    };
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
      track(await listen<{ tool: string; args: Record<string, unknown> }>('agent_step', (p) => {
        add(`${TOOL_MARK}**${p.tool}** ${summarizeArgs(p.args)}`);
        setActivity((prev) => [...prev, {
          id: uid(), tool: p.tool, detail: plainArgs(p.args), state: 'running', startedAt: Date.now(),
        }]);
        touchTask(`Working · ${p.tool}`);
      }));
      track(await listen<{ ok: boolean; text: string }>('agent_result', (p) => {
        add('```\n' + (p.text || '(done)') + '\n```');
        setActivity((prev) => {
          const i = prev.map((a) => a.state).lastIndexOf('running');
          if (i < 0) return prev;
          const next = [...prev];
          next[i] = { ...next[i], state: p.ok ? 'ok' : 'error', endedAt: Date.now() };
          return next;
        });
      }));
      track(await listen<{ text: string }>('agent_answer', (p) => { if (p.text?.trim()) add(p.text); }));
      track(await listen<{ text: string }>('agent_plan', (p) => add('**Plan**\n\n' + p.text)));
      track(await listen<{ error: string }>('agent_error', (p) => {
        add('**Error** — ' + p.error);
        touchTask(p.error.slice(0, 80), 'error');
      }));
      track(await listen<{ tool: string; args: Record<string, unknown> }>('agent_approval', (p) => {
        setPendingApproval(p);
        touchTask('Waiting for you · confirm the tool call', 'waiting');
      }));
      track(await listen('agent_done', () => {
        setStreaming(false);
        setPendingApproval(null);
        touchTask('Finished', 'done');
      }));
    })();
    return () => {
      cancelled = true;
      uns.forEach((u) => u?.());
    };
  }, [activeId]);

  conversationsRef.current = conversations;

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
    // NOTE: no `mode` argument — Rust's `run_agent` takes only (messages,
    // model) and reads the permission mode from its own state. Passing one
    // here looked like it worked and did nothing; use `set_agent_mode`.
    invoke('run_agent', { messages: history, model: selectedModel });
  }

  function approveAgent(approved: boolean) {
    invoke('approve_agent', { approved });
    setPendingApproval(null);
  }
  function stopAgent() {
    invoke('stop_agent');
    setStreaming(false);
  }
  /**
   * Push the permission mode to the backend.
   *
   * This used to be local React state only — nothing ever called
   * `set_agent_mode`, so the backend stayed on its Manual default no matter
   * what the UI showed. `run_agent` reads the mode from Rust state, not from
   * its arguments, so the picker was purely decorative.
   */
  const syncPermMode = useCallback(async (m: AgentPermMode) => {
    setAgentPermMode(m);
    if (!isTauri()) return;
    try {
      await invoke('set_agent_mode', { mode: m });
    } catch (e) {
      // Don't leave the UI claiming a mode the agent isn't in.
      console.error('set_agent_mode failed:', e);
      try {
        const cur = await invoke<{ mode: string }>('get_agent_mode');
        const back = asPermMode(cur?.mode);
        if (back) setAgentPermMode(back);
      } catch { /* ignore */ }
    }
  }, []);

  // Adopt the backend's persisted folder scope on startup, so the composer
  // chips tell the truth before Settings has ever been opened.
  useEffect(() => {
    if (!isTauri()) return;
    (async () => {
      try {
        const s = await invoke<{ workspace_dir: string | null }>('get_settings');
        setWorkspaceDir(s?.workspace_dir ?? null);
      } catch { /* backend not ready; assume unscoped */ }
    })();
  }, []);

  // Adopt the backend's actual mode on startup rather than asserting ours.
  useEffect(() => {
    if (!isTauri()) return;
    (async () => {
      try {
        const cur = await invoke<{ mode: string }>('get_agent_mode');
        const back = asPermMode(cur?.mode);
        if (back) setAgentPermMode(back);
      } catch { /* backend not ready; keep the default */ }
    })();
  }, []);

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
          // `chars` is a NUMBER of characters. The old code did
          // estimateTokens(String(chars)) — tokenising the *digits* of that
          // number ("84" -> 1), then added it to a `ctxUsed` captured before
          // the user's own message existed. The footer therefore disagreed
          // with the status bar (observed: "ctx 11" vs "27" for one turn).
          // Both now derive from the same live transcript.
          ctxUsed: estimateContext(
            conversationsRef.current.find((c) => c.id === convId)?.messages ?? []
          ),
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
            current={storedPersona.current}
            onPick={(p) => {
              choosePersona(p);
              setPhase('readiness');
            }}
          />
        </div>
      </>
    );
  }
  if (phase === 'readiness') {
    return (
      <>
        <IconSprite />
        <div className="lc-shell">
          <ReadinessStep
            persona={persona}
            platform={platform}
            api={readiness}
            onContinue={() => { markReadinessSeen(); setPhase('profiling'); }}
            onBack={() => setPhase('persona')}
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
          persona={persona}
          platform={platform}
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
          onContinue={() => leaveSetup()}
          onBrowseAll={() => leaveSetup(() => setNav('library'))}
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
                onPersona={choosePersona}
                platform={platform}
                prefs={prefs}
                onPrefs={setPrefs}
                onReplayOnboarding={replayOnboarding}
                onWorkspaceDir={setWorkspaceDir}
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
                        secondRow={<CoworkRow permMode={agentPermMode} onPick={syncPermMode} workspaceDir={workspaceDir} onScope={() => setNav('settings')} />}
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
                        secondRow={<CoworkRow permMode={agentPermMode} onPick={syncPermMode} workspaceDir={workspaceDir} onScope={() => setNav('settings')} />}
                      />
                    }
                    tasks={tasks}
                    onClear={() => setTasks([])}
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
                    {/* This used to show the *models* directory, which is where
                        downloads live — nothing to do with where the agent runs
                        commands. It now reflects the real scope. */}
                    <button
                      type="button"
                      className="chip"
                      onClick={() => setNav('settings')}
                      title={workspaceDir ?? 'Commands can run anywhere on this computer — click to choose a folder'}
                    >
                      <Icon name="folder" /> {workspaceDir ? basename(workspaceDir) : 'This computer'}
                    </button>
                    <span className="chip"><Icon name="term" /> {hardware?.os.name ?? 'local'}</span>
                  </div>
                  <InputBar
                    {...composerCommon}
                    variant="code"
                    onSend={runAgent}
                    agentPermMode={agentPermMode}
                    onPermMode={syncPermMode}
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
function CoworkRow({
  permMode, onPick, workspaceDir, onScope,
}: {
  permMode: AgentPermMode;
  onPick: (m: AgentPermMode) => void;
  workspaceDir: string | null;
  onScope: () => void;
}) {
  return (
    <div className="crow2">
      <button
        type="button"
        className="chip"
        onClick={onScope}
        title={workspaceDir ?? 'Commands can run anywhere on this computer — click to choose a folder'}
      >
        <Icon name="folder" /> {workspaceDir ? basename(workspaceDir) : 'This computer'}
      </button>
      <span className="chip"><Icon name="tool" /> Shell · Files · Browser</span>
      <div className="sp" style={{ flex: 1 }} />
      <PermMenu mode={permMode} onPick={onPick} />
    </div>
  );
}

/** Last path segment, for a chip that has no room for the whole path. */
function basename(p: string): string {
  const parts = p.replace(/[/\\]+$/, '').split(/[/\\]/);
  return parts[parts.length - 1] || p;
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
