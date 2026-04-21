export type EditableCellValue = string | number | boolean | null;

const INTEGER_PATTERN = /^[-+]?\d+$/;

function normalizeDbType(columnType?: string | null): string {
  return (columnType ?? '').trim().toLowerCase();
}

function isBooleanType(columnType: string): boolean {
  return (
    columnType === 'bool' ||
    columnType === 'boolean'
  );
}

function isIntegerType(columnType: string): boolean {
  return (
    columnType.includes('bigint') ||
    columnType.includes('int8') ||
    columnType.includes('integer') ||
    columnType.includes('int4') ||
    columnType.includes('smallint') ||
    columnType.includes('int2') ||
    columnType.includes('tinyint') ||
    columnType.includes('mediumint') ||
    columnType.includes('serial')
  );
}

function isExactNumericType(columnType: string): boolean {
  return columnType.includes('decimal') || columnType.includes('numeric');
}

function isApproxNumericType(columnType: string): boolean {
  return (
    columnType.includes('float') ||
    columnType.includes('double') ||
    columnType.includes('real')
  );
}

export function parseDbInputValue(
  rawValue: string,
  columnType?: string | null,
): EditableCellValue {
  const trimmed = rawValue.trim();
  if (trimmed === '' || trimmed.toLowerCase() === 'null') {
    return null;
  }

  const normalizedType = normalizeDbType(columnType);
  const lower = trimmed.toLowerCase();

  if (isBooleanType(normalizedType)) {
    if (trimmed === '1' || lower === 'true') {
      return true;
    }
    if (trimmed === '0' || lower === 'false') {
      return false;
    }
    return trimmed;
  }

  if (isIntegerType(normalizedType)) {
    if (!INTEGER_PATTERN.test(trimmed)) {
      return trimmed;
    }
    const parsed = Number(trimmed);
    return Number.isSafeInteger(parsed) ? parsed : trimmed;
  }

  if (isExactNumericType(normalizedType)) {
    return trimmed;
  }

  if (isApproxNumericType(normalizedType)) {
    const parsed = Number(trimmed);
    return Number.isNaN(parsed) ? trimmed : parsed;
  }

  if (trimmed === 'true') return true;
  if (trimmed === 'false') return false;

  const parsed = Number(trimmed);
  if (Number.isNaN(parsed)) {
    return trimmed;
  }

  if (INTEGER_PATTERN.test(trimmed) && !Number.isSafeInteger(parsed)) {
    return trimmed;
  }

  return parsed;
}
