import type { Conversation, HardwareProfile } from '../types'

// Tauri platform type
declare global {
  interface Window {
    __TAURI__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  }
}

interface SidebarProps {
  open: boolean;
  conversations: Conversation[];
  activeId: string;
  hardware: HardwareProfile | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onToggle: () => void;
}

export function Sidebar({
  open,
  conversations,
  activeId,
  hardware,
  onSelect,
  onNew,
  onDelete,
  onToggle,
}: SidebarProps) {
  if (!open) return null;

  return (
    <div className="flex-shrink-0 bg-sidebar border-r border-border flex flex-col"
         style={{ width: 'var(--platform-sidebar-width)' }}>
      {/* Header */}
      <div className="flex-shrink-0 h-9 border-b border-border flex items-center px-3 gap-2">
        <span className="text-xs font-semibold tracking-wide text-text-secondary">
          FitLLM
        </span>
        <span className="text-[10px] text-text-muted">v0.1</span>
        <button
          onClick={onToggle}
          className="ml-auto text-text-muted hover:text-text-secondary p-0.5"
          title="Close sidebar"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path d="M11 4L5 12M5 4l6 8" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
      </div>

      {/* New conversation */}
      <div className="p-2">
        <button
          onClick={onNew}
          className="w-full flex items-center gap-2 px-2 py-1.5 text-[12px] text-text-secondary
                     border border-border rounded hover:border-border-strong hover:text-text
                     transition-colors"
        >
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
            <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="1.5" />
          </svg>
          New conversation
        </button>
      </div>

      {/* Conversation list */}
      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {conversations.map((conv) => (
          <div
            key={conv.id}
            onClick={() => onSelect(conv.id)}
            className={`group flex items-center gap-2 px-2 py-1.5 rounded text-[12px] cursor-pointer
              transition-colors ${
                conv.id === activeId
                  ? 'bg-accent-dim text-text'
                  : 'text-text-secondary hover:text-text hover:bg-white/[0.03]'
              }`}
          >
            <span className="truncate flex-1">{conv.title}</span>
            {conv.id !== '1' && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(conv.id);
                }}
                className="opacity-0 group-hover:opacity-100 text-text-muted hover:text-error
                           p-0.5 transition-all"
                title="Delete"
              >
                <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                  <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" />
                </svg>
              </button>
            )}
          </div>
        ))}

        {conversations.length === 0 && (
          <div className="text-[11px] text-text-muted text-center py-8">
            No conversations yet
          </div>
        )}
      </div>

      {/* Footer: hardware summary */}
      {hardware && (
        <div className="flex-shrink-0 border-t border-border p-2">
          <div className="text-[10px] text-text-muted space-y-0.5">
            <div className="flex justify-between">
              <span>{hardware.cpu.model.split(' ').slice(0, 3).join(' ')}</span>
              <span>{hardware.cpu.physical_cores}C</span>
            </div>
            <div className="flex justify-between">
              <span>{hardware.gpus[0]?.model ?? 'CPU only'}</span>
              <span>
                {hardware.gpus[0]?.vram_total_mb
                  ? `${(hardware.gpus[0].vram_total_mb / 1024).toFixed(0)}GB`
                  : ''}
              </span>
            </div>
            <div className="flex justify-between">
              <span>RAM</span>
              <span>{(hardware.memory.total_mb / 1024).toFixed(0)}GB</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
