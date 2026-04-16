import { useState } from 'react';
import { Plus, Pencil, Trash2, X, Check, Loader2, KeyRound } from 'lucide-react';
import { databaseApi } from '../../utils/api';
import type { ColumnInfo, AddColumnInput, ModifyColumnInput } from '../../types';
import ConfirmModal from '../common/ConfirmModal';

// ──────────────── 常用类型选项 ────────────────
const MYSQL_TYPES = [
  'INT', 'BIGINT', 'SMALLINT', 'TINYINT',
  'DECIMAL(10,2)', 'FLOAT', 'DOUBLE',
  'VARCHAR(255)', 'VARCHAR(100)', 'TEXT', 'LONGTEXT', 'CHAR(36)',
  'DATE', 'DATETIME', 'TIMESTAMP', 'TIME',
  'BOOLEAN', 'JSON',
];

const PG_TYPES = [
  'integer', 'bigint', 'smallint',
  'numeric(10,2)', 'real', 'double precision',
  'varchar(255)', 'varchar(100)', 'text', 'char(36)',
  'date', 'timestamp', 'timestamptz', 'time',
  'boolean', 'json', 'jsonb', 'uuid',
  'serial', 'bigserial',
];

// ──────────────── ColumnForm（新增/编辑表单） ────────────────
interface ColumnFormData {
  column_name: string;
  column_type: string;
  nullable: boolean;
  default_value: string;
  comment: string;
}

const defaultForm = (): ColumnFormData => ({
  column_name: '',
  column_type: 'VARCHAR(255)',
  nullable: true,
  default_value: '',
  comment: '',
});

interface ColumnFormProps {
  title: string;
  initialData?: Partial<ColumnFormData>;
  isPostgres: boolean;
  onSubmit: (data: ColumnFormData) => Promise<void>;
  onCancel: () => void;
}

function ColumnForm({ title, initialData, isPostgres, onSubmit, onCancel }: ColumnFormProps) {
  const [form, setForm] = useState<ColumnFormData>({ ...defaultForm(), ...initialData });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const typeOptions = isPostgres ? PG_TYPES : MYSQL_TYPES;

  const handleSubmit = async () => {
    if (!form.column_name.trim()) { setError('列名不能为空'); return; }
    if (!form.column_type.trim()) { setError('类型不能为空'); return; }
    setError(null);
    setSubmitting(true);
    try {
      await onSubmit(form);
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="w-80 flex flex-col h-full border-l border-[var(--border)] bg-[var(--bg-secondary)]">
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
        <span className="font-semibold text-sm text-[var(--text-primary)]">{title}</span>
        <button className="btn-ghost p-1" onClick={onCancel}>
          <X size={14} />
        </button>
      </div>

      {/* 表单内容 */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* 列名 */}
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">列名 <span className="text-red-400">*</span></label>
          <input
            className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono"
            value={form.column_name}
            onChange={e => setForm(f => ({ ...f, column_name: e.target.value }))}
            placeholder="column_name"
            autoFocus
          />
        </div>

        {/* 类型 */}
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">类型 <span className="text-red-400">*</span></label>
          <div className="space-y-1.5">
            <input
              className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-purple-400 font-mono"
              value={form.column_type}
              onChange={e => setForm(f => ({ ...f, column_type: e.target.value }))}
              placeholder="VARCHAR(255)"
            />
            <div className="flex flex-wrap gap-1">
              {typeOptions.map(t => (
                <button
                  key={t}
                  className={`px-1.5 py-0.5 text-[10px] rounded border transition-colors font-mono ${
                    form.column_type === t
                      ? 'border-brand-400 bg-brand-500/20 text-brand-300'
                      : 'border-[var(--border)] text-[var(--text-muted)] hover:border-brand-400/50 hover:text-[var(--text-secondary)]'
                  }`}
                  onClick={() => setForm(f => ({ ...f, column_type: t }))}
                >
                  {t}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* 允许 NULL */}
        <div className="flex items-center gap-2">
          <input
            id="nullable-toggle"
            type="checkbox"
            className="rounded"
            checked={form.nullable}
            onChange={e => setForm(f => ({ ...f, nullable: e.target.checked }))}
          />
          <label htmlFor="nullable-toggle" className="text-sm text-[var(--text-secondary)] cursor-pointer select-none">
            允许 NULL
          </label>
        </div>

        {/* 默认值 */}
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">默认值</label>
          <input
            className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono"
            value={form.default_value}
            onChange={e => setForm(f => ({ ...f, default_value: e.target.value }))}
            placeholder="留空则无默认值"
          />
        </div>

        {/* 备注（仅 MySQL 或 PG）*/}
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">备注</label>
          <input
            className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)]"
            value={form.comment}
            onChange={e => setForm(f => ({ ...f, comment: e.target.value }))}
            placeholder="可选"
          />
        </div>

        {/* 错误信息 */}
        {error && (
          <div className="px-3 py-2 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">
            {error}
          </div>
        )}
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center gap-2 px-4 py-3 border-t border-[var(--border)]">
        <button
          className="btn-primary flex-1 py-1.5 text-sm"
          onClick={handleSubmit}
          disabled={submitting}
        >
          {submitting ? (
            <><Loader2 size={12} className="animate-spin mr-1.5" /> 执行中...</>
          ) : (
            <><Check size={12} className="mr-1.5" /> 确认</>
          )}
        </button>
        <button className="btn-ghost flex-1 py-1.5 text-sm" onClick={onCancel} disabled={submitting}>
          取消
        </button>
      </div>
    </div>
  );
}

// ──────────────── SchemaEditor 主组件 ────────────────
interface SchemaEditorProps {
  connectionId: string;
  database: string;
  table: string;
  isPostgres: boolean;
  columns: ColumnInfo[];
  onRefresh: () => void;
}

type PanelMode = 'add' | { type: 'edit'; col: ColumnInfo } | null;

export default function SchemaEditor({
  connectionId, database, table, isPostgres, columns, onRefresh
}: SchemaEditorProps) {
  const [panelMode, setPanelMode] = useState<PanelMode>(null);
  const [confirmDrop, setConfirmDrop] = useState<ColumnInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [opError, setOpError] = useState<string | null>(null);

  const handleAddColumn = async (data: ColumnFormData) => {
    const input: AddColumnInput = {
      column_name: data.column_name,
      column_type: data.column_type,
      nullable: data.nullable,
      default_value: data.default_value || undefined,
      comment: data.comment || undefined,
    };
    await databaseApi.addColumn(connectionId, database, table, input);
    setPanelMode(null);
    setOpError(null);
    onRefresh();
  };

  const handleModifyColumn = async (col: ColumnInfo, data: ColumnFormData) => {
    const input: ModifyColumnInput = {
      old_name: col.name,
      new_name: data.column_name,
      column_type: data.column_type,
      nullable: data.nullable,
      default_value: data.default_value || undefined,
      comment: data.comment || undefined,
    };
    await databaseApi.modifyColumn(connectionId, database, table, input);
    setPanelMode(null);
    setOpError(null);
    onRefresh();
  };

  const handleDropColumn = async (col: ColumnInfo) => {
    setBusy(true);
    try {
      await databaseApi.dropColumn(connectionId, database, table, col.name);
      setConfirmDrop(null);
      setOpError(null);
      onRefresh();
    } catch (e) {
      setOpError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (columns.length === 0) {
    return (
      <div className="flex items-center justify-center h-16 text-xs text-[var(--text-muted)]">
        <Loader2 size={13} className="animate-spin mr-2" /> 加载列信息...
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-hidden">
      {/* 左侧：列列表 */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* 工具栏 */}
        <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
          <span className="text-xs text-[var(--text-muted)]">共 {columns.length} 列</span>
          <div className="flex-1" />
          {opError && (
            <span className="text-xs text-red-400 truncate max-w-xs" title={opError}>⚠ {opError}</span>
          )}
          <button
            className="btn-ghost py-1 px-2.5 text-xs text-green-400 hover:text-green-300 hover:bg-green-500/10"
            onClick={() => { setPanelMode('add'); setOpError(null); }}
            disabled={busy}
          >
            <Plus size={12} className="mr-1" /> 新增字段
          </button>
        </div>

        {/* 列表 */}
        <div className="flex-1 overflow-auto">
          <table className="min-w-full text-xs border-collapse">
            <thead className="sticky top-0 bg-[var(--bg-tertiary)] z-10">
              <tr>
                {['列名', '类型', '可空', '键', '默认值', '备注', '操作'].map(h => (
                  <th key={h} className="px-3 py-2 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap last:border-r-0">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {columns.map(col => {
                const isPk = col.key === 'PRI';
                const isEditing = panelMode !== null && panelMode !== 'add' && panelMode.col.name === col.name;
                return (
                  <tr
                    key={col.name}
                    className={`hover:bg-brand-500/5 even:bg-[var(--bg-secondary)]/30 transition-colors ${isEditing ? 'bg-brand-500/10' : ''}`}
                  >
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono font-medium text-[var(--text-primary)] whitespace-nowrap">
                      <div className="flex items-center gap-1.5">
                        {isPk && <KeyRound size={10} className="text-yellow-400 flex-shrink-0" />}
                        {col.name}
                      </div>
                    </td>
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono text-purple-400 whitespace-nowrap">{col.data_type}</td>
                    <td className="px-3 py-2 border-r border-[var(--border)] text-center">
                      {col.nullable
                        ? <span className="text-yellow-500">YES</span>
                        : <span className="text-[var(--text-muted)]">NO</span>}
                    </td>
                    <td className="px-3 py-2 border-r border-[var(--border)]">
                      {col.key && (
                        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                          col.key === 'PRI' ? 'bg-yellow-500/20 text-yellow-400' :
                          col.key === 'UNI' ? 'bg-blue-500/20 text-blue-400' :
                          'bg-[var(--bg-tertiary)] text-[var(--text-muted)]'
                        }`}>{col.key}</span>
                      )}
                    </td>
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono text-[var(--text-muted)] max-w-[120px] truncate">
                      {col.default_value ?? <span className="italic opacity-50">NULL</span>}
                    </td>
                    <td className="px-3 py-2 border-r border-[var(--border)] text-[var(--text-muted)] max-w-[120px] truncate">
                      {col.comment}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-1">
                        <button
                          className="btn-ghost p-1 text-[var(--text-muted)] hover:text-brand-400"
                          title="修改字段"
                          onClick={() => {
                            setPanelMode({ type: 'edit', col });
                            setOpError(null);
                          }}
                          disabled={busy}
                        >
                          <Pencil size={12} />
                        </button>
                        {!isPk && (
                          <button
                            className="btn-ghost p-1 text-[var(--text-muted)] hover:text-red-400"
                            title="删除字段"
                            onClick={() => { setConfirmDrop(col); setOpError(null); }}
                            disabled={busy}
                          >
                            <Trash2 size={12} />
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      {/* 右侧：新增/编辑表单 */}
      {panelMode !== null && (
        panelMode === 'add' ? (
          <ColumnForm
            title="新增字段"
            isPostgres={isPostgres}
            onSubmit={handleAddColumn}
            onCancel={() => setPanelMode(null)}
          />
        ) : (
          <ColumnForm
            title={`修改字段：${panelMode.col.name}`}
            isPostgres={isPostgres}
            initialData={{
              column_name: panelMode.col.name,
              column_type: panelMode.col.data_type,
              nullable: panelMode.col.nullable,
              default_value: panelMode.col.default_value ?? '',
              comment: panelMode.col.comment ?? '',
            }}
            onSubmit={(data) => handleModifyColumn(panelMode.col, data)}
            onCancel={() => setPanelMode(null)}
          />
        )
      )}

      {/* 删除确认弹窗 */}
      {confirmDrop && (
        <ConfirmModal
          message={`确定删除字段「${confirmDrop.name}」吗？该操作不可撤销，将永久丢失该列的所有数据。`}
          onConfirm={() => handleDropColumn(confirmDrop)}
          onCancel={() => setConfirmDrop(null)}
          danger
        />
      )}
    </div>
  );
}
