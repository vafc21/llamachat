import { useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { Message } from '../types'
import type { TurnStats } from '../runtime'
import { tokensPerSec } from '../runtime'
import type { Persona } from '../persona'
import { Icon } from './Icon'

interface Props {
  msg: Message;
  persona: Persona;
  /** Measured stats for this assistant turn, when we have them. */
  stats?: TurnStats;
  /** Plain-words status for the simple persona, e.g. "Thought for 6 seconds". */
  simpleStatus?: string;
  streaming?: boolean;
}

/**
 * One turn, in v6's shape:
 *   • user → a right-aligned bubble
 *   • assistant → flat prose with a status line above it, and (dev persona
 *     only) a monospace meta line below: model · tok/s · tokens out · ctx · time
 *
 * Tool steps from the agent loop get a left rule instead of the emoji marker
 * the old build used — R7 bans emoji in the chrome.
 */
export function MessageBubble({ msg, persona, stats, simpleStatus, streaming }: Props) {
  const isUser = msg.role === 'user';
  const isSystem = msg.role === 'system';
  const isToolStep = !isUser && msg.content.startsWith(TOOL_MARK);

  if (isSystem) {
    return (
      <div className="turn" style={{ textAlign: 'center' }}>
        <span className="chip" style={{ display: 'inline-flex' }}>{msg.content}</span>
      </div>
    );
  }

  if (isUser) {
    return (
      <div className="turn u animate-fade-in">
        <div className="bubble"><Markdown content={msg.content} /></div>
      </div>
    );
  }

  const body = isToolStep ? msg.content.slice(TOOL_MARK.length) : msg.content;

  return (
    <div className={`turn a animate-fade-in${isToolStep ? ' tool' : ''}`}>
      {!isToolStep && (simpleStatus || stats?.router) && (
        <div className="rstatus">
          <span className="sp" />
          {/* R17/R18: simple sees only the outcome; dev sees the router's call. */}
          <span className="simple-only">{simpleStatus}</span>
          <span className="dev-only">{stats?.router ?? simpleStatus}</span>
        </div>
      )}

      <div className="body">
        {body ? <Markdown content={body} /> : streaming ? <Thinking /> : null}
      </div>

      {/* R16/R19: per-message runtime truth, developer persona only. */}
      {persona === 'dev' && stats && !isToolStep && (
        <div className="metaline">
          <span>{stats.model}{stats.quant ? ` · ${stats.quant}` : ''}</span><span>·</span>
          <span><b>{tokensPerSec(stats).toFixed(1)} tok/s</b></span><span>·</span>
          <span>{stats.tokensOut.toLocaleString()} tokens out</span><span>·</span>
          <span>ctx <b>{stats.ctxUsed.toLocaleString()} / {stats.ctxTotal.toLocaleString()}</b></span><span>·</span>
          <span>{stats.seconds.toFixed(1)} s</span>
        </div>
      )}
    </div>
  );
}

/** Sentinel prefixing agent tool-step notes. Never rendered — it replaces the
 *  wrench emoji the old build used as a visible marker (R7). */
export const TOOL_MARK = '\u0001tool\u0001';

function Thinking() {
  return (
    <span style={{ display: 'inline-flex', gap: 5, color: 'var(--text3)' }}>
      <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse" />
      <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse [animation-delay:150ms]" />
      <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse [animation-delay:300ms]" />
    </span>
  );
}

/** Full markdown rendering (GitHub-flavored) styled to match the app. */
function Markdown({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        h1: ({ children }) => <h1 className="text-[15px] font-semibold text-text mt-1 mb-1">{children}</h1>,
        h2: ({ children }) => <h2 className="text-[14px] font-semibold text-text mt-1 mb-1">{children}</h2>,
        h3: ({ children }) => <h3 className="text-[13px] font-semibold text-text mt-1 mb-0.5">{children}</h3>,
        strong: ({ children }) => <strong className="font-semibold text-text">{children}</strong>,
        em: ({ children }) => <em className="italic">{children}</em>,
        a: ({ children, href }) => (
          <a href={href} target="_blank" rel="noreferrer" className="text-accent underline underline-offset-2 hover:opacity-80">
            {children}
          </a>
        ),
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
    <div className="card" style={{ margin: '10px 0' }}>
      <div className="cardh" style={{ padding: '7px 12px' }}>
        <span className="tag">{lang || 'text'}</span>
        <span style={{ flex: 1 }} />
        <button type="button" className="act" onClick={copy}>
          {copied ? <><Icon name="check" size={12} /> Copied</> : 'Copy'}
        </button>
      </div>
      <pre
        className="overflow-x-auto"
        style={{ margin: 0, padding: '10px 13px', fontFamily: 'var(--mono)', fontSize: 11.5, lineHeight: 1.7, color: 'var(--text2)' }}
      >
        <code>{code}</code>
      </pre>
    </div>
  );
}
