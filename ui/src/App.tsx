import { useState, useRef, useCallback, useEffect } from 'react'
import { Sidebar } from './components/Sidebar'
import { ChatArea } from './components/ChatArea'
import { InputBar } from './components/InputBar'
import { SetupWizard } from './components/SetupWizard'
import type { Message, Conversation, HardwareProfile } from './types'

// Mock conversations for dev
const INITIAL_CONVERSATIONS: Conversation[] = [
  { id: '1', title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
];

export default function App() {
  const [platform, setPlatform] = useState<string>('linux');

  // Detect platform for native-feel CSS
  useEffect(() => {
    async function detect() {
      try {
        if (window.__TAURI__) {
          const p = await window.__TAURI__.invoke('plugin:tauri|platform') as string;
          setPlatform(p);
          return;
        }
      } catch {}
      // Fallback for dev (outside Tauri)
      const ua = navigator.platform || '';
      if (ua.includes('Mac')) setPlatform('macos');
      else if (ua.includes('Win')) setPlatform('windows');
      else setPlatform('linux');
    }
    detect();
  }, []);

  // Set platform class on document for CSS
  useEffect(() => {
    document.documentElement.setAttribute('data-platform', platform);
  }, [platform]);
  const [setupComplete, setSetupComplete] = useState(false);
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  const [activeId, setActiveId] = useState('1');
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [streaming, setStreaming] = useState(false);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const chatRef = useRef<HTMLDivElement>(null);

  const active = conversations.find((c) => c.id === activeId) ?? conversations[0];

  function handleSetupComplete(hw: HardwareProfile) {
    setHardware(hw);
    setSetupComplete(true);
  }

  const addMessage = useCallback((msg: Message) => {
    setConversations((prev) =>
      prev.map((c) =>
        c.id === activeId ? { ...c, messages: [...c.messages, msg] } : c
      )
    );
  }, [activeId]);

  function handleSend(text: string) {
    if (!text.trim() || streaming) return;

    // User message
    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: text,
      timestamp: new Date().toISOString(),
    };
    addMessage(userMsg);

    // Auto-title first message
    if (active.messages.length === 0) {
      const title = text.slice(0, 60) + (text.length > 60 ? '…' : '');
      setConversations((prev) =>
        prev.map((c) => (c.id === activeId ? { ...c, title } : c))
      );
    }

    // Simulate assistant response
    setStreaming(true);
    const assistantMsg: Message = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
    };
    addMessage(assistantMsg);

    // Mock streaming response
    const responses = [
      "I'll help you with that.",
      "\n\nHere's what I found:",
      "\n\n```sh\n$ ls -la /home/vlad/.openclaw/workspace/fitllm\ntotal 80\ndrwxrwxr-x  5 vlad vlad  4096 Jul  9 04:47 .\ndrwx------ 11 vlad vlad  4096 Jul  9 04:41 ..\n-rw-rw-r--  1 vlad vlad  5381 Jul  9 04:46 CONTRACT.md\n-rw-rw-r--  1 vlad vlad 21944 Jul  9 04:47 Cargo.lock\n-rw-rw-r--  1 vlad vlad   824 Jul  9 04:47 Cargo.toml\n-rw-rw-r--  1 vlad vlad  5001 Jul  9 04:45 RECON.md\n-rw-rw-r--  1 vlad vlad 11250 Jul  9 04:42 SPEC.md\n```",
      "\n\nThe project is set up and ready. Is there anything specific you'd like me to work on?",
    ];

    let delay = 0;
    responses.forEach((chunk) => {
      setTimeout(() => {
        setConversations((prev) =>
          prev.map((c) =>
            c.id === activeId
              ? {
                  ...c,
                  messages: c.messages.map((m) =>
                    m.id === assistantMsg.id
                      ? { ...m, content: m.content + chunk }
                      : m
                  ),
                }
              : c
          )
        );
      }, delay);
      delay += chunk.length * 18; // rough typing speed
    });

    setTimeout(() => setStreaming(false), delay + 100);
  }

  function handleNewConversation() {
    const id = crypto.randomUUID();
    setConversations((prev) => [
      { id, title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
      ...prev,
    ]);
    setActiveId(id);
  }

  function handleDeleteConversation(id: string) {
    setConversations((prev) => {
      const next = prev.filter((c) => c.id !== id);
      if (activeId === id && next.length > 0) setActiveId(next[0].id);
      return next;
    });
  }

  function handleToolCall(tool: string) {
    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: `/${tool}`,
      timestamp: new Date().toISOString(),
      toolCall: { name: tool, args: {} },
    };
    addMessage(userMsg);
  }

  // Setup wizard on first launch
  if (!setupComplete) {
    return <SetupWizard onComplete={handleSetupComplete} />;
  }

  return (
    <div className="h-full flex bg-bg">
      {/* Sidebar */}
      <Sidebar
        open={sidebarOpen}
        conversations={conversations}
        activeId={activeId}
        hardware={hardware}
        onSelect={setActiveId}
        onNew={handleNewConversation}
        onDelete={handleDeleteConversation}
        onToggle={() => setSidebarOpen((o) => !o)}
      />

      {/* Main area */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Header bar */}
        <div className="flex-shrink-0 h-9 border-b border-border flex items-center px-3 gap-2">
          {!sidebarOpen && (
            <button
              onClick={() => setSidebarOpen(true)}
              className="text-text-secondary hover:text-text p-0.5"
              title="Toggle sidebar"
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M2 3h12M2 8h12M2 13h12" stroke="currentColor" strokeWidth="1.5" />
              </svg>
            </button>
          )}
          <span className="text-[11px] text-text-muted truncate">
            {active.title}
          </span>
          {hardware && (
            <span className="text-[10px] text-text-muted ml-auto">
              {hardware.cpu.model} · {hardware.gpus[0]?.model ?? 'CPU'}
            </span>
          )}
        </div>

        {/* Chat */}
        <ChatArea
          ref={chatRef}
          messages={active.messages}
          streaming={streaming}
        />

        {/* Input */}
        <InputBar
          onSend={handleSend}
          onToolCall={handleToolCall}
          disabled={streaming}
        />
      </div>
    </div>
  );
}
