import { useCallback, useEffect, useRef, useState } from "react";
import type { ConfigEditor } from "@trek-cli/parser";
import { parseLayoutDoc, type LayoutDoc } from "../types/layout";

function syncSession(editor: ConfigEditor) {
  return {
    layout: parseLayoutDoc(editor.getLayout()),
    lua: editor.print(),
  };
}

export function useConfigEditor(
  source: string,
  ready: boolean,
  wasm: typeof import("@trek-cli/parser") | undefined,
) {
  const editorRef = useRef<ConfigEditor | null>(null);
  const [layout, setLayout] = useState<LayoutDoc | null>(null);
  const [luaOutput, setLuaOutput] = useState(source);
  const [parseError, setParseError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (!ready || !wasm) return;

    editorRef.current?.free();
    editorRef.current = null;

    if (!source.trim()) {
      setLayout(null);
      setLuaOutput("");
      setParseError(null);
      return;
    }

    try {
      const editor = new wasm.ConfigEditor(source);
      editorRef.current = editor;
      const { layout: doc, lua } = syncSession(editor);
      setLayout(doc);
      setLuaOutput(lua);
      setParseError(null);
    } catch (err) {
      setLayout(null);
      setParseError(String(err));
    }

    return () => {
      editorRef.current?.free();
      editorRef.current = null;
    };
  }, [source, ready, wasm]);

  const bump = useCallback(() => setRevision((r) => r + 1), []);

  const applySession = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    const { layout: doc, lua } = syncSession(editor);
    setLayout(doc);
    setLuaOutput(lua);
    bump();
  }, [bump]);

  const getValueAtPath = useCallback((path: string[]): unknown => {
    const editor = editorRef.current;
    if (!editor || path.length === 0) return undefined;
    try {
      return JSON.parse(editor.getValueAtPath(path));
    } catch {
      return undefined;
    }
  }, []);

  const patchValueAtPath = useCallback(
    (path: string[], value: unknown) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.patchValueAtPath(path, JSON.stringify(value));
      applySession();
    },
    [applySession],
  );

  const appendTableRow = useCallback(
    (tablePath: string[], rowKey: string, rowPayload: Record<string, unknown>) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.patchTableAppend(tablePath, rowKey, JSON.stringify(rowPayload));
      applySession();
    },
    [applySession],
  );

  const removeTableRow = useCallback(
    (tablePath: string[], rowKey: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.patchTableRemove(tablePath, rowKey);
      applySession();
    },
    [applySession],
  );

  const refreshLua = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return "";
    const lua = editor.print();
    setLuaOutput(lua);
    return lua;
  }, []);

  return {
    layout,
    luaOutput,
    parseError,
    revision,
    getValueAtPath,
    patchValueAtPath,
    appendTableRow,
    removeTableRow,
    refreshLua,
    hasSession: editorRef.current !== null,
  };
}
