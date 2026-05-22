use full_moon::{
    ast::{Expression, Field, Stmt, TableConstructor},
    ast::punctuated::Pair,
    parse,
    tokenizer::{Token, TokenReference, TokenType},
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

pub fn comma_token() -> TokenReference {
    token_symbol(",\n")
}

fn token_symbol(text: &str) -> TokenReference {
    TokenReference::symbol(text).expect("invalid token symbol")
}

fn lua_string_body(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{{{:04x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn parse_lua_string_literal(s: &str) -> Result<Expression, String> {
    parse_lua_expr(&format!("\"{}\"", lua_string_body(s)))
}

pub fn expression_from_json(value: &Value) -> Result<Expression, String> {
    match value {
        Value::Bool(b) => parse_lua_expr(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let f = n.as_f64().ok_or("invalid number")?;
            let text = if f.fract() == 0.0 {
                format!("{f:.1}")
            } else {
                f.to_string()
            };
            parse_lua_expr(&text)
        }
        // Config string fields must stay string literals — never bare identifiers.
        Value::String(s) => parse_lua_string_literal(s),
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
            let table = table_from_json_object(obj)?;
            Ok(Expression::TableConstructor(table))
        }
        _ => Err("unsupported JSON value for Lua expression".into()),
    }
}

fn table_from_json_object(obj: &serde_json::Map<String, Value>) -> Result<TableConstructor, String> {
    let indent = "    ";
    let mut fields: Vec<Pair<Field>> = Vec::new();
    for (key, val) in obj {
        let field = name_key_field(key, expression_from_json(val)?, indent)?;
        if let Some(last) = fields.last_mut() {
            if last.punctuation().is_none() {
                *last = Pair::Punctuated(last.clone().into_value(), comma_token());
            }
        }
        fields.push(Pair::new(field, None));
    }
    Ok(TableConstructor::new().with_fields(full_moon::ast::punctuated::Punctuated::from_iter(
        fields,
    )))
}

pub fn table_row_field(row_key: &str, payload: &Value) -> Result<Field, String> {
    let indent = "    ";
    let value = match payload {
        Value::Object(obj) => Expression::TableConstructor(table_from_json_object(obj)?),
        other => expression_from_json(other)?,
    };
    name_key_field(row_key, value, indent)
}

pub fn key_token_with_indent(key: &str, indent: &str) -> TokenReference {
    TokenReference::new(
        vec![Token::new(TokenType::Whitespace {
            characters: indent.into(),
        })],
        Token::new(TokenType::Identifier {
            identifier: key.into(),
        }),
        vec![Token::new(TokenType::Whitespace {
            characters: " ".into(),
        })],
    )
}

pub fn name_key_field(key: &str, value: Expression, indent: &str) -> Result<Field, String> {
    Ok(Field::NameKey {
        key: key_token_with_indent(key, indent),
        equal: token_symbol(" = "),
        value,
    })
}
