import type { ColumnDef, ColumnType, TableSchema } from "../types/layout";

/** Lua `@TABLE` schema may use `name` or `field` for the column key. */
export function columnField(col: ColumnDef): string {
  return col.field || (col as ColumnDef & { name?: string }).name || "";
}

export function defaultRowFromSchema(
  schema: TableSchema,
  rowKey: string,
): Record<string, unknown> {
  const row: Record<string, unknown> = {};
  for (const col of schema.columns) {
    const field = columnField(col);
    if (!field) continue;
    switch (col.type) {
      case "key":
        row[field] = rowKey;
        break;
      case "number":
        row[field] = 0;
        break;
      case "boolean":
        row[field] = false;
        break;
      case "enum":
        row[field] = col.values?.[0] ?? "";
        break;
      default:
        row[field] = "";
    }
  }
  return row;
}

export function coerceCellValue(colType: ColumnType, raw: string): unknown {
  switch (colType) {
    case "number": {
      const n = Number(raw);
      return Number.isFinite(n) ? n : raw;
    }
    case "boolean":
      return raw === "true" || raw === "1";
    default:
      return raw;
  }
}

export function cellDisplayValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "boolean") return value ? "true" : "false";
  return String(value);
}
