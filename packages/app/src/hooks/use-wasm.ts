import { useState, useEffect } from "react";
import init, * as wasm from "@trek-cli/parser";
import wasmUrl from "@trek-cli/parser/parser_wasm_bg.wasm?url";

export const useWasm = () => {
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;
    setLoading(true);

    const load = async () => {
      try {
        await init(wasmUrl);
        if (isMounted) setReady(true);
      } catch (err) {
        if (isMounted) setError(err as unknown as Error);
      } finally {
        if (isMounted) setLoading(false);
      }
    };

    load();

    return () => {
      isMounted = false;
    };
  }, []);

  return {
    ready,
    error,
    wasm,
    loading,
  };
};
