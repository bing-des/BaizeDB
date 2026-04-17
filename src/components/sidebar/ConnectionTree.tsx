import { useState, useCallback } from 'react';
import {
  ChevronRight, ChevronDown, Database, Table2,
  Loader2, PlugZap, TerminalSquare, Eye, Layers, Key,
  RefreshCw, Plus, Unplug, Trash2,
} from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { useConnectionStore, useTabStore } from '../../store';
import { connectionApi, databaseApi, redisApi } from '../../utils/api';
import type { ConnectionConfig, TableInfo, RedisKeyInfo, CreateTableInput } from '../../types';
import ContextMenu, { type MenuEntry } from '../common/ContextMenu';
import ConfirmModal from '../common/ConfirmModal';
import CreateTableModal from '../table/CreateTableModal';

interface SchemaNode {
  name: string;
  tables?: TableInfo[];
  expanded?: boolean;
  loading?: boolean;
}

interface RedisDbNode {
  index: number;
  keyCount: number;
  keys?: RedisKeyInfo[];
  expanded?: boolean;
  loading?: boolean;
}

interface DbNode {
  name: string;
  /** MySQL 直接存表 */
  tables?: TableInfo[];
  /** PostgreSQL 存 schema */
  schemas?: SchemaNode[];
  expanded?: boolean;
  loading?: boolean;
}

interface ConnNode {
  dbs?: DbNode[];
  /** Redis 存 db 列表 */
  redisDbs?: RedisDbNode[];
  expanded?: boolean;
  loading?: boolean;
}

type TreeState = Record<string, ConnNode>;

interface ContextMenuState {
  x: number;
  y: number;
  items: MenuEntry[];
}

export default function ConnectionTree() {
  const { connections, connectedIds, setConnected, removeConnection } = useConnectionStore();
  const { tabs, activeTabId, addTab, setActiveTab, removeTab } = useTabStore();
  const [tree, setTree] = useState<TreeState>({});
  const [connecting, setConnecting] = useState<string | null>(null);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);
  const [confirmState, setConfirmState] = useState<{
    message: string;
    onConfirm: () => void;
    danger: boolean;
  } | null>(null);

  // 创建表弹窗状态
  const [createTableState, setCreateTableState] = useState<{
    conn: ConnectionConfig;
    dbName: string;
    schema?: string;
  } | null>(null);

  const updateConn = (id: string, fn: (n: ConnNode) => ConnNode) =>
    setTree((p) => ({ ...p, [id]: fn(p[id] ?? {}) }));

  /* ─── 连接 ─── */
  const handleConnect = async (conn: ConnectionConfig) => {
    if (connectedIds.has(conn.id)) {
      updateConn(conn.id, (n) => ({ ...n, expanded: !n.expanded }));
      return;
    }
    setConnecting(conn.id);
    try {
      await connectionApi.connect(conn.id);
      setConnected(conn.id, true);
      updateConn(conn.id, (n) => ({ ...n, expanded: true, loading: true }));

      if (conn.db_type === 'redis') {
        const redisDbs = await redisApi.listDbs(conn.id);
        updateConn(conn.id, (n) => ({
          ...n, loading: false,
          redisDbs: redisDbs.map((d) => ({ index: d.index, keyCount: d.key_count })),
        }));
      } else {
        const dbs = await databaseApi.listDatabases(conn.id);
        updateConn(conn.id, (n) => ({
          ...n, loading: false,
          dbs: dbs.map((d) => ({ name: d.name })),
        }));
      }
    } catch (e) {
      alert(`连接失败: ${e}`);
    } finally {
      setConnecting(null);
    }
  };

  const handleDisconnect = async (id: string) => {
    await connectionApi.disconnect(id);
    setConnected(id, false);
    setTree((p) => { const n = { ...p }; delete n[id]; return n; });
  };

  /* ─── 刷新单个连接 ─── */
  const handleRefreshConn = async (conn: ConnectionConfig) => {
    if (!connectedIds.has(conn.id)) return;
    updateConn(conn.id, (n) => ({ ...n, loading: true, dbs: undefined, redisDbs: undefined }));
    try {
      if (conn.db_type === 'redis') {
        const redisDbs = await redisApi.listDbs(conn.id);
        updateConn(conn.id, (n) => ({
          ...n, loading: false,
          redisDbs: redisDbs.map((d) => ({ index: d.index, keyCount: d.key_count })),
        }));
      } else {
        const dbs = await databaseApi.listDatabases(conn.id);
        updateConn(conn.id, (n) => ({
          ...n, loading: false,
          dbs: dbs.map((d) => ({ name: d.name })),
        }));
      }
    } catch (e) {
      alert(`刷新失败: ${e}`);
      updateConn(conn.id, (n) => ({ ...n, loading: false }));
    }
  };

  /* ─── 刷新所有连接 ─── */
  const handleRefreshAll = async () => {
    const ids = Array.from(connectedIds);
    await Promise.allSettled(
      ids.map(async (id) => {
        const conn = connections.find((c) => c.id === id);
        if (conn) await handleRefreshConn(conn);
      })
    );
  };

  /* ─── 展开 DB ─── */
  const handleExpandDb = async (conn: ConnectionConfig, dbName: string) => {
    const db = tree[conn.id]?.dbs?.find((d) => d.name === dbName);
    if (!db) return;
    if (db.expanded) {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, expanded: false } : d),
      }));
      return;
    }
    if (conn.db_type === 'mysql') {
      if (!db.tables) {
        updateConn(conn.id, (n) => ({
          ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, loading: true } : d),
        }));
        const tables = await databaseApi.listTables(conn.id, dbName);
        updateConn(conn.id, (n) => ({
          ...n, dbs: n.dbs?.map((d) =>
            d.name === dbName ? { ...d, tables, loading: false, expanded: true } : d
          ),
        }));
      } else {
        updateConn(conn.id, (n) => ({
          ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, expanded: true } : d),
        }));
      }
      return;
    }
    // PostgreSQL
    if (!db.schemas) {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, loading: true } : d),
      }));
      const schemas = await databaseApi.listSchemas(conn.id, dbName);
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) =>
          d.name === dbName
            ? { ...d, schemas: schemas.map((s) => ({ name: s.name })), loading: false, expanded: true }
            : d
        ),
      }));
    } else {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, expanded: true } : d),
      }));
    }
  };

  /* ─── 展开 Schema ─── */
  const handleExpandSchema = async (conn: ConnectionConfig, dbName: string, schemaName: string) => {
    const db = tree[conn.id]?.dbs?.find((d) => d.name === dbName);
    const schema = db?.schemas?.find((s) => s.name === schemaName);
    if (!schema) return;
    if (schema.expanded) {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, expanded: false } : s) }
          : d
        ),
      }));
      return;
    }
    if (!schema.tables) {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, loading: true } : s) }
          : d
        ),
      }));
      const tables = await databaseApi.listTables(conn.id, dbName, schemaName);
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, tables, loading: false, expanded: true } : s) }
          : d
        ),
      }));
    } else {
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, expanded: true } : s) }
          : d
        ),
      }));
    }
  };

  /* ─── 打开查询 ─── */
  const openQuery = (conn: ConnectionConfig, db?: string) => {
    // PG 必须有 database 才能查询，缺省用连接配置的默认库
    const effectiveDb = db ?? conn.database;
    addTab({
      id: uuidv4(),
      title: effectiveDb ? `查询·${effectiveDb}` : `查询·${conn.name}`,
      type: 'query',
      connectionId: conn.id,
      database: effectiveDb,
      content: `-- ${conn.name}${effectiveDb ? ' > ' + effectiveDb : ''}\n\n`,
    });
  };

  const openVisualization = (conn: ConnectionConfig, db: string, schema?: string) => {
    const title = schema ? `可视化·${db}.${schema}` : `可视化·${db}`;
    addTab({
      id: uuidv4(),
      title,
      type: 'visualization',
      connectionId: conn.id,
      database: db,
      schema,
    });
  };

  const openChartDB = (conn: ConnectionConfig, db: string, schema?: string) => {
    const title = schema ? `ChartDB·${db}.${schema}` : `ChartDB·${db}`;
    addTab({
      id: uuidv4(),
      title,
      type: 'chartdb',
      connectionId: conn.id,
      database: db,
      schema,
    });
  };

  const openTable = (conn: ConnectionConfig, db: string, table: string, schema?: string) => {
    // PG 的表名需要带 schema 前缀（如 "platform_app.app_role"），否则后续查询找不到表
    const fullTableName = schema && schema !== 'public' ? `${schema}.${table}` : table;
    
    // 检查是否已存在相同的表标签页
    const existingTab = tabs.find(t => 
      t.type === 'table' && 
      t.connectionId === conn.id && 
      t.database === db && 
      t.table === fullTableName
    );
    
    if (existingTab) {
      // 切换到已存在的标签页
      setActiveTab(existingTab.id);
    } else {
      // 创建新标签页
      addTab({
        id: uuidv4(),
        title: table,
        type: 'table',
        connectionId: conn.id,
        database: db,
        table: fullTableName,
      });
    }
  };

  /* ─── Redis ─── */
  const handleExpandRedisDb = async (conn: ConnectionConfig, dbIndex: number) => {
    const redisDb = tree[conn.id]?.redisDbs?.find((d) => d.index === dbIndex);
    if (!redisDb) return;
    if (redisDb.expanded) {
      updateConn(conn.id, (n) => ({
        ...n, redisDbs: n.redisDbs?.map((d) => d.index === dbIndex ? { ...d, expanded: false } : d),
      }));
      return;
    }
    if (!redisDb.keys) {
      updateConn(conn.id, (n) => ({
        ...n, redisDbs: n.redisDbs?.map((d) => d.index === dbIndex ? { ...d, loading: true } : d),
      }));
      const result = await redisApi.listKeys(conn.id, dbIndex);
      updateConn(conn.id, (n) => ({
        ...n, redisDbs: n.redisDbs?.map((d) =>
          d.index === dbIndex ? { ...d, keys: result.keys, loading: false, expanded: true } : d
        ),
      }));
    } else {
      updateConn(conn.id, (n) => ({
        ...n, redisDbs: n.redisDbs?.map((d) => d.index === dbIndex ? { ...d, expanded: true } : d),
      }));
    }
  };

  const openRedisKey = (conn: ConnectionConfig, dbIndex: number, key: string) => {
    addTab({
      id: uuidv4(),
      title: key,
      type: 'redis-key',
      connectionId: conn.id,
      redisDbIndex: dbIndex,
      redisKey: key,
    });
  };

  /* ─── 刷新 Database（重新加载表/schema 列表） ─── */
  const handleRefreshDb = async (conn: ConnectionConfig, dbName: string) => {
    updateConn(conn.id, (n) => ({
      ...n, dbs: n.dbs?.map((d) =>
        d.name === dbName ? { ...d, loading: true, tables: undefined, schemas: undefined } : d
      ),
    }));
    try {
      if (conn.db_type === 'mysql') {
        const tables = await databaseApi.listTables(conn.id, dbName);
        updateConn(conn.id, (n) => ({
          ...n, dbs: n.dbs?.map((d) =>
            d.name === dbName ? { ...d, tables, loading: false, expanded: true } : d
          ),
        }));
      } else {
        const schemas = await databaseApi.listSchemas(conn.id, dbName);
        updateConn(conn.id, (n) => ({
          ...n, dbs: n.dbs?.map((d) =>
            d.name === dbName
              ? { ...d, schemas: schemas.map((s) => ({ name: s.name })), loading: false, expanded: true }
              : d
          ),
        }));
      }
    } catch (e) {
      alert(`刷新失败: ${e}`);
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName ? { ...d, loading: false } : d),
      }));
    }
  };

  /* ─── 刷新 Schema（重新加载表列表） ─── */
  const handleRefreshSchema = async (conn: ConnectionConfig, dbName: string, schemaName: string) => {
    updateConn(conn.id, (n) => ({
      ...n, dbs: n.dbs?.map((d) => d.name === dbName
        ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, loading: true, tables: undefined } : s) }
        : d
      ),
    }));
    try {
      const tables = await databaseApi.listTables(conn.id, dbName, schemaName);
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, tables, loading: false, expanded: true } : s) }
          : d
        ),
      }));
    } catch (e) {
      alert(`刷新失败: ${e}`);
      updateConn(conn.id, (n) => ({
        ...n, dbs: n.dbs?.map((d) => d.name === dbName
          ? { ...d, schemas: d.schemas?.map((s) => s.name === schemaName ? { ...s, loading: false } : s) }
          : d
        ),
      }));
    }
  };

  /* ─── 右键菜单 ─── */
  const showConnContextMenu = useCallback((e: React.MouseEvent, conn: ConnectionConfig) => {
    e.preventDefault();
    e.stopPropagation();
    const isConnected = connectedIds.has(conn.id);
    const items: MenuEntry[] = [];
    if (isConnected && conn.db_type !== 'redis') {
      items.push({
        label: '新建查询',
        icon: <TerminalSquare size={13} />,
        onClick: () => openQuery(conn),
      });
    }
    items.push({
      label: '刷新',
      icon: <RefreshCw size={13} />,
      onClick: () => handleRefreshConn(conn),
      disabled: !isConnected,
    });
    if (isConnected) {
      items.push({ separator: true });
      items.push({
        label: '断开连接',
        icon: <Unplug size={13} />,
        onClick: () => handleDisconnect(conn.id),
        danger: true,
      });
    }
    items.push({ separator: true });
    items.push({
      label: '删除连接',
      icon: <Trash2 size={13} />,
      onClick: () => handleDeleteConnection(conn),
      danger: true,
    });
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, [connectedIds, connections]);

  const showDbContextMenu = useCallback((e: React.MouseEvent, conn: ConnectionConfig, dbName: string) => {
    e.preventDefault();
    e.stopPropagation();
    const items: MenuEntry[] = [
      {
        label: '新建查询',
        icon: <TerminalSquare size={13} />,
        onClick: () => openQuery(conn, dbName),
      },
      {
        label: '新建表',
        icon: <Plus size={13} />,
        onClick: () => setCreateTableState({ conn, dbName }),
      },
      {
        label: '查看可视化',
        icon: <Eye size={13} />,
        onClick: () => openVisualization(conn, dbName),
      },
      {
        label: 'ChartDB 可视化',
        icon: <Eye size={13} />,
        onClick: () => openChartDB(conn, dbName),
      },
      {
        label: '刷新',
        icon: <RefreshCw size={13} />,
        onClick: () => handleRefreshDb(conn, dbName),
      },
      { separator: true },
      {
        label: '删除数据库',
        icon: <Trash2 size={13} />,
        onClick: () => handleDeleteDatabase(conn, dbName),
        danger: true,
      },
    ];
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, [connections]);

  const showSchemaContextMenu = useCallback((e: React.MouseEvent, conn: ConnectionConfig, dbName: string, schemaName: string) => {
    e.preventDefault();
    e.stopPropagation();
    const items: MenuEntry[] = [
      {
        label: '新建查询',
        icon: <TerminalSquare size={13} />,
        onClick: () => openQuery(conn, dbName),
      },
      {
        label: '新建表',
        icon: <Plus size={13} />,
        onClick: () => setCreateTableState({ conn, dbName, schema: schemaName }),
      },
      {
        label: '查看可视化',
        icon: <Eye size={13} />,
        onClick: () => openVisualization(conn, dbName, schemaName),
      },
      {
        label: 'ChartDB 可视化',
        icon: <Eye size={13} />,
        onClick: () => openChartDB(conn, dbName, schemaName),
      },
      {
        label: '刷新',
        icon: <RefreshCw size={13} />,
        onClick: () => handleRefreshSchema(conn, dbName, schemaName),
      },
    ];
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, [connections]);

  const showTableContextMenu = useCallback((e: React.MouseEvent, conn: ConnectionConfig, dbName: string, schemaName: string | undefined, tableName?: string) => {
    e.preventDefault();
    e.stopPropagation();
    const items: MenuEntry[] = [
      {
        label: '刷新',
        icon: <RefreshCw size={13} />,
        onClick: () => {
          if (schemaName) {
            handleRefreshSchema(conn, dbName, schemaName);
          } else {
            handleRefreshDb(conn, dbName);
          }
        },
      },
      { separator: true },
      {
        label: '删除表',
        icon: <Trash2 size={13} />,
        onClick: () => {
          // tableName 来自事件绑定时的闭包（见下面 onContextMenu 绑定）
          const tbl = tableName ?? '';
          if (tbl) handleDeleteTable(conn, dbName, tbl, schemaName);
        },
        danger: true,
      },
    ];
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, [connections]);

  const showBlankContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const items: MenuEntry[] = [
      {
        label: '新建连接',
        icon: <Plus size={13} />,
        onClick: () => {
          window.dispatchEvent(new CustomEvent('baizedb:new-connection'));
        },
      },
      {
        label: '刷新全部',
        icon: <RefreshCw size={13} />,
        onClick: () => handleRefreshAll(),
        disabled: connectedIds.size === 0,
      },
    ];
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, [connectedIds]);

  /* ─── 删除操作 ─── */
  const handleDeleteConnection = (conn: ConnectionConfig) => {
    setConfirmState({
      message: `确定要删除连接「${conn.name}」吗？`,
      danger: true,
      onConfirm: async () => {
        if (connectedIds.has(conn.id)) {
          await connectionApi.disconnect(conn.id);
          setConnected(conn.id, false);
        }
        await connectionApi.remove(conn.id);
        removeConnection(conn.id);
        setTree((p) => { const n = { ...p }; delete n[conn.id]; return n; });
        setConfirmState(null);
      },
    });
  };

  const handleDeleteDatabase = (conn: ConnectionConfig, dbName: string) => {
    setConfirmState({
      message: `确定要删除数据库「${dbName}」吗？此操作不可恢复！`,
      danger: true,
      onConfirm: async () => {
        try {
          await databaseApi.dropDatabase(conn.id, dbName);
          // 从树中移除该库
          updateConn(conn.id, (n) => ({
            ...n, dbs: n.dbs?.filter((d) => d.name !== dbName),
          }));
          // 关闭相关标签页
          const relatedTabs = tabs.filter(t =>
            t.type === 'table' || t.type === 'query'
          ).filter(t => t.database === dbName && t.connectionId === conn.id);
          for (const tab of relatedTabs) {
            removeTab(tab.id);
        }
        } catch (err: any) {
          alert(`删除数据库失败: ${err}`);
        }
        setConfirmState(null);
      },
    });
  };

  const handleDeleteTable = (conn: ConnectionConfig, db: string, tableName: string, schema?: string) => {
    setConfirmState({
      message: `确定要删除表「${tableName}」吗？此操作不可恢复！`,
      danger: true,
      onConfirm: async () => {
        try {
          await databaseApi.dropTable(conn.id, db, tableName, schema);
          // 刷新父节点
          if (schema) {
            handleRefreshSchema(conn, db, schema);
          } else {
            handleRefreshDb(conn, db);
          }
          // 关闭相关标签页
          const fullTable = schema && schema !== 'public' ? `${schema}.${tableName}` : tableName;
          const existingTab = tabs.find(t =>
            t.type === 'table' &&
            t.connectionId === conn.id &&
            t.table === fullTable
          );
          if (existingTab) removeTab(existingTab.id);
        } catch (err: any) {
          alert(`删除表失败: ${err}`);
        }
        setConfirmState(null);
      },
    });
  };

  const handleCreateTable = async (input: CreateTableInput) => {
    if (!createTableState) return;
    const { conn, dbName, schema } = createTableState;
    try {
      await databaseApi.createTable(conn.id, dbName, schema, input);
      // 刷新父节点
      if (schema) {
        handleRefreshSchema(conn, dbName, schema);
      } else {
        handleRefreshDb(conn, dbName);
      }
    } catch (err: any) {
      throw err;
    }
  };

  /* ─── 空状态 ─── */
  if (connections.length === 0) {
    return (
      <div
        className="flex flex-col items-center justify-center h-32 gap-2 text-[var(--text-muted)]"
        onContextMenu={showBlankContextMenu}
      >
        <Database size={28} strokeWidth={1} />
        <p className="text-xs text-center">暂无连接，点击 + 新建</p>
      </div>
    );
  }

  return (
    <div
      className="px-1 space-y-0.5 h-full"
      onContextMenu={showBlankContextMenu}
    >
      {connections.map((conn) => {
        const node = tree[conn.id] ?? {};
        const isConnected = connectedIds.has(conn.id);
        const isConnecting = connecting === conn.id;

        return (
          <div key={conn.id}>
            {/* Connection row */}
            <div
              className="tree-item group justify-between"
              onClick={() => handleConnect(conn)}
              onContextMenu={(e) => showConnContextMenu(e, conn)}
            >
              <div className="flex items-center gap-1.5 min-w-0">
                {isConnecting ? (
                  <Loader2 size={13} className="animate-spin text-brand-400 flex-shrink-0" />
                ) : isConnected && node.expanded ? (
                  <ChevronDown size={13} className="flex-shrink-0 text-[var(--text-muted)]" />
                ) : (
                  <ChevronRight size={13} className="flex-shrink-0 text-[var(--text-muted)]" />
                )}
                <div className={`status-dot ${isConnected ? 'connected' : 'disconnected'}`} />
                <DbBadge type={conn.db_type} />
                <span className="truncate text-[var(--text-primary)] font-medium">{conn.name}</span>
              </div>
              {/* hover 小按钮保留：查询 + 断开，作为快捷入口 */}
              <div className="hidden group-hover:flex items-center gap-0.5 ml-1 flex-shrink-0">
                {isConnected && conn.db_type !== 'redis' && (
                  <ActionBtn title="新建查询" onClick={(e) => { e.stopPropagation(); openQuery(conn); }}>
                    <TerminalSquare size={11} />
                  </ActionBtn>
                )}
                {isConnected && (
                  <ActionBtn title="断开" onClick={(e) => { e.stopPropagation(); handleDisconnect(conn.id); }}>
                    <PlugZap size={11} />
                  </ActionBtn>
                )}
              </div>
            </div>

            {/* Databases (MySQL/PG) */}
            {isConnected && node.expanded && conn.db_type !== 'redis' && (
              <div className="ml-4 pl-2 border-l border-[var(--border)]">
                {node.loading ? (
                  <div className="flex items-center gap-1.5 py-1.5 text-xs text-[var(--text-muted)]">
                    <Loader2 size={11} className="animate-spin" /> 加载中...
                  </div>
                ) : (
                  node.dbs?.map((db) => (
                    <div key={db.name}>
                      {/* Database row */}
                      <div className="tree-item group justify-between" onClick={() => handleExpandDb(conn, db.name)} onContextMenu={(e) => showDbContextMenu(e, conn, db.name)}>
                        <div className="flex items-center gap-1.5 min-w-0">
                          {db.loading ? (
                            <Loader2 size={11} className="animate-spin flex-shrink-0" />
                          ) : db.expanded ? (
                            <ChevronDown size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                          ) : (
                            <ChevronRight size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                          )}
                          <Database size={12} className="text-brand-400 flex-shrink-0" />
                          <span className="truncate">{db.name}</span>
                        </div>
                        <div className="hidden group-hover:flex ml-1 flex-shrink-0">
                          <ActionBtn title="新建查询" onClick={(e) => { e.stopPropagation(); openQuery(conn, db.name); }}>
                            <TerminalSquare size={11} />
                          </ActionBtn>
                        </div>
                      </div>

                      {/* MySQL: 直接显示表列表 */}
                      {db.expanded && db.tables && (
                        <div className="ml-4 pl-2 border-l border-[var(--border)]">
                          {db.tables.length === 0 && (
                            <div className="py-1 text-xs text-[var(--text-muted)] px-2">无表</div>
                          )}
                          {db.tables.map((tbl) => (
                            <div
                              key={tbl.name}
                              className="tree-item"
                              onClick={(e) => { e.stopPropagation(); openTable(conn, db.name, tbl.name); }}
                              onContextMenu={(e) => showTableContextMenu(e, conn, db.name, undefined, tbl.name)}
                            >
                              {tbl.table_type?.includes('VIEW') ? (
                                <Eye size={11} className="text-purple-400 flex-shrink-0" />
                              ) : (
                                <Table2 size={11} className="text-[var(--text-secondary)] flex-shrink-0" />
                              )}
                              <span className="truncate flex-1">{tbl.name}</span>
                              {tbl.row_count != null && (
                                <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0">
                                  {tbl.row_count.toLocaleString()}
                                </span>
                              )}
                            </div>
                          ))}
                        </div>
                      )}

                      {/* PostgreSQL: 显示 schema 列表 */}
                      {db.expanded && db.schemas && (
                        <div className="ml-4 pl-2 border-l border-[var(--border)]">
                          {db.schemas?.map((schema) => (
                            <div key={schema.name}>
                              {/* Schema row */}
                              <div className="tree-item group justify-between" onClick={(e) => { e.stopPropagation(); handleExpandSchema(conn, db.name, schema.name); }} onContextMenu={(e) => showSchemaContextMenu(e, conn, db.name, schema.name)}>
                                <div className="flex items-center gap-1.5 min-w-0">
                                  {schema.loading ? (
                                    <Loader2 size={11} className="animate-spin flex-shrink-0" />
                                  ) : schema.expanded ? (
                                    <ChevronDown size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                                  ) : (
                                    <ChevronRight size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                                  )}
                                  <Layers size={11} className="text-[var(--text-secondary)] flex-shrink-0" />
                                  <span className="truncate text-[var(--text-secondary)]">{schema.name}</span>
                                </div>
                              </div>

                              {/* Tables under schema */}
                              {schema.expanded && (
                                <div className="ml-4 pl-2 border-l border-[var(--border)]">
                                  {schema.tables?.length === 0 && (
                                    <div className="py-1 text-xs text-[var(--text-muted)] px-2">无表</div>
                                  )}
                                  {schema.tables?.map((tbl) => (
                                    <div
                                      key={tbl.name}
                                      className="tree-item"
                                      onClick={(e) => { e.stopPropagation(); openTable(conn, db.name, tbl.name, schema.name); }}
                                      onContextMenu={(e) => showTableContextMenu(e, conn, db.name, schema.name, tbl.name)}
                                    >
                                      {tbl.table_type?.includes('VIEW') ? (
                                        <Eye size={11} className="text-purple-400 flex-shrink-0" />
                                      ) : (
                                        <Table2 size={11} className="text-[var(--text-secondary)] flex-shrink-0" />
                                      )}
                                      <span className="truncate flex-1">{tbl.name}</span>
                                      {tbl.row_count != null && (
                                        <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0">
                                          {tbl.row_count.toLocaleString()}
                                        </span>
                                      )}
                                    </div>
                                  ))}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ))
                )}
              </div>
            )}

            {/* Redis: DB 列表 */}
            {isConnected && node.expanded && conn.db_type === 'redis' && (
              <div className="ml-4 pl-2 border-l border-[var(--border)]">
                {node.loading ? (
                  <div className="flex items-center gap-1.5 py-1.5 text-xs text-[var(--text-muted)]">
                    <Loader2 size={11} className="animate-spin" /> 加载中...
                  </div>
                ) : (
                  node.redisDbs?.map((redisDb) => (
                    <div key={redisDb.index}>
                      {/* Redis DB row */}
                      <div className="tree-item group justify-between" onClick={(e) => { e.stopPropagation(); handleExpandRedisDb(conn, redisDb.index); }}>
                        <div className="flex items-center gap-1.5 min-w-0">
                          {redisDb.loading ? (
                            <Loader2 size={11} className="animate-spin flex-shrink-0" />
                          ) : redisDb.expanded ? (
                            <ChevronDown size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                          ) : (
                            <ChevronRight size={11} className="flex-shrink-0 text-[var(--text-muted)]" />
                          )}
                          <Database size={12} className="text-red-400 flex-shrink-0" />
                          <span className="truncate">db{redisDb.index}</span>
                          <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0">
                            {redisDb.keyCount} keys
                          </span>
                        </div>
                      </div>

                      {/* Keys under Redis DB */}
                      {redisDb.expanded && (
                        <div className="ml-4 pl-2 border-l border-[var(--border)]">
                          {redisDb.keys?.length === 0 && (
                            <div className="py-1 text-xs text-[var(--text-muted)] px-2">无 key</div>
                          )}
                          {redisDb.keys?.map((k) => (
                            <div
                              key={k.name}
                              className="tree-item"
                              onClick={(e) => { e.stopPropagation(); openRedisKey(conn, redisDb.index, k.name); }}
                            >
                              <Key size={11} className={`flex-shrink-0 ${
                                k.key_type === 'string' ? 'text-green-400' :
                                k.key_type === 'hash' ? 'text-blue-400' :
                                k.key_type === 'list' ? 'text-yellow-400' :
                                k.key_type === 'set' ? 'text-purple-400' :
                                k.key_type === 'zset' ? 'text-orange-400' :
                                'text-[var(--text-secondary)]'
                              }`} />
                              <span className="truncate flex-1">{k.name}</span>
                              <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0">
                                {k.key_type}
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ))
                )}
              </div>
            )}
          </div>
        );
      })}

      {/* 右键菜单 */}
      {ctxMenu && (
        <ContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          items={ctxMenu.items}
          onClose={() => setCtxMenu(null)}
        />
      )}

      {/* 删除确认弹窗 */}
      {confirmState && (
        <ConfirmModal
          message={confirmState.message}
          onConfirm={() => confirmState.onConfirm()}
          onCancel={() => setConfirmState(null)}
          danger={confirmState.danger}
        />
      )}

      {/* 创建表弹窗 */}
      {createTableState && (
        <CreateTableModal
          isOpen={true}
          isPostgres={createTableState.conn.db_type === 'postgresql'}
          database={createTableState.dbName}
          schema={createTableState.schema}
          onClose={() => setCreateTableState(null)}
          onSubmit={handleCreateTable}
        />
      )}
    </div>
  );
}

function ActionBtn({ children, title, onClick }: {
  children: React.ReactNode;
  title: string;
  onClick: (e: React.MouseEvent) => void;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      className="p-0.5 rounded hover:bg-[var(--bg-primary)] hover:text-brand-400 text-[var(--text-muted)] transition-colors"
    >
      {children}
    </button>
  );
}

function DbBadge({ type }: { type: string }) {
  const colors: Record<string, string> = {
    mysql: '#4479A1',
    postgresql: '#336791',
    redis: '#DC382D',
  };
  const labels: Record<string, string> = {
    mysql: 'M',
    postgresql: 'P',
    redis: 'R',
  };
  return (
    <div
      className="w-4 h-4 rounded flex-shrink-0 flex items-center justify-center text-[9px] font-bold text-white"
      style={{ background: colors[type] || '#666' }}
    >
      {labels[type] || '?'}
    </div>
  );
}
