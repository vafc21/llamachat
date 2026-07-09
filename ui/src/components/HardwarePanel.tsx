import { type HardwareProfile } from '../api'

export function HardwarePanel({ hardware }: { hardware: HardwareProfile }) {
  const { cpu, gpus, memory, storage, os, backends } = hardware;

  const gpuVram = gpus.reduce((sum, g) => sum + (g.vram_total_mb ?? 0), 0);
  const freeVram = gpus.reduce((sum, g) => sum + (g.vram_free_mb ?? 0), 0);

  return (
    <div className="bg-fitllm-card border border-fitllm-border rounded-xl p-5">
      <div className="flex items-center gap-2 mb-4">
        <span className="text-sm">🖥️</span>
        <h2 className="text-sm font-semibold text-fitllm-text uppercase tracking-wide">
          Your Machine
        </h2>
        <span className="text-[10px] text-fitllm-muted ml-auto">
          detected {new Date(hardware.detected_at).toLocaleTimeString()}
        </span>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <Stat
          label="CPU"
          value={cpu.model}
          detail={`${cpu.physical_cores}C/${cpu.logical_cores}T · ${cpu.max_clock_mhz ? (cpu.max_clock_mhz / 1000).toFixed(1) + ' GHz' : '—'}`}
        />
        <Stat
          label="GPU"
          value={gpus.length > 0 ? gpus[0].model : 'None detected'}
          detail={gpus.length > 0 ? `${(gpuVram / 1024).toFixed(0)}GB VRAM · ${gpus[0].backend.toUpperCase()}` : 'CPU-only'}
        />
        <Stat
          label="RAM"
          value={`${(memory.total_mb / 1024).toFixed(0)} GB`}
          detail={`${(memory.available_mb / 1024).toFixed(0)} GB available`}
        />
        <Stat
          label="Storage"
          value={`${(storage.free_mb / 1024).toFixed(0)} GB free`}
          detail={os.name}
        />
      </div>

      {/* Details row */}
      <div className="mt-4 pt-4 border-t border-fitllm-border grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
        <Detail label="OS" value={`${os.name} ${os.version} (${os.arch})`} />
        <Detail
          label="CPU Flags"
          value={[
            cpu.flags.avx2 && 'AVX2',
            cpu.flags.avx512 && 'AVX-512',
            cpu.flags.fma && 'FMA',
            cpu.flags.f16c && 'F16C',
            cpu.flags.neon && 'NEON',
          ]
            .filter(Boolean)
            .join(' · ') || 'none'}
        />
        <Detail label="Backends" value={backends.map((b) => b.toUpperCase()).join(' · ')} />
        <Detail
          label="GPU Details"
          value={
            gpus.length > 0
              ? [
                  gpus[0].cuda_version && `CUDA ${gpus[0].cuda_version}`,
                  gpus[0].compute_capability && `CC ${gpus[0].compute_capability}`,
                  `Free ${(freeVram / 1024).toFixed(1)}GB`,
                ]
                  .filter(Boolean)
                  .join(' · ')
              : '—'
          }
        />
      </div>
    </div>
  );
}

function Stat({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div>
      <div className="text-[10px] text-fitllm-muted uppercase tracking-wider mb-1">{label}</div>
      <div className="text-sm font-medium text-fitllm-text truncate" title={value}>{value}</div>
      <div className="text-[11px] text-fitllm-muted mt-0.5">{detail}</div>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="text-fitllm-muted">{label}: </span>
      <span className="text-fitllm-text">{value}</span>
    </div>
  );
}
