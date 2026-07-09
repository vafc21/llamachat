import { type Tier } from '../api'

const TIER_STYLES: Record<Tier, { bg: string; text: string; dot: string }> = {
  blazing: {
    bg: 'rgba(245, 158, 11, 0.1)',
    text: '#f59e0b',
    dot: '#f59e0b',
  },
  great: {
    bg: 'rgba(34, 197, 94, 0.1)',
    text: '#22c55e',
    dot: '#22c55e',
  },
  okay: {
    bg: 'rgba(59, 130, 246, 0.1)',
    text: '#3b82f6',
    dot: '#3b82f6',
  },
  slow: {
    bg: 'rgba(249, 115, 22, 0.1)',
    text: '#f97316',
    dot: '#f97316',
  },
  wont_run: {
    bg: 'rgba(107, 114, 128, 0.1)',
    text: '#6b7280',
    dot: '#6b7280',
  },
};

const TIER_LABELS: Record<Tier, string> = {
  blazing: 'Blazing',
  great: 'Great',
  okay: 'Okay',
  slow: 'Slow',
  wont_run: "Won't Run",
};

export function TierBadge({ tier }: { tier: Tier }) {
  const style = TIER_STYLES[tier];

  return (
    <span
      className="inline-flex items-center gap-1.5 text-[11px] font-medium rounded-full px-2.5 py-0.5"
      style={{ background: style.bg, color: style.text }}
    >
      <span
        className="w-1.5 h-1.5 rounded-full"
        style={{ background: style.dot }}
      />
      {TIER_LABELS[tier]}
    </span>
  );
}
