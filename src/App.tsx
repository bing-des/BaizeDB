import { useEffect } from 'react';
import { useThemeStore, useConnectionStore } from './store';
import MainLayout from './components/layout/MainLayout';

function App() {
  const { theme } = useThemeStore();
  const loadFromBackend = useConnectionStore((s) => s.loadFromBackend);

  // 主题
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
    } else if (theme === 'light') {
      root.classList.remove('dark');
    } else {
      const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      isDark ? root.classList.add('dark') : root.classList.remove('dark');
    }
  }, [theme]);

  // 启动时从后端加载连接配置（后端已从磁盘读取）
  useEffect(() => {
    loadFromBackend();
  }, [loadFromBackend]);

  return <MainLayout />;
}

export default App;
