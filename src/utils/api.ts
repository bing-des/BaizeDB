import { invoke } from '@tauri-apps/api/core';
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
  /** 连接，configs 可选（重启后重连时需要传入以便恢复连接池） */
  connect: (id: string, configs?: ConnectionConfig[]) =>
    invoke<void>('connect_db', { id, options: configs ? { configs } : null }),
  disconnect: (id: string) =>
    invoke<void>('disconnect_db', { id }),
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
