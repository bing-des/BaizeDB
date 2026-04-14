import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ConnectionConfig, Tab } from '../types';
import { connectionApi } from '../utils/api';

interface ConnectionState {
  connections: ConnectionConfig[];
  activeConnectionId: string | null;
  connectedIds: Set<string>;
  addConnection: (conn: ConnectionConfig) => void;
  removeConnection: (id: string) => void;
  setActiveConnection: (id: string | null) => void;
  setConnected: (id: string, connected: boolean) => void;
  loadFromBackend: () => Promise<void>;
}

export const useConnectionStore = create<ConnectionState>()(
  (set) => ({
    connections: [],
    activeConnectionId: null,
    connectedIds: new Set(),
    addConnection: (conn) =>
      set((state) => ({ connections: [...state.connections, conn] })),
    removeConnection: (id) =>
      set((state) => ({
        connections: state.connections.filter((c) => c.id !== id),
        activeConnectionId: state.activeConnectionId === id ? null : state.activeConnectionId,
      })),
    setActiveConnection: (id) => set({ activeConnectionId: id }),
    setConnected: (id, connected) =>
      set((state) => {
        const next = new Set(state.connectedIds);
        connected ? next.add(id) : next.delete(id);
        return { connectedIds: next };
      }),
    loadFromBackend: async () => {
      try {
        const conns = await connectionApi.list();
        set({ connections: conns });
      } catch (e) {
        console.error('从后端加载连接配置失败:', e);
      }
    },
  })
);

interface TabState {
  tabs: Tab[];
  activeTabId: string | null;
  addTab: (tab: Tab) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  updateTabContent: (id: string, content: string) => void;
  updateTabResults: (id: string, results: import('../types').QueryResult[]) => void;
  /** 关闭除指定标签外的所有标签 */
  closeOtherTabs: (keepId: string) => void;
  /** 关闭右侧所有标签 */
  closeRightTabs: (leftId: string) => void;
  /** 关闭全部标签 */
  clearAllTabs: () => void;
}

export const useTabStore = create<TabState>((set) => ({
  tabs: [],
  activeTabId: null,
  addTab: (tab) =>
    set((state) => ({ tabs: [...state.tabs, tab], activeTabId: tab.id })),
  removeTab: (id) =>
    set((state) => {
      const idx = state.tabs.findIndex((t) => t.id === id);
      const newTabs = state.tabs.filter((t) => t.id !== id);
      let newActiveId = state.activeTabId;
      if (state.activeTabId === id) {
        newActiveId = newTabs.length > 0 ? newTabs[Math.max(0, idx - 1)].id : null;
      }
      return { tabs: newTabs, activeTabId: newActiveId };
    }),
  setActiveTab: (id) => set({ activeTabId: id }),
  updateTabContent: (id, content) =>
    set((state) => ({
      tabs: state.tabs.map((t) => (t.id === id ? { ...t, content } : t)),
    })),
  updateTabResults: (id, results) =>
    set((state) => ({
      tabs: state.tabs.map((t) => (t.id === id ? { ...t, results } : t)),
    })),
  closeOtherTabs: (keepId) =>
    set((state) => ({
      tabs: state.tabs.filter((t) => t.id === keepId),
      activeTabId: keepId,
    })),
  closeRightTabs: (leftId) =>
    set((state) => {
      const idx = state.tabs.findIndex((t) => t.id === leftId);
      if (idx < 0) return state;
      const newTabs = state.tabs.slice(0, idx + 1);
      let newActiveId = state.activeTabId;
      if (!newTabs.find((t) => t.id === state.activeTabId)) {
        newActiveId = newTabs[newTabs.length - 1].id ?? null;
      }
      return { tabs: newTabs, activeTabId: newActiveId };
    }),
  clearAllTabs: () => set({ tabs: [], activeTabId: null }),
}));

interface ThemeState {
  theme: 'light' | 'dark' | 'system';
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: 'dark',
      setTheme: (theme) => set({ theme }),
    }),
    { name: 'baizedb-theme' }
  )
);
