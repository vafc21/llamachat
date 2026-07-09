import { type Recommendation } from '../api'
import { TierBadge } from './TierBadge'

export function RecommendationsList({
  recommendations,
}: {
  recommendations: Recommendation[];
}) {
  // Group by tier
  const blazing = recommendations.filter((r) => r.tier === 'blazing');
  const great = recommendations.filter((r) => r.tier === 'great');
  const okay = recommendations.filter((r) => r.tier === 'okay');
  const slow = recommendations.filter((r) => r.tier === 'slow');
  const wontRun = recommendations.filter((r) => r.tier === 'wont_run');

  const groups = [
    { label: 'Blazing', items: blazing },
    { label: 'Runs Great', items: great },
    { label: 'Runs Okay', items: okay },
    { label: 'Runs But Slow', items: slow },
    { label: "Won't Run", items: wontRun },
  ].filter((g) => g.items.length > 0);

  return (
    <div className="space-y-4">
      {groups.map((group) => (
        <div key={group.label}>
          <div className="text-xs text-fitllm-muted uppercase tracking-wide mb-2 font-medium">
            {group.label}
          </div>
          <div className="space-y-2">
            {group.items.map((rec) => (
              <RecommendationCard key={rec.model_id} rec={rec} />
            ))}
          </div>
        </div>
      ))}

      {recommendations.length === 0 && (
        <div className="text-center py-12 text-fitllm-muted text-sm">
          No recommendations yet. Run hardware profiling first.
        </div>
      )}
    </div>
  );
}

function RecommendationCard({ rec }: { rec: Recommendation }) {
  const tps = rec.measured_tokens_per_sec ?? rec.estimated_tokens_per_sec;
  const isMeasured = rec.source === 'measured';

  return (
    <div className="bg-fitllm-card border border-fitllm-border rounded-lg p-4 hover:border-fitllm-accent/30 transition-colors">
      <div className="flex items-start justify-between gap-3">
        {/* Left: model info */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="text-sm font-medium text-fitllm-text truncate">
              {rec.display_name}
            </h3>
            <span className="text-[11px] text-fitllm-muted shrink-0">
              {rec.params_b}B
            </span>
            {isMeasured && (
              <span className="text-[10px] bg-fitllm-success/10 text-fitllm-success rounded px-1.5 py-0.5 shrink-0">
                measured
              </span>
            )}
          </div>

          {/* Why explanation */}
          <p className="text-xs text-fitllm-muted mb-2">{rec.why}</p>

          {/* Detail chips */}
          <div className="flex flex-wrap gap-2">
            {tps && (
              <Chip
                label={`${tps.toFixed(0)} tok/s`}
                hint={isMeasured ? 'measured' : 'estimated'}
              />
            )}
            {rec.ttft_ms && <Chip label={`TTFT ${rec.ttft_ms.toFixed(0)}ms`} />}
            <Chip label={`${rec.memory_fit.required_mb} MB`} hint="weight" />
            <Chip label={`${rec.context_comfortable.toLocaleString()} ctx`} hint="context" />
            <Chip label={`${rec.quant}`} hint="quant" />
            {rec.memory_fit.offload && (
              <Chip
                label={`${(rec.memory_fit.gpu_layers_fraction * 100).toFixed(0)}% GPU`}
                hint="offload"
              />
            )}
            <Chip label={`ollama pull ${rec.ollama_pull}`} hint="pull" />
          </div>
        </div>

        {/* Right: tier badge + quality */}
        <div className="shrink-0 flex flex-col items-end gap-1">
          <TierBadge tier={rec.tier} />
          <span className="text-[10px] text-fitllm-muted">
            quality {rec.quality_score.toFixed(0)}
          </span>
        </div>
      </div>
    </div>
  );
}

function Chip({ label, hint }: { label: string; hint?: string }) {
  return (
    <span
      className="inline-flex items-center gap-1 text-[10px] bg-fitllm-surface border border-fitllm-border rounded px-1.5 py-0.5 text-fitllm-muted"
      title={hint}
    >
      {label}
    </span>
  );
}
