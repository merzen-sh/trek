use wasm_bindgen::prelude::*;

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
    let schema = schemars::schema_for!(parser::models::ConfigDoc);
    serde_json::to_string_pretty(&schema).unwrap()
}

