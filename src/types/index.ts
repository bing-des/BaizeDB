export type DbType = 'mysql' | 'postgresql' | 'redis';

export interface ConnectionConfig {
  id: string;
  name: string;
  db_type: DbType;
  host: string;
  port: number;
  username: string;
  password: string;
  database?: string;
  ssl: boolean;
}

export interface DatabaseInfo {
  name: string;
}

export interface SchemaInfo {
  name: string;
}

export interface TableInfo {
  name: string;
  table_type: string;
  row_count?: number;
}

export interface ColumnInfo {
  name: string;
  data_type: string;
  nullable: boolean;
  key?: string;
  default_value?: string;
  comment?: string;
}

export interface QueryResult {
  columns: string[];
  rows: (string | number | boolean | null)[][];
  affected_rows?: number;
  execution_time_ms: number;
  error?: string;
}

export interface TableDataResult {
  columns: string[];
  column_types: string[];
  rows: (string | number | boolean | null)[][];
  total: number;
}

export interface RedisDbInfo {
  index: number;
  key_count: number;
}

export interface RedisKeyInfo {
  name: string;
  key_type: string;
  ttl: number;
}

export interface RedisScanResult {
  cursor: number;
  keys: RedisKeyInfo[];
}

export interface RedisKeyValue {
  key: string;
  key_type: string;
  value: any;
  ttl: number;
}

export interface Tab {
  id: string;
  title: string;
  type: 'query' | 'table' | 'redis-key' | 'visualization' | 'chartdb';
  connectionId: string;
  database?: string;
  table?: string;
  schema?: string;
  content?: string;
  redisDbIndex?: number;
  redisKey?: string;
  /** 查询结果（用于 SQL 控制台标签） */
  results?: QueryResult[];
}

export interface TableMapping {
  source_table: string;
  target_table?: string;
  column_mappings?: ColumnMapping[];
}

export interface ColumnMapping {
  source_column: string;
  target_column?: string;
  /** 是否忽略此列（不迁移） */
  ignored?: boolean;
}

export interface MigrationInput {
  source_connection_id: string;
  target_connection_id: string;
  source_database: string;
  target_database?: string;
  tables?: string[];
  migrate_structure?: boolean;
  migrate_data?: boolean;
  truncate_target?: boolean;
  batch_size?: number;
  table_mappings?: TableMapping[];
}

export interface MigrationProgress {
  migration_id: string;
  current_table: string;
  total_tables: number;
  tables_completed: number;
  rows_migrated: number;
  current_table_rows: number;
  status: MigrationStatus;
  error?: string;
}

export type MigrationStatus =
  | 'NotStarted'
  | 'Preparing'
  | 'MigratingStructure'
  | 'MigratingData'
  | 'Completed'
  | 'Failed';

// ─────────── 表结构管理 ───────────

/** 新增列的输入参数 */
export interface AddColumnInput {
  column_name: string;
  column_type: string;
  nullable: boolean;
  default_value?: string;
  comment?: string;
}

/** 修改列的输入参数 */
export interface ModifyColumnInput {
  old_name: string;
  new_name: string;
  column_type: string;
  nullable: boolean;
  default_value?: string;
  comment?: string;
}

/** 创建表的列定义 */
export interface CreateTableColumn {
  name: string;
  data_type: string;
  nullable: boolean;
  default_value?: string;
  comment?: string;
  is_primary_key: boolean;
}

/** 创建表的输入参数 */
export interface CreateTableInput {
  table_name: string;
  columns: CreateTableColumn[];
  comment?: string;
}

// ─────────── 数据库可视化 ───────────

/** 外键信息 */
export interface ForeignKeyInfo {
  column_name: string;
  referenced_table: string;
  referenced_column: string;
}

/** 被引用信息（哪些表引用了当前表） */
export interface ReferencedByInfo {
  table_name: string;
  column_name: string;
  referenced_column: string;
}

/** 表元数据（用于可视化） */
export interface TableMetadata {
  name: string;
  comment?: string;
  columns: ColumnInfo[];
  foreign_keys: ForeignKeyInfo[];
  referenced_by: ReferencedByInfo[];
}

/** 数据库元数据（用于可视化） */
export interface DatabaseMetadata {
  database: string;
  schema?: string;
  tables: TableMetadata[];
  /** LLM 分析的表关系（虚线显示） */
  llm_relations: TableRelationAnalysis[];
}

// ─────────── LLM 表关系分析 ───────────

/** 表关系分析结果 */
export interface TableRelationAnalysis {
  source_table: string;
  source_column: string;
  target_table: string;
  target_column: string;
  relation_type: string; // "one_to_one", "one_to_many", "many_to_many"
  confidence: number; // 0.0 - 1.0
  reason: string;
}

/** LLM 分析响应 */
export interface AnalyzeRelationsResponse {
  relations: TableRelationAnalysis[];
  from_cache: boolean;
}

/** LLM 配置 */
export interface LlmConfig {
  api_key: string;
  api_url: string;
  model: string;
  enabled: boolean;
}

/** LLM 配置响应 */
export interface LlmConfigResponse {
  config: LlmConfig;
}

// ─────────── Harness 工具调用 ───────────

/** 工具执行结果 */
export interface ToolResult {
  success: boolean;
  result?: unknown;
  error?: string;
}
