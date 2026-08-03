// ── The router (R18) ───────────────────────────────────────────────────────
//
// Simple persona never picks a model. This does, per message, modelled on the
// unified-system behaviour described in REQUIREMENTS §"The router (research)":
// route on conversation type, complexity, tool needs and explicit intent, and
// never select a model this machine can't run.
//
// This is a local heuristic, not a trained router — the requirements doc is
// explicit that OpenAI's is trained on usage signals, which we have none of.
// It is real logic driving a real model choice, and dev persona is shown the
// decision verbatim (R19).

import type { TierModel, TierId } from './types'

export type Effort = 'low' | 'medium' | 'high';

export interface RouteDecision {
  /** Ollama tag actually used for the request. */
  tag: string;
  /** Which tier the router aimed for (may fall back if that tier isn't ready). */
  tier: TierId;
  effort: Effort;
  /** Short human reason, printed to developers as `reason: …`. */
  reason: string;
  /** True when the wanted tier wasn't ready and we fell back. */
  fellBack: boolean;
}

/** "Think hard about this" and friends force the deep path. */
const FORCE_DEEP = /\b(think (really |very )?hard|think step[- ]by[- ]step|deep(ly)? (think|reason)|take your time|be thorough|reason carefully)\b/i;

/** Signals that the message is technical and/or wants tools. */
const TECHNICAL = /\b(code|compile|stack ?trace|regex|typescript|rust|python|sql|api|refactor|debug|kernel|docker|kubernetes|git|shell|bash|algorithm|benchmark|quantiz|vram|cuda|tensor)\b/i;

/** Multi-step / planning language. */
const MULTISTEP = /\b(plan|steps?|first.*then|compare|trade[- ]?offs?|design|architect|migrate|refactor|walk me through|end[- ]to[- ]end)\b/i;

/** Short factual lookups. */
const FACTUAL = /^(what|who|when|where|how many|how much|is|are|does|do|did|can)\b/i;

/** Pick a ready tier, walking down from the wanted one. */
function resolve(tiers: TierModel[], want: TierId): { t: TierModel | null; fellBack: boolean } {
  const order: TierId[] = ['best', 'smart', 'quick'];
  const from = order.indexOf(want);
  const chain = from >= 0 ? order.slice(from) : order;
  for (const id of chain) {
    // Never route to a model this machine can't run (REQUIREMENTS routing table).
    const t = tiers.find((x) => x.tier === id && x.status === 'ready' && x.rec.tier !== 'wont_run' && x.rec.tier !== 'slow');
    if (t) return { t, fellBack: id !== want };
  }
  // Last resort: anything ready at all, then anything at all.
  const any = tiers.find((x) => x.status === 'ready') ?? tiers[0] ?? null;
  return { t: any, fellBack: any?.tier !== want };
}

/**
 * Choose the model + effort for one message.
 *
 * `fallbackTag` is used when no tier is ready yet (fresh install mid-download)
 * so the app still has something to send.
 */
export function route(text: string, tiers: TierModel[], fallbackTag: string): RouteDecision {
  const t = text.trim();
  const words = t.split(/\s+/).length;

  let want: TierId;
  let effort: Effort;
  let reason: string;

  if (FORCE_DEEP.test(t)) {
    want = 'best';
    effort = 'high';
    reason = 'you asked for deep thinking';
  } else if (TECHNICAL.test(t) && MULTISTEP.test(t)) {
    want = 'best';
    effort = 'high';
    reason = 'technical, multi-step';
  } else if (words > 90 || MULTISTEP.test(t)) {
    want = 'best';
    effort = 'high';
    reason = words > 90 ? 'long, multi-part request' : 'multi-step';
  } else if (TECHNICAL.test(t)) {
    want = 'smart';
    effort = 'medium';
    reason = 'technical';
  } else if (words <= 12 && FACTUAL.test(t)) {
    want = 'quick';
    effort = 'low';
    reason = 'short, factual, single-step';
  } else {
    want = 'smart';
    effort = 'medium';
    reason = 'everyday work, some reasoning';
  }

  const { t: picked, fellBack } = resolve(tiers, want);
  return {
    tag: picked?.rec.ollama_pull ?? fallbackTag,
    tier: picked?.tier ?? want,
    effort,
    reason: fellBack ? `${reason} · ${want} not ready` : reason,
    fellBack,
  };
}

/** `Router → Qwen3 30B A3B · high effort · reason: technical, multi-step` (R19). */
export function describeRoute(d: RouteDecision, tiers: TierModel[]): string {
  const name = tiers.find((t) => t.rec.ollama_pull === d.tag)?.rec.display_name ?? d.tag;
  return `Router → ${name} · ${d.effort} effort · reason: ${d.reason}`;
}
