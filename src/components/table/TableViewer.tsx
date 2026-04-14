import { useState, useEffect, useCallback, useRef } from 'react';
import { RefreshCw, ChevronLeft, ChevronRight, Loader2, Table2, Download, Columns3, Save, Undo2, Plus, Trash2 } from 'lucide-react';
import { databaseApi } from '../../utils/api';
import type { Tab, ColumnInfo } from '../../types';
import ResultTable from '../editor/ResultTable';
import ConfirmModal from '../common/ConfirmModal';

/** 记录单元格的变更 */
interface CellChange {
  rowIndex: number;
  colIndex: number;
  oldValue: string | number | boolean | null;
  newValue: string | number | boolean | null;
}

export default function TableViewer({ tab }: { tab: Tab }) {
  const [columns, setColumns] = useState<string[]>([]);
  const [columnTypes, setColumnTypes] = useState<string[]>([]);
  const [rows, setRows] = useState<(string | number | boolean | null)[][]>([]);
  const [colInfos, setColInfos] = useState<ColumnInfo[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [loading, setLoading] = useState(false);
  const [panel, setPanel] = useState<'data' | 'columns'>('data');

  // 排序状态
  const [sortColumn, setSortColumn] = useState<number | null>(null);
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc');
  // 过滤状态（列名 -> 过滤文本）- 保留兼容性
  const [filterValues, setFilterValues] = useState<Record<string, string>>({});

  // 编辑状态
  const [changes, setChanges] = useState<CellChange[]>([]);
  const [saving, setSaving] = useState(false);
  const changesRef = useRef<CellChange[]>([]);

  // 选中的行（用于删除）
  const [selectedRowIndices, setSelectedRowIndices] = useState<Set<number>>(new Set());

  // 新增行状态（行内）
  const [insertingRow, setInsertingRow] = useState<boolean>(false);
  const [newRowValues, setNewRowValues] = useState<Record<string, string>>({});
  const insertRef = useRef<HTMLDivElement>(null);

  // 确认弹窗状态
  const [confirmModal, setConfirmModal] = useState<{ message: string; onConfirm: () => void } | null>(null);

  const { connectionId, database, table } = tab;

  // 主键列信息
  const primaryKeyColIndex = colInfos.findIndex(c => c.key === 'PRI');
  const hasPrimaryKey = primaryKeyColIndex >= 0;
  const pkColumn = columns[primaryKeyColIndex] ?? 'id';
  const pkColumnType = columnTypes[primaryKeyColIndex] ?? null;

  // 过滤操作符类型（前端选择）
  type FilterOp = '=' | '!=' | '>' | '<' | '>=' | '<=' | 'LIKE' | 'NOT LIKE' | 'IS NULL' | 'IS NOT NULL';
  
  interface FilterCondition {
    column: string;
    op: FilterOp;
    value: string;
  }
  
  const [filterConditions, setFilterConditions] = useState<Record<string, FilterCondition>>({});
  
  // 构建过滤参数（发送到后端）
  const buildFilters = useCallback(() => {
    const result: Record<string, string> = {};
    for (const [col, cond] of Object.entries(filterConditions)) {
      if (cond.value || cond.op === 'IS NULL' || cond.op === 'IS NOT NULL') {
        result[col] = `${cond.op}|${cond.value}`;
      }
    }
    return Object.keys(result).length > 0 ? result : null;
  }, [filterConditions]);
  
  const loadData = useCallback(async (p: number, size: number = pageSize) => {
    if (!database || !table) return;
    setLoading(true);
    try {
      const sortBy = sortColumn !== null ? columns[sortColumn] : null;
      const sortOrder = sortDirection;
      const filters = buildFilters();
      
      console.log(`[TableViewer] loadData connectionId=${connectionId} database=${database} table=${table} page=${p} size=${size}`, { sortBy, sortOrder, filters });
      const r = await databaseApi.getTableData(connectionId, database, table, p, size, sortBy, sortOrder, filters);
      console.log(`[TableViewer] loaded columns=${r.columns.length} rows=${r.rows.length} total=${r.total}`);
      setColumns(r.columns);
      setColumnTypes(r.column_types || []);
      setRows(r.rows);
      setTotal(r.total);
      setChanges([]);
      changesRef.current = [];
      setSelectedRowIndices(new Set());
    } catch (e) {
      console.error(`[TableViewer] loadData error:`, e);
    } finally {
      setLoading(false);
    }
  }, [connectionId, database, table, pageSize, sortColumn, sortDirection, columns, buildFilters]);

  useEffect(() => {
    loadData(1);
    if (database && table) {
      databaseApi.listColumns(connectionId, database, table)
        .then((cols) => { console.log(`[TableViewer] listColumns columns=${cols.length}`); setColInfos(cols); })
        .catch((e) => { console.error(`[TableViewer] listColumns error:`, e); });
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const go = (p: number) => { setPage(p); loadData(p); };

  // 切换排序：asc -> desc -> cancel，实时生效
  const toggleSort = (colIndex: number) => {
    let newSortColumn: number | null = sortColumn;
    let newSortDirection: 'asc' | 'desc' = sortDirection;
    
    if (sortColumn === colIndex) {
      if (sortDirection === 'asc') {
        newSortDirection = 'desc';
      } else {
        // 取消排序
        newSortColumn = null;
        newSortDirection = 'asc';
      }
    } else {
      newSortColumn = colIndex;
      newSortDirection = 'asc';
    }
    
    // 先更新状态
    setSortColumn(newSortColumn);
    setSortDirection(newSortDirection);
    setPage(1);
    
    // 立即加载数据（使用新值）
    if (database && table) {
      setLoading(true);
      databaseApi.getTableData(
        connectionId,
        database,
        table,
        1,
        pageSize,
        newSortColumn !== null ? columns[newSortColumn] : undefined,
        newSortDirection,
        buildFilters()
      ).then((r) => {
        setColumns(r.columns);
        setColumnTypes(r.column_types || []);
        setRows(r.rows);
        setTotal(r.total);
        setChanges([]);
        changesRef.current = [];
        setSelectedRowIndices(new Set());
      }).catch((e) => {
        console.error(`[TableViewer] loadData error:`, e);
      }).finally(() => {
        setLoading(false);
      });
    }
  };
  
  // 应用排序和过滤
  const applyFilters = () => { loadData(1); };

  // 重置所有过滤
  const resetFilters = () => {
    setFilterConditions({});
    setSortColumn(null);
    setSortDirection('asc');
    setPage(1);
    loadData(1);
  };

  // 处理过滤条件变更并立即应用（同步执行）
  const handleFilterChange = (column: string, condition: { column: string; op: FilterOp; value: string } | null) => {
    // 计算新的过滤条件
    let newConditions: Record<string, { column: string; op: FilterOp; value: string }>;
    if (condition === null) {
      newConditions = { ...filterConditions };
      delete newConditions[column];
    } else {
      newConditions = { ...filterConditions, [column]: condition };
    }
    
    // 同步更新状态
    setFilterConditions(newConditions);
    
    // 立即使用新条件加载数据
    if (database && table) {
      setLoading(true);
      setPage(1);
      
      // 构建 filters 对象
      const filters: Record<string, string> = {};
      Object.values(newConditions).forEach((cond) => {
        if (cond.op === 'IS NULL' || cond.op === 'IS NOT NULL') {
          filters[cond.column] = `${cond.op}|`;
        } else {
          filters[cond.column] = `${cond.op}|${cond.value}`;
        }
      });
      
      databaseApi.getTableData(
        connectionId,
        database,
        table,
        1,
        pageSize,
        sortColumn !== null ? columns[sortColumn] : undefined,
        sortDirection,
        Object.keys(filters).length > 0 ? filters : undefined
      ).then((r) => {
        setColumns(r.columns);
        setColumnTypes(r.column_types || []);
        setRows(r.rows);
        setTotal(r.total);
        setChanges([]);
        changesRef.current = [];
        setSelectedRowIndices(new Set());
      }).catch((e) => {
        console.error(`[TableViewer] loadData error:`, e);
      }).finally(() => {
        setLoading(false);
      });
    }
  };

  const exportCSV = () => {
    const lines = [columns.join(','), ...rows.map((r) => r.map((v) => v === null ? '' : `"${String(v).replace(/"/g, '""')}"`).join(','))];
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([lines.join('\n')], { type: 'text/csv' }));
    a.download = `${table}.csv`;
    a.click();
  };

  // 处理单元格编辑变更
  const handleCellChange = useCallback((rowIndex: number, colIndex: number, value: string | number | boolean | null) => {
    setRows(prevRows => {
      const newRows = [...prevRows];
      const newRow = [...newRows[rowIndex]];
      newRow[colIndex] = value;
      newRows[rowIndex] = newRow;
      return newRows;
    });

    setChanges(prev => {
      const existingIdx = prev.findIndex(c => c.rowIndex === rowIndex && c.colIndex === colIndex);
      const change: CellChange = { rowIndex, colIndex, oldValue: rows[rowIndex]?.[colIndex], newValue: value };
      const next = existingIdx >= 0
        ? prev.map((c, i) => i === existingIdx ? change : c)
        : [...prev, change];
      changesRef.current = next;
      return next;
    });
  }, [rows]);

  // 处理行选中/取消选中
  const handleRowSelect = (rowIndex: number, selected: boolean) => {
    setSelectedRowIndices(prev => {
      const next = new Set(prev);
      if (selected) next.add(rowIndex); else next.delete(rowIndex);
      return next;
    });
  };

  // 删除指定行（右键菜单触发）
  /** 右键菜单删除单行（触发确认弹窗） */
  const deleteRow = (rowIndex: number) => {
    if (!hasPrimaryKey || !database || !table || !pkColumn) return;
    setConfirmModal({
      message: `确定删除第 ${rowIndex + 1} 行数据吗？此操作不可撤销。`,
      onConfirm: async () => {
        try {
          const pkValue = rows[rowIndex][primaryKeyColIndex!];
          console.log('[TableViewer] deleteRow: pkColumn=', pkColumn, 'pkColumnType=', pkColumnType, 'pkValue=', pkValue);
          await databaseApi.deleteTableData(connectionId, database, table, pkColumn!, pkColumnType, [pkValue]);
          setSelectedRowIndices(prev => { const next = new Set(prev); next.delete(rowIndex); return next; });
          loadData(page);
        } catch (e) {
          console.error('[TableViewer] deleteRow error:', e);
          alert(`删除失败: ${e}`);
        } finally {
          setConfirmModal(null);
        }
      },
    });
  };

  // 右键菜单：修改（进入编辑模式）
  const handleContextMenuEdit = useCallback((rowIndex: number, colIndex: number) => {
    // ResultTable 内部会通过双击机制处理，这里不需要额外操作
    // 只需确保该行被选中
    handleRowSelect(rowIndex, true);
  }, []);

  // 开始新增行（行内）
  const startInsertRow = () => {
    setNewRowValues({});
    setInsertingRow(true);
    setTimeout(() => {
      insertRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
    }, 50);
  };

  // 取消新增行
  const cancelInsertRow = () => {
    setInsertingRow(false);
    setNewRowValues({});
  };

  // 提交新行
  const submitInsertRow = async () => {
    if (!database || !table) return;

    const columnValues: Record<string, any> = {};
    let hasValue = false;
    for (const col of columns) {
      const val = newRowValues[col];
      if (val !== undefined && val !== '') {
        const colInfo = colInfos.find(c => c.name === col);
        const dt = colInfo?.data_type?.toLowerCase() || '';
        if (/^(int|bigint|smallint|integer|serial|bigserial|smallserial|tinyint|numeric|decimal|float|real|double)/.test(dt)) {
          columnValues[col] = val.includes('.') ? parseFloat(val) : parseInt(val, 10);
        } else {
          columnValues[col] = val;
        }
        hasValue = true;
      }
    }

    if (!hasValue) {
      alert('请至少填写一个字段');
      return;
    }

    try {
      console.log('[TableViewer] submitInsertRow:', columnValues);
      const columnTypes: Record<string, string> = {};
      for (const col of colInfos) {
        columnTypes[col.name] = col.data_type;
      }
      await databaseApi.insertTableData(connectionId, database, table, columnValues, columnTypes);
      setInsertingRow(false);
      setNewRowValues({});
      const newTotal = total + 1;
      const lastPage = Math.ceil(newTotal / pageSize);
      go(lastPage);
    } catch (e) {
      console.error('[TableViewer] submitInsertRow error:', e);
      alert(`插入失败: ${e}`);
    }
  };

  // 撤销所有修改
  const undoChanges = () => {
    setRows(prevRows => {
      const restored = prevRows.map(row => [...row]);
      for (const ch of changes) {
        if (restored[ch.rowIndex]) {
          restored[ch.rowIndex][ch.colIndex] = ch.oldValue;
        }
      }
      return restored;
    });
    setChanges([]);
    changesRef.current = [];
  };

  // 保存修改到数据库
  const saveChanges = async () => {
    // 使用 changesRef 获取最新 changes，避免 onBlur 竞态导致读到旧值
    const currentChanges = changesRef.current;
    if (!hasPrimaryKey || !database || !table || currentChanges.length === 0) return;

    setSaving(true);
    try {
      const rowChangeMap = new Map<number, CellChange[]>();
      for (const ch of currentChanges) {
        const list = rowChangeMap.get(ch.rowIndex) ?? [];
        list.push(ch);
        rowChangeMap.set(ch.rowIndex, list);
      }

      const colTypeMap = Object.fromEntries(colInfos.map(c => [c.name, c.data_type]));

      const updates = Array.from(rowChangeMap.entries()).map(([ri, cellChanges]) => ({
        row_index: ri,
        primary_key_value: rows[ri][primaryKeyColIndex],
        column_values: Object.fromEntries(
          cellChanges.map(ch => [columns[ch.colIndex], ch.newValue])
        ),
        column_types: colTypeMap,
      }));

      console.log(`[TableViewer] saveChanges: sending ${updates.length} row updates`);
      const affected = await databaseApi.updateTableData(connectionId, database, table, pkColumn, pkColumnType, updates);
      console.log(`[TableViewer] saveChanges: ${affected} rows affected`);
      changesRef.current = [];
      setChanges([]);
    } catch (e) {
      console.error('[TableViewer] saveChanges error:', e);
      alert(`保存失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const changedCount = new Set(changes.map(c => c.rowIndex)).size;

  /** 批量删除选中的行 */
  const deleteSelectedRows = async () => {
    if (selectedRowIndices.size === 0 || !hasPrimaryKey || !pkColumn || !database || !table) return;
    try {
      console.log('[TableViewer] deleteSelectedRows: pkColumn=', pkColumn, 'indices=', Array.from(selectedRowIndices));
      const pkValues = Array.from(selectedRowIndices).map(idx => rows[idx]?.[primaryKeyColIndex!]);
      console.log('[TableViewer] deleteSelectedRows: pkValues=', pkValues);
      await databaseApi.deleteTableData(connectionId, database, table, pkColumn!, pkColumnType, pkValues);
      setSelectedRowIndices(new Set());
      await loadData(page);
    } catch (e) {
      console.error('[TableViewer] deleteSelectedRows error:', e);
      alert(`删除失败: ${e}`);
    }
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
            {/* 编辑操作按钮 */}
            {hasPrimaryKey && (
              <>
                <button
                  className="btn-ghost py-1 px-2 text-xs text-green-400 hover:text-green-300"
                  onClick={() => setInsertingRow(true)}
                  title="新增一行"
                >
                  <Plus size={12} />
                  <span className="ml-1">新增</span>
                </button>
                {selectedRowIndices.size > 0 && (
                  <button
                    className="btn-ghost py-1 px-2 text-xs text-red-400 hover:text-red-300"
                    onClick={() => {
                      setConfirmModal({
                        message: `确定删除选中的 ${selectedRowIndices.size} 行数据吗？此操作不可撤销。`,
                        onConfirm: async () => {
                          await deleteSelectedRows();
                          setConfirmModal(null);
                        },
                      });
                    }}
                    title={`删除选中的 ${selectedRowIndices.size} 行`}
                  >
                    <Trash2 size={12} />
                    <span className="ml-1">删除 ({selectedRowIndices.size})</span>
                  </button>
                )}
                {changes.length > 0 && (
                  <>
                    <div className="h-4 w-px bg-[var(--border)]" />
                    <button
                      className="btn-ghost py-1 px-2 text-xs text-yellow-400 hover:text-yellow-300"
                      onClick={undoChanges}
                      title="撤销所有修改"
                    >
                      <Undo2 size={12} />
                      <span className="ml-1">撤销 ({changedCount})</span>
                    </button>
                    <div className="h-4 w-px bg-[var(--border)]" />
                    <button
                      className="btn-primary py-1 px-2.5 text-xs"
                      onClick={saveChanges}
                      disabled={saving}
                      title="保存到数据库"
                    >
                      <Save size={12} className={saving ? '' : 'mr-1'} />
                      {saving ? '保存中...' : `保存 (${changes.length})`}
                    </button>
                  </>
                )}
              </>
            )}

            <div className="h-4 w-px bg-[var(--border)]" />

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

              <select
                className="ml-2 bg-[var(--bg-secondary)] border border-[var(--border)] rounded px-1.5 py-0.5 text-xs text-[var(--text-primary)] outline-none"
                value={pageSize}
                onChange={(e) => {
                  const newSize = Number(e.target.value);
                  setPageSize(newSize);
                  setPage(1); // 切换条数时回到第一页
                  loadData(1, newSize);
                }}
              >
                {[10, 20, 50, 100, 200].map((n) => (
                  <option key={n} value={n}>{n} 条/页</option>
                ))}
              </select>
            </div>

            {!hasPrimaryKey && (
              <span className="text-xs text-yellow-500 ml-2">⚠ 表无主键，不可编辑</span>
            )}
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
              <ResultTable
                columns={columns}
                rows={rows}
                editable={true}
                primaryKeyColumn={primaryKeyColIndex}
                primaryKeyValues={rows.map(r => r[primaryKeyColIndex] as string | number | null)}
                onCellChange={handleCellChange}
                selectedRows={selectedRowIndices}
                onRowSelect={handleRowSelect}
                onEdit={handleContextMenuEdit}
                onDelete={deleteRow}
                sortColumn={sortColumn}
                sortDirection={sortDirection}
                onSort={toggleSort}
                filterConditions={filterConditions}
                onFilterChange={handleFilterChange}
                onApplyFilters={applyFilters}
              />
              {/* 行内新增行区域 */}
              {insertingRow ? (
                <div ref={insertRef} className="border-t border-[var(--border)] bg-[var(--bg-secondary)]">
                  <div className="flex items-center justify-end gap-2 px-3 py-1 border-b border-[var(--border)]">
                    <span className="text-xs text-[var(--text-muted)] mr-auto">✏ 新增行</span>
                    <button
                      className="btn-ghost py-0.5 px-2 text-xs text-green-400 hover:bg-green-500/10"
                      onClick={submitInsertRow}
                    >确认</button>
                    <button
                      className="btn-ghost py-0.5 px-2 text-xs text-[var(--text-muted)] hover:bg-[var(--bg-tertiary)]"
                      onClick={cancelInsertRow}
                    >取消</button>
                  </div>
                  <div className="grid grid-cols-subgrid overflow-x-auto" style={{ gridTemplateColumns: `40px repeat(${columns.length}, minmax(120px, 1fr))` }}>
                    <div className="px-2 py-1.5 text-right text-[var(--text-muted)] font-mono text-xs border-r border-b border-[var(--border)]">*</div>
                    {columns.map((col) => {
                      const info = colInfos.find(c => c.name === col);
                      return (
                        <div key={col} className="relative group">
                          <input
                            type="text"
                            className="w-full px-2 py-1.5 text-xs font-mono bg-transparent border-r border-b border-[var(--border)] focus:outline-none focus:bg-brand-500/5 transition-colors"
                            placeholder={`${info?.data_type || '...'}${info?.nullable ? '' : '*'}`}
                            value={newRowValues[col] ?? ''}
                            onChange={(e) => setNewRowValues(prev => ({ ...prev, [col]: e.target.value }))}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') submitInsertRow();
                              else if (e.key === 'Escape') cancelInsertRow();
                            }}
                          />
                        </div>
                      );
                    })}
                  </div>
                </div>
              ) : (
                /* 行内新增按钮（表格底部） */
                hasPrimaryKey && (
                  <div
                    className="flex items-center gap-2 px-3 py-1.5 border-t border-dashed border-[var(--border)] text-xs text-[var(--text-muted)] hover:text-brand-400 hover:bg-brand-500/5 cursor-pointer transition-colors"
                    onClick={startInsertRow}
                  >
                    <Plus size={12} />
                    <span>点击新增一行</span>
                  </div>
                )
              )}
            </div>
          )
        ) : (
          <ColumnsPanel columns={colInfos} />
        )}
      </div>

      {/* 确认弹窗 */}
      {confirmModal && (
        <ConfirmModal
          message={confirmModal.message}
          onConfirm={confirmModal.onConfirm}
          onCancel={() => setConfirmModal(null)}
          danger
        />
      )}
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
