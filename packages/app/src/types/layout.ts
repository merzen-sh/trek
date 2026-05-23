/** Mirrors parser `LayoutDoc` / `LayoutNode` (lean WASM layout payload). */

export type ColumnType =
  | "key"
  | "string"
  | "number"
  | "enum"
  | "boolean"
  | "unknown";

export interface ScalarMeta {
  description?: string[];
  range?: string[];
}

export interface EnumMeta {
  description?: string[];
  options: string[];
}

export interface ColumnDef {
  field: string;
  type: ColumnType;
  label: string;
  values?: string[];
}

export interface TableSchema {
  allow_add?: boolean;
  allow_delete?: boolean;
  allow_edit?: boolean;
  columns: ColumnDef[];
}

export interface TableMeta {
  description?: string[];
  schema?: TableSchema;
}

export interface CfxFunctionMeta {
  description?: string[];
  args_schema: {
    name: string;
    type: ColumnType;
    label: string;
    required?: boolean;
  }[];
}

export interface LayoutScalarNode {
  ast_path: string[];
  metadata?: ScalarMeta;
}

export interface LayoutEnumNode {
  ast_path: string[];
  metadata?: EnumMeta;
}

export interface LayoutTableNode {
  ast_path: string[];
  metadata?: TableMeta;
  fields?: Record<string, LayoutNode>;
}

export interface LayoutCfxFunctionNode {
  ast_path: string[];
  metadata: CfxFunctionMeta;
}

export interface LayoutVectorNode {
  ast_path: string[];
  metadata?: ScalarMeta;
}

export type LayoutNode =
  | { type: "string"; ast_path: string[]; metadata?: ScalarMeta }
  | { type: "number"; ast_path: string[]; metadata?: ScalarMeta }
  | { type: "float"; ast_path: string[]; metadata?: ScalarMeta }
  | { type: "boolean"; ast_path: string[]; metadata?: ScalarMeta }
  | { type: "enum"; ast_path: string[]; metadata?: EnumMeta }
  | { type: "table"; ast_path: string[]; metadata?: TableMeta; fields?: Record<string, LayoutNode> }
  | { type: "cfx_function"; ast_path: string[]; metadata: CfxFunctionMeta }
  | { type: "vector2"; ast_path: string[]; metadata?: ScalarMeta }
  | { type: "vector3"; ast_path: string[]; metadata?: ScalarMeta };

export interface LayoutDoc {
  meta?: Record<string, unknown>;
  fields: Record<string, LayoutNode>;
}

/** Normalize tagged enum from WASM JSON into discriminated union. */
export function parseLayoutNode(raw: Record<string, unknown>): LayoutNode {
  const type = raw.type as LayoutNode["type"];
  switch (type) {
    case "string":
    case "number":
    case "float":
    case "boolean":
      return {
        type,
        ast_path: raw.ast_path as string[],
        metadata: raw.metadata as ScalarMeta | undefined,
      };
    case "enum":
      return {
        type: "enum",
        ast_path: raw.ast_path as string[],
        metadata: raw.metadata as EnumMeta | undefined,
      };
    case "table":
      return {
        type: "table",
        ast_path: raw.ast_path as string[],
        metadata: raw.metadata as TableMeta | undefined,
        fields: raw.fields
          ? Object.fromEntries(
              Object.entries(raw.fields as Record<string, Record<string, unknown>>).map(
                ([k, v]) => [k, parseLayoutNode(v)],
              ),
            )
          : undefined,
      };
    case "cfx_function":
      return {
        type: "cfx_function",
        ast_path: raw.ast_path as string[],
        metadata: raw.metadata as CfxFunctionMeta,
      };
    case "vector2":
    case "vector3":
      return {
        type,
        ast_path: raw.ast_path as string[],
        metadata: raw.metadata as ScalarMeta | undefined,
      };
    default:
      throw new Error(`unknown layout node type: ${String(raw.type)}`);
  }
}

export function parseLayoutDoc(json: string): LayoutDoc {
  const raw = JSON.parse(json) as {
    meta?: Record<string, unknown>;
    fields: Record<string, Record<string, unknown>>;
  };
  const fields: Record<string, LayoutNode> = {};
  for (const [key, node] of Object.entries(raw.fields ?? {})) {
    fields[key] = parseLayoutNode(node);
  }
  return { meta: raw.meta, fields };
}
