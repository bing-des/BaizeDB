import { useState, useRef, useEffect, useCallback } from 'react';
import { Copy, Pencil, Trash2 } from 'lucide-react';
import ContextMenu, { type MenuEntry } from '../common/ContextMenu';

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
}: Props) {
  // 编辑状态
  const [editingCell, setEditingCell] = useState<{ row: number; col: number } | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  const inputRef = useRef<HTMLInputElement>(null);

  // 选中状态（用于 Ctrl+C 复制）
  const [selectedCells, setSelectedCells] = useState<Set<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const selectStartRef = useRef<{ row: number; col: number } | null>(null);

  // 右键菜单状态
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; row: number; col: number } | null>(null);

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
    const handler = (e: ClipboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && selectedCells.size > 0) {
        e.preventDefault();
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
    document.addEventListener('copy', handler);
    return () => document.removeEventListener('copy', handler);
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
          <tr className="bg-[var(--bg-tertiary)]">
            <th className="px-2 py-1.5 text-right text-[var(--text-muted)] font-mono border-b border-r border-[var(--border)] w-10 select-none">
              #
            </th>
            {columns.map((col) => (
              <th key={col} className="px-3 py-1.5 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap">
                {col}
              </th>
            ))}
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
    </>
  );
}
