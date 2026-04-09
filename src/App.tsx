import { useEffect } from 'react';
import { useThemeStore } from './store';
import MainLayout from './components/layout/MainLayout';

function App() {
  const { theme } = useThemeStore();

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

  return <MainLayout />;
}

export default App;
