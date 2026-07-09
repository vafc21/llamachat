import type { Message } from '../types'

export function MessageBubble({ msg }: { msg: Message }) {
  const isUser = msg.role === 'user';
  const isSystem = msg.role === 'system';

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <span className="text-[10px] text-text-muted bg-white/[0.02] border border-border
                         rounded px-2 py-0.5">
          {msg.content}
        </span>
      </div>
    );
  }

  return (
    <div className={`flex gap-3 animate-fade-in ${isUser ? 'flex-row-reverse' : ''}`}>
      {/* Avatar */}
      <div
        className={`flex-shrink-0 w-6 h-6 rounded flex items-center justify-center text-[11px]
          ${isUser
            ? 'bg-white/5 text-text-secondary'
            : 'bg-accent-dim text-accent'
          }`}
      >
        {isUser ? 'V' : 'F'}
      </div>

      {/* Content */}
      <div className={`min-w-0 flex-1 ${isUser ? 'flex flex-col items-end' : ''}`}>
        {/* Tool call */}
        {msg.toolCall && (
          <div className="mb-1.5">
            <span className="inline-flex items-center gap-1 text-[10px] text-text-muted
                           bg-accent-dim border border-accent/20 rounded px-1.5 py-0.5 font-mono">
              <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                <path d="M5 3l5 5-5 5" stroke="currentColor" strokeWidth="1.5" />
              </svg>
              {msg.toolCall.name}
            </span>
          </div>
        )}

        {/* Actual message text */}
        <div
          className={`text-[13px] leading-relaxed prose max-w-[85%]
            ${isUser
              ? 'bg-white/[0.04] border border-border rounded-lg px-3 py-2'
              : 'text-text'
            }`}
        >
          {msg.content ? (
            <MessageContent content={msg.content} />
          ) : (
            <span className="text-text-muted italic">...</span>
          )}
        </div>

        {/* Timestamp */}
        <span className="text-[10px] text-text-muted mt-0.5 px-1">
          {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      </div>
    </div>
  );
}

/** Simple markdown-like rendering for code blocks and inline code */
function MessageContent({ content }: { content: string }) {
  // Split on code blocks (```...```)
  const parts = content.split(/(```[\s\S]*?```)/g);

  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('```')) {
          // Strip the ``` markers and optional language tag
          const lines = part.split('\n');
          // Remove first line (```lang) and last line (```)
          const code = lines.slice(1, -1).join('\n');
          const lang = lines[0].replace('```', '').trim();
          return <CodeBlock key={i} code={code} lang={lang} />;
        }
        // Inline formatting
        return <InlineText key={i} text={part} />;
      })}
    </>
  );
}

function CodeBlock({ code, lang }: { code: string; lang: string }) {
  return (
    <div className="my-1.5 -mx-1">
      {lang && (
        <div className="text-[10px] text-text-muted px-3 pt-2 pb-0.5 font-mono">
          {lang}
        </div>
      )}
      <pre className="!mt-0 !mb-0 !rounded !text-[12px]">
        <code>{code}</code>
      </pre>
    </div>
  );
}

function InlineText({ text }: { text: string }) {
  // Handle inline code (`...`)
  const parts = text.split(/(`[^`]+`)/g);
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('`') && part.endsWith('`')) {
          return (
            <code key={i} className="!text-[12px]">
              {part.slice(1, -1)}
            </code>
          );
        }
        return <span key={i}>{part}</span>;
      })}
    </>
  );
}
