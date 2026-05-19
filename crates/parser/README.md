# parser

Lua configuration parser, linter, and JSON converter for the **trek** project. Parses Lua config files into a strongly-typed intermediate representation (`ConfigIR`), with full round-trip support back to formatted Lua.

## Public API

| Function              | Description                                                  |
| --------------------- | ------------------------------------------------------------ |
| `lua_to_json(source)` | Parse Lua source → `ConfigIR` → pretty JSON                  |
| `json_to_lua(json)`   | Deserialize JSON → `ConfigIR` → formatted Lua (via `stylua`) |
| `lint(source)`        | Validate Lua source with structured diagnostics (JSON)       |
| `generate_schema()`   | Emit a JSON Schema for the `ConfigIR` type                   |

## Modules

- **`models`** — The `ConfigIR` type system: `String`, `Number`, `Boolean`, `Enum`, `Vector2`, `Vector3`, `Table`, `DynamicTable`, `Array`, `Function`, `Expression`, `Nil` — each carrying optional metadata (descriptions, range bounds, enum options, table schemas, CFX function bindings, map flags).

- **`visitor`** — Walks a `full_moon` AST, extracts annotation metadata from trivia (comments), detects `vector2()`/`vector3()` calls, and builds `ConfigIR`.

- **`generator`** — Serializes `ConfigIR` back to Lua source, formatting with `stylua` and performing a round-trip validation via `full_moon::parse`.

- **`linter`** — Three-category linting:
  - **A**: Lua syntax errors
  - **B**: Annotation syntax & semantic validation (ENUM options, RANGE bounds, MAP booleans, TABLE schemas, CFX_FUNCTION fields)
  - **C**: Type mismatch checks and dangling annotation detection

- **`trivia_parser`** — Parses annotations embedded in Lua comments:
  - `--!KEY = value` (single-line key-value)
  - `--[[ KEY = { ... } ]]` (multi-line block annotations)

## Dependencies

`full_moon`, `serde`/`serde_json`, `indexmap`, `stylua`, `schemars` — all kept minimal per the trek project conventions.
