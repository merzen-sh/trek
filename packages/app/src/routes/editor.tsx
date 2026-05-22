import { useCallback, useEffect, useState } from "react";
import Editor from "@monaco-editor/react";
import { useWasm } from "../hooks/use-wasm";
import { useConfigEditor } from "../hooks/use-config-editor";
import { useAppSetting } from "../lib/use-app-setting";
import { DEFAULT_LUA_TEMPLATE } from "../lib/default-lua-template";
import { ConfigFieldTree } from "../components/config-field-tree";
import { ConfigFieldForm } from "../components/config-field-form";
import { Button } from "ui";
import type { LayoutNode } from "../types/layout";
import {
  AlertTriangle,
  Check,
  Copy,
  Download,
  FileCode,
  RefreshCw,
  Save,
  TreePine,
} from "lucide-react";

export function EditorPage() {
  const { ready, error: wasmError, wasm, loading } = useWasm();
  const theme = useAppSetting((s) => s.theme);

  const [editorLua, setEditorLua] = useState(DEFAULT_LUA_TEMPLATE);
  const [debouncedSource, setDebouncedSource] = useState(DEFAULT_LUA_TEMPLATE);
  const [selectedPath, setSelectedPath] = useState<string[] | null>(null);
  const [selectedNode, setSelectedNode] = useState<LayoutNode | null>(null);
  const [copied, setCopied] = useState(false);

  const {
    layout,
    luaOutput,
    parseError,
    revision,
    getValueAtPath,
    patchValueAtPath,
    appendTableRow,
    removeTableRow,
    hasSession,
  } = useConfigEditor(debouncedSource, ready, wasm);

  const [isDark, setIsDark] = useState(false);
  useEffect(() => {
    const checkDark = () => {
      if (theme === "dark") return true;
      if (theme === "light") return false;
      return window.matchMedia("(prefers-color-scheme: dark)").matches;
    };
    setIsDark(checkDark());
    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = (e: MediaQueryListEvent) => setIsDark(e.matches);
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [theme]);

  useEffect(() => {
    const t = setTimeout(() => setDebouncedSource(editorLua), 400);
    return () => clearTimeout(t);
  }, [editorLua]);

  useEffect(() => {
    if (revision > 0 && hasSession && luaOutput) {
      setEditorLua(luaOutput);
      setDebouncedSource(luaOutput);
    }
  }, [revision, luaOutput, hasSession]);

  const handleSelect = useCallback((path: string[], node: LayoutNode) => {
    setSelectedPath(path);
    setSelectedNode(node);
  }, []);

  const handleCopy = async () => {
    const text = hasSession ? luaOutput : editorLua;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error(e);
    }
  };

  const handleDownload = () => {
    const text = hasSession ? luaOutput : editorLua;
    const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "config.lua";
    link.click();
    URL.revokeObjectURL(url);
  };

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <RefreshCw className="h-8 w-8 animate-spin text-primary" />
        <p className="text-sm text-muted-foreground animate-pulse">Loading WASM AST editor…</p>
      </div>
    );
  }

  if (wasmError) {
    return (
      <div className="flex h-full flex-col items-center justify-center p-6 gap-3">
        <AlertTriangle className="h-10 w-10 text-destructive" />
        <h2 className="text-lg font-semibold">Failed to load WASM</h2>
        <pre className="text-xs bg-muted p-4 rounded-lg border max-w-lg overflow-auto">
          {wasmError.message}
        </pre>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      <header className="flex flex-shrink-0 items-center justify-between border-b bg-card/30 backdrop-blur-md px-4 py-2 gap-3">
        <div className="flex items-center gap-2">
          <TreePine className="h-5 w-5 text-primary" />
          <div>
            <h1 className="text-sm font-bold tracking-tight">Config Editor</h1>
            <p className="text-[10px] text-muted-foreground">Lossless AST patching via WASM</p>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => {
              setEditorLua(DEFAULT_LUA_TEMPLATE);
              setDebouncedSource(DEFAULT_LUA_TEMPLATE);
            }}
          >
            <FileCode className="h-3 w-3 mr-1" />
            Template
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={handleCopy}
            disabled={!editorLua}
          >
            {copied ? <Check className="h-3 w-3 mr-1 text-emerald-500" /> : <Copy className="h-3 w-3 mr-1" />}
            Copy
          </Button>
          <Button variant="outline" size="sm" className="h-7 text-xs" onClick={handleDownload}>
            <Download className="h-3 w-3 mr-1" />
            Download
          </Button>
          {hasSession && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-500/10 text-emerald-600 border border-emerald-500/20">
              <Save className="h-3 w-3" />
              AST session active
            </span>
          )}
        </div>
      </header>

      {parseError && (
        <div className="mx-4 mt-3 flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
          <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
          <pre className="font-mono whitespace-pre-wrap break-words flex-1">{parseError}</pre>
        </div>
      )}

      <div className="flex flex-1 min-h-0 p-3 gap-3 flex-col lg:flex-row">
        {/* Layout tree */}
        <div className="flex flex-col border rounded-xl bg-card shadow-sm lg:w-56 shrink-0 overflow-hidden">
          <div className="px-3 py-2 border-b text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
            Layout
          </div>
          <div className="flex-1 overflow-y-auto p-2 min-h-[120px] max-h-[200px] lg:max-h-none">
            {layout ? (
              <ConfigFieldTree
                fields={layout.fields}
                selectedPath={selectedPath}
                onSelect={handleSelect}
              />
            ) : (
              <p className="text-xs text-muted-foreground p-2">Parse valid Lua to see layout.</p>
            )}
          </div>
        </div>

        {/* Value form */}
        <div className="flex flex-col border rounded-xl bg-card shadow-sm flex-1 min-w-0 overflow-hidden">
          <div className="px-3 py-2 border-b text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
            Field value
          </div>
          <div className="flex-1 min-h-0 overflow-y-auto">
            <ConfigFieldForm
              node={selectedNode}
              getValueAtPath={getValueAtPath}
              patchValueAtPath={patchValueAtPath}
              appendTableRow={appendTableRow}
              removeTableRow={removeTableRow}
              revision={revision}
            />
          </div>
        </div>

        {/* Lua source */}
        <div className="flex flex-col border rounded-xl bg-card shadow-sm flex-1 min-w-0 overflow-hidden min-h-[240px]">
          <div className="px-3 py-2 border-b flex items-center justify-between">
            <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
              Lua source
            </span>
            {hasSession && luaOutput !== debouncedSource && (
              <span className="text-[10px] text-amber-500">patched output available</span>
            )}
          </div>
          <div className="flex-1 min-h-0">
            <Editor
              height="100%"
              language="lua"
              theme={isDark ? "vs-dark" : "light"}
              value={editorLua}
              onChange={(v) => setEditorLua(v ?? "")}
              options={{
                minimap: { enabled: false },
                fontSize: 12,
                fontFamily: "var(--font-mono, monospace)",
                lineNumbers: "on",
                wordWrap: "on",
                automaticLayout: true,
                scrollBeyondLastLine: false,
                padding: { top: 8, bottom: 8 },
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
