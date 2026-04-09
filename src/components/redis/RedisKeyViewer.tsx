import { useState, useEffect } from 'react';
import { Key, Loader2, RefreshCw, Trash2, Clock, Tag } from 'lucide-react';
import { redisApi } from '../../utils/api';
import type { Tab, RedisKeyValue } from '../../types';

export default function RedisKeyViewer({ tab }: { tab: Tab }) {
  const [data, setData] = useState<RedisKeyValue | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    if (!tab.redisDbIndex !== undefined && !tab.redisKey) return;
    setLoading(true);
    setError(null);
    try {
      const result = await redisApi.getKey(tab.connectionId, tab.redisDbIndex ?? 0, tab.redisKey ?? '');
      setData(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, [tab.connectionId, tab.redisDbIndex, tab.redisKey]);

  const handleDelete = async () => {
    if (!confirm(`确定删除 key "${tab.redisKey}"?`)) return;
    try {
      await redisApi.delKey(tab.connectionId, tab.redisDbIndex ?? 0, tab.redisKey ?? '');
      setData(null);
      setError('Key 已删除');
    } catch (e) {
      setError(String(e));
    }
  };

  const formatTtl = (ttl: number) => {
    if (ttl === -1) return '永不过期';
    if (ttl === -2) return 'Key 不存在';
    if (ttl < 60) return `${ttl} 秒`;
    if (ttl < 3600) return `${Math.floor(ttl / 60)} 分 ${ttl % 60} 秒`;
    if (ttl < 86400) return `${Math.floor(ttl / 3600)} 时 ${Math.floor((ttl % 3600) / 60)} 分`;
    return `${Math.floor(ttl / 86400)} 天 ${Math.floor((ttl % 86400) / 3600)} 时`;
  };

  const typeColors: Record<string, string> = {
    string: 'text-green-400 bg-green-500/10',
    list: 'text-yellow-400 bg-yellow-500/10',
    set: 'text-purple-400 bg-purple-500/10',
    zset: 'text-orange-400 bg-orange-500/10',
    hash: 'text-blue-400 bg-blue-500/10',
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
        <div className="flex items-center gap-3 min-w-0">
          <Key size={16} className="text-red-400 flex-shrink-0" />
          <div className="min-w-0">
            <div className="text-sm font-medium text-[var(--text-primary)] truncate">{tab.redisKey}</div>
            <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
              <span className="flex items-center gap-1">
                <Database2Icon />
                db{tab.redisDbIndex}
              </span>
              {data && (
                <>
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${typeColors[data.key_type] || 'text-[var(--text-secondary)] bg-[var(--bg-primary)]'}`}>
                    {data.key_type}
                  </span>
                  <span className="flex items-center gap-1">
                    <Clock size={10} />
                    {formatTtl(data.ttl)}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            className="btn-ghost p-1.5"
            onClick={fetchData}
            title="刷新"
          >
            <RefreshCw size={13} />
          </button>
          {data && (
            <button
              className="btn-ghost p-1.5 hover:text-red-400"
              onClick={handleDelete}
              title="删除 Key"
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4">
        {loading ? (
          <div className="flex items-center justify-center py-12 text-[var(--text-muted)]">
            <Loader2 size={20} className="animate-spin" />
          </div>
        ) : error ? (
          <div className="px-3 py-2.5 rounded-lg text-sm bg-red-500/10 border border-red-500/30 text-red-400">
            {error}
          </div>
        ) : data ? (
          <ValueDisplay data={data} />
        ) : null}
      </div>
    </div>
  );
}

function Database2Icon() {
  return (
    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <ellipse cx="12" cy="5" rx="9" ry="3"/>
      <path d="M3 5V19A9 3 0 0 0 21 19V5"/>
      <path d="M3 12A9 3 0 0 0 21 12"/>
    </svg>
  );
}

function ValueDisplay({ data }: { data: RedisKeyValue }) {
  const renderValue = () => {
    switch (data.key_type) {
      case 'string':
        return (
          <pre className="whitespace-pre-wrap break-all text-sm text-[var(--text-primary)] font-mono bg-[var(--bg-secondary)] border border-[var(--border)] rounded-lg p-4">
            {typeof data.value === 'string' ? data.value : JSON.stringify(data.value, null, 2)}
          </pre>
        );

      case 'hash':
        return (
          <div className="border border-[var(--border)] rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-[var(--bg-secondary)]">
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)]">Field</th>
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)]">Value</th>
                </tr>
              </thead>
              <tbody>
                {Object.entries(data.value as Record<string, string>).map(([field, value]) => (
                  <tr key={field} className="border-b border-[var(--border)] last:border-0">
                    <td className="px-3 py-2 font-mono text-brand-400 border-r border-[var(--border)]">{field}</td>
                    <td className="px-3 py-2 font-mono text-[var(--text-primary)]">{value}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );

      case 'list':
        return (
          <div className="border border-[var(--border)] rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-[var(--bg-secondary)]">
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)] w-16">Index</th>
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)]">Value</th>
                </tr>
              </thead>
              <tbody>
                {(data.value as string[]).map((item, i) => (
                  <tr key={i} className="border-b border-[var(--border)] last:border-0">
                    <td className="px-3 py-2 text-[var(--text-muted)] border-r border-[var(--border)]">{i}</td>
                    <td className="px-3 py-2 font-mono text-[var(--text-primary)]">{item}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );

      case 'set':
        return (
          <div className="flex flex-wrap gap-2">
            {(data.value as string[]).map((item, i) => (
              <span key={i} className="px-2.5 py-1 rounded-md bg-purple-500/10 text-purple-400 text-sm font-mono">
                {item}
              </span>
            ))}
          </div>
        );

      case 'zset':
        return (
          <div className="border border-[var(--border)] rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-[var(--bg-secondary)]">
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)] w-20">Score</th>
                  <th className="text-left px-3 py-2 font-medium text-[var(--text-muted)] border-b border-[var(--border)]">Member</th>
                </tr>
              </thead>
              <tbody>
                {(data.value as Array<{member: string; score: number}>).map((item, i) => (
                  <tr key={i} className="border-b border-[var(--border)] last:border-0">
                    <td className="px-3 py-2 text-orange-400 font-mono border-r border-[var(--border)]">{item.score}</td>
                    <td className="px-3 py-2 font-mono text-[var(--text-primary)]">{item.member}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );

      default:
        return (
          <pre className="whitespace-pre-wrap text-sm text-[var(--text-muted)]">
            {JSON.stringify(data.value, null, 2)}
          </pre>
        );
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
        <Tag size={12} />
        <span>类型: <strong className="text-[var(--text-secondary)]">{data.key_type}</strong></span>
        <span>·</span>
        <span>大小: <strong className="text-[var(--text-secondary)]">{getValueSize(data)}</strong></span>
      </div>
      {renderValue()}
    </div>
  );
}

function getValueSize(data: RedisKeyValue): string {
  switch (data.key_type) {
    case 'string':
      return `${(data.value as string).length} chars`;
    case 'list':
    case 'set':
      return `${(data.value as string[]).length} items`;
    case 'zset':
      return `${(data.value as unknown[]).length} members`;
    case 'hash':
      return `${Object.keys(data.value as Record<string, string>).length} fields`;
    default:
      return '-';
  }
}
