# trek

## Binary size

Every dep uses `default-features = false` with minimal features. The release profile is aggressive:

```
opt-level = "z", lto = true, panic = "abort", codegen-units = 1, strip = true
```

Do not add new deps with default features on. Do not add `#[tokio::main]` — the runtime is explicitly constructed in `run_server()` via `Builder::new_current_thread().enable_io().enable_time().build().block_on(...)`.

## Proxy architecture

`/*` catch-all proxies to `TREK_APP_PUBLIC_URL` (default `http://localhost:5173`). Uses `hyper_util::client::legacy::Client` directly — **never add reqwest**. The proxy is always compiled (no feature gate).

## Swagger feature

Optional, off by default: `--features swagger` adds `/api/swagger-ui` + `/api/openapi.json`. Only the UI/documentation deps (`utoipa`, `utoipa-swagger-ui`) are behind this flag.

## Commands

| what          | how                                                                       |
| ------------- | ------------------------------------------------------------------------- |
| run backend   | `just dev` (= `cargo run -p trek`)                                        |
| format all    | `just fmt` (= `cargo fmt && pnpm oxfmt .`)                                |
| check         | `cargo check`                                                             |
| release build | `cargo build --release`                                                   |
| with swagger  | `cargo check --features swagger` / `cargo run -p trek --features swagger` |

## Toolchain

- **Rust edition 2024** — `resolver = "2"` in workspace.
- **JS** — `pnpm` (not npm/yarn), `oxlint` (not eslint), `oxfmt` (not prettier).
- Frontend lives in `packages/app/` (Vite + React 19).
- Backend entrypoint: `crates/trek/src/main.rs` → `cli.rs` → `server/`.
- No CI, no tests, no README.

## Routing

Router is built in `src/server/router.rs`:

- `mod swagger` — behind `#[cfg(feature = "swagger")]`, mounted at `/api`
- `mod proxy` — always compiled, catch-all fallback forwarding to upstream
- New routes or API handlers go in `router.rs` or a new module referenced from it.

## AST Order Preservation (Critical)

The transformation pipeline between Lua AST and JSON must strictly preserve the native physical order of fields as they appear in source code. Alphabetical sorting of JSON keys is strictly forbidden.

### Lua to JSON Order Tracking

When compiling valid Lua into JSON, fields inside objects or dynamic tables must follow the exact sequential appearance in the source file.

1. Maps must maintain insertion order. If `locale` comes before `enable_shop` in Lua, the resulting JSON output inside the `"data"` block must reflect that exact structure.
2. In objects with sub-tables (like `shop`), the `fields` map must respect the exact chronological extraction from the AST.

### JSON to Lua Generation Order

When reconstructing a Lua configuration file from a JSON schema:

1. The formatting engine must traverse JSON objects based on their insertion sequence in the JSON file.
2. Every associated comment, annotation syntax (`--!`, `--[[ ]]`), and variable declaration must be written in the target file respecting that exact order.
3. Re-ordering, sorting, or shuffling fields via any optimization process is a structural diagnostic failure.

### Diagnostic Verification

Order Mismatch Linting: If the generator detects an out-of-order schema serialization or a mutation in the sequential topology during a sync operation, it must flag it as an operational error.

### Implementation

`serde_json` must be used with `features = ["preserve_order"]` to ensure `serde_json::Map` uses `IndexMap` internally (not `BTreeMap`), guaranteeing insertion-order preservation.
