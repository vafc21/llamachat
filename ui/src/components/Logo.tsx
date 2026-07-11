/** FitLLM mark — the "F" monogram on a blue→violet squircle. Matches the app icon. */
export function Logo({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" aria-label="FitLLM" className="flex-shrink-0">
      <defs>
        <linearGradient id="fitllm-logo-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#5b8cff" />
          <stop offset="0.55" stopColor="#5566ff" />
          <stop offset="1" stopColor="#7c4dff" />
        </linearGradient>
      </defs>
      <rect x="0" y="0" width="1024" height="1024" rx="232" fill="url(#fitllm-logo-bg)" />
      <g fill="#ffffff">
        <rect x="352" y="296" width="120" height="432" rx="34" />
        <rect x="352" y="296" width="346" height="120" rx="34" />
        <rect x="352" y="474" width="252" height="112" rx="34" />
        <circle cx="726" cy="300" r="26" />
      </g>
    </svg>
  );
}
