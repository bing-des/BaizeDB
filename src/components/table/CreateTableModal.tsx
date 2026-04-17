import { useState, useMemo } from 'react';
import { X, Plus, Trash2, KeyRound, Loader2 } from 'lucide-react';
import type { CreateTableColumn, CreateTableInput } from '../../types';

// ──────────────── SQL 预览组件（区分 PG/MySQL） ────────────────
interface SqlPreviewProps {
  isPostgres: boolean;
  fullName: string;
  tableName: string;
  schema?: string;
  tableComment: string;
  columns: CreateTableColumn[];
}

function SqlPreview({ isPostgres, fullName, tableName, schema, tableComment, columns }: SqlPreviewProps) {
  const sql = useMemo(() => {
    const validColumns = columns.filter(c => c.name.trim());
    if (validColumns.length === 0) return '';

    if (isPostgres) {
      // PostgreSQL 语法
      const lines: string[] = [];
      
      // 1. CREATE TABLE
      const colDefs = validColumns.map(c => {
        let def = `  ${c.name} ${c.data_type}`;
        if (!c.nullable) def += ' NOT NULL';
        if (c.default_value) def += ` DEFAULT ${c.default_value}`;
        return def;
      });
      
      // 主键约束（单独一行）
      const pkColumns = validColumns.filter(c => c.is_primary_key).map(c => c.name);
      if (pkColumns.length > 0) {
        colDefs.push(`  CONSTRAINT ${tableName}_pkey PRIMARY KEY (${pkColumns.join(', ')})`);
      }
      
      lines.push(`CREATE TABLE ${fullName} (`);
      lines.push(colDefs.join(',\n'));
      lines.push(');');
      
      // 2. 列注释
      validColumns.forEach(c => {
        if (c.comment) {
          lines.push(`\nCOMMENT ON COLUMN ${fullName}.${c.name} IS '${c.comment}';`);
        }
      });
      
      // 3. 表注释
      if (tableComment) {
        lines.push(`\nCOMMENT ON TABLE ${fullName} IS '${tableComment}';`);
      }
      
      return lines.join('\n');
    } else {
      // MySQL 语法
      const lines: string[] = [];
      
      // 列定义（包含主键）
      const colDefs = validColumns.map(c => {
        let def = `  ${c.name} ${c.data_type}`;
        if (!c.nullable) def += ' NOT NULL';
        if (c.default_value) def += ` DEFAULT ${c.default_value}`;
        if (c.comment) def += ` COMMENT '${c.comment}'`;
        if (c.is_primary_key) def += ' PRIMARY KEY';
        return def;
      });
      
      let createSql = `CREATE TABLE ${fullName} (\n${colDefs.join(',\n')}\n)`;
      
      // 表注释（MySQL 在 CREATE TABLE 后加 COMMENT）
      if (tableComment) {
        createSql += `\nCOMMENT = '${tableComment}'`;
      }
      
      createSql += ';';
      lines.push(createSql);
      
      return lines.join('\n');
    }
  }, [isPostgres, fullName, tableName, schema, tableComment, columns]);

  return (
    <pre className="text-[10px] font-mono text-[var(--text-secondary)] whitespace-pre-wrap">
      {sql}
    </pre>
  );
}

const MYSQL_TYPES = ['INT','BIGINT','VARCHAR(255)','VARCHAR(100)','TEXT','DATETIME','TIMESTAMP','DATE','DECIMAL(10,2)','BOOLEAN','JSON'];
const PG_TYPES = ['integer','bigint','varchar(255)','varchar(100)','text','timestamp','timestamptz','date','numeric(10,2)','boolean','json','jsonb','serial','bigserial'];

interface CreateTableModalProps {
  isOpen: boolean;
  isPostgres: boolean;
  database: string;
  schema?: string;
  onClose: () => void;
  onSubmit: (input: CreateTableInput) => Promise<void>;
}

const defaultColumn = (isPostgres: boolean): CreateTableColumn => ({
  name: '',
  data_type: isPostgres ? 'varchar(255)' : 'VARCHAR(255)',
  nullable: true,
  default_value: '',
  comment: '',
  is_primary_key: false,
});

export default function CreateTableModal({ isOpen, isPostgres, database, schema, onClose, onSubmit }: CreateTableModalProps) {
  const [tableName, setTableName] = useState('');
  const [tableComment, setTableComment] = useState('');
  const [columns, setColumns] = useState<CreateTableColumn[]>([defaultColumn(isPostgres)]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const typeOptions = isPostgres ? PG_TYPES : MYSQL_TYPES;

  if (!isOpen) return null;

  const handleAddColumn = () => {
    setColumns(prev => [...prev, defaultColumn(isPostgres)]);
  };

  const handleRemoveColumn = (index: number) => {
    setColumns(prev => prev.filter((_, i) => i !== index));
  };

  const handleColumnChange = (index: number, field: keyof CreateTableColumn, value: any) => {
    setColumns(prev => prev.map((col, i) => i === index ? { ...col, [field]: value } : col));
  };

  const handleSubmit = async () => {
    if (!tableName.trim()) { setError('表名不能为空'); return; }
    if (columns.length === 0) { setError('至少需要定义一列'); return; }
    for (const col of columns) {
      if (!col.name.trim()) { setError('列名不能为空'); return; }
      if (!col.data_type.trim()) { setError('列类型不能为空'); return; }
    }
    
    setError(null);
    setSubmitting(true);
    try {
      const input: CreateTableInput = {
        table_name: tableName.trim(),
        columns: columns.map(c => ({
          ...c,
          default_value: c.default_value || undefined,
          comment: c.comment || undefined,
        })),
        comment: tableComment || undefined,
      };
      await onSubmit(input);
      // 重置表单
      setTableName('');
      setTableComment('');
      setColumns([defaultColumn(isPostgres)]);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const fullName = schema ? `${schema}.${tableName}` : tableName;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[700px] max-h-[85vh] flex flex-col bg-[var(--bg-primary)] border border-[var(--border)] rounded-lg shadow-xl">
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
          <div>
            <span className="font-semibold text-sm text-[var(--text-primary)]">新建表</span>
            <span className="ml-2 text-xs text-[var(--text-muted)]">{database}{schema ? ` / ${schema}` : ''}</span>
          </div>
          <button className="btn-ghost p-1" onClick={onClose} disabled={submitting}><X size={14} /></button>
        </div>

        {/* 表单内容 */}
        <div className="flex-1 overflow-auto p-4 space-y-4">
          {/* 表名和备注 */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-[var(--text-muted)] mb-1">表名 <span className="text-red-400">*</span></label>
              <input
                className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono"
                value={tableName}
                onChange={e => setTableName(e.target.value)}
                placeholder="table_name"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-xs text-[var(--text-muted)] mb-1">表备注</label>
              <input
                className="w-full px-2.5 py-1.5 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-400 text-[var(--text-primary)]"
                value={tableComment}
                onChange={e => setTableComment(e.target.value)}
                placeholder="可选"
              />
            </div>
          </div>

          {/* 列定义 */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-[var(--text-muted)]">列定义 <span className="text-red-400">*</span></label>
              <button className="btn-ghost py-0.5 px-2 text-xs text-green-400 hover:text-green-300" onClick={handleAddColumn} disabled={submitting}>
                <Plus size={11} className="mr-1" /> 添加列
              </button>
            </div>
            
            <div className="border border-[var(--border)] rounded-md overflow-hidden">
              <table className="w-full text-xs">
                <thead className="bg-[var(--bg-tertiary)]">
                  <tr>
                    <th className="px-2 py-1.5 text-left font-medium text-[var(--text-secondary)] w-8">#</th>
                    <th className="px-2 py-1.5 text-left font-medium text-[var(--text-secondary)]">列名</th>
                    <th className="px-2 py-1.5 text-left font-medium text-[var(--text-secondary)]">类型</th>
                    <th className="px-2 py-1.5 text-center font-medium text-[var(--text-secondary)] w-16">NULL</th>
                    <th className="px-2 py-1.5 text-center font-medium text-[var(--text-secondary)] w-16">主键</th>
                    <th className="px-2 py-1.5 text-left font-medium text-[var(--text-secondary)]">默认值</th>
                    <th className="px-2 py-1.5 text-left font-medium text-[var(--text-secondary)]">备注</th>
                    <th className="px-2 py-1.5 text-center font-medium text-[var(--text-secondary)] w-10"></th>
                  </tr>
                </thead>
                <tbody>
                  {columns.map((col, idx) => (
                    <tr key={idx} className="border-t border-[var(--border)]">
                      <td className="px-2 py-1.5 text-[var(--text-muted)]">{idx + 1}</td>
                      <td className="px-1 py-1">
                        <input
                          className="w-full px-1.5 py-1 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono"
                          value={col.name}
                          onChange={e => handleColumnChange(idx, 'name', e.target.value)}
                          placeholder="column"
                        />
                      </td>
                      <td className="px-1 py-1">
                        <select
                          className="w-full px-1.5 py-1 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded focus:outline-none focus:border-brand-400 text-purple-400 font-mono"
                          value={col.data_type}
                          onChange={e => handleColumnChange(idx, 'data_type', e.target.value)}
                        >
                          {typeOptions.map(t => <option key={t} value={t}>{t}</option>)}
                        </select>
                      </td>
                      <td className="px-1 py-1 text-center">
                        <input
                          type="checkbox"
                          checked={col.nullable}
                          onChange={e => handleColumnChange(idx, 'nullable', e.target.checked)}
                          className="rounded"
                        />
                      </td>
                      <td className="px-1 py-1 text-center">
                        <button
                          className={`p-1 rounded ${col.is_primary_key ? 'text-yellow-400 bg-yellow-400/10' : 'text-[var(--text-muted)] hover:text-yellow-400'}`}
                          onClick={() => handleColumnChange(idx, 'is_primary_key', !col.is_primary_key)}
                          title={col.is_primary_key ? '主键' : '设为主键'}
                        >
                          <KeyRound size={12} />
                        </button>
                      </td>
                      <td className="px-1 py-1">
                        <input
                          className="w-full px-1.5 py-1 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded focus:outline-none focus:border-brand-400 text-[var(--text-primary)] font-mono"
                          value={col.default_value}
                          onChange={e => handleColumnChange(idx, 'default_value', e.target.value)}
                          placeholder="NULL"
                        />
                      </td>
                      <td className="px-1 py-1">
                        <input
                          className="w-full px-1.5 py-1 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded focus:outline-none focus:border-brand-400 text-[var(--text-primary)]"
                          value={col.comment}
                          onChange={e => handleColumnChange(idx, 'comment', e.target.value)}
                          placeholder="-"
                        />
                      </td>
                      <td className="px-1 py-1 text-center">
                        {columns.length > 1 && (
                          <button
                            className="btn-ghost p-1 text-[var(--text-muted)] hover:text-red-400"
                            onClick={() => handleRemoveColumn(idx)}
                            disabled={submitting}
                          >
                            <Trash2 size={12} />
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* 错误信息 */}
          {error && (
            <div className="px-3 py-2 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">
              {error}
            </div>
          )}

          {/* SQL 预览 */}
          {tableName && columns.some(c => c.name) && (
            <div className="p-3 bg-[var(--bg-secondary)] rounded-md">
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs text-[var(--text-muted)]">SQL 预览</span>
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-tertiary)] text-[var(--text-muted)]">
                  {isPostgres ? 'PostgreSQL' : 'MySQL'}
                </span>
              </div>
              <SqlPreview 
                isPostgres={isPostgres} 
                fullName={fullName} 
                tableName={tableName}
                schema={schema}
                tableComment={tableComment}
                columns={columns}
              />
            </div>
          )}
        </div>

        {/* 操作按钮 */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[var(--border)]">
          <button className="btn-ghost px-4 py-1.5 text-sm" onClick={onClose} disabled={submitting}>取消</button>
          <button className="btn-primary px-4 py-1.5 text-sm" onClick={handleSubmit} disabled={submitting}>
            {submitting ? <><Loader2 size={12} className="animate-spin mr-1.5" /> 创建中...</> : '创建表'}
          </button>
        </div>
      </div>
    </div>
  );
}
