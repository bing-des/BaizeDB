import { useState } from 'react';
import { X, Database, Loader2, TestTube2, Plus } from 'lucide-react';
import { connectionApi, type NewConnectionInput } from '../../utils/api';
import { useConnectionStore } from '../../store';

interface Props {
  onClose: () => void;
}

const defaultForm: NewConnectionInput = {
  name: '',
  db_type: 'mysql',
  host: 'localhost',
  port: 3306,
  username: 'root',
  password: '',
  database: '',
  ssl: false,
};

export default function ConnectionDialog({ onClose }: Props) {
  const [form, setForm] = useState<NewConnectionInput>(defaultForm);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const { addConnection } = useConnectionStore();

  const set = (key: keyof NewConnectionInput, value: unknown) => {
    setForm((prev) => {
      const next = { ...prev, [key]: value };
      if (key === 'db_type') {
        if (value === 'mysql') {
          next.port = 3306;
          next.username = 'root';
        } else if (value === 'postgresql') {
          next.port = 5432;
          next.username = 'postgres';
        } else if (value === 'redis') {
          next.port = 6379;
          next.username = '';
          next.database = '0';
        }
      }
      return next;
    });
    setTestResult(null);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const msg = await connectionApi.test(form);
      setTestResult({ ok: true, msg });
    } catch (e) {
      setTestResult({ ok: false, msg: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!form.name.trim()) return;
    setSaving(true);
    try {
      const conn = await connectionApi.add(form);
      addConnection(conn);
      onClose();
    } catch (e) {
      setTestResult({ ok: false, msg: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-[var(--bg-secondary)] border border-[var(--border)] rounded-xl shadow-2xl w-[480px] max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-lg bg-brand-500/15 flex items-center justify-center">
              <Database size={14} className="text-brand-400" />
            </div>
            <span className="font-semibold text-[var(--text-primary)]">新建数据库连接</span>
          </div>
          <button onClick={onClose} className="btn-ghost p-1">
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-5 space-y-4">
          {/* DB Type */}
          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-2 uppercase tracking-wider">
              数据库类型
            </label>
            <div className="grid grid-cols-3 gap-2">
              {(['mysql', 'postgresql', 'redis'] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => set('db_type', t)}
                  className={`flex items-center gap-2 px-3 py-2.5 rounded-lg border text-sm font-medium transition-all ${
                    form.db_type === t
                      ? 'border-brand-500 bg-brand-500/10 text-brand-400'
                      : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--text-muted)]'
                  }`}
                >
                  <span
                    className="w-5 h-5 rounded flex items-center justify-center text-[10px] font-bold text-white flex-shrink-0"
                    style={{ background: t === 'mysql' ? '#4479A1' : t === 'postgresql' ? '#336791' : '#DC382D' }}
                  >
                    {t === 'mysql' ? 'M' : t === 'postgresql' ? 'P' : 'R'}
                  </span>
                  {t === 'mysql' ? 'MySQL' : t === 'postgresql' ? 'PostgreSQL' : 'Redis'}
                </button>
              ))}
            </div>
          </div>

          {/* Name */}
          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">连接名称 *</label>
            <input
              className="input-field"
              placeholder="我的生产数据库"
              value={form.name}
              onChange={(e) => set('name', e.target.value)}
            />
          </div>

          {/* Host + Port */}
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2">
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">主机地址</label>
              <input className="input-field" placeholder="localhost" value={form.host} onChange={(e) => set('host', e.target.value)} />
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">端口</label>
              <input className="input-field" type="number" value={form.port} onChange={(e) => set('port', parseInt(e.target.value) || 3306)} />
            </div>
          </div>

          {/* Username + Password (Redis 不需要用户名) */}
          {form.db_type !== 'redis' && (
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">用户名</label>
                <input className="input-field" value={form.username} onChange={(e) => set('username', e.target.value)} />
              </div>
              <div>
                <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">密码</label>
                <input className="input-field" type="password" value={form.password} onChange={(e) => set('password', e.target.value)} />
              </div>
            </div>
          )}

          {/* Redis: 只显示密码 */}
          {form.db_type === 'redis' && (
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">密码 <span className="normal-case">(可选)</span></label>
              <input className="input-field" type="password" value={form.password} onChange={(e) => set('password', e.target.value)} />
            </div>
          )}

          {/* Default DB */}
          <div>
            <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">
              {form.db_type === 'redis' ? '默认 DB 编号' : '默认数据库'}{' '}
              <span className="normal-case">(可选)</span>
            </label>
            <input
              className="input-field"
              placeholder={form.db_type === 'redis' ? '0' : '留空则列出所有数据库'}
              value={form.database ?? ''}
              onChange={(e) => set('database', e.target.value || '')}
            />
          </div>

          {/* SSL */}
          {form.db_type !== 'redis' && (
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={form.ssl}
                onChange={(e) => set('ssl', e.target.checked)}
                className="w-4 h-4 rounded accent-brand-500"
              />
              <span className="text-sm text-[var(--text-secondary)]">启用 SSL/TLS</span>
            </label>
          )}

          {/* Test result */}
          {testResult && (
            <div className={`px-3 py-2.5 rounded-lg text-sm border ${
              testResult.ok
                ? 'bg-green-500/10 border-green-500/30 text-green-400'
                : 'bg-red-500/10 border-red-500/30 text-red-400'
            }`}>
              {testResult.ok ? '✓ ' : '✗ '}{testResult.msg}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-4 border-t border-[var(--border)]">
          <button className="btn-secondary" onClick={handleTest} disabled={testing}>
            {testing ? <Loader2 size={13} className="animate-spin" /> : <TestTube2 size={13} />}
            测试连接
          </button>
          <div className="flex gap-2">
            <button className="btn-secondary" onClick={onClose}>取消</button>
            <button className="btn-primary" onClick={handleSave} disabled={!form.name.trim() || saving}>
              {saving ? <Loader2 size={13} className="animate-spin" /> : <Plus size={13} />}
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
