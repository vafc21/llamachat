import { useState } from 'react'
import { MemorySeed } from './MemorySeed'
import { AgentSetup } from './AgentSetup'

/** First-run optional steps: seed memory, then enable agent permissions. */
export function WelcomeSteps({ onFinish }: { onFinish: () => void }) {
  const [step, setStep] = useState(0);

  return (
    <div className="h-full overflow-y-auto bg-bg">
      <div className="min-h-full flex items-center justify-center py-10">
        <div className="w-full max-w-lg px-5">
          <div className="flex items-center justify-center gap-2 mb-6">
            {[0, 1].map((i) => (
              <span key={i} className={`h-1.5 rounded-full transition-all ${i === step ? 'w-6 bg-accent' : 'w-1.5 bg-border'}`} />
            ))}
          </div>

          {step === 0 && <MemorySeed onNext={() => setStep(1)} onSkip={() => setStep(1)} />}

          {step === 1 && (
            <div className="space-y-4">
              <div>
                <p className="text-sm text-text font-medium">Enable agent abilities <span className="text-text-muted font-normal">· optional</span></p>
                <p className="text-[11px] text-text-muted mt-1 leading-relaxed">
                  To let the agent control your Mac — open apps, read the screen, click and type — grant these. A green ✓ means it's ready.
                  You can always do this later in Settings.
                </p>
              </div>
              <AgentSetup />
              <button
                onClick={onFinish}
                className="w-full py-2.5 bg-accent text-white text-[13px] font-medium rounded-lg hover:opacity-90 transition-opacity"
              >
                Start using FitLLM
              </button>
              <button onClick={() => setStep(0)} className="w-full text-[11px] text-text-muted hover:text-text-secondary">
                ← Back
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
