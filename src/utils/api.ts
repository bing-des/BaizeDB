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
  AddColumnInput,
  ModifyColumnInput,
  CreateTableInput,
  DatabaseMetadata,
  TableRelationAnalysis,
  AnalyzeRelationsResponse,
  LlmConfig,
  LlmConfigResponse,
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
  getTableData: (connectionId: string, database: string, table: string, page: number, pageSize: number, sortBy?: string | null, sortOrder?: string | null, filters?: Record<string, string> | null) =>
    invoke<TableDataResult>('get_table_data', { connectionId, database, table, page, pageSize, sortBy, sortOrder, filters }),
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
  /** 删除数据库（DROP DATABASE） */
  dropDatabase: (connectionId: string, databaseName: string) =>
    invoke<number>('drop_database', { connectionId, databaseName }),
  /** 删除表（DROP TABLE） */
  dropTable: (connectionId: string, database: string, table: string, schema?: string) =>
    invoke<number>('drop_table', { connectionId, database, table, schema }),
  /** 创建表（CREATE TABLE） */
  createTable: (connectionId: string, database: string, schema: string | undefined, input: CreateTableInput) =>
    invoke<void>('create_table', { connectionId, database, schema, input }),
  /** 新增列（ALTER TABLE ... ADD COLUMN） */
  addColumn: (connectionId: string, database: string, table: string, input: AddColumnInput) =>
    invoke<void>('add_column', { connectionId, database, table, input }),
  /** 删除列（ALTER TABLE ... DROP COLUMN） */
  dropColumn: (connectionId: string, database: string, table: string, columnName: string) =>
    invoke<void>('drop_column', { connectionId, database, table, columnName }),
  /** 修改列定义（ALTER TABLE ... MODIFY/ALTER COLUMN） */
  modifyColumn: (connectionId: string, database: string, table: string, input: ModifyColumnInput) =>
    invoke<void>('modify_column', { connectionId, database, table, input }),
  /** 获取数据库完整元数据（用于可视化） */
  getDatabaseMetadata: (connectionId: string, database: string, schema?: string) =>
    invoke<DatabaseMetadata>('get_database_metadata', { connectionId, database, schema }),
  /** 保存可视化元数据到本地文件 */
  saveVisualizationMetadata: (connectionId: string, database: string, schema: string | undefined, metadata: DatabaseMetadata) =>
    invoke<string>('save_visualization_metadata', { connectionId, database, schema, metadata }),
  /** 从本地文件加载可视化元数据 */
  loadVisualizationMetadata: (connectionId: string, database: string, schema?: string) =>
    invoke<DatabaseMetadata | null>('load_visualization_metadata', { connectionId, database, schema }),
  /** 删除本地保存的可视化元数据 */
  deleteVisualizationMetadata: (connectionId: string, database: string, schema?: string) =>
    invoke<void>('delete_visualization_metadata', { connectionId, database, schema }),
  /** 列出所有已保存的可视化文件 */
  listSavedVisualizations: () =>
    invoke<string[]>('list_saved_visualizations'),
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

export const llmApi = {
  /** 获取表关系分析（优先从 SQLite 读取，不存在则调用 LLM） */
  getTableRelations: (connectionId: string, database: string, schema?: string) =>
    invoke<AnalyzeRelationsResponse>('get_table_relations', { connectionId, database, schema }),
  /** 强制刷新 - 重新调用 LLM 分析 */
  refreshTableRelations: (connectionId: string, database: string, schema?: string) =>
    invoke<AnalyzeRelationsResponse>('refresh_table_relations', { connectionId, database, schema }),
  /** 检查是否有缓存的分析结果 */
  hasRelationAnalysis: (connectionId: string, database: string) =>
    invoke<boolean>('has_relation_analysis', { connectionId, database }),
  /** 删除分析结果 */
  clearRelationAnalysis: (connectionId: string, database: string) =>
    invoke<void>('clear_relation_analysis', { connectionId, database }),
  /** 获取 LLM 配置 */
  getConfig: () =>
    invoke<LlmConfigResponse>('get_llm_config'),
  /** 保存 LLM 配置 */
  saveConfig: (config: LlmConfig) =>
    invoke<void>('save_llm_config', { req: config }),
  /** 测试 LLM 配置 */
  testConfig: (config: LlmConfig) =>
    invoke<string>('test_llm_config', { apiKey: config.api_key, apiUrl: config.api_url, model: config.model }),
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
