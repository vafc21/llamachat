import { useState } from 'react'
import { setConsent } from '../api'

export function OnboardingWizard({ onConsent }: { onConsent: () => void }) {
  const [step, setStep] = useState(0);
  const [agreed, setAgreed] = useState(false);

  async function handleAccept() {
    await setConsent(true);
    onConsent();
  }

  return (
    <div className="min-h-screen flex items-center justify-center px-4">
      <div className="max-w-lg w-full">
        {/* Step indicators */}
        <div className="flex justify-center gap-2 mb-8">
          {[0, 1].map((i) => (
            <div
              key={i}
              className={`w-2 h-2 rounded-full transition-colors ${
                step >= i ? 'bg-fitllm-accent' : 'bg-fitllm-border'
              }`}
            />
          ))}
        </div>

        {step === 0 && (
          <div className="bg-fitllm-card border border-fitllm-border rounded-2xl p-8">
            <div className="text-4xl mb-4">⚡</div>
            <h1 className="text-2xl font-bold text-fitllm-text mb-3">
              Welcome to FitLLM
            </h1>
            <p className="text-fitllm-muted text-sm leading-relaxed mb-6">
              Find out which AI models actually run on <em>your</em> machine — measured,
              not guessed. FitLLM profiles your hardware, runs quick on-device
              benchmarks, and gives you honest "Won't run → Blazing" ratings for
              every model.
            </p>
            <div className="space-y-3 mb-6">
              {[
                '🖥️  Profiles your CPU, GPU, RAM, and storage',
                '🏃  Runs real benchmarks on your hardware',
                '🔒  Everything stays on your device — zero telemetry',
                '💬  Explains every recommendation in plain language',
              ].map((item) => (
                <div key={item} className="text-sm text-fitllm-text flex items-start gap-2">
                  <span>{item}</span>
                </div>
              ))}
            </div>
            <button
              onClick={() => setStep(1)}
              className="w-full py-3 bg-fitllm-accent text-white font-medium rounded-xl hover:opacity-90 transition-opacity"
            >
              Continue
            </button>
          </div>
        )}

        {step === 1 && (
          <div className="bg-fitllm-card border border-fitllm-border rounded-2xl p-8">
            <div className="text-3xl mb-4">🔒</div>
            <h2 className="text-xl font-bold text-fitllm-text mb-3">
              Your Privacy
            </h2>
            <p className="text-fitllm-muted text-sm leading-relaxed mb-6">
              Before we start, here&apos;s exactly what FitLLM reads — and what it
              never does.
            </p>

            <div className="space-y-3 mb-6">
              <div className="bg-fitllm-surface rounded-lg p-3">
                <h3 className="text-xs font-semibold text-fitllm-success mb-2 uppercase tracking-wide">
                  What FitLLM reads
                </h3>
                <ul className="text-xs text-fitllm-text space-y-1.5">
                  <li>• CPU model, core count, and instruction set flags</li>
                  <li>• GPU model, VRAM, drivers, and compute capability</li>
                  <li>• Total and available system RAM</li>
                  <li>• Free disk space (where models would be stored)</li>
                  <li>• Operating system name and version</li>
                </ul>
              </div>

              <div className="bg-fitllm-surface rounded-lg p-3">
                <h3 className="text-xs font-semibold text-fitllm-danger mb-2 uppercase tracking-wide">
                  What FitLLM NEVER does
                </h3>
                <ul className="text-xs text-fitllm-text space-y-1.5">
                  <li>• Never sends data off your device</li>
                  <li>• Never reads your files, browser history, or personal data</li>
                  <li>• Never requires an account or internet connection</li>
                  <li>• Never auto-downloads models without your explicit consent</li>
                </ul>
              </div>
            </div>

            <label className="flex items-start gap-3 mb-6 cursor-pointer">
              <input
                type="checkbox"
                checked={agreed}
                onChange={(e) => setAgreed(e.target.checked)}
                className="mt-0.5 w-4 h-4 rounded border-fitllm-border bg-fitllm-surface accent-fitllm-accent"
              />
              <span className="text-xs text-fitllm-text leading-relaxed">
                I understand what FitLLM reads and I&apos;m okay with it. My data stays
                on my device.
              </span>
            </label>

            <div className="flex gap-3">
              <button
                onClick={() => setStep(0)}
                className="px-4 py-3 text-sm text-fitllm-muted hover:text-fitllm-text transition-colors"
              >
                Back
              </button>
              <button
                onClick={handleAccept}
                disabled={!agreed}
                className={`flex-1 py-3 rounded-xl font-medium transition-all ${
                  agreed
                    ? 'bg-fitllm-accent text-white hover:opacity-90'
                    : 'bg-fitllm-border text-fitllm-muted cursor-not-allowed'
                }`}
              >
                Start Profiling
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
