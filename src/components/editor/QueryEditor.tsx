import { useState, useCallback, useRef } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { sql, MySQL, PostgreSQL } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';
import { EditorView } from '@codemirror/view';
import { Play, Loader2, Download, Copy, CheckCircle, ChevronDown, Square, Maximize2 } from 'lucide-react';
import { useThemeStore, useConnectionStore } from '../../store';
import { queryApi } from '../../utils/api';
import { parseSql, getSqlStatementsFromSelection, getSqlStatementsFromCurrentLine } from '../../utils/sqlParser';
import type { Tab, QueryResult } from '../../types';
import ResultTable from './ResultTable';

const lightTheme = EditorView.theme({
  '&': { background: '#f8fafc', color: '#1e293b' },
  '.cm-content': { caretColor: '#0ea5e9' },
  '.cm-gutters': { background: '#f1f5f9', border: 'none', borderRight: '1px solid #e2e8f0', color: '#94a3b8' },
  '.cm-activeLineGutter': { background: '#e8f4fd' },
  '.cm-activeLine': { background: '#f0f9ff' },
  '.cm-selectionBackground, .cm-focused .cm-selectionBackground': { background: '#bae6fd !important' },
});

export default function QueryEditor({ tab }: { tab: Tab }) {
  const [code, setCode] = useState(tab.content ?? '');
  const [results, setResults] = useState<QueryResult[]>([]);
  const [activeResultIndex, setActiveResultIndex] = useState(0);
  const [running, setRunning] = useState(false);
  const [copied, setCopied] = useState(false);
  const [runMode, setRunMode] = useState<'current-line' | 'selection' | 'all'>('current-line');
  const [showRunOptions, setShowRunOptions] = useState(false);
  const editorViewRef = useRef<EditorView | null>(null);
  const { theme } = useThemeStore();
  const { connections } = useConnectionStore();

  const conn = connections.find((c) => c.id === tab.connectionId);
  const dialect = conn?.db_type === 'postgresql' ? PostgreSQL : MySQL;
  const isDark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  const getEditorState = useCallback(() => {
    if (!editorViewRef.current) {
      return null;
    }
    const view = editorViewRef.current;
    return {
      view,
      selection: view.state.selection,
      fullText: view.state.doc.toString(),
    };
  }, []);

  const getSqlStatementsToExecute = useCallback(() => {
    const editorState = getEditorState();
    if (!editorState) {
      return parseSql(code);
    }

    const { view, selection, fullText } = editorState;

    switch (runMode) {
      case 'current-line': {
        // 执行当前行
        const cursorPos = selection.main.head;
        return getSqlStatementsFromCurrentLine(fullText, cursorPos);
      }
      case 'selection': {
        // 执行选中部分
        if (!selection.main.empty) {
          const selectedText = view.state.sliceDoc(selection.main.from, selection.main.to);
          return parseSql(selectedText);
        }
        // 如果没有选中，回退到当前行
        const cursorPos = selection.main.head;
        return getSqlStatementsFromCurrentLine(fullText, cursorPos);
      }
      case 'all': {
        // 执行全部
        return parseSql(fullText);
      }
      default:
        return [];
    }
  }, [code, runMode, getEditorState]);

  const handleRun = useCallback(async () => {
    const statements = getSqlStatementsToExecute();
    if (statements.length === 0) return;
    
    setRunning(true);
    setResults([]);
    setActiveResultIndex(0);
    
    const allResults: QueryResult[] = [];
    let totalExecutionTime = 0;
    let totalAffectedRows = 0;
    
    try {
      for (let i = 0; i < statements.length; i++) {
        const statement = statements[i];
        if (!statement.trim()) continue;
        
        try {
          const res = await queryApi.execute(tab.connectionId, statement, tab.database);
          console.log(`Statement ${i + 1}/${statements.length} result:`, res);
          
          // 累加执行时间
          totalExecutionTime += res.execution_time_ms || 0;
          
          // 累加影响行数
          if (res.affected_rows != null) {
            totalAffectedRows += res.affected_rows;
          }
          
          // 将结果存入数组
          allResults.push(res);
          
          // 如果有错误，停止执行并设置结果
          if (res.error) {
            setResults([...allResults]); // 包括错误结果
            setRunning(false);
            return;
          }
        } catch (e) {
          // 语句执行出错
          const errorResult: QueryResult = {
            columns: [],
            rows: [],
            execution_time_ms: totalExecutionTime,
            error: `语句 ${i + 1}/${statements.length} 执行失败: ${e}`
          };
          allResults.push(errorResult);
          setResults([...allResults]);
          setRunning(false);
          return;
        }
      }
      
      // 所有语句执行成功
      setResults(allResults);
    } finally {
      setRunning(false);
    }
  }, [getSqlStatementsToExecute, tab.connectionId, tab.database]);

  const handleCopy = async () => {
    if (results.length === 0) return;
    // 复制当前tab的结果
    const currentResult = results[activeResultIndex];
    if (!currentResult || currentResult.columns.length === 0) return;
    const text = [currentResult.columns.join('\t'), ...currentResult.rows.map((r) => r.map((v) => v ?? 'NULL').join('\t'))].join('\n');
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleExport = () => {
    if (results.length === 0) return;
    // 导出当前tab的结果
    const currentResult = results[activeResultIndex];
    if (!currentResult || currentResult.columns.length === 0) return;
    const lines = [
      currentResult.columns.join(','),
      ...currentResult.rows.map((r) => r.map((v) => v === null ? '' : `"${String(v).replace(/"/g, '""')}"`).join(',')),
    ];
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([lines.join('\n')], { type: 'text/csv' }));
    a.download = 'result.csv';
    a.click();
  };

  const hasData = results.length > 0 && !results[activeResultIndex]?.error && results[activeResultIndex]?.columns.length > 0;

  return (
    <div
      className="h-full flex flex-col"
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); handleRun(); }
      }}
    >
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
        {/* 运行下拉菜单 - 优质样式 */}
        <div className="relative z-50">
          <div className="run-button-group">
            <button
              className="run-main-btn"
              onClick={handleRun}
              disabled={running || !code.trim()}
              title={`运行${runMode === 'current-line' ? '当前行' : runMode === 'selection' ? '选中' : '全部'} (Ctrl+Enter)`}
            >
              {running ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />}
              {running ? '执行中' : (runMode === 'current-line' ? '运行当前行' : runMode === 'selection' ? '运行选中' : '运行全部')}
            </button>
            <button
              className="run-dropdown-btn"
              onClick={(e) => { e.stopPropagation(); setShowRunOptions(!showRunOptions); }}
              disabled={running}
              title="选择运行模式"
            >
              <ChevronDown size={13} />
            </button>
          </div>
          
          {/* 优质下拉菜单 */}
          {showRunOptions && (
            <div className="premium-dropdown">
              <div className="py-1">
                <button
                  className={`dropdown-item ${runMode === 'current-line' ? 'active' : ''}`}
                  onClick={(e) => { e.stopPropagation(); setRunMode('current-line'); setShowRunOptions(false); }}
                >
                  <div className="icon">
                    <Square size={12} />
                  </div>
                  <span>运行当前行</span>
                  {runMode === 'current-line' && <span className="run-mode-badge">当前</span>}
                </button>
                <button
                  className={`dropdown-item ${runMode === 'selection' ? 'active' : ''}`}
                  onClick={(e) => { e.stopPropagation(); setRunMode('selection'); setShowRunOptions(false); }}
                >
                  <div className="icon">
                    <Maximize2 size={12} />
                  </div>
                  <span>运行选中</span>
                  {runMode === 'selection' && <span className="run-mode-badge">当前</span>}
                </button>
                <button
                  className={`dropdown-item ${runMode === 'all' ? 'active' : ''}`}
                  onClick={(e) => { e.stopPropagation(); setRunMode('all'); setShowRunOptions(false); }}
                >
                  <div className="icon">
                    <Play size={12} />
                  </div>
                  <span>运行全部</span>
                  {runMode === 'all' && <span className="run-mode-badge">当前</span>}
                </button>
              </div>
              <div className="border-t border-[var(--border)] px-4 py-3 text-xs text-[var(--text-muted)] bg-[var(--bg-tertiary)]">
                {runMode === 'current-line' && '执行光标所在行（选中时执行选中部分）'}
                {runMode === 'selection' && '执行选中内容（忽略注释，按分号分隔）'}
                {runMode === 'all' && '执行全部 SQL（忽略注释，按分号分隔逐条执行）'}
              </div>
            </div>
          )}
        </div>

        {/* 点击外部关闭下拉菜单 */}
        {showRunOptions && (
          <div className="fixed inset-0 z-30" onClick={() => setShowRunOptions(false)} />
        )}

        <div className="h-4 w-px bg-[var(--border)]" />

        <div className="flex items-center gap-1.5 text-xs text-[var(--text-muted)]">
          <span
            className="px-1.5 py-0.5 rounded text-[10px] font-medium"
            style={{
              background: conn?.db_type === 'mysql' ? 'rgba(68,121,161,0.2)' : 'rgba(51,103,145,0.2)',
              color: conn?.db_type === 'mysql' ? '#7ab5d4' : '#6baed0',
            }}
          >
            {conn?.db_type === 'mysql' ? 'MySQL' : 'PostgreSQL'}
          </span>
          <span>{conn?.name ?? '未知连接'}</span>
          {tab.database && <><span>/</span><span className="text-brand-400">{tab.database}</span></>}
        </div>

        <div className="flex-1" />

        {results.length > 0 && (
          <div className="flex items-center gap-2">
            {hasData && (
              <>
                <button className="btn-ghost py-1 text-xs" onClick={handleCopy}>
                  {copied ? <CheckCircle size={12} className="text-green-400" /> : <Copy size={12} />}
                  {copied ? '已复制' : '复制'}
                </button>
                <button className="btn-ghost py-1 text-xs" onClick={handleExport}>
                  <Download size={12} /> CSV
                </button>
              </>
            )}
            <span className="text-xs text-[var(--text-muted)]">
              {results.length === 1 ? (
                <>
                  {results[0].error ? (
                    <span className="text-red-400">失败</span>
                  ) : results[0].affected_rows != null ? (
                    <span className="text-green-400">影响 {results[0].affected_rows} 行</span>
                  ) : (
                    `${results[0].rows.length} 行`
                  )}
                  <span className="ml-1.5">{results[0].execution_time_ms}ms</span>
                </>
              ) : (
                <>
                  <span className="text-brand-400">共 {results.length} 个结果</span>
                  <span className="ml-1.5">
                    {results.filter(r => r.error).length > 0 
                      ? <span className="text-red-400">有错误</span>
                      : <span className="text-green-400">全部成功</span>}
                  </span>
                  <span className="ml-1.5">|</span>
                  <span className="ml-1.5">{results[activeResultIndex]?.execution_time_ms}ms</span>
                </>
              )}
            </span>
          </div>
        )}
      </div>

      {/* Editor area */}
      <div className={`overflow-hidden ${results.length > 0 ? 'flex-none' : 'flex-1'}`} style={{ height: results.length > 0 ? '55%' : '100%' }}>
        <CodeMirror
          value={code}
          onChange={setCode}
          extensions={[sql({ dialect })]}
          theme={isDark ? oneDark : lightTheme}
          height="100%"
          style={{ height: '100%', fontSize: '13px' }}
          basicSetup={{
            lineNumbers: true,
            foldGutter: true,
            autocompletion: true,
            bracketMatching: true,
            closeBrackets: true,
            highlightActiveLine: true,
          }}
          onCreateEditor={(view) => { editorViewRef.current = view; }}
          placeholder="-- 输入 SQL，Ctrl+Enter 执行当前行（选中内容则执行选中部分）"
        />
      </div>

      {/* Results */}
      {results.length > 0 && (
        <div className="flex-1 overflow-hidden border-t border-[var(--border)] flex flex-col min-h-0">
          {/* Results header with tabs */}
          <div className="flex items-center px-3 py-1.5 border-b border-[var(--border)] bg-[var(--bg-secondary)] text-xs flex-shrink-0">
            <span className="text-[var(--text-secondary)] font-medium">
              {results.length === 1 ? (
                results[0].error ? '错误输出' : `查询结果  ·  ${results[0].rows.length} 行`
              ) : (
                <span className="flex items-center gap-1.5">
                  <span>执行结果</span>
                  <span className="px-1.5 py-0.5 bg-brand-500/10 text-brand-600 dark:text-brand-400 text-[10px] rounded">
                    {results.length} 个语句
                  </span>
                </span>
              )}
            </span>
            <div className="flex-1" />
            {results.length > 1 && (
              <div className="flex items-center gap-2 text-[11px] text-[var(--text-muted)]">
                <span className={`px-1.5 py-0.5 rounded ${results.filter(r => r.error).length === 0 ? 'bg-green-500/10 text-green-600 dark:text-green-400' : 'bg-red-500/10 text-red-600 dark:text-red-400'}`}>
                  {results.filter(r => r.error).length === 0 ? '全部成功' : `${results.filter(r => r.error).length} 个失败`}
                </span>
                <span className="px-1.5 py-0.5 bg-blue-500/10 text-blue-600 dark:text-blue-400 rounded">
                  总计 {results.reduce((sum, r) => sum + (r.execution_time_ms || 0), 0)}ms
                </span>
              </div>
            )}
          </div>

          {/* Tab navigation */}
          {results.length > 1 && (
            <div className="flex items-center gap-1 px-3 py-1.5 border-b border-[var(--border)] bg-[var(--bg-tertiary)] flex-shrink-0">
              {results.map((result, index) => (
                <button
                  key={index}
                  onClick={() => setActiveResultIndex(index)}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150 flex items-center gap-2 ${
                    activeResultIndex === index
                      ? 'bg-[var(--bg-primary)] text-brand-500 shadow-sm'
                      : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-secondary)]'
                  }`}
                >
                  <span className={`w-2 h-2 rounded-full ${result.error ? 'bg-red-400' : result.affected_rows != null ? 'bg-green-400' : 'bg-brand-400'}`} />
                  <span>语句 {index + 1}</span>
                  <span className="text-[10px] opacity-70">
                    {result.error ? '失败' : result.affected_rows != null ? `${result.affected_rows}行` : `${result.rows.length}行`}
                  </span>
                </button>
              ))}
            </div>
          )}

          {/* Single result header */}
          {results.length === 1 && (
            <div className="px-3 py-1.5 border-b border-[var(--border)] bg-[var(--bg-tertiary)] flex-shrink-0">
              <div className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-2">
                  <span className={`w-2 h-2 rounded-full ${results[0].error ? 'bg-red-400' : results[0].affected_rows != null ? 'bg-green-400' : 'bg-brand-400'}`} />
                  <span className="text-[var(--text-secondary)] font-medium">
                    语句 1
                  </span>
                  {results[0].error ? (
                    <span className="text-red-400">失败</span>
                  ) : results[0].affected_rows != null ? (
                    <span className="text-green-400">影响 {results[0].affected_rows} 行</span>
                  ) : (
                    <span className="text-brand-400">{results[0].rows.length} 行</span>
                  )}
                </div>
                <span className="text-[var(--text-muted)]">
                  {results[0].execution_time_ms}ms
                </span>
              </div>
            </div>
          )}

          {/* Result content */}
          <div className="flex-1 overflow-auto">
            {(() => {
              const result = results[activeResultIndex];
              if (!result) return null;
              return result.error ? (
                <div className="px-4 py-3 flex items-start gap-2">
                  <span className="text-red-400 mt-0.5 flex-shrink-0">✗</span>
                  <pre className="text-red-400 text-sm font-mono whitespace-pre-wrap break-words">{result.error}</pre>
                </div>
              ) : result.affected_rows != null ? (
                <div className="flex items-center gap-2 px-4 py-3 text-green-400 text-sm">
                  <CheckCircle size={16} />
                  执行成功，影响 {result.affected_rows} 行
                </div>
              ) : (
                <ResultTable columns={result.columns} rows={result.rows} />
              );
            })()}
          </div>
        </div>
      )}
    </div>
  );
}
