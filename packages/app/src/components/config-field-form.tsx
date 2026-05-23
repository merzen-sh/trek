import { useCallback, useEffect, useState } from "react";
import { Badge, Button, cn, Slider } from "ui";
import type { ColumnDef, LayoutNode, ScalarMeta, TableSchema } from "../types/layout";
import {
  cellDisplayValue,
  coerceCellValue,
  columnField,
  defaultRowFromSchema,
} from "../lib/table-schema";
import { Plus, Trash2 } from "lucide-react";

const inputClass =
  "w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring";

interface ConfigFieldFormProps {
  node: LayoutNode | null;
  getValueAtPath: (path: string[]) => unknown;
  patchValueAtPath: (path: string[], value: unknown) => void;
  appendTableRow: (tablePath: string[], rowKey: string, row: Record<string, unknown>) => void;
  removeTableRow: (tablePath: string[], rowKey: string) => void;
  revision: number;
}

function Description({ lines }: { lines?: string[] }) {
  if (!lines?.length) return null;
  return (
    <p className="text-xs text-muted-foreground leading-relaxed border-l-2 border-primary/30 pl-3">
      {lines.join(" ")}
    </p>
  );
}

function ScalarEditor({
  node,
  value,
  onChange,
}: {
  node: LayoutNode;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const [draft, setDraft] = useState(String(value ?? ""));

  useEffect(() => {
    setDraft(String(value ?? ""));
  }, [value, node.ast_path.join(".")]);

  if (node.type === "boolean") {
    const checked = value === true || value === "true";
    return (
      <label className="flex items-center gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="h-4 w-4 rounded border-input"
        />
        <span className="text-sm font-mono">{checked ? "true" : "false"}</span>
      </label>
    );
  }

  if (node.type === "enum" && node.metadata?.options) {
    return (
      <select
        value={String(value ?? "")}
        onChange={(e) => onChange(e.target.value)}
        className={inputClass}
      >
        {node.metadata.options.map((opt) => (
          <option key={opt} value={opt}>
            {opt}
          </option>
        ))}
      </select>
    );
  }

  if (node.type === "number" || node.type === "float") {
    const meta = node.metadata as ScalarMeta | undefined;
    const range = meta?.range;
    if (range?.length === 2) {
      const min = Number(range[0]);
      const max = Number(range[1]);
      if (Number.isFinite(min) && Number.isFinite(max) && min < max) {
        const numVal = Number(value ?? min);
        const clamped = Math.min(max, Math.max(min, Number.isFinite(numVal) ? numVal : min));
        const step = node.type === "float" ? (max - min) / 100 : 1;
        return (
          <div className="flex items-center gap-3">
            <span className="text-xs font-mono text-muted-foreground w-12 text-right tabular-nums">
              {clamped}
            </span>
            <Slider
              min={min}
              max={max}
              step={step}
              value={clamped}
              onChange={(v) => onChange(v)}
              className="flex-1"
            />
          </div>
        );
      }
    }
    return (
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => {
          const n = Number(draft);
          onChange(Number.isFinite(n) ? n : draft);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            const n = Number(draft);
            onChange(Number.isFinite(n) ? n : draft);
          }
        }}
        className={inputClass}
      />
    );
  }

  return (
    <input
      type="text"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        onChange(draft);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          onChange(draft);
        }
      }}
      className={inputClass}
    />
  );
}

function VectorEditor({
  dims,
  value,
  onChange,
}: {
  dims: 2 | 3;
  value: Record<string, number> | null;
  onChange: (v: { x: number; y: number; z?: number }) => void;
}) {
  const [x, setX] = useState(String(value?.x ?? 0));
  const [y, setY] = useState(String(value?.y ?? 0));
  const [z, setZ] = useState(String(value?.z ?? 0));

  useEffect(() => {
    setX(String(value?.x ?? 0));
    setY(String(value?.y ?? 0));
    setZ(String(value?.z ?? 0));
  }, [value]);

  const commit = () => {
    const payload: { x: number; y: number; z?: number } = {
      x: Number(x) || 0,
      y: Number(y) || 0,
    };
    if (dims === 3) payload.z = Number(z) || 0;
    onChange(payload);
  };

  return (
    <div className="grid grid-cols-3 gap-2">
      <div>
        <label className="text-[10px] font-bold uppercase text-muted-foreground">x</label>
        <input type="text" value={x} onChange={(e) => setX(e.target.value)} onBlur={commit} className={cn(inputClass, "mt-1")} />
      </div>
      <div>
        <label className="text-[10px] font-bold uppercase text-muted-foreground">y</label>
        <input type="text" value={y} onChange={(e) => setY(e.target.value)} onBlur={commit} className={cn(inputClass, "mt-1")} />
      </div>
      {dims === 3 && (
        <div>
          <label className="text-[10px] font-bold uppercase text-muted-foreground">z</label>
          <input type="text" value={z} onChange={(e) => setZ(e.target.value)} onBlur={commit} className={cn(inputClass, "mt-1")} />
        </div>
      )}
    </div>
  );
}

function SchemaCellInput({
  col,
  value,
  disabled,
  onChange,
}: {
  col: ColumnDef;
  value: unknown;
  disabled?: boolean;
  onChange: (v: unknown) => void;
}) {
  const display = cellDisplayValue(value);

  if (col.type === "boolean") {
    const checked = value === true || value === "true";
    return (
      <label className="flex items-center gap-2 mt-1 cursor-pointer">
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="h-4 w-4 rounded border-input"
        />
        <span className="text-xs font-mono">{checked ? "true" : "false"}</span>
      </label>
    );
  }

  if (col.type === "enum" && col.values?.length) {
    return (
      <select
        value={display}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className={cn(inputClass, "mt-1")}
      >
        {col.values.map((opt) => (
          <option key={opt} value={opt}>
            {opt}
          </option>
        ))}
      </select>
    );
  }

  return (
    <input
      type="text"
      value={display}
      disabled={disabled || col.type === "key"}
      onChange={(e) => onChange(coerceCellValue(col.type, e.target.value))}
      className={cn(inputClass, "mt-1")}
    />
  );
}

function TableRowsPanel({
  path,
  schema,
  getValueAtPath,
  patchValueAtPath,
  appendTableRow,
  removeTableRow,
  revision,
}: {
  path: string[];
  schema: TableSchema;
  getValueAtPath: (path: string[]) => unknown;
  patchValueAtPath: (path: string[], value: unknown) => void;
  appendTableRow: (tablePath: string[], rowKey: string, row: Record<string, unknown>) => void;
  removeTableRow: (tablePath: string[], rowKey: string) => void;
  revision: number;
}) {
  const [rows, setRows] = useState<Record<string, Record<string, unknown>>>({});
  const [newRowKey, setNewRowKey] = useState("");
  const allowAdd = schema.allow_add ?? false;
  const allowDelete = schema.allow_delete ?? false;
  const allowEdit = schema.allow_edit ?? true;
  const columns = schema.columns ?? [];
  const keyColumn = columns.find((c) => c.type === "key");
  const keyField = keyColumn ? columnField(keyColumn) : null;

  useEffect(() => {
    const raw = getValueAtPath(path);
    if (raw && typeof raw === "object" && !Array.isArray(raw)) {
      setRows(raw as Record<string, Record<string, unknown>>);
    } else {
      setRows({});
    }
  }, [path.join("."), revision, getValueAtPath]);

  const updateCell = (rowKey: string, col: ColumnDef, cellValue: unknown) => {
    const field = columnField(col);
    if (!field) return;
    patchValueAtPath([...path, rowKey, field], cellValue);
    setRows((prev) => ({
      ...prev,
      [rowKey]: { ...prev[rowKey], [field]: cellValue },
    }));
  };

  const handleAddRow = () => {
    const rowKey = newRowKey.trim();
    if (!rowKey) return;
    if (rows[rowKey]) return;
    const payload = defaultRowFromSchema(schema, rowKey);
    appendTableRow(path, rowKey, payload);
    setNewRowKey("");
  };

  const handleRemoveRow = (rowKey: string) => {
    removeTableRow(path, rowKey);
  };

  return (
    <div className="space-y-3">
      {Object.keys(rows).length === 0 ? (
        <p className="text-xs text-muted-foreground italic">No rows yet.</p>
      ) : (
        Object.entries(rows).map(([rowKey, row]) => (
          <div key={rowKey} className="rounded-lg border bg-muted/20 p-3 space-y-2">
            <div className="flex items-center justify-between gap-2">
              <div className="font-mono text-xs font-semibold text-primary">{rowKey}</div>
              {allowDelete && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 text-xs text-destructive hover:bg-destructive/10"
                  onClick={() => handleRemoveRow(rowKey)}
                >
                  <Trash2 className="h-3 w-3 mr-1" />
                  Remove
                </Button>
              )}
            </div>
            {columns.map((col) => {
              const field = columnField(col);
              if (!field) return null;
              const cellValue = row[field] ?? (col.type === "key" ? rowKey : undefined);
              return (
                <div key={`${rowKey}-${field}`}>
                  <label className="text-[10px] font-bold uppercase text-muted-foreground">
                    {col.label || field}
                  </label>
                  <SchemaCellInput
                    col={col}
                    value={cellValue}
                    disabled={!allowEdit && col.type !== "key"}
                    onChange={(v) => updateCell(rowKey, col, v)}
                  />
                </div>
              );
            })}
          </div>
        ))
      )}

      {allowAdd && (
        <div className="flex flex-wrap items-end gap-2 pt-2 border-t border-dashed">
          <div className="flex-1 min-w-[120px]">
            <label className="text-[10px] font-bold uppercase text-muted-foreground">
              {keyColumn?.label ?? "New row ID"}
            </label>
            <input
              type="text"
              value={newRowKey}
              onChange={(e) => setNewRowKey(e.target.value)}
              placeholder={keyField ? `e.g. ${keyField}` : "row_key"}
              className={cn(inputClass, "mt-1")}
              onKeyDown={(e) => e.key === "Enter" && handleAddRow()}
            />
          </div>
          <Button type="button" size="sm" className="h-9" onClick={handleAddRow} disabled={!newRowKey.trim()}>
            <Plus className="h-3.5 w-3.5 mr-1" />
            Add row
          </Button>
        </div>
      )}
    </div>
  );
}

function timeAgo(date: Date, now: number): string {
  const secs = Math.floor((now - date.getTime()) / 1000);
  if (secs < 5) return "just now";
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ago`;
}

function isScalarType(t: string): boolean {
  return t === "string" || t === "number" || t === "float" || t === "boolean" || t === "enum";
}

export function ConfigFieldForm({
  node,
  getValueAtPath,
  patchValueAtPath,
  appendTableRow,
  removeTableRow,
  revision,
}: ConfigFieldFormProps) {
  const [original, setOriginal] = useState<unknown>(undefined);
  const [draft, setDraft] = useState<unknown>(undefined);
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null);
  const [now, setNow] = useState(Date.now());

  const isDirty = draft !== original;

  useEffect(() => {
    if (!lastSavedAt) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [lastSavedAt]);

  useEffect(() => {
    setDraft(undefined);
    setOriginal(undefined);
    setLastSavedAt(null);
  }, [node?.ast_path.join(".")]);

  useEffect(() => {
    if (!node) {
      setOriginal(undefined);
      setDraft(undefined);
      return;
    }
    if (
      node.type === "table" &&
      node.metadata?.schema &&
      (node.metadata.schema.columns?.length || node.metadata.schema.allow_add)
    ) {
      setOriginal(undefined);
      setDraft(undefined);
      return;
    }
    if (node.type === "table" && node.fields && Object.keys(node.fields).length > 0) {
      setOriginal(undefined);
      setDraft(undefined);
      return;
    }
    if (node.type === "cfx_function") {
      setOriginal(undefined);
      setDraft(undefined);
      return;
    }
    if (!isScalarType(node.type) && node.type !== "vector2" && node.type !== "vector3") {
      setOriginal(undefined);
      setDraft(undefined);
      return;
    }
    const v = getValueAtPath(node.ast_path);
    setOriginal(v);
    setDraft(v);
  }, [node, revision, getValueAtPath]);

  const handleSave = useCallback(() => {
    if (!node || !isScalarType(node.type)) return;
    patchValueAtPath(node.ast_path, draft);
    setOriginal(draft);
    setLastSavedAt(new Date());
    setNow(Date.now());
  }, [node, draft, patchValueAtPath]);

  const handleDiscard = useCallback(() => {
    setDraft(original);
  }, [original]);

  if (!node) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center p-6 text-muted-foreground">
        <p className="text-sm">Select a field from the layout tree to load its value from the AST.</p>
      </div>
    );
  }

  const desc =
    node.type === "enum"
      ? node.metadata?.description
      : node.type === "table"
        ? node.metadata?.description
        : node.type === "cfx_function"
          ? node.metadata.description
          : node.metadata?.description;

  if (node.type === "cfx_function") {
    return (
      <div className="space-y-4 p-4">
        <Description lines={desc} />
        <Badge variant="outline" className="font-mono text-[10px]">
          {node.ast_path.join(" → ")}
        </Badge>
        <p className="text-xs text-muted-foreground">
          Function bodies are preserved losslessly in the AST. Edit the Lua panel directly for implementation
          changes.
        </p>
        <div className="space-y-2">
          {node.metadata.args_schema.map((arg) => (
            <div key={arg.name} className="text-xs border rounded-md px-3 py-2 bg-muted/30">
              <span className="font-mono font-semibold">{arg.name}</span>
              <span className="text-muted-foreground ml-2">{arg.label}</span>
              {arg.required && (
                <span className="ml-2 text-[10px] text-amber-500 uppercase">required</span>
              )}
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (node.type === "table") {
    const schema = node.metadata?.schema;
    const isSchemaTable =
      schema && (schema.columns.length > 0 || schema.allow_add || schema.allow_delete);
    const hasChildLayout = node.fields && Object.keys(node.fields).length > 0;

    return (
      <div className="space-y-4 p-4 overflow-y-auto">
        <Description lines={desc} />
        <Badge variant="outline" className="font-mono text-[10px]">
          {node.ast_path.join(" → ")}
        </Badge>
        {isSchemaTable ? (
          <TableRowsPanel
            path={node.ast_path}
            schema={schema}
            getValueAtPath={getValueAtPath}
            patchValueAtPath={patchValueAtPath}
            appendTableRow={appendTableRow}
            removeTableRow={removeTableRow}
            revision={revision}
          />
        ) : hasChildLayout ? (
          <p className="text-xs text-muted-foreground">
            Nested fields are listed in the tree. Select a leaf field to edit its value.
          </p>
        ) : (
          <p className="text-xs text-muted-foreground italic">Empty table.</p>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-4 p-4">
      <Description lines={desc} />
      <Badge variant="outline" className="font-mono text-[10px]">
        {node.ast_path.join(" → ")}
      </Badge>
      {node.type === "vector2" && (
        <VectorEditor
          dims={2}
          value={draft as Record<string, number> | null}
          onChange={(v) => setDraft(v)}
        />
      )}
      {node.type === "vector3" && (
        <VectorEditor
          dims={3}
          value={draft as Record<string, number> | null}
          onChange={(v) => setDraft(v)}
        />
      )}
      {isScalarType(node.type) && (
        <ScalarEditor
          node={node}
          value={draft}
          onChange={(v) => setDraft(v)}
        />
      )}
      {node.type === "string" && node.metadata?.range && (
        <p className="text-[10px] text-muted-foreground font-mono">
          range: {node.metadata.range.join(", ")}
        </p>
      )}
      {(isScalarType(node.type) || node.type === "vector2" || node.type === "vector3") && (
        <div className="flex items-center gap-2 pt-2 border-t">
          <Button type="button" size="sm" onClick={handleSave} disabled={!isDirty}>
            Save
          </Button>
          <Button type="button" size="sm" variant="outline" onClick={handleDiscard} disabled={!isDirty}>
            Discard
          </Button>
          <span className="ml-auto text-[10px] text-muted-foreground font-mono">
            {lastSavedAt ? `Saved ${timeAgo(lastSavedAt, now)}` : "Unsaved"}
          </span>
        </div>
      )}
    </div>
  );
}
