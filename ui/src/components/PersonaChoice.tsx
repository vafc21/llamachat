import { useState } from 'react'
import { Icon } from './Icon'
import type { Persona } from '../persona'

/**
 * The startup question (R14): developer or not, asked exactly once, defaulting
 * to the simple side. Ported from v6's `.setup` screen. It runs as the first
 * step of the first-run flow, before hardware profiling finishes, so answering
 * it costs the user nothing.
 *
 * R15 — the same choice is changeable any time in Settings (the same two cards
 * are reused there).
 */
export function PersonaChoice({ current, onPick }: { current?: Persona | null; onPick: (p: Persona) => void }) {
  // Default is the simple side (R14) — but if the user has already answered and
  // stepped back to change it, show what they picked rather than silently
  // resetting the card to 'simple' under them.
  const [choice, setChoice] = useState<Persona>(current ?? 'simple');

  return (
    <div className="setup">
      <div className="mk"><Icon name="llama" /></div>
      <h1>Welcome to LlamaChat.</h1>
      <p className="sub">
        Everything runs on this machine. Nothing is sent anywhere.
        One question so we can set the app up for you.
      </p>

      <div className="choices">
        <button
          type="button"
          className={`choice${choice === 'simple' ? ' on' : ''}`}
          onClick={() => setChoice('simple')}
        >
          <div className="ct"><Icon name="user" /><b>Just use it</b></div>
          <p>Type, get an answer. LlamaChat picks the right model and how hard to think, on its own.</p>
          <ul>
            <li><Icon name="check" />Chat and Cowork</li>
            <li><Icon name="check" />Answers tuned to this machine automatically</li>
            <li className="off"><Icon name="dash" />No model names</li>
            <li className="off"><Icon name="dash" />No context or token counters</li>
          </ul>
        </button>

        <button
          type="button"
          className={`choice${choice === 'dev' ? ' on' : ''}`}
          onClick={() => setChoice('dev')}
        >
          <div className="ct"><Icon name="code" /><b>I&apos;m a developer</b></div>
          <p>Show me everything. Pick models by hand, watch the runtime, and get the Code workspace.</p>
          <ul>
            <li><Icon name="check" />Chat, Cowork <b style={{ fontWeight: 500 }}>and Code</b></li>
            <li><Icon name="check" />Choose the model and effort yourself</li>
            <li><Icon name="check" />Tokens/sec, context, CPU · GPU · VRAM</li>
            <li><Icon name="check" />See what the router picked, and why</li>
          </ul>
        </button>
      </div>

      <div className="go">
        <button type="button" onClick={() => onPick(choice)}>Continue</button>
        <span className="note">You can switch at any time in Settings.</span>
      </div>
    </div>
  );
}

/** The same two cards, as a settings control (R15). */
export function PersonaCards({ persona, onPick }: { persona: Persona; onPick: (p: Persona) => void }) {
  return (
    <div className="modecards">
      <button
        type="button"
        className={`mc${persona === 'simple' ? ' on' : ''}`}
        onClick={() => onPick('simple')}
      >
        {persona === 'simple' && <span className="badge2">Active</span>}
        <div className="top"><Icon name="user" /><b>Just use it</b></div>
        <p>LlamaChat chooses the model and the effort for every message.</p>
        <ul>
          <li><Icon name="check" />Chat + Cowork</li>
          <li className="off"><Icon name="dash" />Model names hidden</li>
          <li className="off"><Icon name="dash" />No counters</li>
        </ul>
      </button>

      <button
        type="button"
        className={`mc${persona === 'dev' ? ' on' : ''}`}
        onClick={() => onPick('dev')}
      >
        {persona === 'dev' && <span className="badge2">Active</span>}
        <div className="top"><Icon name="code" /><b>Developer</b></div>
        <p>Full control and full visibility into the local runtime.</p>
        <ul>
          <li><Icon name="check" />Adds Code</li>
          <li><Icon name="check" />Manual model + effort</li>
          <li><Icon name="check" />Runtime insights</li>
        </ul>
      </button>
    </div>
  );
}
