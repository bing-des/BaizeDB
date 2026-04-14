import { createPortal } from 'react-dom';
import { AlertTriangle } from 'lucide-react';

interface Props {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
  danger?: boolean;
}

export default function ConfirmModal({ message, onConfirm, onCancel, danger = true }: Props) {
  return createPortal(
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm animate-in fade-in duration-150"
      onClick={onCancel}
    >
      <div
        className="min-w-[320px] max-w-[480px] p-5 rounded-xl border border-[var(--border)] bg-[var(--bg-secondary)] shadow-2xl animate-in zoom-in-95 duration-150"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-3">
          {danger && (
            <div className="flex-shrink-0 w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center">
              <AlertTriangle size={20} className="text-red-400" />
            </div>
          )}
          <div className="flex-1 pt-1">
            <p className="text-sm text-[var(--text-primary)] leading-relaxed">{message}</p>
          </div>
        </div>
        <div className="flex justify-end gap-2 mt-5">
          <button
            className="px-4 py-2 text-sm rounded-lg border border-[var(--border)] bg-[var(--bg-tertiary)] text-[var(--text-secondary)] hover:bg-[var(--bg-secondary)] transition-colors"
            onClick={onCancel}
          >
            取消
          </button>
          <button
            className={`px-4 py-2 text-sm rounded-lg font-medium transition-colors ${
              danger
                ? 'bg-red-500 text-white hover:bg-red-600'
                : 'bg-brand-500 text-white hover:bg-brand-600'
            }`}
            onClick={onConfirm}
          >
            确认
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
