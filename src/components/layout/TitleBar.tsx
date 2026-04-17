import { Sun, Moon, Monitor, Database, Settings } from 'lucide-react';
import { useState } from 'react';
import { useThemeStore } from '../../store';
import LlmSettingsModal from '../settings/LlmSettingsModal';

export default function TitleBar() {
  const { theme, setTheme } = useThemeStore();
  const [showSettings, setShowSettings] = useState(false);

  return (
    <>
      <div
        className="h-10 flex items-center justify-between px-3 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0"
        data-tauri-drag-region
      >
        <div className="flex items-center gap-2 pointer-events-none select-none">
          <div className="w-6 h-6 rounded-md bg-gradient-to-br from-brand-400 to-brand-600 flex items-center justify-center">
            <Database size={13} className="text-white" />
          </div>
          <span className="font-semibold text-sm">BaizeDB</span>
          <span className="text-xs text-[var(--text-muted)] font-mono">v0.1.0</span>
        </div>

        <div className="flex items-center gap-2">
          {/* 主题切换 */}
          <div className="flex items-center gap-0.5 bg-[var(--bg-tertiary)] rounded-md p-0.5">
            {([
              { id: 'light' as const, icon: Sun, label: '亮色' },
              { id: 'dark' as const, icon: Moon, label: '暗色' },
              { id: 'system' as const, icon: Monitor, label: '系统' },
            ] as const).map(({ id, icon: Icon, label }) => (
              <button
                key={id}
                onClick={() => setTheme(id)}
                title={label}
                className={`p-1 rounded transition-all ${
                  theme === id
                    ? 'bg-[var(--bg-secondary)] text-brand-400 shadow-sm'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
              >
                <Icon size={13} />
              </button>
            ))}
          </div>

          {/* 设置按钮 */}
          <button
            onClick={() => setShowSettings(true)}
            title="设置"
            className="p-1.5 rounded-md text-[var(--text-muted)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-tertiary)] transition-all"
          >
            <Settings size={15} />
          </button>
        </div>
      </div>

      {/* LLM 设置弹窗 */}
      {showSettings && <LlmSettingsModal onClose={() => setShowSettings(false)} />}
    </>
  );
}
