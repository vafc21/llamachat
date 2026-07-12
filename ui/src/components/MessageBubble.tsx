import { useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { Message } from '../types'
import { Logo } from './Logo'

export function MessageBubble({ msg }: { msg: Message }) {
  const isUser = msg.role === 'user';
  const isSystem = msg.role === 'system';
  // Agent tool-step messages (rendered by App as "🔧 …") get a distinct card look.
  const isToolStep = !isUser && msg.content.trimStart().startsWith('🔧');

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <span className="text-[10px] text-text-muted bg-white/[0.02] border border-border rounded px-2 py-0.5">
          {msg.content}
        </span>
      </div>
    );
  }

  return (
    <div className={`flex gap-3 animate-fade-in ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className={`flex-shrink-0 w-6 h-6 rounded flex items-center justify-center text-[11px]
          ${isUser ? 'bg-white/5 text-text-secondary' : 'bg-accent-dim text-accent'}`}
      >
        {isUser ? 'V' : <Logo size={20} />}
      </div>

      <div className={`min-w-0 flex-1 ${isUser ? 'flex flex-col items-end' : ''}`}>
        <div
          className={`text-[13px] leading-relaxed max-w-[88%] min-w-0
            ${isUser
              ? 'bg-white/[0.04] border border-border rounded-lg px-3 py-2'
              : isToolStep
                ? 'border-l-2 border-accent/40 pl-3 text-text-secondary'
                : 'text-text'
            }`}
        >
          {msg.content ? (
            <Markdown content={msg.content} />
          ) : (
            <span className="inline-flex gap-1 text-text-muted">
              <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse" />
              <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse [animation-delay:150ms]" />
              <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse [animation-delay:300ms]" />
            </span>
          )}
        </div>

        <span className="text-[10px] text-text-muted mt-0.5 px-1">
          {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      </div>
    </div>
  );
}

/** Full markdown rendering (GitHub-flavored) styled to match the app. */
function Markdown({ content }: { content: string }) {
  return (
    <div className="fitllm-md space-y-2">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1: ({ children }) => <h1 className="text-[15px] font-semibold text-text mt-1 mb-1">{children}</h1>,
          h2: ({ children }) => <h2 className="text-[14px] font-semibold text-text mt-1 mb-1">{children}</h2>,
          h3: ({ children }) => <h3 className="text-[13px] font-semibold text-text mt-1 mb-0.5">{children}</h3>,
          p: ({ children }) => <p className="leading-relaxed">{children}</p>,
          strong: ({ children }) => <strong className="font-semibold text-text">{children}</strong>,
          em: ({ children }) => <em className="italic">{children}</em>,
          a: ({ children, href }) => (
            <a href={href} target="_blank" rel="noreferrer" className="text-accent underline underline-offset-2 hover:opacity-80">
              {children}
            </a>
          ),
          ul: ({ children }) => <ul className="list-disc pl-5 space-y-0.5">{children}</ul>,
          ol: ({ children }) => <ol className="list-decimal pl-5 space-y-0.5">{children}</ol>,
          li: ({ children }) => <li className="leading-relaxed">{children}</li>,
          blockquote: ({ children }) => (
            <blockquote className="border-l-2 border-border pl-3 text-text-secondary italic">{children}</blockquote>
          ),
          hr: () => <hr className="border-border my-2" />,
          table: ({ children }) => (
            <div className="overflow-x-auto my-1">
              <table className="text-[12px] border-collapse">{children}</table>
            </div>
          ),
          th: ({ children }) => <th className="border border-border px-2 py-1 text-left font-medium bg-white/[0.03]">{children}</th>,
          td: ({ children }) => <td className="border border-border px-2 py-1">{children}</td>,
          pre: ({ children }) => <>{children}</>,
          code: ({ className, children }) => {
            const match = /language-(\w+)/.exec(className || '');
            const text = String(children).replace(/\n$/, '');
            if (!className && !text.includes('\n')) {
              return <code className="px-1 py-0.5 rounded bg-white/[0.06] text-[12px] font-mono">{children}</code>;
            }
            return <CodeBlock lang={match?.[1] ?? ''} code={text} />;
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}

function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }).catch(() => {});
  };
  return (
    <div className="my-2 rounded-lg border border-border overflow-hidden bg-black/20">
      <div className="flex items-center justify-between px-3 py-1 border-b border-border bg-white/[0.02]">
        <span className="text-[10px] text-text-muted font-mono">{lang || 'text'}</span>
        <button onClick={copy} className="text-[10px] text-text-muted hover:text-text transition-colors">
          {copied ? 'Copied ✓' : 'Copy'}
        </button>
      </div>
      <pre className="!m-0 !bg-transparent overflow-x-auto px-3 py-2 text-[12px] leading-relaxed">
        <code>{code}</code>
      </pre>
    </div>
  );
}
