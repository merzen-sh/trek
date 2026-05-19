import { useState, useEffect } from "react";
import Editor from "@monaco-editor/react";
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

export function ConverterPage() {
  const { ready, error: wasmError, wasm, loading } = useWasm();
  const theme = useAppSetting((s) => s.theme);

  // States
  const [direction, setDirection] = useState<"lua_to_json" | "json_to_lua">("lua_to_json");
  const [inputCode, setInputCode] = useState(DEFAULT_LUA_TEMPLATE);
  const [outputCode, setOutputCode] = useState("");
  const [convError, setConvError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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

  // Live Conversion
  useEffect(() => {
    if (!ready || !wasm) return;

    if (!inputCode.trim()) {
      setOutputCode("");
      setConvError(null);
      return;
    }

    try {
      if (direction === "lua_to_json") {
        const res = wasm.lua_to_json(inputCode);
        setOutputCode(res);
        setConvError(null);
      } else {
        const res = wasm.json_to_lua(inputCode);
        setOutputCode(res);
        setConvError(null);
      }
    } catch (err) {
      // In Rust WASM, errors returned as String are caught as JS exceptions
      setConvError(String(err));
    }
  }, [inputCode, direction, ready, wasm]);

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
              value={convError ? "" : outputCode}
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

      {/* Visual Diagnostic Panel */}
      {convError && (
        <div className="flex-shrink-0 mx-4 mb-4 border border-destructive/30 rounded-xl bg-destructive/5 overflow-hidden">
          <div className="flex items-center gap-2 border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-destructive">
            <AlertTriangle className="h-4 w-4 shrink-0" />
            <span className="text-xs font-bold uppercase tracking-wider">
              Diagnostic Error Report
            </span>
          </div>
          <div className="p-4 max-h-[300px] overflow-auto">
            <pre className="text-[12px] leading-relaxed font-mono whitespace-pre text-destructive-foreground dark:text-red-300">
              {convError}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
