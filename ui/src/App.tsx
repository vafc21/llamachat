import { useState, useRef, useCallback, useEffect } from 'react'
import { Sidebar } from './components/Sidebar'
import { ChatArea } from './components/ChatArea'
import { InputBar } from './components/InputBar'
import { SetupWizard } from './components/SetupWizard'
import type { Message, Conversation, HardwareProfile } from './types'

function uid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

const INITIAL_CONVERSATIONS: Conversation[] = [
  { id: uid(), title: 'New conversation', messages: [], createdAt: new Date().toISOString() },
];

export default function App() {
  type Platform = 'linux' | 'macos' | 'windows';
  const [platform, setPlatform] = useState<Platform>('linux');
  const [setupComplete, setSetupComplete] = useState(false);
  const [conversations, setConversations] = useState<Conversation[]>(INITIAL_CONVERSATIONS);
  const [activeId, setActiveId] = useState(INITIAL_CONVERSATIONS[0].id);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [streaming, setStreaming] = useState(false);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [selectedModel, setSelectedModel] = useState('llama3.2:3b');
  const chatRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    async function detect() {
      try {
        if (window.__TAURI__) {
          const p = await window.__TAURI__.invoke('plugin:tauri|platform') as string;
          setPlatform(p as Platform);
          return;
        }
      } catch {}
      const ua = navigator.platform || '';
      if (ua.includes('Mac')) setPlatform('macos');
      else if (ua.includes('Win')) setPlatform('windows');
      else setPlatform('linux');
    }
    detect();
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute('data-platform', platform);
  }, [platform]);

  const active = conversations.find((c) => c.id === activeId) ?? conversations[0];

  function handleSetupComplete(hw: HardwareProfile, model: string) {
    setHardware(hw);
    setSelectedModel(model);
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

    const userMsg: Message = {
      id: uid(),
      role: 'user',
      content: text,
      timestamp: new Date().toISOString(),
    };
    addMessage(userMsg);

    if (active.messages.length === 0) {
      const title = text.slice(0, 60) + (text.length > 60 ? '...' : '');
      setConversations((prev) =>
        prev.map((c) => (c.id === activeId ? { ...c, title } : c))
      );
    }

    setStreaming(true);
    const assistantMsg: Message = {
      id: uid(),
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
    };
    addMessage(assistantMsg);

    streamResponse(text, assistantMsg.id);
  }

  async function streamResponse(userText: string, msgId: string) {
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 60000);

      const resp = await fetch('/api/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          adapter: 'ollama',
          model: selectedModel,
          messages: [{ role: 'user', content: userText }],
          system: 'You are a helpful AI assistant running locally. Be concise and direct.',
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
                      ? { ...c, messages: c.messages.map((m) =>
                          m.id === msgId ? { ...m, content: m.content + data.token } : m) }
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
      mockResponse(msgId);
    } finally {
      setStreaming(false);
    }
  }

  function mockResponse(msgId: string) {
    const chunks = [
      "I'm running locally on your machine.",
      "\n\nSidecar not connected — start it: python -m fitllm_sidecar dev-server",
    ];
    let delay = 0;
    chunks.forEach((chunk) => {
      setTimeout(() => {
        setConversations((prev) =>
          prev.map((c) =>
            c.id === activeId
              ? { ...c, messages: c.messages.map((m) =>
                  m.id === msgId ? { ...m, content: m.content + chunk } : m) }
              : c
          )
        );
      }, delay);
      delay += chunk.length * 15;
    });
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
    setConversations((prev) => {
      const next = prev.filter((c) => c.id !== id);
      if (activeId === id && next.length > 0) setActiveId(next[0].id);
      return next;
    });
  }

  function handleToolCall(tool: string) {
    const msg: Message = {
      id: uid(),
      role: 'user',
      content: '/' + tool,
      timestamp: new Date().toISOString(),
      toolCall: { name: tool, args: {} },
    };
    addMessage(msg);
  }

  if (!setupComplete) {
    return <SetupWizard onComplete={handleSetupComplete} />;
  }

  return (
    <div className="h-full flex bg-bg">
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

      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex-shrink-0 h-9 border-b border-border flex items-center px-3 gap-2">
          {!sidebarOpen && (
            <button onClick={() => setSidebarOpen(true)} className="text-text-secondary hover:text-text p-0.5" title="Toggle sidebar">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M2 3h12M2 8h12M2 13h12" stroke="currentColor" strokeWidth="1.5" />
              </svg>
            </button>
          )}
          <span className="text-[11px] text-text-muted truncate">{active.title}</span>
          <span className="text-[10px] text-accent ml-auto">{selectedModel}</span>
          {hardware && (
            <span className="text-[10px] text-text-muted">
              &middot; {hardware.cpu.model.split(' ').slice(0, 2).join(' ')}
            </span>
          )}
        </div>

        <ChatArea ref={chatRef} messages={active.messages} streaming={streaming} />
        <InputBar onSend={handleSend} onToolCall={handleToolCall} disabled={streaming} />
      </div>
    </div>
  );
}
