import { useState, useEffect } from 'react'
import { HardwarePanel } from './components/HardwarePanel'
import { RecommendationsList } from './components/RecommendationsList'
import { OnboardingWizard } from './components/OnboardingWizard'
import {
  type HardwareProfile,
  type Recommendation,
  getHardwareProfile,
  getRecommendations,
  getConsent,
} from './api'

type View = 'loading' | 'onboarding' | 'dashboard';

export default function App() {
  const [view, setView] = useState<View>('loading');
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [benchmarkRunning, setBenchmarkRunning] = useState(false);

  useEffect(() => {
    async function init() {
      const consented = await getConsent();
      if (!consented) {
        setView('onboarding');
        return;
      }
      await loadDashboard();
    }
    init();
  }, []);

  async function loadDashboard() {
    setView('loading');
    const [hw, recs] = await Promise.all([
      getHardwareProfile(),
      getRecommendations(),
    ]);
    setHardware(hw);
    setRecommendations(recs);
    setView('dashboard');
  }

  async function handleConsent() {
    await loadDashboard();
  }

  return (
    <div className="min-h-screen bg-fitllm-bg">
      {view === 'loading' && (
        <div className="flex items-center justify-center h-screen">
          <div className="text-center">
            <div className="w-12 h-12 border-2 border-fitllm-accent border-t-transparent rounded-full animate-spin mx-auto mb-4" />
            <p className="text-fitllm-muted text-sm">Profiling hardware &hellip;</p>
          </div>
        </div>
      )}

      {view === 'onboarding' && (
        <OnboardingWizard onConsent={handleConsent} />
      )}

      {view === 'dashboard' && hardware && (
        <Dashboard
          hardware={hardware}
          recommendations={recommendations}
          benchmarkRunning={benchmarkRunning}
          onStartBenchmark={() => setBenchmarkRunning(true)}
        />
      )}
    </div>
  );
}

function Dashboard({
  hardware,
  recommendations,
  benchmarkRunning,
  onStartBenchmark,
}: {
  hardware: HardwareProfile;
  recommendations: Recommendation[];
  benchmarkRunning: boolean;
  onStartBenchmark: () => void;
}) {
  const measured = recommendations.filter((r) => r.source === 'measured');

  return (
    <div className="max-w-6xl mx-auto px-4 py-8">
      {/* Header */}
      <header className="mb-8">
        <div className="flex items-center gap-3 mb-2">
          <span className="text-2xl">⚡</span>
          <h1 className="text-2xl font-bold text-fitllm-text">FitLLM</h1>
          <span className="text-xs bg-fitllm-card border border-fitllm-border rounded-full px-3 py-0.5 text-fitllm-muted">
            Phase 1 MVP
          </span>
        </div>
        <p className="text-fitllm-muted text-sm">
          Which AI models actually run on your machine? Measured, not guessed.
        </p>
      </header>

      {/* Hardware Panel */}
      <section className="mb-8">
        <HardwarePanel hardware={hardware} />
      </section>

      {/* Benchmark Status */}
      <section className="mb-8">
        <div className="bg-fitllm-card border border-fitllm-border rounded-xl p-4 flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-fitllm-text">
              {benchmarkRunning
                ? 'Benchmark running&hellip;'
                : measured.length > 0
                  ? 'Measurements ready'
                  : 'Provisional ratings'}
            </h3>
            <p className="text-xs text-fitllm-muted mt-0.5">
              {benchmarkRunning
                ? 'Running quick benchmark in the background — your machine stays responsive.'
                : measured.length > 0
                  ? `${measured.length} models benchmarked. Refresh for real numbers.`
                  : 'Estimates based on your hardware specs. Run a quick benchmark for measured numbers.'}
            </p>
          </div>
          {!benchmarkRunning && (
            <button
              onClick={onStartBenchmark}
              className="px-4 py-2 bg-fitllm-accent text-white text-sm font-medium rounded-lg hover:opacity-90 transition-opacity"
            >
              {measured.length > 0 ? 'Re-run Benchmark' : 'Run Quick Benchmark'}
            </button>
          )}
          {benchmarkRunning && (
            <div className="flex items-center gap-2 text-fitllm-accent text-sm">
              <div className="w-4 h-4 border-2 border-fitllm-accent border-t-transparent rounded-full animate-spin" />
              Running&hellip;
            </div>
          )}
        </div>
      </section>

      {/* Recommendations */}
      <section>
        <div className="flex items-center gap-2 mb-4">
          <h2 className="text-lg font-semibold text-fitllm-text">Recommendations</h2>
          <span className="text-xs text-fitllm-muted bg-fitllm-card border border-fitllm-border rounded-full px-2 py-0.5">
            {recommendations.length} models
          </span>
        </div>
        <RecommendationsList recommendations={recommendations} />
      </section>
    </div>
  );
}
