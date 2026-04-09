/**
 * SQL 解析工具
 * 功能：忽略注释，按分号分割 SQL 语句
 */

/**
 * 移除 SQL 中的注释
 * 支持：
 * 1. 单行注释：--
 * 2. 多行注释：/* *\/
 */
export function stripComments(sql: string): string {
  let result = '';
  let i = 0;
  const length = sql.length;

  while (i < length) {
    // 检查单行注释
    if (i + 1 < length && sql[i] === '-' && sql[i + 1] === '-') {
      i += 2; // 跳过 --
      // 跳过直到行尾
      while (i < length && sql[i] !== '\n') {
        i++;
      }
      if (i < length && sql[i] === '\n') {
        i++; // 保留换行符
      }
      continue;
    }

    // 检查多行注释
    if (i + 1 < length && sql[i] === '/' && sql[i + 1] === '*') {
      i += 2; // 跳过 /*
      while (i < length && !(sql[i] === '*' && i + 1 < length && sql[i + 1] === '/')) {
        i++;
      }
      if (i + 1 < length) {
        i += 2; // 跳过 */
      }
      continue;
    }

    // 普通字符
    result += sql[i];
    i++;
  }

  return result;
}

/**
 * 按分号分割 SQL 语句
 * 忽略空语句和只包含空格的语句
 */
export function splitStatements(sql: string): string[] {
  const stripped = stripComments(sql);
  const statements: string[] = [];
  let current = '';
  let inString = false;
  let stringChar = ''; // ' 或 "
  let escaped = false;

  for (let i = 0; i < stripped.length; i++) {
    const char = stripped[i];

    // 处理转义
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }

    // 处理字符串
    if (inString) {
      current += char;
      if (char === '\\') {
        escaped = true;
      } else if (char === stringChar) {
        inString = false;
        stringChar = '';
      }
      continue;
    }

    // 检查字符串开始
    if (char === "'" || char === '"') {
      inString = true;
      stringChar = char;
      current += char;
      continue;
    }

    // 分号分割
    if (char === ';') {
      const trimmed = current.trim();
      if (trimmed) {
        statements.push(trimmed);
      }
      current = '';
      continue;
    }

    // 普通字符
    current += char;
  }

  // 最后一条语句（如果没有分号结尾）
  const trimmed = current.trim();
  if (trimmed) {
    statements.push(trimmed);
  }

  return statements;
}

/**
 * 解析 SQL 文本，返回语句数组
 */
export function parseSql(sql: string): string[] {
  return splitStatements(sql);
}

/**
 * 检查 SQL 是否包含多条语句
 */
export function hasMultipleStatements(sql: string): boolean {
  const statements = splitStatements(sql);
  return statements.length > 1;
}

/**
 * 获取指定区域的 SQL（用于选中文本）
 * 并解析为语句数组
 */
export function getSqlStatementsFromSelection(
  fullSql: string,
  selectionStart?: number,
  selectionEnd?: number
): string[] {
  if (selectionStart === undefined || selectionEnd === undefined || selectionStart === selectionEnd) {
    // 没有选中，返回整个 SQL
    return parseSql(fullSql);
  }

  // 获取选中部分的 SQL
  const selectedSql = fullSql.substring(selectionStart, selectionEnd);
  return parseSql(selectedSql);
}

/**
 * 获取当前行的 SQL
 * 根据光标位置找到所在行
 */
export function getSqlStatementsFromCurrentLine(
  fullSql: string,
  cursorPosition: number
): string[] {
  // 找到光标所在行的开始和结束
  let lineStart = cursorPosition;
  let lineEnd = cursorPosition;

  // 向前找到行首
  while (lineStart > 0 && fullSql[lineStart - 1] !== '\n') {
    lineStart--;
  }

  // 向后找到行尾
  while (lineEnd < fullSql.length && fullSql[lineEnd] !== '\n') {
    lineEnd++;
  }

  // 获取行内容
  const lineSql = fullSql.substring(lineStart, lineEnd).trim();
  return parseSql(lineSql);
}