import { useState, useCallback } from 'react';
import { X, TerminalSquare, Table2, Key, RefreshCw, Trash2, PanelRightClose, PanelsTopLeft, CircleX } from 'lucide-react';
import { useTabStore } from '../../store';
import QueryEditor from './QueryEditor';
import TableViewer from '../table/TableViewer';
import RedisKeyViewer from '../redis/RedisKeyViewer';
import ContextMenu from '../common/ContextMenu';

/** 自定义刷新事件，子组件通过 dispatchEvent 触发 */
const TAB_REFRESH_EVENT = `tab-refresh`;

export default function TabsArea() {
  const { tabs, activeTabId, removeTab, setActiveTab, closeOtherTabs, closeRightTabs, clearAllTabs } = useTabStore();
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; tabId: string } | null>(null);

  const handleContextMenu = (e: React.MouseEvent, tabId: string) => {
    e.preventDefault();
    setActiveTab(tabId);
    setContextMenu({ x: e.clientX, y: e.clientY, tabId });
  };

  /** 向当前激活的标签页内容发送刷新事件 */
  const refreshActiveTab = useCallback(() => {
    if (!activeTabId) return;
    window.dispatchEvent(new CustomEvent(`${TAB_REFRESH_EVENT}-${activeTabId}`));
  }, [activeTabId]);

  // 右键菜单项
  const menuItems = contextMenu
    ? [
        {
          label: '关闭',
          icon: <X size={14} />,
          onClick: () => removeTab(contextMenu.tabId),
        },
        {
          label: '刷新',
          icon: <RefreshCw size={14} />,
          onClick: () => {
            refreshActiveTab();
          },
        },
        { separator: true as const },
        {
          label: '关闭其他',
          icon: <PanelsTopLeft size={14} />,
          onClick: () => closeOtherTabs(contextMenu.tabId),
        },
        {
          label: '关闭右侧',
          icon: <PanelRightClose size={14} />,
          disabled: tabs[tabs.length - 1]?.id === contextMenu.tabId,
          onClick: () => closeRightTabs(contextMenu.tabId),
        },
        { separator: true as const },
        {
          label: '关闭全部',
          icon: <CircleX size={14} />,
          danger: true,
          onClick: () => clearAllTabs(),
        },
      ]
    : [];

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
            onContextMenu={(e) => handleContextMenu(e, tab.id)}
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
          <TabContent key={tab.id} tab={tab} active={activeTabId === tab.id} />
        ))}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <ContextMenu x={contextMenu.x} y={contextMenu.y} items={menuItems} onClose={() => setContextMenu(null)} />
      )}
    </div>
  );
}

/** 单个标签页内容，监听刷新事件并转发给子组件 ref */
function TabContent({ tab, active }: { tab: import('../../types').Tab; active: boolean }) {
  const [refreshKey, setRefreshKey] = useState(0);

  // 监听来自 TabsArea 的刷新事件
  useState(() => {
    const handler = () => setRefreshKey((k) => k + 1);
    window.addEventListener(`${TAB_REFRESH_EVENT}-${tab.id}`, handler);
    return () => window.removeEventListener(`${TAB_REFRESH_EVENT}-${tab.id}`, handler);
  });

  if (!active) return null;

  return (
    <div className="h-full" key={refreshKey}>
      {tab.type === 'query'
        ? <QueryEditor tab={tab} />
        : tab.type === 'redis-key'
          ? <RedisKeyViewer tab={tab} />
          : <TableViewer tab={tab} />}
    </div>
  );
}
