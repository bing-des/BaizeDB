import { useState, useEffect } from 'react';
import { Plus, Copy } from 'lucide-react';
import ConnectionTree from './ConnectionTree';
import ConnectionDialog from '../connection/ConnectionDialog';
import MigrationDialog from '../migration/MigrationDialog';

export default function Sidebar() {
  const [showDialog, setShowDialog] = useState(false);
  const [showMigrationDialog, setShowMigrationDialog] = useState(false);

  // 监听右键菜单的「新建连接」事件
  useEffect(() => {
    const handler = () => setShowDialog(true);
    window.addEventListener('baizedb:new-connection', handler);
    return () => window.removeEventListener('baizedb:new-connection', handler);
  }, []);

  return (
    <div className="h-full flex flex-col bg-[var(--bg-secondary)] border-r border-[var(--border)]">
      <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border)] flex-shrink-0">
        <span className="text-xs font-semibold uppercase tracking-wider text-[var(--text-muted)]">
          数据库连接
        </span>
        <div className="flex gap-1">
          <button
            className="btn-ghost p-1"
            title="数据迁移"
            onClick={() => setShowMigrationDialog(true)}
          >
            <Copy size={14} />
          </button>
          <button
            className="btn-ghost p-1"
            title="新建连接"
            onClick={() => setShowDialog(true)}
          >
            <Plus size={14} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto py-1">
        <ConnectionTree />
      </div>

      {showDialog && <ConnectionDialog onClose={() => setShowDialog(false)} />}
      {showMigrationDialog && <MigrationDialog onClose={() => setShowMigrationDialog(false)} />}
    </div>
  );
}
