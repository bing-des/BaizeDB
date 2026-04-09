import { X, TerminalSquare, Table2, Key } from 'lucide-react';
import { useTabStore } from '../../store';
import QueryEditor from './QueryEditor';
import TableViewer from '../table/TableViewer';
import RedisKeyViewer from '../redis/RedisKeyViewer';

export default function TabsArea() {
  const { tabs, activeTabId, removeTab, setActiveTab } = useTabStore();

  if (tabs.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-3 text-[var(--text-muted)]">
        <div className="w-14 h-14 rounded-2xl bg-[var(--bg-secondary)] border border-[var(--border)] flex items-center justify-center">
          <TerminalSquare size={26} strokeWidth={1} />
        </div>
        <div className="text-center">
          <p className="text-sm font-medium text-[var(--text-secondary)]">从左侧选择表或打开查询</p>
          <p className="text-xs mt-1 text-[var(--text-muted)]">点击表名查看数据，点击 SQL 图标打开编辑器</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Tab bar */}
      <div className="flex items-end border-b border-[var(--border)] bg-[var(--bg-secondary)] overflow-x-auto flex-shrink-0 px-1 pt-1">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className={`tab-item flex-shrink-0 ${activeTabId === tab.id ? 'active' : ''}`}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.type === 'query' ? (
              <TerminalSquare size={12} className="text-brand-400" />
            ) : tab.type === 'redis-key' ? (
              <Key size={12} className="text-red-400" />
            ) : (
              <Table2 size={12} className="text-purple-400" />
            )}
            <span className="max-w-[140px] truncate">{tab.title}</span>
            <button
              className="ml-1 p-0.5 rounded hover:bg-[var(--bg-tertiary)] hover:text-red-400 opacity-40 hover:opacity-100 transition-opacity"
              onClick={(e) => { e.stopPropagation(); removeTab(tab.id); }}
            >
              <X size={10} />
            </button>
          </div>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {tabs.map((tab) => (
          <div key={tab.id} className={`h-full ${activeTabId === tab.id ? 'block' : 'hidden'}`}>
            {tab.type === 'query' ? <QueryEditor tab={tab} /> : tab.type === 'redis-key' ? <RedisKeyViewer tab={tab} /> : <TableViewer tab={tab} />}
          </div>
        ))}
      </div>
    </div>
  );
}
