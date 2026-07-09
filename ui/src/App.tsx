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

    // Try real inference via sidecar dev server, fall back to mock
    setStreaming(true);
    const assistantMsg: Message = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
    };
    addMessage(assistantMsg);

    tryRealOrMock(text, assistantMsg.id);
  }

  async function tryRealOrMock(text: string, msgId: string) {
    try {
      // Try the sidecar HTTP dev server
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 30000);

      const resp = await fetch('/api/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          adapter: 'ollama',
          model: 'llama3.2:1b',
          messages: [{ role: 'user', content: text }],
          system: 'You are a helpful assistant running locally. Be concise and direct.',
        }),
        signal: controller.signal,
      });

      clearTimeout(timeout);

      if (!resp.ok || !resp.body) throw new Error('no body');

      const reader = resp.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        // Parse SSE events
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            try {
              const data = JSON.parse(line.slice(6));
              if (data.token) {
                setConversations((prev) =>
                  prev.map((c) =>
                    c.id === activeId
                      ? {
                          ...c,
                          messages: c.messages.map((m) =>
                            m.id === msgId
                              ? { ...m, content: m.content + data.token }
                              : m
                          ),
                        }
                      : c
                  )
                );
              }
              if (data.done) break;
            } catch {}
          }
        }
      }
    } catch {
      // Fall back to mock if sidecar isn't running
      mockResponse(text, msgId);
    } finally {
      setStreaming(false);
    }
  }

  function mockResponse(_text: string, msgId: string) {
    const responses = [
      "I'll help you with that.",
      "\n\nHere's what I found:",
      "\n\n```sh\n$ ls -la\n```",
      "\n\n*(Connect the sidecar dev server for real responses: `python -m fitllm_sidecar dev-server`)*",
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
                    m.id === msgId
                      ? { ...m, content: m.content + chunk }
                      : m
                  ),
                }
              : c
          )
        );
      }, delay);
      delay += chunk.length * 20;
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
