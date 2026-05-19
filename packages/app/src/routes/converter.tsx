import { useState, useEffect, useRef } from "react";
import Editor, { useMonaco } from "@monaco-editor/react";
import { useWasm } from "../hooks/use-wasm";
import { useAppSetting } from "../lib/use-app-setting";
import { Button } from "ui";
import {
  ArrowLeftRight,
  Copy,
  Trash2,
  FileCode,
  AlertTriangle,
  Zap,
  Download,
  Check,
  RefreshCw,
  Sparkles,
  Terminal,
} from "lucide-react";

const DEFAULT_LUA_TEMPLATE = `--// Top Level Config
config = {
    --// Language setting
    --!ENUM = { "jp", "en" }
    locale = "en",

    --// Enable shop
    enable_shop = true,

    --// Shop list
    shop = {
        --// Blip position
        --!MAP = true
        blip_pos = vector2(1.0, 2.0),

        --// Shop position
        --!MAP = true
        pos = vector3(10.0, 95.0, 20.0),

        --// Shop price
        price = 10,

        --// enum
        --!ENUM = { "a", "b", "c" }
        test_enum = "a",
    },

    --// Table example
    --[[TABLE = {
        layout = "items",
        schema = {
            { name = "key", type = "string", is_key = true, label = "key" },
            { name = "label", type = "string", label = "label" },
            { name = "price", type = "number", label = "price" },
            { name = "type", type = "string", label = "type" }
        }
    }]]
    items = {
        water_bottle = {
            key = "water_bottle",
            label = "Drinking Water",
            price = 5,
            type = "food",
        },
    },
}`;

const DEFAULT_JSON_TEMPLATE = `{
  "locale": {
    "type": "enum",
    "value": "en",
    "metadata": {
      "description": "Language setting",
      "enum_options": [
        "jp",
        "en"
      ]
    }
  },
  "enable_shop": {
    "type": "boolean",
    "value": true,
    "metadata": {
      "description": "Enable shop"
    }
  },
  "shop": {
    "type": "table",
    "fields": {
      "blip_pos": {
        "type": "vector2",
        "x": 1.0,
        "y": 2.0,
        "metadata": {
          "description": "Blip position",
          "map": true
        }
      },
      "pos": {
        "type": "vector3",
        "x": 10.0,
        "y": 95.0,
        "z": 20.0,
        "metadata": {
          "description": "Shop position",
          "map": true
        }
      }
    }
  }
}`;

interface Diagnostic {
  severity: "Error" | "Warning";
  line: number;
  character: number;
  message: string;
  source: string;
}

interface LintResult {
  success: boolean;
  diagnostics: Diagnostic[];
  data?: any;
}

export function ConverterPage() {
  const { ready, error: wasmError, wasm, loading } = useWasm();
  const theme = useAppSetting((s) => s.theme);
  const monaco = useMonaco();

  // References
  const editorRef = useRef<any>(null);

  // States
  const [direction, setDirection] = useState<"lua_to_json" | "json_to_lua">("lua_to_json");
  const [inputCode, setInputCode] = useState(DEFAULT_LUA_TEMPLATE);
  const [outputCode, setOutputCode] = useState("");
  const [convError, setConvError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [, setLintSuccess] = useState(true);

  // Resolve Dark Mode for Monaco
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

  // Live Conversion & Linting
  useEffect(() => {
    if (!ready || !wasm) return;

    if (!inputCode.trim()) {
      setOutputCode("");
      setConvError(null);
      setDiagnostics([]);
      setLintSuccess(true);
      return;
    }

    try {
      if (direction === "lua_to_json") {
        // Run full custom linter
        const lintResStr = wasm.lint(inputCode);
        const lintRes: LintResult = JSON.parse(lintResStr);

        setDiagnostics(lintRes.diagnostics);
        setLintSuccess(lintRes.success);

        if (lintRes.success && lintRes.data) {
          setOutputCode(JSON.stringify(lintRes.data, null, 2));
          setConvError(null);
        } else {
          setOutputCode("");
          // Gather first error for basic panel fallback
          const firstError = lintRes.diagnostics.find((d) => d.severity === "Error");
          if (firstError) {
            setConvError(
              `[Line ${firstError.line}, Col ${firstError.character}] ${firstError.message}`,
            );
          } else {
            setConvError("Linter detected configuration errors.");
          }
        }
      } else {
        // json_to_lua conversion
        const res = wasm.json_to_lua(inputCode);
        setOutputCode(res);
        setConvError(null);
        setDiagnostics([]);
        setLintSuccess(true);
      }
    } catch (err) {
      setConvError(String(err));
      setDiagnostics([]);
      setLintSuccess(false);
    }
  }, [inputCode, direction, ready, wasm]);

  // Live Editor Markers / Highlighting
  useEffect(() => {
    if (monaco && editorRef.current) {
      const model = editorRef.current.getModel();
      if (model) {
        if (direction === "lua_to_json" && diagnostics.length > 0) {
          const markers = diagnostics.map((d) => ({
            startLineNumber: d.line,
            startColumn: d.character,
            endLineNumber: d.line,
            endColumn: d.character + 8, // highlight standard field width or char boundaries
            message: d.message,
            severity: d.severity === "Error" ? 8 : 4, // 8 = Error, 4 = Warning in Monaco
          }));
          monaco.editor.setModelMarkers(model, "configir-linter", markers);
        } else {
          monaco.editor.setModelMarkers(model, "configir-linter", []);
        }
      }
    }
  }, [diagnostics, monaco, direction, inputCode]);

  // Monaco Editor Mounting Callback
  const handleEditorDidMount = (editor: any) => {
    editorRef.current = editor;
  };

  // Cursor Focus Line Navigation
  const focusOnLine = (line: number, character: number) => {
    if (editorRef.current) {
      editorRef.current.revealLineInCenter(line);
      editorRef.current.setPosition({ lineNumber: line, column: character });
      editorRef.current.focus();
    }
  };

  // Preload Helpers
  const loadTemplate = (dir: typeof direction) => {
    if (dir === "lua_to_json") {
      setInputCode(DEFAULT_LUA_TEMPLATE);
    } else {
      setInputCode(DEFAULT_JSON_TEMPLATE);
    }
  };

  const toggleDirection = () => {
    const nextDir = direction === "lua_to_json" ? "json_to_lua" : "lua_to_json";
    setDirection(nextDir);
    loadTemplate(nextDir);
  };

  const handleCopy = async () => {
    if (!outputCode) return;
    try {
      await navigator.clipboard.writeText(outputCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error("Failed to copy", e);
    }
  };

  const handleDownload = () => {
    if (!outputCode) return;
    const blob = new Blob([outputCode], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = direction === "lua_to_json" ? "config.json" : "config.lua";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-background text-foreground gap-4">
        <RefreshCw className="h-8 w-8 animate-spin text-primary" />
        <p className="text-sm text-muted-foreground animate-pulse">
          Initializing WebAssembly Parser Runtime...
        </p>
      </div>
    );
  }

  if (wasmError) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-background text-foreground p-6 gap-3">
        <AlertTriangle className="h-10 w-10 text-destructive" />
        <h2 className="text-lg font-semibold">Failed to load WASM Parser Module</h2>
        <pre className="text-xs bg-muted p-4 rounded-lg border max-w-lg overflow-auto">
          {wasmError.message}
        </pre>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-background text-foreground overflow-hidden">
      {/* Top Banner Control Panel */}
      <header className="flex flex-shrink-0 flex-col md:flex-row md:items-center justify-between border-b bg-card/30 backdrop-blur-md px-6 py-3 gap-4">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Zap className="h-5 w-5" />
          </div>
          <div>
            <h1 className="text-base font-bold tracking-tight">
              Lua <span className="text-primary font-extrabold">↔</span> JSON Converter
            </h1>
            <p className="text-xs text-muted-foreground">Rust WASM configuration compiler</p>
          </div>
        </div>

        {/* Global Controls */}
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={toggleDirection}
            className="flex items-center gap-1.5 h-8 font-medium cursor-pointer"
          >
            <ArrowLeftRight className="h-3.5 w-3.5 text-primary" />
            <span>{direction === "lua_to_json" ? "Lua ➔ JSON" : "JSON ➔ Lua"}</span>
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={() => loadTemplate(direction)}
            className="flex items-center gap-1.5 h-8 cursor-pointer"
          >
            <FileCode className="h-3.5 w-3.5" />
            <span>Load Template</span>
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setInputCode("")}
            className="flex items-center gap-1.5 h-8 text-destructive hover:bg-destructive/10 cursor-pointer"
          >
            <Trash2 className="h-3.5 w-3.5" />
            <span>Clear</span>
          </Button>

          <div className="h-5 w-px bg-border mx-1 hidden md:block" />

          <Button
            variant="outline"
            size="sm"
            onClick={handleCopy}
            disabled={!outputCode || !!convError}
            className="flex items-center gap-1.5 h-8 cursor-pointer"
          >
            {copied ? (
              <Check className="h-3.5 w-3.5 text-emerald-500" />
            ) : (
              <Copy className="h-3.5 w-3.5" />
            )}
            <span>{copied ? "Copied!" : "Copy Output"}</span>
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={handleDownload}
            disabled={!outputCode || !!convError}
            className="flex items-center gap-1.5 h-8 cursor-pointer"
          >
            <Download className="h-3.5 w-3.5" />
            <span>Download</span>
          </Button>
        </div>
      </header>

      {/* Main Split Editors */}
      <div className="flex flex-1 flex-col md:flex-row overflow-hidden p-4 gap-4">
        {/* Input Panel */}
        <div className="flex flex-1 flex-col border rounded-xl overflow-hidden bg-card shadow-sm">
          <div className="flex items-center justify-between px-4 py-2 border-b bg-card/60">
            <span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
              Input: {direction === "lua_to_json" ? "Lua Code" : "JSON Model"}
            </span>
          </div>
          <div className="flex-1 overflow-hidden relative">
            <Editor
              height="100%"
              language={direction === "lua_to_json" ? "lua" : "json"}
              theme={isDark ? "vs-dark" : "light"}
              value={inputCode}
              onChange={(val) => setInputCode(val || "")}
              onMount={handleEditorDidMount}
              options={{
                minimap: { enabled: false },
                fontSize: 13,
                fontFamily: "var(--font-mono, monospace)",
                lineNumbers: "on",
                scrollBeyondLastLine: false,
                wordWrap: "on",
                automaticLayout: true,
                padding: { top: 8, bottom: 8 },
              }}
            />
          </div>
        </div>

        {/* Output Panel */}
        <div className="flex flex-1 flex-col border rounded-xl overflow-hidden bg-card shadow-sm">
          <div className="flex items-center justify-between px-4 py-2 border-b bg-card/60">
            <span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
              Output: {direction === "lua_to_json" ? "JSON Model" : "Lua Code"}
            </span>
          </div>
          <div className="flex-1 overflow-hidden relative">
            <Editor
              height="100%"
              language={direction === "lua_to_json" ? "json" : "lua"}
              theme={isDark ? "vs-dark" : "light"}
              value={convError && direction === "lua_to_json" ? "" : outputCode}
              options={{
                readOnly: true,
                minimap: { enabled: false },
                fontSize: 13,
                fontFamily: "var(--font-mono, monospace)",
                lineNumbers: "on",
                scrollBeyondLastLine: false,
                wordWrap: "on",
                automaticLayout: true,
                padding: { top: 8, bottom: 8 },
              }}
            />
          </div>
        </div>
      </div>

      {/* Visual Diagnostic Panel for Lua to JSON */}
      {direction === "lua_to_json" && (
        <div className="flex-shrink-0 mx-4 mb-4 border rounded-xl bg-card shadow-sm overflow-hidden border-border">
          <div className="flex items-center justify-between border-b px-4 py-2 bg-muted/40">
            <div className="flex items-center gap-2">
              <Terminal className="h-4 w-4 text-primary" />
              <span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                Linter & Diagnostics
              </span>
            </div>
            <div className="flex items-center gap-2">
              {diagnostics.length === 0 ? (
                <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
                  <Check className="h-3 w-3" /> Clean Config
                </span>
              ) : (
                <div className="flex items-center gap-1.5">
                  {diagnostics.filter((d) => d.severity === "Error").length > 0 && (
                    <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-semibold bg-destructive/10 text-destructive border border-destructive/20">
                      {diagnostics.filter((d) => d.severity === "Error").length} Errors
                    </span>
                  )}
                  {diagnostics.filter((d) => d.severity === "Warning").length > 0 && (
                    <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-500 border border-amber-500/20">
                      {diagnostics.filter((d) => d.severity === "Warning").length} Warnings
                    </span>
                  )}
                </div>
              )}
            </div>
          </div>

          <div className="p-3 max-h-[220px] overflow-y-auto bg-card/50">
            {diagnostics.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-6 text-center text-muted-foreground">
                <Sparkles className="h-8 w-8 text-emerald-500 mb-2 animate-pulse" />
                <p className="text-sm font-medium">
                  Your configuration matches all schema constraints perfectly!
                </p>
                <p className="text-xs text-muted-foreground/80 mt-1">
                  Annotations are correctly bound to fields and types are fully matched.
                </p>
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                {diagnostics.map((d, idx) => {
                  const isError = d.severity === "Error";
                  return (
                    <button
                      key={idx}
                      onClick={() => focusOnLine(d.line, d.character)}
                      className={`flex flex-col p-3 rounded-lg border text-left cursor-pointer transition-all hover:-translate-y-0.5 shadow-sm hover:shadow-md ${
                        isError
                          ? "bg-destructive/5 hover:bg-destructive/10 border-destructive/20 hover:border-destructive/40"
                          : "bg-amber-500/5 hover:bg-amber-500/10 border-amber-500/20 hover:border-amber-500/40"
                      }`}
                    >
                      <div className="flex items-center justify-between mb-1.5 w-full">
                        <span
                          className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider ${
                            isError
                              ? "bg-destructive/15 text-destructive"
                              : "bg-amber-500/15 text-amber-500"
                          }`}
                        >
                          {d.severity}
                        </span>
                        <span className="text-[11px] font-mono font-medium text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
                          Line {d.line}:{d.character}
                        </span>
                      </div>
                      <p className="text-xs font-mono font-medium leading-relaxed break-words text-foreground dark:text-gray-100 flex-1">
                        {d.message}
                      </p>
                      <div className="text-[10px] text-muted-foreground/60 mt-1.5 w-full text-right font-semibold">
                        Click to navigate in editor
                      </div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}

      {/* JSON to Lua parsing error presentation */}
      {direction === "json_to_lua" && convError && (
        <div className="flex-shrink-0 mx-4 mb-4 border border-destructive/30 rounded-xl bg-destructive/5 overflow-hidden">
          <div className="flex items-center gap-2 border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-destructive">
            <AlertTriangle className="h-4 w-4 shrink-0" />
            <span className="text-xs font-bold uppercase tracking-wider">JSON Parsing Error</span>
          </div>
          <div className="p-4 max-h-[150px] overflow-auto">
            <pre className="text-[12px] leading-relaxed font-mono whitespace-pre text-destructive-foreground dark:text-red-300">
              {convError}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
