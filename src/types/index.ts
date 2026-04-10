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
  type: 'query' | 'table' | 'redis-key';
  connectionId: string;
  database?: string;
  table?: string;
  content?: string;
  redisDbIndex?: number;
  redisKey?: string;
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
