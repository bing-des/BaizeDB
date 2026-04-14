import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ConnectionConfig,
  DatabaseInfo,
  SchemaInfo,
  TableInfo,
  ColumnInfo,
  QueryResult,
  TableDataResult,
  RedisDbInfo,
  RedisScanResult,
  RedisKeyValue,
  MigrationInput,
  MigrationProgress,
} from '../types';

export type NewConnectionInput = Omit<ConnectionConfig, 'id'>;

export const connectionApi = {
  add: (input: NewConnectionInput) =>
    invoke<ConnectionConfig>('add_connection', { input }),
  remove: (id: string) =>
    invoke<void>('remove_connection', { id }),
  list: () =>
    invoke<ConnectionConfig[]>('list_connections'),
  test: (input: NewConnectionInput) =>
    invoke<string>('test_connection', { input }),
  /** 连接数据库（后端已自动从磁盘加载配置，无需传 configs） */
  connect: (id: string) =>
    invoke<void>('connect_db', { id, options: null }),
  disconnect: (id: string) =>
    invoke<void>('disconnect_db', { id }),
  /** 手动保存连接配置到磁盘 */
  save: () =>
    invoke<void>('save_connections'),
  /** 从磁盘重新加载连接配置 */
  load: () =>
    invoke<ConnectionConfig[]>('load_connections'),
};

export const databaseApi = {
  listDatabases: (connectionId: string) =>
    invoke<DatabaseInfo[]>('list_databases', { connectionId }),
  listSchemas: (connectionId: string, database: string) =>
    invoke<SchemaInfo[]>('list_schemas', { connectionId, database }),
  listTables: (connectionId: string, database: string, schema?: string) =>
    invoke<TableInfo[]>('list_tables', { connectionId, database, schema }),
  listColumns: (connectionId: string, database: string, table: string) =>
    invoke<ColumnInfo[]>('list_columns', { connectionId, database, table }),
  getTableData: (connectionId: string, database: string, table: string, page: number, pageSize: number) =>
    invoke<TableDataResult>('get_table_data', { connectionId, database, table, page, pageSize }),
  getRowCount: (connectionId: string, database: string, table: string) =>
    invoke<number>('get_table_row_count', { connectionId, database, table }),
  /** 更新表格数据（批量更新多行） */
  updateTableData: (connectionId: string, database: string, table: string, primaryKey: string, primaryKeyType: string, updates: Array<{ row_index: number; primary_key_value: any; column_values: Record<string, any>; column_types: Record<string, string> }>) =>
    invoke<number>('update_table_data', { connectionId, database, table, primaryKey, primaryKeyType, updates }),
  /** 删除表格数据（根据主键删除多行） */
  deleteTableData: (connectionId: string, database: string, table: string, primaryKey: string, primaryKeyType: string, primaryKeyValues: Array<any>) =>
    invoke<number>('delete_table_data', { connectionId, database, table, primaryKey, primaryKeyType, primaryKeyValues }),
  /** 插入一行新数据到表格 */
  insertTableData: (connectionId: string, database: string, table: string, columnValues: Record<string, any>, columnTypes?: Record<string, string>) =>
    invoke<number>('insert_table_data', { connectionId, database, table, columnValues, columnTypes }),
};

export const redisApi = {
  listDbs: (connectionId: string) =>
    invoke<RedisDbInfo[]>('redis_list_dbs', { connectionId }),
  listKeys: (connectionId: string, dbIndex: number, pattern?: string, cursor?: number, count?: number) =>
    invoke<RedisScanResult>('redis_list_keys', { connectionId, dbIndex: dbIndex, pattern, cursor, count }),
  getKey: (connectionId: string, dbIndex: number, key: string) =>
    invoke<RedisKeyValue>('redis_get_key', { connectionId, dbIndex: dbIndex, key }),
  setKey: (connectionId: string, dbIndex: number, key: string, value: string, keyType: string) =>
    invoke<void>('redis_set_key', { connectionId, dbIndex: dbIndex, key, value, keyType }),
  delKey: (connectionId: string, dbIndex: number, key: string) =>
    invoke<void>('redis_del_key', { connectionId, dbIndex: dbIndex, key }),
};

export const queryApi = {
  execute: (connectionId: string, sql: string, database?: string) =>
    invoke<QueryResult>('execute_query', { connectionId, sql, database: database ?? null }),
  executePaged: (connectionId: string, sql: string, page: number, pageSize: number, database?: string) =>
    invoke<QueryResult>('execute_query_paged', { input: { connectionId, sql, page, pageSize, database: database ?? null } }),
};

export const migrationApi = {
  /** 启动迁移任务，返回 migration_id，进度通过事件推送 */
  startMigration: (input: MigrationInput) =>
    invoke<string>('start_migration_v2', { input }),
  /** 监听迁移进度事件，返回取消监听函数 */
  onProgress: (callback: (progress: MigrationProgress) => void): Promise<UnlistenFn> =>
    listen<MigrationProgress>('migration-progress', (event) => {
      callback(event.payload);
    }),
};
