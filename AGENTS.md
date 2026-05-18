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

| what | how |
|---|---|
| run backend | `just dev` (= `cargo run -p trek`) |
| format all | `just fmt` (= `cargo fmt && pnpm oxfmt .`) |
| check | `cargo check` |
| release build | `cargo build --release` |
| with swagger | `cargo check --features swagger` / `cargo run -p trek --features swagger` |

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
