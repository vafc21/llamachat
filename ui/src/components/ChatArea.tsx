import { forwardRef } from 'react'
import type { Message } from '../types'
import { MessageBubble } from './MessageBubble'

interface ChatAreaProps {
  messages: Message[];
  streaming: boolean;
}

export const ChatArea = forwardRef<HTMLDivElement, ChatAreaProps>(
  function ChatArea({ messages, streaming }, ref) {
    return (
      <div ref={ref} className="flex-1 overflow-y-auto px-4 py-3 space-y-3">
        {messages.length === 0 && <EmptyState />}
        {messages.map((msg) => (
          <MessageBubble key={msg.id} msg={msg} />
        ))}
        {streaming && messages[messages.length - 1]?.role === 'assistant' && (
          <span className="inline-block w-2 h-4 bg-accent animate-blink ml-1" />
        )}
        <div className="h-2" />
      </div>
    );
  }
);

function EmptyState() {
  return (
    <div className="flex items-center justify-center h-full">
      <div className="text-center max-w-sm">
        <div className="text-text-muted text-[11px] leading-relaxed space-y-1">
          <p>Your machine is profiled and a model is ready.</p>
          <p>
            <span className="text-text-secondary">Ask me anything</span>
            {' — '}I have access to your shell, files, and browser.
          </p>
          <div className="pt-3 space-y-1 text-[10px]">
            <p>
              <kbd className="px-1 py-0.5 bg-white/5 border border-border rounded text-[10px]">
                /shell
              </kbd>
              {' '}run commands
            </p>
            <p>
              <kbd className="px-1 py-0.5 bg-white/5 border border-border rounded text-[10px]">
                /file
              </kbd>
              {' '}read or write files
            </p>
            <p>
              <kbd className="px-1 py-0.5 bg-white/5 border border-border rounded text-[10px]">
                /browser
              </kbd>
              {' '}open a webpage
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
