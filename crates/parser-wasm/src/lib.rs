use wasm_bindgen::prelude::*;

use parser::ConfigSession;
use serde_json::Value;

#[wasm_bindgen]
pub fn json_to_lua(json: &str) -> Result<String, String> {
    parser::json_to_lua(json)
}

#[wasm_bindgen]
pub fn lua_to_json(source: &str) -> Result<String, String> {
    parser::lua_to_json(source)
}

#[wasm_bindgen]
pub fn generate_schema() -> String {
    let schema = schemars::schema_for!(parser::models::LayoutDoc);
    serde_json::to_string_pretty(&schema).unwrap()
}

/// Stateful config editor backed by an in-memory lossless AST.
#[wasm_bindgen]
pub struct ConfigEditor {
    session: ConfigSession,
}

#[wasm_bindgen]
impl ConfigEditor {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<ConfigEditor, String> {
        Ok(ConfigEditor {
            session: ConfigSession::from_source(source)?,
        })
    }

    /// Lean layout tree (metadata + `ast_path` only).
    #[wasm_bindgen(js_name = getLayout)]
    pub fn get_layout(&self) -> Result<String, String> {
        self.session.get_layout_json()
    }

    /// Runtime value at `path` as a JSON string.
    #[wasm_bindgen(js_name = getValueAtPath)]
    pub fn get_value_at_path(&self, path: Vec<String>) -> Result<String, String> {
        self.session.get_value_json_at_path(path)
    }

    /// Apply a scalar/table patch; `payload_json` is a JSON-encoded value.
    #[wasm_bindgen(js_name = patchValueAtPath)]
    pub fn patch_value_at_path(
        &mut self,
        path: Vec<String>,
        payload_json: &str,
    ) -> Result<(), String> {
        let payload: Value =
            serde_json::from_str(payload_json).map_err(|e| format!("invalid payload JSON: {e}"))?;
        self.session.patch_value_at_path(&path, &payload)
    }

    /// Append a schema table row at `table_path` with the given row key and row object JSON.
    #[wasm_bindgen(js_name = patchTableAppend)]
    pub fn patch_table_append(
        &mut self,
        table_path: Vec<String>,
        row_key: &str,
        row_payload_json: &str,
    ) -> Result<(), String> {
        let payload: Value = serde_json::from_str(row_payload_json)
            .map_err(|e| format!("invalid row payload JSON: {e}"))?;
        self.session.patch_table_append(&table_path, row_key, &payload)
    }

    /// Remove a row by key from the table at `table_path`.
    #[wasm_bindgen(js_name = patchTableRemove)]
    pub fn patch_table_remove(&mut self, table_path: Vec<String>, row_key: &str) -> Result<(), String> {
        self.session.patch_table_remove_row(&table_path, row_key)
    }

    /// Lossless Lua source from the current AST.
    #[wasm_bindgen(js_name = print)]
    pub fn print_lua(&self) -> String {
        self.session.print()
    }
}
