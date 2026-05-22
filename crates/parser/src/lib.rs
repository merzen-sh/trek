use full_moon::parse;

pub mod models;
mod visitor;
mod trivia_parser;
mod lua_gen;

#[cfg(not(target_arch = "wasm32"))]
fn guarded_parse(source: &str) -> Result<full_moon::ast::Ast, String> {
    use std::sync::mpsc;
    use std::thread;

    let source = source.to_owned();
    let (tx, rx) = mpsc::channel();
    let builder = thread::Builder::new()
        .name("lua-parse".into())
        .stack_size(64 << 20); // 64 MB

    builder
        .spawn(move || {
            let result = parse(&source).map_err(|e| format!("parse error: {e:?}"));
            let _ = tx.send(result);
        })
        .map_err(|e| format!("spawn parse thread: {e}"))?
        .join()
        .map_err(|_| "parse thread panicked (stack overflow?)".to_string())?;

    rx.recv().map_err(|_| "parse result channel closed".to_string())?
}

#[cfg(target_arch = "wasm32")]
fn guarded_parse(source: &str) -> Result<full_moon::ast::Ast, String> {
    parse(source).map_err(|e| format!("parse error: {e:?}"))
}

pub fn lua_to_json(source: &str) -> Result<String, String> {
    let ast = guarded_parse(source)?;
    let doc = visitor::LuaAstVisitor::new(&ast).extract();
    serde_json::to_string_pretty(&doc).map_err(|e| format!("serialize error: {e}"))
}

pub fn json_to_lua(json: &str) -> Result<String, String> {
    let doc: models::ConfigDoc =
        serde_json::from_str(json).map_err(|e| format!("deserialize error: {e}"))?;
    Ok(lua_gen::generate(&doc))
}
