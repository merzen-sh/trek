# parser-wasm

A WebAssembly wrapper for the `parser` crate.
It exposes three functions:

- `json_to_lua(json: string): Promise<string>`
- `lua_to_json(source: string): Promise<string>`
- `generate_schema(): string`

## Build

```bash
just build-wasm
```
