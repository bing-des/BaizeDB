import { useState, useEffect, useCallback } from 'react';
import { RefreshCw, ChevronLeft, ChevronRight, Loader2, Table2, Download, Columns3 } from 'lucide-react';
import { databaseApi } from '../../utils/api';
import type { Tab, ColumnInfo } from '../../types';
import ResultTable from '../editor/ResultTable';

export default function TableViewer({ tab }: { tab: Tab }) {
  const [columns, setColumns] = useState<string[]>([]);
  const [rows, setRows] = useState<(string | number | boolean | null)[][]>([]);
  const [colInfos, setColInfos] = useState<ColumnInfo[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const pageSize = 200;
  const [loading, setLoading] = useState(false);
  const [panel, setPanel] = useState<'data' | 'columns'>('data');

  const { connectionId, database, table } = tab;

  const loadData = useCallback(async (p: number) => {
    if (!database || !table) return;
    setLoading(true);
    try {
      console.log(`[TableViewer] loadData connectionId=${connectionId} database=${database} table=${table} page=${p} pageSize=${pageSize}`);
      const r = await databaseApi.getTableData(connectionId, database, table, p, pageSize);
      console.log(`[TableViewer] loaded columns=${r.columns.length} rows=${r.rows.length} total=${r.total}`);
      setColumns(r.columns);
      setRows(r.rows);
      setTotal(r.total);
    } catch (e) {
      console.error(`[TableViewer] loadData error:`, e);
    } finally {
      setLoading(false);
    }
  }, [connectionId, database, table, pageSize]);

  useEffect(() => {
    loadData(1);
    if (database && table) {
      databaseApi.listColumns(connectionId, database, table)
        .then((cols) => { console.log(`[TableViewer] listColumns columns=${cols.length}`); setColInfos(cols); })
        .catch((e) => { console.error(`[TableViewer] listColumns error:`, e); });
    }
  }, []);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const go = (p: number) => { setPage(p); loadData(p); };

  const exportCSV = () => {
    const lines = [columns.join(','), ...rows.map((r) => r.map((v) => v === null ? '' : `"${String(v).replace(/"/g, '""')}"`).join(','))];
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([lines.join('\n')], { type: 'text/csv' }));
    a.download = `${table}.csv`;
    a.click();
  };

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
        <Table2 size={14} className="text-purple-400" />
        <span className="font-medium text-sm text-[var(--text-primary)]">
          <span className="text-[var(--text-muted)]">{database}.</span>
          <span className="text-purple-300">{table}</span>
        </span>

        <div className="h-4 w-px bg-[var(--border)]" />

        <button
          className={`btn-ghost py-1 text-xs ${panel === 'data' ? 'text-brand-400' : ''}`}
          onClick={() => setPanel('data')}
        >
          数据
        </button>
        <button
          className={`btn-ghost py-1 text-xs ${panel === 'columns' ? 'text-brand-400' : ''}`}
          onClick={() => setPanel('columns')}
        >
          <Columns3 size={12} /> 结构
        </button>

        <div className="flex-1" />

        {panel === 'data' && (
          <div className="flex items-center gap-1.5">
            <button className="btn-ghost py-1 text-xs" onClick={() => loadData(page)} disabled={loading}>
              <RefreshCw size={12} className={loading ? 'animate-spin' : ''} />
            </button>
            <button className="btn-ghost py-1 text-xs" onClick={exportCSV}>
              <Download size={12} /> CSV
            </button>
            <div className="flex items-center gap-0.5 text-xs text-[var(--text-muted)]">
              <button className="btn-ghost p-1" onClick={() => go(page - 1)} disabled={page <= 1 || loading}>
                <ChevronLeft size={12} />
              </button>
              <span className="px-1.5">{page} / {totalPages}</span>
              <button className="btn-ghost p-1" onClick={() => go(page + 1)} disabled={page >= totalPages || loading}>
                <ChevronRight size={12} />
              </button>
              <span className="ml-1 text-[var(--text-muted)]">共 {total.toLocaleString()} 行</span>
            </div>
          </div>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {panel === 'data' ? (
          loading ? (
            <div className="flex items-center justify-center h-24 gap-2 text-[var(--text-muted)]">
              <Loader2 size={16} className="animate-spin" /> 加载数据...
            </div>
          ) : (
            <div className="h-full overflow-auto">
              <ResultTable columns={columns} rows={rows} />
            </div>
          )
        ) : (
          <ColumnsPanel columns={colInfos} />
        )}
      </div>
    </div>
  );
}

function ColumnsPanel({ columns }: { columns: ColumnInfo[] }) {
  if (!columns.length) return (
    <div className="flex items-center justify-center h-16 text-xs text-[var(--text-muted)]">
      <Loader2 size={13} className="animate-spin mr-2" /> 加载列信息...
    </div>
  );

  return (
    <div className="overflow-auto h-full">
      <table className="min-w-full text-xs border-collapse">
        <thead className="sticky top-0 bg-[var(--bg-tertiary)] z-10">
          <tr>
            {['列名', '类型', '可空', '键', '默认值', '备注'].map((h) => (
              <th key={h} className="px-3 py-2 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {columns.map((col) => (
            <tr key={col.name} className="hover:bg-brand-500/5 even:bg-[var(--bg-secondary)]/30">
              <td className="px-3 py-1.5 border-r border-[var(--border)] font-mono font-medium text-[var(--text-primary)]">{col.name}</td>
              <td className="px-3 py-1.5 border-r border-[var(--border)] font-mono text-purple-400">{col.data_type}</td>
              <td className="px-3 py-1.5 border-r border-[var(--border)]">
                {col.nullable ? <span className="text-yellow-500">YES</span> : <span className="text-[var(--text-muted)]">NO</span>}
              </td>
              <td className="px-3 py-1.5 border-r border-[var(--border)]">
                {col.key && (
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                    col.key === 'PRI' ? 'bg-yellow-500/20 text-yellow-400' :
                    col.key === 'UNI' ? 'bg-blue-500/20 text-blue-400' :
                    'bg-[var(--bg-tertiary)] text-[var(--text-muted)]'
                  }`}>{col.key}</span>
                )}
              </td>
              <td className="px-3 py-1.5 border-r border-[var(--border)] font-mono text-[var(--text-muted)]">
                {col.default_value ?? <span className="italic text-[var(--text-muted)]">NULL</span>}
              </td>
              <td className="px-3 py-1.5 border-r border-[var(--border)] text-[var(--text-muted)]">{col.comment}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
