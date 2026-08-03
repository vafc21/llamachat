import { useState } from 'react'
import { MemorySeed } from './MemorySeed'
import { ReadinessChecklist } from './Readiness'
import { usePermissions } from '../permissions'
import { hasSystemPermissions, type Platform } from '../platform'
import { isTauri } from '../tauri'
import type { Persona } from '../persona'

interface Props {
  persona: Persona;
  platform: Platform;
  onFinish: () => void;
}

/**
 * First-run optional steps, shown after the models land.
 *
 * The permission checklist used to live here unconditionally, which meant a
 * user who had just worked through the readiness step got asked the same
 * questions a second time. It is now a *recap*: it appears only when something
 * is still outstanding, so a fully-granted machine sees one step, not two.
 */
export function WelcomeSteps({ persona, platform, onFinish }: Props) {
  const [step, setStep] = useState(0);
  const api = usePermissions();

  const p = api.perms;
  const macGates = hasSystemPermissions(platform);
  // Only nag about things this platform + persona actually uses.
  const outstanding = isTauri() && api.polled && (
    !p ||
    !p.ollama ||
    (macGates && !p.accessibility) ||
    (macGates && persona === 'dev' && !p.screen_recording)
  );

  const steps = outstanding ? [0, 1] : [0];
  const last = step === steps.length - 1;
  // Ollama down is not "optional" — don't tell the user it is.
  const broken = isTauri() && api.polled && !!p && !p.ollama;

  return (
    <div className="h-full overflow-y-auto bg-bg">
      <div className="min-h-full flex items-center justify-center py-10">
        <div className="w-full max-w-lg px-5">
          {steps.length > 1 && (
            <div className="flex items-center justify-center gap-2 mb-6">
              {steps.map((i) => (
                <span key={i} className={`h-1.5 rounded-full transition-all ${i === step ? 'w-6 bg-accent' : 'w-1.5 bg-border'}`} />
              ))}
            </div>
          )}

          {step === 0 && (
            <MemorySeed
              onNext={() => (last ? onFinish() : setStep(1))}
              onSkip={() => (last ? onFinish() : setStep(1))}
            />
          )}

          {step === 1 && (
            <div className="space-y-4">
              <div>
                <p className="text-sm text-text font-medium">
                  Still outstanding
                  {!broken && <span className="text-text-muted font-normal">&middot; optional</span>}
                </p>
                <p className="text-[11px] text-text-muted mt-1 leading-relaxed">
                  {broken
                    ? (persona === 'dev'
                      ? 'The model server is not up, so completions will fail until it is. Everything else here is optional and gates the agent loop only.'
                      : "The part that does the thinking isn't running yet, so answers won't work until it is. Anything else here is optional.")
                    : (persona === 'dev'
                      ? 'These items are not satisfied yet. None of them block chat; they gate the agent loop.'
                      : "A couple of things aren't set up yet. You can leave them \u2014 chat works either way.")}
                </p>
              </div>
              <ReadinessChecklist persona={persona} platform={platform} api={api} />
              <button
                onClick={onFinish}
                className="w-full py-2.5 bg-accent text-white text-[13px] font-medium rounded-lg hover:opacity-90 transition-opacity"
              >
                Start using LlamaChat
              </button>
              <button onClick={() => setStep(0)} className="w-full text-[11px] text-text-muted hover:text-text-secondary">
                Back
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
