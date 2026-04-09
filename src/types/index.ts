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
