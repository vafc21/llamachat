import { forwardRef } from 'react'
import type { Message } from '../types'
import type { TurnStats } from '../runtime'
import type { Persona } from '../persona'
import { MessageBubble } from './MessageBubble'

interface ChatAreaProps {
  messages: Message[];
  streaming: boolean;
  persona: Persona;
  /** Measured stats per assistant message id. */
  stats: Record<string, TurnStats>;
  /** Plain-words status per assistant message id ("Thought for 6 seconds"). */
  simpleStatus: Record<string, string>;
}

/** The conversation thread — v6's `.thread`. */
export const ChatArea = forwardRef<HTMLDivElement, ChatAreaProps>(
  function ChatArea({ messages, streaming, persona, stats, simpleStatus }, ref) {
    const lastId = messages[messages.length - 1]?.id;
    return (
      <div ref={ref} className="thread">
        {messages.map((msg) => (
          <MessageBubble
            key={msg.id}
            msg={msg}
            persona={persona}
            stats={stats[msg.id]}
            simpleStatus={simpleStatus[msg.id]}
            streaming={streaming && msg.id === lastId}
          />
        ))}
      </div>
    );
  }
);
