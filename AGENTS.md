# trek

## Workspace

Three crates, `trek` (web server binary), `parser` (Lua↔JSON library), `parser-wasm` (WASM bindings). The `trek` binary does **not** depend on `parser` — they are independent workspace members. Backend entrypoint: `crates/trek/src/main.rs` → `cli.rs` → `server/`.

## Commands

| what                     | how                                                   |
| ------------------------ | ----------------------------------------------------- |
| run dev (with swagger)   | `just dev` (= `cargo run -p trek --features swagger`) |
| run without swagger      | `cargo run -p trek`                                   |
| check all crates         | `just check` (= `cargo check --workspace`)            |
| format all               | `just fmt` (= `cargo fmt && pnpm oxfmt .`)            |
| test trek crate          | `cargo test -p trek`                                  |
| test all (cargo-nextest) | `just next-test`                                      |
| release build            | `cargo build --release`                               |
| build WASM               | `just build-wasm`                                     |
| generate schema.json     | `just generate-schema`                                |

## Binary size

Every dep uses `default-features = false` with minimal features. Release profile: `opt-level = "z", lto = true, panic = "abort", codegen-units = 1, strip = true`. Do not add deps with default features on.

## Runtime

Do **not** add `#[tokio::main]`. Runtime is explicitly built in `run_server()` via `Builder::new_current_thread().enable_io().enable_time().build().block_on(...)` (`crates/trek/src/server/mod.rs:52`).

## Proxy

`/*` catch-all proxies to `TREK_APP_PUBLIC_URL` (default `http://localhost:5173`). Uses `hyper_util::client::legacy::Client` directly — **never add reqwest**. Always compiled (no feature gate).

## Routing

Router at `crates/trek/src/server/router.rs`. Add new routes or API handlers there or in a new module referenced from it.

- `mod swagger` — `#[cfg(feature = "swagger")]`, mounted at `/swagger-ui` + `/api/openapi.json`
- `mod proxy` — catch-all fallback

## Swagger

Optional: `--features swagger` adds Swagger UI + OpenAPI docs. Only `utoipa` / `utoipa-swagger-ui` behind this flag.

## Config

Stored at `~/.config/trek/config.toml` (XDG_CONFIG_HOME, fallback `~/.config` on Unix, `%APPDATA%` on Windows). `workspace_dir` field: a name in `[name]` format. If unset or invalid on startup, prompts user via `inquire::Text`. Module: `crates/trek/src/config.rs`.

## Toolchain

- Rust edition 2024, channel 1.95.0, `resolver = "2"`
- JS: `pnpm` (not npm/yarn), `oxlint` (not eslint), `oxfmt` (not prettier)
- Frontend: `packages/app/` (Vite + React 19)
- No CI, no tests, no README

## Parser crate (`crates/parser`)

Crate name `parser`, public API at `src/lib.rs`. Two functions:

| call | direction |
|------|-----------|
| `parser::lua_to_json(src)` | Lua source string → pretty JSON |
| `parser::json_to_lua(json)` | JSON string → Lua source string |

All internal modules are private: `visitor`, `trivia_parser`, `lua_gen`. Only `models` is `pub`.

### Key gotchas

- **Number values are stored as raw `String` tokens** — `ScalarNode<String>`. This is lossless round-trip. Do not parse into `f64` or `i64`.
- **Field insertion order is sacred**. Never sort fields. `serde_json` uses `preserve_order`, `IndexMap` uses `serde` feature — the types enforce this.
- **Stack overflow guard**: `guarded_parse()` runs `full_moon::parse` on a dedicated thread with a **64 MB stack**. Deeply nested Lua (hundreds of `{`) will overflow the default thread stack.
- **`lua_gen.rs` named over `gen.rs`** because `gen` is a reserved keyword in Rust 2024 edition.
- **`lua_gen::fmt_float`** formats whole floats with `{:.1}` to preserve `.0` suffix through round-trip (e.g. `vector3(1.0, 2.0, 3.0)`).
- **serde_json does NOT guarantee f64 round-trip** for all values. Proptest strategies restrict to integers + halves (exact IEEE 754 binary representations).

### Commands

| what | how |
|------|-----|
| test parser | `cargo test -p parser` |
| proptest (single) | same command — only test in crate |
| cargo-fuzz target | `cargo +nightly fuzz run lua_roundtrip` (from workspace root) |
| fuzz campaign + HTML report | `cargo run -p xtask -- fuzz` |
| report from artifacts only | `cargo run -p xtask -- report` |

### Fuzz structure

- `crates/parser/fuzz/` — workspace member `parser-fuzz`, two targets (`lua_roundtrip`, `json_roundtrip`)
- `xtask/fuzz` — workspace member for fuzzing orchestration, wraps `cargo fuzz` with HTML report generation (Tailwind + Chart.js)
- libfuzzer flags use `=` syntax: `-max_total_time=300` not `-max_total_time 300`
- libfuzzer stats output on **stderr**, not stdout
- xtask fuzz paths resolve as `crates/parser/fuzz/artifacts/{target}` / `crates/parser/fuzz/corpus/{target}`

### Model layout (`src/models.rs`)

ConfigNode enum (`#[serde(tag = "type", rename_all = "snake_case")]`):
- `String`, `Number`, `Boolean` → `ScalarNode<T>` with optional `ScalarMeta` (description + range)
- `Enum` → `EnumNode` with `EnumMeta` (description + options — always required for round-trip)
- `Table` → `TableNode` with `TableMeta` (description + optional `TableSchema`)
- `CfxFunction` → `CfxFunctionNode` with `CfxFunctionMeta` (description + args_schema)
- `Vector2`, `Vector3` → `{Vector2,Vector3}Node` with f64 x/y/z values + optional `ScalarMeta`

### Proptest constraints (`tests/fuzz_test.rs`)

The fuzz test normalizes empty metadata before comparison because Lua has no way to represent semantically-empty annotations. Key restrictions in generated data:
- Enum nodes **must** have options metadata (otherwise collapses to String in round-trip)
- Description lines pre-trimmed (parser strips leading/trailing whitespace)
- Boolean nodes never get range metadata (parser discards it)
- Lua keywords excluded from key strategies via `prop_filter`
- String strategies use alphanumeric only (avoids Lua escaping representation changes)
- Floats: integers + halves only (exact IEEE 754, avoids serde_json precision loss)
- Negative number values avoided in range (produces UnaryOperator expressions that break round-trip)
- Max 3 top-level fields, 2-level nesting, small entry counts

### Annotation syntax

Parsed from Lua trivia by `trivia_parser.rs`:

| syntax | populates |
|--------|-----------|
| `--! text` | description |
| `--@ENUM = { "a", "b" }` | enum options |
| `--@RANGE = { min, max }` | range constraint |
| `--[[@TABLE = { … }]]` | table schema |
| `--[[@CFX_FUNCTION = { … }]]` | CFX function metadata |

### Vector detection

Vectors are detected as function call expressions in `visitor::try_parse_vector()`:
```lua
vector2(1.0, 2.0)
vector3(-150.0, 50.0, 28.0)
```
