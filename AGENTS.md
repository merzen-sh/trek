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

## Parser order preservation

The Lua↔JSON pipeline must preserve field insertion order. Alphabetical sorting is forbidden. `serde_json` is configured with `features = ["preserve_order"]` and `indexmap` with `features = ["serde"]` — the types enforce this. See `crates/parser/`.
