import { useState, useRef, useEffect, useCallback } from 'react';
import { Copy, Pencil, Trash2, Filter } from 'lucide-react';
import ContextMenu, { type MenuEntry } from '../common/ContextMenu';

type FilterOp = '=' | '!=' | '>' | '<' | '>=' | '<=' | 'LIKE' | 'NOT LIKE' | 'IS NULL' | 'IS NOT NULL';

interface FilterCondition {
  column: string;
  op: FilterOp;
  value: string;
}

interface Props {
  columns: string[];
  rows: (string | number | boolean | null)[][];
  /** 是否启用编辑模式（双击可修改） */
  editable?: boolean;
  /** 主键列索引（用于 UPDATE WHERE） */
  primaryKeyColumn?: number;
  /** 主键值列表（与 rows 一一对应，用于定位行） */
  primaryKeyValues?: (string | number | null)[];
  /** 单元格变更回调 */
  onCellChange?: (rowIndex: number, colIndex: number, value: string | number | boolean | null) => void;
  /** 选中的行索引集合（用于删除操作） */
  selectedRows?: Set<number>;
  /** 行选中/取消选中回调 */
  onRowSelect?: (rowIndex: number, selected: boolean) => void;
  /** 右键菜单：复制回调 */
  onCopy?: (rowIndex: number, colIndex: number) => void;
  /** 右键菜单：编辑回调 */
  onEdit?: (rowIndex: number, colIndex: number) => void;
  /** 右键菜单：删除回调 */
  onDelete?: (rowIndex: number) => void;
  /** 排序列索引 */
  sortColumn?: number | null;
  /** 排序方向 */
  sortDirection?: 'asc' | 'desc';
  /** 排序切换回调 */
  onSort?: (colIndex: number) => void;
  /** 过滤条件 */
  filterConditions?: Record<string, FilterCondition>;
  /** 过滤条件变更回调 */
  onFilterChange?: (column: string, condition: FilterCondition | null) => void;
  /** 应用过滤回调 */
  onApplyFilters?: () => void;
  /** 是否显示表头筛选漏斗（SQL查询结果中隐藏） */
  showFilter?: boolean;
}

export default function ResultTable({
  columns,
  rows,
  editable = false,
  primaryKeyColumn = 0,
  primaryKeyValues,
  onCellChange,
  selectedRows: externalSelectedRows,
  onRowSelect,
  onCopy,
  onEdit,
  onDelete,
  sortColumn,
  sortDirection,
  onSort,
  filterConditions = {},
  onFilterChange,
  onApplyFilters,
  showFilter = true,
}: Props) {
  // 编辑状态
  const [editingCell, setEditingCell] = useState<{ row: number; col: number } | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  const inputRef = useRef<HTMLInputElement>(null);

  // 选中状态（用于 Ctrl+C 复制）
  const [selectedCells, setSelectedCells] = useState<Set<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const selectStartRef = useRef<{ row: number; col: number } | null>(null);
  const copyIntentRef = useRef(false);

  // 右键菜单状态
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; row: number; col: number } | null>(null);

  // 筛选下拉框状态
  const [filterDropdown, setFilterDropdown] = useState<{ col: string; x: number; y: number } | null>(null);
  const [tempFilterOp, setTempFilterOp] = useState<FilterOp>('=');
  const [tempFilterValue, setTempFilterValue] = useState('');

  // 双击进入编辑模式
  const handleDoubleClick = useCallback((rowIndex: number, colIndex: number) => {
    if (!editable) return;
    const cellValue = rows[rowIndex]?.[colIndex];
    setEditingCell({ row: rowIndex, col: colIndex });
    setEditValue(cellValue === null ? '' : String(cellValue));
    setTimeout(() => inputRef.current?.focus(), 0);
  }, [editable, rows]);

  // 确认编辑
  const confirmEdit = () => {
    if (!editingCell || !onCellChange) return;
    let newValue: string | number | boolean | null | undefined;

    const trimmed = editValue.trim();
    if (trimmed === '' || trimmed.toLowerCase() === 'null') {
      newValue = null;
    } else if (trimmed === 'true') {
      newValue = true;
    } else if (trimmed === 'false') {
      newValue = false;
    } else {
      const num = Number(trimmed);
      newValue = Number.isNaN(num) ? trimmed : num;
    }

    onCellChange(editingCell.row, editingCell.col, newValue ?? null);
    setEditingCell(null);
  };

  // 取消编辑
  const cancelEdit = () => {
    setEditingCell(null);
  };

  // 键盘事件：Esc 取消、Enter/Tab 确认
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { e.preventDefault(); cancelEdit(); }
    else if (e.key === 'Enter') { e.preventDefault(); confirmEdit(); }
    else if (e.key === 'Tab') { e.preventDefault(); confirmEdit(); }
  };

  // 鼠标选中（用于复制）
  const handleMouseDown = (e: React.MouseEvent, row: number, col: number) => {
    if (editingCell) return;
    e.preventDefault();
    setSelecting(true);
    selectStartRef.current = { row, col };
    setSelectedCells(new Set([`${row},${col}`]));
  };

  const handleMouseEnter = (row: number, col: number) => {
    if (!selecting || !selectStartRef.current) return;
    const start = selectStartRef.current;
    const newSet = new Set<string>();
    for (let r = Math.min(start.row, row); r <= Math.max(start.row, row); r++) {
      for (let c = Math.min(start.col, col); c <= Math.max(start.col, col); c++) {
        newSet.add(`${r},${c}`);
      }
    }
    setSelectedCells(newSet);
  };

  // 点击行切换选中（mouseup 时如果没有拖选范围，视为点击行）
  const handleMouseUp = useCallback((row: number, col: number) => {
    if (selectStartRef.current) {
      const { row: sr, col: sc } = selectStartRef.current;
      // 只点击了一个格子（没有拖选），触发行选中
      if (sr === row && sc === col) {
        onRowSelect?.(row, !externalSelectedRows?.has(row));
      }
    }
    setSelecting(false);
    selectStartRef.current = null;
  }, [onRowSelect, externalSelectedRows]);

  useEffect(() => {
    const handleGlobalMouseUp = () => { setSelecting(false); selectStartRef.current = null; };
    window.addEventListener('mouseup', handleGlobalMouseUp);
    return () => window.removeEventListener('mouseup', handleGlobalMouseUp);
  }, []);

  // 复制单元格值到剪贴板
  const copyCellValue = useCallback((rowIndex: number, colIndex: number) => {
    const val = rows[rowIndex]?.[colIndex];
    const text = val === null ? 'NULL' : String(val);
    navigator.clipboard.writeText(text).catch(() => {});
  }, [rows]);

  // Ctrl+C 复制选中内容
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // 检测 Ctrl+C 或 Cmd+C
      if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        copyIntentRef.current = true;
      }
    };
    const handleCopy = (e: ClipboardEvent) => {
      if (copyIntentRef.current && selectedCells.size > 0) {
        e.preventDefault();
        copyIntentRef.current = false; // 重置
        const sorted = Array.from(selectedCells)
          .map(s => { const [r, c] = s.split(',').map(Number); return { r, c }; })
          .sort((a, b) => a.r - b.r || a.c - b.c);

        if (sorted.length === 1) {
          const { r, c } = sorted[0];
          const val = rows[r]?.[c];
          e.clipboardData?.setData('text/plain', val === null ? 'NULL' : String(val));
        } else {
          const minRow = Math.min(...sorted.map(s => s.r));
          const maxRow = Math.max(...sorted.map(s => s.r));
          const minCol = Math.min(...sorted.map(s => s.c));
          const maxCol = Math.max(...sorted.map(s => s.c));

          const lines: string[] = [];
          for (let r = minRow; r <= maxRow; r++) {
            const cells: string[] = [];
            for (let c = minCol; c <= maxCol; c++) {
              if (selectedCells.has(`${r},${c}`)) {
                const val = rows[r]?.[c];
                cells.push(val === null ? 'NULL' : String(val));
              } else {
                cells.push('');
              }
            }
            lines.push(cells.join('\t'));
          }
          e.clipboardData?.setData('text/plain', lines.join('\n'));
        }
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('copy', handleCopy);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('copy', handleCopy);
    };
  }, [selectedCells, rows]);

  // 右键菜单
  const handleContextMenu = useCallback((e: React.MouseEvent, row: number, col: number) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, row, col });
  }, []);

  const contextMenuItems: MenuEntry[] = contextMenu ? [
    {
      label: '复制',
      icon: <Copy size={14} />,
      onClick: () => {
        if (onCopy) {
          onCopy(contextMenu.row, contextMenu.col);
        } else {
          copyCellValue(contextMenu.row, contextMenu.col);
        }
      },
    },
    ...(editable ? [{
      label: '修改',
      icon: <Pencil size={14} />,
      onClick: () => {
        if (onEdit) {
          onEdit(contextMenu.row, contextMenu.col);
        } else {
          handleDoubleClick(contextMenu.row, contextMenu.col);
        }
      },
    }] : []),
    ...(editable && onDelete ? [{
      label: '删除',
      icon: <Trash2 size={14} />,
      danger: true as const,
      onClick: () => onDelete(contextMenu.row),
    }] : []),
  ] : [];

  if (columns.length === 0) {
    return <div className="flex items-center justify-center h-16 text-xs text-[var(--text-muted)]">无结果</div>;
  }

  return (
    <>
      <table className="min-w-full text-xs border-collapse" style={{ userSelect: 'none' }}>
        <thead className="sticky top-0 z-10">
          {/* 表头行：列名 + 排序 + 漏斗筛选 */}
          <tr className="bg-[var(--bg-tertiary)]">
            <th className="px-2 py-1.5 text-right text-[var(--text-muted)] font-mono border-b border-r border-[var(--border)] w-10 select-none">
              #
            </th>
            {columns.map((col, idx) => {
              const cond = filterConditions[col];
              const hasFilter = cond && (cond.value || cond.op === 'IS NULL' || cond.op === 'IS NOT NULL');
              return (
                <th
                  key={col}
                  className={`px-3 py-1.5 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap select-none hover:bg-[var(--bg-secondary)] transition-colors ${
                    sortColumn === idx ? 'bg-brand-500/10' : ''
                  }`}
                >
                  <div className="flex items-center justify-between gap-1">
                    <div
                      className="flex items-center gap-1 cursor-pointer flex-1"
                      onClick={() => onSort?.(idx)}
                    >
                      {col}
                      {sortColumn === idx && (
                        <span className="text-brand-400">
                          {sortDirection === 'asc' ? '▲' : '▼'}
                        </span>
                      )}
                    </div>
                    {/* 漏斗图标 - 仅 showFilter 时显示 */}
                    {showFilter && (
                    <button
                      className={`p-0.5 rounded hover:bg-[var(--bg-secondary)] transition-colors ${
                        hasFilter ? 'text-yellow-400' : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                      }`}
                      onClick={(e) => {
                        e.stopPropagation();
                        const rect = (e.currentTarget as HTMLButtonElement).getBoundingClientRect();
                        setFilterDropdown({ col, x: rect.left, y: rect.bottom + 4 });
                        setTempFilterOp(cond?.op || '=');
                        setTempFilterValue(cond?.value || '');
                      }}
                      title="筛选"
                    >
                      <Filter size={12} />
                    </button>
                    )}
                  </div>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri} className={`hover:bg-brand-500/5 even:bg-[var(--bg-secondary)]/30 transition-colors ${
              externalSelectedRows?.has(ri) ? '!bg-red-500/10' : ''
            }`}>
              {/* 行号列 */}
              <td
                className={`px-2 py-1 text-right font-mono border-r border-[var(--border)] select-none ${
                  externalSelectedRows?.has(ri) ? 'bg-red-500/15 text-red-400 font-bold' : 'text-[var(--text-muted)]'
                }`}
              >
                {ri + 1}
              </td>
              {row.map((cell, ci) => {
                const isSelected = selectedCells.has(`${ri},${ci}`);
                const isEditing = editingCell?.row === ri && editingCell?.col === ci;

                return (
                  <td
                    key={ci}
                    className={`px-3 py-1 border-r border-[var(--border)] max-w-xs overflow-hidden text-ellipsis whitespace-nowrap relative cursor-pointer ${
                      isSelected ? '!bg-brand-500/20 ring-1 ring-inset ring-brand-400/30' : ''
                    } ${editable ? 'hover:bg-brand-500/10' : ''}`}
                    title={cell !== null ? String(cell) : 'NULL'}
                    onMouseDown={(e) => handleMouseDown(e, ri, ci)}
                    onMouseEnter={() => selecting && handleMouseEnter(ri, ci)}
                    onMouseUp={() => handleMouseUp(ri, ci)}
                    onDoubleClick={() => handleDoubleClick(ri, ci)}
                    onContextMenu={(e) => handleContextMenu(e, ri, ci)}
                  >
                    {isEditing ? (
                      <input
                        ref={inputRef}
                        type="text"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                        onBlur={confirmEdit}
                        onKeyDown={handleKeyDown}
                        className="w-full px-0.5 py-0 text-xs bg-white dark:bg-gray-800 border border-brand-500 rounded font-mono outline-none focus:ring-1 focus:ring-brand-400"
                        style={{ minWidth: '60px' }}
                        autoFocus
                      />
                    ) : cell === null ? (
                      <span className="text-[var(--text-muted)] italic font-mono text-[11px]">NULL</span>
                    ) : typeof cell === 'number' ? (
                      <span className="text-purple-400 font-mono">{cell}</span>
                    ) : typeof cell === 'boolean' ? (
                      <span className={cell ? 'text-green-400' : 'text-red-400'}>{cell ? 'true' : 'false'}</span>
                    ) : (
                      <span className="text-[var(--text-primary)]">{String(cell)}</span>
                    )}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>

      {/* 右键菜单 */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* 筛选下拉框 */}
      {filterDropdown && (
        <>
          {/* 遮罩层 */}
          <div
            className="fixed inset-0 z-40"
            onClick={() => setFilterDropdown(null)}
          />
          {/* 下拉框 */}
          <div
            className="fixed z-50 bg-[var(--bg-primary)] border border-[var(--border)] rounded-lg shadow-lg p-3 min-w-[200px]"
            style={{ left: filterDropdown.x, top: filterDropdown.y }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="text-xs font-medium text-[var(--text-secondary)] mb-2">
              筛选: {filterDropdown.col}
            </div>
            <div className="space-y-2">
              <select
                className="w-full text-xs bg-[var(--bg-secondary)] border border-[var(--border)] rounded px-2 py-1.5 outline-none focus:border-brand-400/50"
                value={tempFilterOp}
                onChange={(e) => setTempFilterOp(e.target.value as FilterOp)}
              >
                <option value="=">等于 (=)</option>
                <option value="!=">不等于 (!=)</option>
                <option value=">">大于 ({'>'})</option>
                <option value="<">小于 ({'<'})</option>
                <option value=">=">大于等于 ({'>='})</option>
                <option value="<=">小于等于 ({'<='})</option>
                <option value="LIKE">包含 (LIKE)</option>
                <option value="NOT LIKE">不包含 (NOT LIKE)</option>
                <option value="IS NULL">为空 (IS NULL)</option>
                <option value="IS NOT NULL">不为空 (IS NOT NULL)</option>
              </select>
              {tempFilterOp !== 'IS NULL' && tempFilterOp !== 'IS NOT NULL' && (
                <input
                  type="text"
                  className="w-full text-xs bg-[var(--bg-secondary)] border border-[var(--border)] rounded px-2 py-1.5 outline-none focus:border-brand-400/50 font-mono"
                  placeholder={tempFilterOp === 'LIKE' || tempFilterOp === 'NOT LIKE' ? '%keyword%' : '输入值...'}
                  value={tempFilterValue}
                  onChange={(e) => setTempFilterValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      onFilterChange?.(filterDropdown.col, {
                        column: filterDropdown.col,
                        op: tempFilterOp,
                        value: tempFilterValue
                      });
                      setFilterDropdown(null);
                      onApplyFilters?.();
                    }
                  }}
                  autoFocus
                />
              )}
              <div className="flex gap-2 pt-1">
                <button
                  className="flex-1 text-xs bg-brand-500 hover:bg-brand-600 text-white rounded px-3 py-1.5 transition-colors"
                  onClick={() => {
                    onFilterChange?.(filterDropdown.col, {
                      column: filterDropdown.col,
                      op: tempFilterOp,
                      value: tempFilterValue
                    });
                    setFilterDropdown(null);
                    onApplyFilters?.();
                  }}
                >
                  确定
                </button>
                <button
                  className="text-xs bg-[var(--bg-secondary)] hover:bg-[var(--bg-tertiary)] text-[var(--text-secondary)] border border-[var(--border)] rounded px-3 py-1.5 transition-colors"
                  onClick={() => {
                    onFilterChange?.(filterDropdown.col, null);
                    setFilterDropdown(null);
                    onApplyFilters?.();
                  }}
                >
                  清除
                </button>
                <button
                  className="text-xs bg-[var(--bg-secondary)] hover:bg-[var(--bg-tertiary)] text-[var(--text-secondary)] border border-[var(--border)] rounded px-3 py-1.5 transition-colors"
                  onClick={() => setFilterDropdown(null)}
                >
                  取消
                </button>
              </div>
            </div>
          </div>
        </>
      )}
    </>
  );
}
