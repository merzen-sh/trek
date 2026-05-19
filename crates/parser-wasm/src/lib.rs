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
pub fn lint(source: &str) -> String {
    parser::lint(source)
}

#[wasm_bindgen]
pub fn generate_schema() -> String {
    parser::generate_schema()
}
