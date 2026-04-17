import { useState, useMemo } from 'react';
import { Plus, Pencil, Trash2, X, Check, Loader2, KeyRound, Save, Undo2, Eye, Copy, Trash } from 'lucide-react';
import { databaseApi } from '../../utils/api';
import type { ColumnInfo, AddColumnInput, ModifyColumnInput } from '../../types';
import ConfirmModal from '../common/ConfirmModal';

const MYSQL_TYPES = ['INT','BIGINT','SMALLINT','TINYINT','DECIMAL(10,2)','FLOAT','DOUBLE','VARCHAR(255)','VARCHAR(100)','TEXT','LONGTEXT','CHAR(36)','DATE','DATETIME','TIMESTAMP','TIME','BOOLEAN','JSON'];
const PG_TYPES = ['integer','bigint','smallint','numeric(10,2)','real','double precision','varchar(255)','varchar(100)','text','char(36)','date','timestamp','timestamptz','time','boolean','json','jsonb','uuid','serial','bigserial'];

type PendingOperation =
  | { type: 'add'; data: AddColumnInput }
  | { type: 'drop'; columnName: string }
  | { type: 'modify'; oldColumn: ColumnInfo; newData: ModifyColumnInput };

interface ColumnFormData { column_name: string; column_type: string; nullable: boolean; default_value: string; comment: string; }
const defaultForm = (isPostgres: boolean): ColumnFormData => ({ column_name: '', column_type: isPostgres ? 'varchar(255)' : 'VARCHAR(255)', nullable: true, default_value: '', comment: '' });

interface ColumnFormProps { title: string; initialData?: Partial<ColumnFormData>; isPostgres: boolean; onSubmit: (data: ColumnFormData) => void; onCancel: () => void; }
function ColumnForm({ title, initialData, isPostgres, onSubmit, onCancel }: ColumnFormProps) {
  const [form, setForm] = useState<ColumnFormData>({ ...defaultForm(isPostgres), ...initialData });
  const [error, setError] = useState<string | null>(null);
  const typeOptions = isPostgres ? PG_TYPES : MYSQL_TYPES;
  const handleSubmit = () => {
    if (!form.column_name.trim()) { setError('列名不能为空'); return; }
    if (!form.column_type.trim()) { setError('类型不能为空'); return; }
    setError(null); onSubmit(form);
  };
  return (
    <div className="w-80 flex flex-col h-full border-l border-[var(--border)] bg-[var(--bg-secondary)]">
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
        <span className="font-semibold text-sm text-[var(--text-primary)]">{title}</span>
        <button className="btn-ghost p-1" onClick={onCancel}><X size={14} /></button>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">列名 <span className="text-red-400">*</span></label>
          <input className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono" value={form.column_name} onChange={e => setForm(f => ({ ...f, column_name: e.target.value }))} placeholder="column_name" autoFocus />
        </div>
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">类型 <span className="text-red-400">*</span></label>
          <div className="space-y-1.5">
            <input className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-purple-400 font-mono" value={form.column_type} onChange={e => setForm(f => ({ ...f, column_type: e.target.value }))} placeholder="VARCHAR(255)" />
            <div className="flex flex-wrap gap-1">
              {typeOptions.map(t => (
                <button key={t} className={`px-1.5 py-0.5 text-[10px] rounded border transition-colors font-mono ${form.column_type === t ? 'border-brand-400 bg-brand-500/20 text-brand-300' : 'border-[var(--border)] text-[var(--text-muted)] hover:border-brand-400/50 hover:text-[var(--text-secondary)]'}`} onClick={() => setForm(f => ({ ...f, column_type: t }))}>{t}</button>
              ))}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <input id="nullable-toggle" type="checkbox" className="rounded" checked={form.nullable} onChange={e => setForm(f => ({ ...f, nullable: e.target.checked }))} />
          <label htmlFor="nullable-toggle" className="text-sm text-[var(--text-secondary)] cursor-pointer select-none">允许 NULL</label>
        </div>
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">默认值</label>
          <input className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono" value={form.default_value} onChange={e => setForm(f => ({ ...f, default_value: e.target.value }))} placeholder="留空则无默认值" />
        </div>
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">备注</label>
          <input className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)]" value={form.comment} onChange={e => setForm(f => ({ ...f, comment: e.target.value }))} placeholder="可选" />
        </div>
        {error && <div className="px-3 py-2 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">{error}</div>}
      </div>
      <div className="flex items-center gap-2 px-4 py-3 border-t border-[var(--border)]">
        <button className="btn-primary flex-1 py-1.5 text-sm" onClick={handleSubmit}><><Check size={12} className="mr-1.5" /> 确认</></button>
        <button className="btn-ghost flex-1 py-1.5 text-sm" onClick={onCancel}>取消</button>
      </div>
    </div>
  );
}

interface SqlPreviewModalProps { operations: PendingOperation[]; isPostgres: boolean; tableName: string; onClose: () => void; onRemoveOp?: (index: number) => void; }
function SqlPreviewModal({ operations, isPostgres, tableName, onClose, onRemoveOp }: SqlPreviewModalProps) {
  const [copied, setCopied] = useState(false);
  const opWithSql = useMemo(() => {
    return operations.map(op => {
      let sql = '';
      let label = '';
      switch (op.type) {
        case 'add': {
          const d = op.data; label = `新增字段: ${d.column_name}`;
          sql = `ALTER TABLE ${tableName} ADD COLUMN ${d.column_name} ${d.column_type}`;
          if (!d.nullable) sql += ' NOT NULL';
          if (d.default_value) sql += ` DEFAULT ${d.default_value}`;
          if (d.comment) {
            if (isPostgres) sql += `;\nCOMMENT ON COLUMN ${tableName}.${d.column_name} IS '${d.comment}';`;
            else sql += ` COMMENT '${d.comment}'`;
          } else sql += ';';
          break;
        }
        case 'drop': {
          label = `删除字段: ${op.columnName}`;
          sql = `ALTER TABLE ${tableName} DROP COLUMN ${op.columnName};`;
          break;
        }
        case 'modify': {
          const d = op.newData; const old = op.oldColumn; label = `修改字段: ${old.name} → ${d.new_name}`;
          if (isPostgres) {
            const steps: string[] = [];
            if (d.column_type !== old.data_type) steps.push(`ALTER TABLE ${tableName} ALTER COLUMN ${d.old_name} TYPE ${d.column_type} USING ${d.old_name}::${d.column_type};`);
            if (d.nullable !== old.nullable) steps.push(d.nullable ? `ALTER TABLE ${tableName} ALTER COLUMN ${d.old_name} DROP NOT NULL;` : `ALTER TABLE ${tableName} ALTER COLUMN ${d.old_name} SET NOT NULL;`);
            if (d.default_value !== (old.default_value ?? '')) steps.push(d.default_value ? `ALTER TABLE ${tableName} ALTER COLUMN ${d.old_name} SET DEFAULT ${d.default_value};` : `ALTER TABLE ${tableName} ALTER COLUMN ${d.old_name} DROP DEFAULT;`);
            if (d.new_name !== d.old_name) steps.push(`ALTER TABLE ${tableName} RENAME COLUMN ${d.old_name} TO ${d.new_name};`);
            if (d.comment !== (old.comment ?? '')) steps.push(d.comment ? `COMMENT ON COLUMN ${tableName}.${d.new_name} IS '${d.comment}';` : `COMMENT ON COLUMN ${tableName}.${d.new_name} IS NULL;`);
            sql = steps.join('\n');
          } else {
            sql = `ALTER TABLE ${tableName} CHANGE COLUMN ${d.old_name} ${d.new_name} ${d.column_type}`;
            if (!d.nullable) sql += ' NOT NULL';
            if (d.default_value) sql += ` DEFAULT ${d.default_value}`;
            if (d.comment) sql += ` COMMENT '${d.comment}'`;
            sql += ';';
          }
          break;
        }
      }
      return { op, sql, label };
    });
  }, [operations, isPostgres, tableName]);
  const fullSql = opWithSql.map(o => o.sql).join('\n\n');
  const handleCopy = async () => {
    try { await navigator.clipboard.writeText(fullSql); setCopied(true); setTimeout(() => setCopied(false), 2000); } catch {}
  };
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[700px] max-h-[80vh] flex flex-col bg-[var(--bg-primary)] border border-[var(--border)] rounded-lg shadow-xl">
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
          <span className="font-semibold text-sm text-[var(--text-primary)]">待执行 SQL 预览</span>
          <button className="btn-ghost p-1" onClick={onClose}><X size={14} /></button>
        </div>
        <div className="flex-1 overflow-auto p-4 space-y-3">
          {operations.length === 0 ? <div className="text-center text-[var(--text-muted)] py-8">暂无待执行的操作</div> : opWithSql.map((item, idx) => (
            <div key={idx} className="border border-[var(--border)] rounded-md overflow-hidden">
              <div className="flex items-center justify-between px-3 py-2 bg-[var(--bg-secondary)] border-b border-[var(--border)]">
                <div className="flex items-center gap-2">
                  <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${item.op.type === 'add' ? 'bg-green-500/20 text-green-400' : item.op.type === 'drop' ? 'bg-red-500/20 text-red-400' : 'bg-yellow-500/20 text-yellow-400'}`}>{item.op.type === 'add' ? '新增' : item.op.type === 'drop' ? '删除' : '修改'}</span>
                  <span className="text-xs text-[var(--text-secondary)]">{item.label}</span>
                </div>
                {onRemoveOp && <button className="btn-ghost p-1 text-[var(--text-muted)] hover:text-red-400" title="取消此操作" onClick={() => onRemoveOp(idx)}><Trash size={12} /></button>}
              </div>
              <pre className="text-xs font-mono text-[var(--text-secondary)] whitespace-pre-wrap break-all p-3">{item.sql}</pre>
            </div>
          ))}
        </div>
        <div className="flex items-center justify-between px-4 py-3 border-t border-[var(--border)]">
          <button className="btn-ghost py-1.5 px-3 text-xs flex items-center gap-1.5" onClick={handleCopy}>{copied ? <><Check size={12} className="text-green-400" /> 已复制</> : <><Copy size={12} /> 复制全部 SQL</>}</button>
          <button className="btn-ghost px-4 py-1.5 text-sm" onClick={onClose}>关闭</button>
        </div>
      </div>
    </div>
  );
}

interface SchemaEditorProps { connectionId: string; database: string; table: string; isPostgres: boolean; columns: ColumnInfo[]; onRefresh: () => void; }
type PanelMode = 'add' | { type: 'edit'; col: ColumnInfo } | null;

export default function SchemaEditor({ connectionId, database, table, isPostgres, columns, onRefresh }: SchemaEditorProps) {
  const [panelMode, setPanelMode] = useState<PanelMode>(null);
  const [confirmDrop, setConfirmDrop] = useState<ColumnInfo | null>(null);
  const [pendingOps, setPendingOps] = useState<PendingOperation[]>([]);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [showSqlPreview, setShowSqlPreview] = useState(false);

  const previewColumns = useMemo(() => {
    let result = [...columns];
    for (const op of pendingOps) {
      switch (op.type) {
        case 'add': result = [...result, { name: op.data.column_name, data_type: op.data.column_type, nullable: op.data.nullable, key: '', default_value: op.data.default_value ?? undefined, comment: op.data.comment ?? undefined }]; break;
        case 'drop': result = result.filter(c => c.name !== op.columnName); break;
        case 'modify': result = result.map(c => c.name === op.oldColumn.name ? { ...c, name: op.newData.new_name, data_type: op.newData.column_type, nullable: op.newData.nullable, default_value: op.newData.default_value ?? undefined, comment: op.newData.comment ?? undefined } : c); break;
      }
    }
    return result;
  }, [columns, pendingOps]);

  const handleAddPending = (data: ColumnFormData) => {
    const input: AddColumnInput = { column_name: data.column_name, column_type: data.column_type, nullable: data.nullable, default_value: data.default_value || undefined, comment: data.comment || undefined };
    setPendingOps(prev => [...prev, { type: 'add', data: input }]); setPanelMode(null); setSaveError(null);
  };
  const handleEditPending = (oldCol: ColumnInfo, data: ColumnFormData) => {
    const input: ModifyColumnInput = { old_name: oldCol.name, new_name: data.column_name, column_type: data.column_type, nullable: data.nullable, default_value: data.default_value || undefined, comment: data.comment || undefined };
    setPendingOps(prev => [...prev, { type: 'modify', oldColumn: oldCol, newData: input }]); setPanelMode(null); setSaveError(null);
  };
  const handleDropPending = (col: ColumnInfo) => { setPendingOps(prev => [...prev, { type: 'drop', columnName: col.name }]); setConfirmDrop(null); setSaveError(null); };
  const handleUndoAll = () => { setPendingOps([]); setSaveError(null); };
  const handleRemoveOp = (index: number) => { setPendingOps(prev => prev.filter((_, i) => i !== index)); };
  const handleSaveAll = async () => {
    if (pendingOps.length === 0) return;
    setSaving(true); setSaveError(null);
    try {
      for (const op of pendingOps) {
        if (op.type === 'add') await databaseApi.addColumn(connectionId, database, table, op.data);
        else if (op.type === 'drop') await databaseApi.dropColumn(connectionId, database, table, op.columnName);
        else if (op.type === 'modify') await databaseApi.modifyColumn(connectionId, database, table, op.newData);
      }
      setPendingOps([]); onRefresh();
    } catch (e) { setSaveError(String(e)); }
    finally { setSaving(false); }
  };
  const isColumnDropped = (colName: string) => pendingOps.some(op => op.type === 'drop' && op.columnName === colName);
  const getColumnModifyOp = (colName: string) => pendingOps.find(op => op.type === 'modify' && op.oldColumn.name === colName);

  if (columns.length === 0) return <div className="flex items-center justify-center h-16 text-xs text-[var(--text-muted)]"><Loader2 size={13} className="animate-spin mr-2" /> 加载列信息...</div>;

  return (
    <div className="flex h-full overflow-hidden">
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
          <span className="text-xs text-[var(--text-muted)]">共 {previewColumns.length} 列 {pendingOps.length > 0 && <span className="ml-2 text-yellow-400">({pendingOps.length} 个待执行)</span>}</span>
          <div className="flex-1" />
          {saveError && <span className="text-xs text-red-400 truncate max-w-xs" title={saveError}>⚠ {saveError}</span>}
          <button className="btn-ghost py-1 px-2.5 text-xs text-green-400 hover:text-green-300 hover:bg-green-500/10" onClick={() => { setPanelMode('add'); setSaveError(null); }} disabled={saving}><Plus size={12} className="mr-1" /> 新增字段</button>
        </div>
        <div className="flex-1 overflow-auto">
          <table className="min-w-full text-xs border-collapse">
            <thead className="sticky top-0 bg-[var(--bg-tertiary)] z-10"><tr>{['列名','类型','可空','键','默认值','备注','操作'].map(h => <th key={h} className="px-3 py-2 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap last:border-r-0">{h}</th>)}</tr></thead>
            <tbody>
              {previewColumns.map(col => {
                const isPk = col.key === 'PRI', isDropped = isColumnDropped(col.name), modifyOp = getColumnModifyOp(col.name), isModified = !!modifyOp;
                const isNew = pendingOps.some(op => op.type === 'add' && op.data.column_name === col.name);
                const isEditing = panelMode !== null && panelMode !== 'add' && panelMode.col.name === col.name;
                return (
                  <tr key={col.name} className={`hover:bg-brand-500/5 even:bg-[var(--bg-secondary)]/30 transition-colors ${isEditing ? 'bg-brand-500/10' : ''} ${isDropped ? 'opacity-40 line-through' : ''} ${isNew ? 'bg-green-500/5' : ''} ${isModified ? 'bg-yellow-500/5' : ''}`}>
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono font-medium text-[var(--text-primary)] whitespace-nowrap">
                      <div className="flex items-center gap-1.5">
                        {isPk && <KeyRound size={10} className="text-yellow-400 flex-shrink-0" />}
                        {isNew && <span className="text-[10px] px-1 bg-green-500/20 text-green-400 rounded">新增</span>}
                        {isModified && <span className="text-[10px] px-1 bg-yellow-500/20 text-yellow-400 rounded">修改</span>}
                        {isDropped && <span className="text-[10px] px-1 bg-red-500/20 text-red-400 rounded">删除</span>}
                        <span className={isDropped ? 'line-through' : ''}>{col.name}</span>
                      </div>
                    </td>
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono text-purple-400 whitespace-nowrap">{col.data_type}</td>
                    <td className="px-3 py-2 border-r border-[var(--border)] text-center">{col.nullable ? <span className="text-yellow-500">YES</span> : <span className="text-[var(--text-muted)]">NO</span>}</td>
                    <td className="px-3 py-2 border-r border-[var(--border)]">{col.key && <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${col.key === 'PRI' ? 'bg-yellow-500/20 text-yellow-400' : col.key === 'UNI' ? 'bg-blue-500/20 text-blue-400' : 'bg-[var(--bg-tertiary)] text-[var(--text-muted)]'}`}>{col.key}</span>}</td>
                    <td className="px-3 py-2 border-r border-[var(--border)] font-mono text-[var(--text-muted)] max-w-[120px] truncate">{col.default_value ?? <span className="italic opacity-50">NULL</span>}</td>
                    <td className="px-3 py-2 border-r border-[var(--border)] text-[var(--text-muted)] max-w-[120px] truncate">{col.comment}</td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-1">
                        {!isDropped && !isNew && <button className="btn-ghost p-1 text-[var(--text-muted)] hover:text-brand-400" title="修改字段" onClick={() => { setPanelMode({ type: 'edit', col }); setSaveError(null); }} disabled={saving}><Pencil size={12} /></button>}
                        {!isPk && !isDropped && !isNew && <button className="btn-ghost p-1 text-[var(--text-muted)] hover:text-red-400" title="删除字段" onClick={() => { setConfirmDrop(col); setSaveError(null); }} disabled={saving}><Trash2 size={12} /></button>}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {pendingOps.length > 0 && (
          <div className="flex items-center gap-2 px-3 py-2 border-t border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
            <span className="text-xs text-[var(--text-muted)]">{pendingOps.length} 个操作待执行</span>
            <div className="flex-1" />
            <button className="btn-ghost py-1 px-2.5 text-xs text-[var(--text-muted)] hover:text-[var(--text-secondary)]" onClick={() => setShowSqlPreview(true)} disabled={saving}><Eye size={12} className="mr-1" /> 查看 SQL</button>
            <button className="btn-ghost py-1 px-2.5 text-xs text-yellow-400 hover:text-yellow-300" onClick={handleUndoAll} disabled={saving}><Undo2 size={12} className="mr-1" /> 撤销全部</button>
            <button className="btn-primary py-1 px-3 text-xs" onClick={handleSaveAll} disabled={saving}>{saving ? <><Loader2 size={12} className="animate-spin mr-1" /> 保存中...</> : <><Save size={12} className="mr-1" /> 保存 ({pendingOps.length})</>}</button>
          </div>
        )}
      </div>
      {panelMode !== null && (panelMode === 'add' ? <ColumnForm title="新增字段" isPostgres={isPostgres} onSubmit={handleAddPending} onCancel={() => setPanelMode(null)} /> : <ColumnForm title={`修改字段：${panelMode.col.name}`} isPostgres={isPostgres} initialData={{ column_name: panelMode.col.name, column_type: panelMode.col.data_type, nullable: panelMode.col.nullable, default_value: panelMode.col.default_value ?? '', comment: panelMode.col.comment ?? '' }} onSubmit={(data) => handleEditPending(panelMode.col, data)} onCancel={() => setPanelMode(null)} />)}
      {confirmDrop && <ConfirmModal message={`确定删除字段「${confirmDrop.name}」吗？该操作将加入待执行列表，点击保存后生效。`} onConfirm={() => handleDropPending(confirmDrop)} onCancel={() => setConfirmDrop(null)} danger />}
      {showSqlPreview && <SqlPreviewModal operations={pendingOps} isPostgres={isPostgres} tableName={table} onClose={() => setShowSqlPreview(false)} onRemoveOp={handleRemoveOp} />}
    </div>
  );
}
