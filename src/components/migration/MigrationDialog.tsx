import { useState, useEffect, useRef, useCallback } from 'react';
import { X, Database, Loader2, Copy, CheckCircle, ClipboardCopy, ArrowRight } from 'lucide-react';
import { migrationApi, connectionApi, databaseApi } from '../../utils/api';
import { useConnectionStore } from '../../store';
import type { MigrationInput, MigrationProgress, MigrationStatus } from '../../types';

interface Props {
  onClose: () => void;
}

const defaultForm: Omit<MigrationInput, 'source_connection_id' | 'target_connection_id' | 'source_database'> = {
  target_database: undefined,
  tables: undefined,
  migrate_structure: true,
  migrate_data: true,
  truncate_target: false,
  batch_size: 1000,
};

/** 状态文案映射 */
const statusLabel: Record<string, string> = {
  NotStarted: '未开始',
  Preparing: '准备中...',
  MigratingStructure: '迁移表结构...',
  MigratingData: '迁移数据...',
  Completed: '迁移完成',
  Failed: '迁移失败',
};

/** 状态对应的进度条颜色 */
const statusColor = (status: MigrationStatus): string => {
  if (status === 'Completed') return 'bg-emerald-500';
  if (status === 'Failed') return 'bg-red-500';
  if (status === 'MigratingData' || status === 'MigratingStructure') return 'bg-brand-500';
  return 'bg-brand-400';
};

export default function MigrationDialog({ onClose }: Props) {
  const { connections, connectedIds } = useConnectionStore();
  const [form, setForm] = useState({
    sourceConnectionId: '',
    targetConnectionId: '',
    sourceDatabase: '',
    ...defaultForm,
  });
  const [sourceDatabases, setSourceDatabases] = useState<string[]>([]);
  const [targetDatabases, setTargetDatabases] = useState<string[]>([]);
  const [sourceTables, setSourceTables] = useState<string[]>([]);
  const [selectedTables, setSelectedTables] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState<MigrationProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const unlistenRef = useRef<(() => void) | null>(null);
  const migrationIdRef = useRef<string | null>(null);

  // 清理事件监听
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  // 加载源数据库列表
  useEffect(() => {
    if (!form.sourceConnectionId) return;
    const load = async () => {
      try {
        const dbs = await databaseApi.listDatabases(form.sourceConnectionId);
        setSourceDatabases(dbs.map(db => db.name));
      } catch (e) {
        console.error('Failed to load source databases', e);
      }
    };
    load();
  }, [form.sourceConnectionId]);

  // 加载目标数据库列表
  useEffect(() => {
    if (!form.targetConnectionId) return;
    const load = async () => {
      try {
        const dbs = await databaseApi.listDatabases(form.targetConnectionId);
        setTargetDatabases(dbs.map(db => db.name));
      } catch (e) {
        console.error('Failed to load target databases', e);
      }
    };
    load();
  }, [form.targetConnectionId]);

  // 加载源表列表
  useEffect(() => {
    if (!form.sourceConnectionId || !form.sourceDatabase) return;
    const load = async () => {
      try {
        const tables = await databaseApi.listTables(form.sourceConnectionId, form.sourceDatabase);
        setSourceTables(tables.map(t => t.name));
        setSelectedTables([]);
      } catch (e) {
        console.error('Failed to load source tables', e);
      }
    };
    load();
  }, [form.sourceConnectionId, form.sourceDatabase]);

  const handleStart = useCallback(async () => {
    if (!form.sourceConnectionId || !form.targetConnectionId || !form.sourceDatabase) {
      setError('请选择源连接、目标连接和源数据库');
      return;
    }
    setLoading(true);
    setError(null);
    setProgress(null);

    try {
      // 先注册事件监听
      const unlisten = await migrationApi.onProgress((p) => {
        // 只处理当前迁移任务的进度
        if (migrationIdRef.current && p.migration_id !== migrationIdRef.current) return;
        setProgress(p);

        // 迁移完成或失败时，停止 loading
        if (p.status === 'Completed' || p.status === 'Failed') {
          setLoading(false);
          if (p.status === 'Failed' && p.error) {
            setError(p.error);
          }
        }
      });
      unlistenRef.current = unlisten;

      const input: MigrationInput = {
        source_connection_id: form.sourceConnectionId,
        target_connection_id: form.targetConnectionId,
        source_database: form.sourceDatabase,
        target_database: form.target_database || undefined,
        tables: selectedTables.length > 0 ? selectedTables : undefined,
        migrate_structure: form.migrate_structure,
        migrate_data: form.migrate_data,
        truncate_target: form.truncate_target,
        batch_size: form.batch_size,
      };

      // 启动迁移，获得 migration_id
      const mid = await migrationApi.startMigration(input);
      migrationIdRef.current = mid;

    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, [form, selectedTables]);

  const handleSelectAllTables = () => {
    setSelectedTables([...sourceTables]);
  };

  const handleDeselectAllTables = () => {
    setSelectedTables([]);
  };

  const toggleTable = (table: string) => {
    setSelectedTables(prev =>
      prev.includes(table) ? prev.filter(t => t !== table) : [...prev, table]
    );
  };

  const handleCopyError = async () => {
    if (!error) return;
    try {
      await navigator.clipboard.writeText(error);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error('Failed to copy error', e);
    }
  };

  // 计算进度百分比
  const progressPercent = progress
    ? progress.total_tables > 0
      ? Math.round((progress.tables_completed / progress.total_tables) * 100)
      : 0
    : 0;

  const isTerminal = progress?.status === 'Completed' || progress?.status === 'Failed';

  const connectedConnections = connections.filter(c => connectedIds.has(c.id));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-[var(--bg-secondary)] border border-[var(--border)] rounded-xl shadow-2xl w-[640px] max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-lg bg-brand-500/15 flex items-center justify-center">
              <Copy size={14} className="text-brand-400" />
            </div>
            <span className="font-semibold text-[var(--text-primary)]">数据迁移</span>
          </div>
          <button onClick={onClose} className="btn-ghost p-1">
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-5 space-y-4">
          {/* Source and Target connections */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5 uppercase tracking-wider">
                源连接
              </label>
              <select
                className="input-field"
                value={form.sourceConnectionId}
                onChange={e => setForm({ ...form, sourceConnectionId: e.target.value })}
                disabled={loading}
              >
                <option value="">请选择</option>
                {connectedConnections.map(c => (
                  <option key={c.id} value={c.id}>
                    {c.name} ({c.db_type})
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5 uppercase tracking-wider">
                目标连接
              </label>
              <select
                className="input-field"
                value={form.targetConnectionId}
                onChange={e => setForm({ ...form, targetConnectionId: e.target.value })}
                disabled={loading}
              >
                <option value="">请选择</option>
                {connectedConnections.map(c => (
                  <option key={c.id} value={c.id}>
                    {c.name} ({c.db_type})
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Databases */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5 uppercase tracking-wider">
                源数据库
              </label>
              <select
                className="input-field"
                value={form.sourceDatabase}
                onChange={e => setForm({ ...form, sourceDatabase: e.target.value })}
                disabled={!form.sourceConnectionId || loading}
              >
                <option value="">请选择</option>
                {sourceDatabases.map(db => (
                  <option key={db} value={db}>{db}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5 uppercase tracking-wider">
                目标数据库（可选）
              </label>
              <input
                className="input-field"
                list="target-db-list"
                placeholder={form.sourceDatabase || '留空则使用源数据库名'}
                value={form.target_database || ''}
                onChange={e => setForm({ ...form, target_database: e.target.value || undefined })}
                disabled={!form.targetConnectionId || loading}
              />
              <datalist id="target-db-list">
                {targetDatabases.map(db => (
                  <option key={db} value={db} />
                ))}
              </datalist>
            </div>
          </div>

          {/* Tables selection */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="block text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider">
                选择表（留空则迁移所有表）
              </label>
              <div className="flex gap-2">
                <button className="text-xs btn-ghost" onClick={handleSelectAllTables} disabled={loading}>
                  全选
                </button>
                <button className="text-xs btn-ghost" onClick={handleDeselectAllTables} disabled={loading}>
                  清空
                </button>
              </div>
            </div>
            <div className="max-h-48 overflow-y-auto border border-[var(--border)] rounded-lg p-2">
              {sourceTables.length === 0 ? (
                <div className="text-center text-sm text-[var(--text-muted)] py-4">
                  {form.sourceDatabase ? '没有表' : '请先选择源数据库'}
                </div>
              ) : (
                <div className="grid grid-cols-3 gap-2">
                  {sourceTables.map(table => (
                    <label key={table} className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={selectedTables.includes(table)}
                        onChange={() => toggleTable(table)}
                        disabled={loading}
                        className="w-4 h-4 rounded accent-brand-500"
                      />
                      <span className="text-sm text-[var(--text-secondary)] truncate">{table}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Options */}
          <div className="space-y-3">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={form.migrate_structure}
                onChange={e => setForm({ ...form, migrate_structure: e.target.checked })}
                disabled={loading}
                className="w-4 h-4 rounded accent-brand-500"
              />
              <span className="text-sm text-[var(--text-secondary)]">迁移表结构（CREATE TABLE）</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={form.migrate_data}
                onChange={e => setForm({ ...form, migrate_data: e.target.checked })}
                disabled={loading}
                className="w-4 h-4 rounded accent-brand-500"
              />
              <span className="text-sm text-[var(--text-secondary)]">迁移数据（INSERT）</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={form.truncate_target}
                onChange={e => setForm({ ...form, truncate_target: e.target.checked })}
                disabled={loading}
                className="w-4 h-4 rounded accent-brand-500"
              />
              <span className="text-sm text-[var(--text-secondary)]">迁移前清空目标表数据（TRUNCATE）</span>
            </label>
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">批量大小</label>
              <input
                className="input-field w-32"
                type="number"
                min="1"
                max="10000"
                value={form.batch_size}
                onChange={e => setForm({ ...form, batch_size: parseInt(e.target.value) || 1000 })}
                disabled={loading}
              />
            </div>
          </div>

          {/* Error */}
          {error && (
            <div className="px-3 py-2.5 rounded-lg text-sm border bg-red-500/10 border-red-500/30 text-red-400">
              <div className="flex items-start gap-2">
                <span className="flex-1 break-all">✗ {error}</span>
                <button
                  onClick={handleCopyError}
                  className="shrink-0 p-1 hover:bg-red-500/20 rounded transition-colors"
                  title="复制错误信息"
                >
                  {copied ? <CheckCircle size={14} /> : <ClipboardCopy size={14} />}
                </button>
              </div>
            </div>
          )}

          {/* Progress Bar */}
          {progress && (
            <div className="rounded-lg border border-[var(--border)] bg-[var(--bg-primary)] overflow-hidden">
              {/* Progress bar */}
              <div className="h-1.5 bg-[var(--bg-tertiary)]">
                <div
                  className={`h-full transition-all duration-300 ease-out ${statusColor(progress.status)}`}
                  style={{ width: `${progressPercent}%` }}
                />
              </div>

              {/* Progress info */}
              <div className="px-3.5 py-3 space-y-2">
                {/* Status line */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    {progress.status === 'Completed' ? (
                      <CheckCircle size={14} className="text-emerald-400" />
                    ) : progress.status === 'Failed' ? (
                      <X size={14} className="text-red-400" />
                    ) : (
                      <Loader2 size={14} className="text-brand-400 animate-spin" />
                    )}
                    <span className="text-sm font-medium text-[var(--text-primary)]">
                      {statusLabel[progress.status] || progress.status}
                    </span>
                  </div>
                  <span className="text-xs text-[var(--text-muted)] font-mono">
                    {progressPercent}%
                  </span>
                </div>

                {/* Current table */}
                {progress.current_table && (
                  <div className="flex items-center gap-1.5 text-xs text-[var(--text-secondary)]">
                    <Database size={12} className="text-[var(--text-muted)]" />
                    <span className="truncate">{progress.current_table}</span>
                    {progress.status === 'MigratingData' && progress.current_table_rows > 0 && (
                      <>
                        <ArrowRight size={10} className="text-[var(--text-muted)]" />
                        <span>{progress.current_table_rows.toLocaleString()} 行</span>
                      </>
                    )}
                  </div>
                )}

                {/* Stats */}
                <div className="flex items-center gap-4 text-xs text-[var(--text-muted)]">
                  <span>表 {progress.tables_completed}/{progress.total_tables}</span>
                  {progress.rows_migrated > 0 && (
                    <span>总行数 {progress.rows_migrated.toLocaleString()}</span>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-4 border-t border-[var(--border)]">
          <div className="text-sm text-[var(--text-muted)]">
            支持 MySQL / PostgreSQL 之间互迁移
          </div>
          <div className="flex gap-2">
            <button className="btn-secondary" onClick={onClose}>
              {isTerminal ? '关闭' : '取消'}
            </button>
            <button
              className="btn-primary"
              onClick={handleStart}
              disabled={loading || !form.sourceConnectionId || !form.targetConnectionId || !form.sourceDatabase}
            >
              {loading ? <Loader2 size={13} className="animate-spin" /> : <Copy size={13} />}
              开始迁移
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
