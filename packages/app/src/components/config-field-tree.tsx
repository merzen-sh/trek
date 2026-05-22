import { ChevronDown, ChevronRight, FolderTree, FunctionSquare } from "lucide-react";
import { useState } from "react";
import { cn } from "ui";
import type { LayoutNode } from "../types/layout";

const TYPE_LABELS: Record<LayoutNode["type"], string> = {
  string: "String",
  number: "Number",
  boolean: "Boolean",
  enum: "Enum",
  table: "Table",
  cfx_function: "CFX Function",
  vector2: "Vector2",
  vector3: "Vector3",
};

interface TreeProps {
  fields: Record<string, LayoutNode>;
  selectedPath: string[] | null;
  onSelect: (path: string[], node: LayoutNode) => void;
  depth?: number;
}

function FieldTreeNode({
  name,
  node,
  selectedPath,
  onSelect,
  depth = 0,
}: {
  name: string;
  node: LayoutNode;
  selectedPath: string[] | null;
  onSelect: (path: string[], node: LayoutNode) => void;
  depth?: number;
}) {
  const [open, setOpen] = useState(depth < 2);
  const path = node.ast_path;
  const selected =
    selectedPath !== null &&
    selectedPath.length === path.length &&
    selectedPath.every((s, i) => s === path[i]);
  const isTable = node.type === "table";
  const childFields = isTable ? node.fields : undefined;
  const hasChildren = childFields && Object.keys(childFields).length > 0;

  return (
    <div>
      <button
        type="button"
        onClick={() => {
          onSelect(path, node);
          if (hasChildren) setOpen((o) => !o);
        }}
        className={cn(
          "flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-sm transition-colors",
          selected
            ? "bg-primary/15 text-primary font-medium"
            : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        )}
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
      >
        {hasChildren ? (
          open ? (
            <ChevronDown className="h-3.5 w-3.5 shrink-0 opacity-60" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 shrink-0 opacity-60" />
          )
        ) : (
          <span className="w-3.5 shrink-0" />
        )}
        {node.type === "cfx_function" ? (
          <FunctionSquare className="h-3.5 w-3.5 shrink-0 opacity-70" />
        ) : (
          <FolderTree className="h-3.5 w-3.5 shrink-0 opacity-50" />
        )}
        <span className="truncate font-mono text-xs">{name}</span>
        <span className="ml-auto text-[10px] uppercase tracking-wide opacity-50">
          {TYPE_LABELS[node.type]}
        </span>
      </button>
      {isTable && hasChildren && open && (
        <ConfigFieldTree
          fields={childFields}
          selectedPath={selectedPath}
          onSelect={onSelect}
          depth={depth + 1}
        />
      )}
    </div>
  );
}

export function ConfigFieldTree({ fields, selectedPath, onSelect, depth = 0 }: TreeProps) {
  return (
    <div className="space-y-0.5">
      {Object.entries(fields).map(([name, node]) => (
        <FieldTreeNode
          key={node.ast_path.join(".")}
          name={name}
          node={node}
          selectedPath={selectedPath}
          onSelect={onSelect}
          depth={depth}
        />
      ))}
    </div>
  );
}
