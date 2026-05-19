pub mod generator;
pub mod linter;
pub mod models;
pub mod trivia_parser;
pub mod visitor;

pub fn lint(source: &str) -> String {
    linter::lint(source)
}

pub fn lua_to_json(source: &str) -> Result<String, String> {
    let ir = visitor::build_ir(source)?;
    serde_json::to_string_pretty(&ir).map_err(|e| format!("serialization error: {e}"))
}

pub fn json_to_lua(json: &str) -> Result<String, String> {
    let ir: models::ConfigIR =
        serde_json::from_str(json).map_err(|e| format!("deserialization error: {e}"))?;
    generator::generate_lua(&ir)
}

pub fn generate_schema() -> String {
    let schema = schemars::schema_for!(models::ConfigIR);
    serde_json::to_string_pretty(&schema).unwrap()
}
