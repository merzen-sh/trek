use full_moon::{
    ast::{Expression, Stmt},
    parse,
};

use serde_json::Value;

fn parse_lua_expr(src: &str) -> Result<Expression, String> {
    let source = format!("local _ = {src}");
    let ast = parse(&source).map_err(|e| format!("parse expr: {e:?}"))?;
    for stmt in ast.nodes().stmts() {
        if let Stmt::LocalAssignment(local) = stmt {
            if let Some(expr) = local.expressions().iter().next() {
                return Ok(expr.clone());
            }
        }
    }
    Err("no expression in parsed snippet".into())
}

/// Parse the source as a table constructor expression.
pub fn parse_table(src: &str) -> Result<Expression, String> {
    parse_lua_expr(src)
}

/// Generate formatted Lua source for a JSON value.
/// `indent` is the whitespace prefix for this level (closing brace level).
/// Inner fields use one additional `"    "` level.
pub fn value_to_lua(value: &Value, indent: &str) -> String {
    match value {
        Value::Null => "nil".into(),
        Value::Bool(b) => (if *b { "true" } else { "false" }).into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
        }
        Value::String(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{escaped}\"")
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                return "{}".into();
            }
            let inner = format!("{}{}", indent, "    ");
            let mut out = String::from("{\n");
            for (key, val) in obj {
                out.push_str(&inner);
                out.push_str(&lua_key(key));
                out.push_str(" = ");
                out.push_str(&value_to_lua(val, &inner));
                out.push_str(",\n");
            }
            out.push_str(indent);
            out.push('}');
            out
        }
        _ => "nil".into(),
    }
}

/// Format a table key in Lua syntax — bare identifier for valid idents,
/// `[number]` for numeric keys, `["string"]` otherwise.
pub fn lua_key(key: &str) -> String {
    if key.is_empty() {
        return "[\"\"]".into();
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    let is_ident = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if is_ident {
        key.to_string()
    } else if key.chars().all(|c| c.is_ascii_digit() || c == '.') {
        format!("[{key}]")
    } else {
        let escaped = key
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        format!("[\"{escaped}\"]")
    }
}

pub fn expression_from_json(value: &Value) -> Result<Expression, String> {
    match value {
        Value::Bool(b) => parse_lua_expr(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let text = if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().ok_or("invalid number")?;
                if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            };
            parse_lua_expr(&text)
        }
        Value::String(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            parse_lua_expr(&format!("\"{escaped}\""))
        }
        Value::Object(obj) if obj.contains_key("x") && obj.contains_key("y") => {
            let x = obj["x"].as_f64().ok_or("vector x must be number")?;
            let y = obj["y"].as_f64().ok_or("vector y must be number")?;
            if obj.contains_key("z") {
                let z = obj["z"].as_f64().ok_or("vector z must be number")?;
                parse_lua_expr(&format!("vector3({x:.1}, {y:.1}, {z:.1})"))
            } else {
                parse_lua_expr(&format!("vector2({x:.1}, {y:.1})"))
            }
        }
        Value::Object(obj) => {
            let lua = value_to_lua(&Value::Object(obj.clone()), "    ");
            parse_lua_expr(&lua)
        }
        _ => Err("unsupported JSON value for Lua expression".into()),
    }
}
